use crate::db::queries;
use crate::error::Result;
use crate::models::{Coordinates, Poi, PoiCategory};
use crate::services::overpass::OverpassClient;
use sqlx::PgPool;

// POI service constants
const POI_COUNT_MULTIPLIER: f64 = 2.5;  // Multiplier for calculating minimum POI count based on radius

pub struct PoiService {
    db_pool: PgPool,
    overpass_client: OverpassClient,
}

impl PoiService {
    pub fn new(db_pool: PgPool) -> Self {
        PoiService {
            db_pool,
            overpass_client: OverpassClient::new(),
        }
    }

    /// Find POIs within a radius, with optional category filtering
    /// First checks database, falls back to Overpass API if needed
    pub async fn find_pois(
        &self,
        center: &Coordinates,
        radius_km: f64,
        categories: Option<&[PoiCategory]>,
        limit: usize,
    ) -> Result<Vec<Poi>> {
        let radius_meters = radius_km * 1000.0;

        // Try database first
        let db_pois = queries::find_pois_within_radius(
            &self.db_pool,
            center,
            radius_meters,
            categories,
            limit as i64,
        )
        .await?;

        // Calculate minimum POI count based on search area
        // Larger areas should have proportionally more POIs to justify skipping Overpass
        // For 5km: ~12 POIs, for 10km: ~25 POIs, for 15km: ~37 POIs
        let min_poi_count = ((radius_km * POI_COUNT_MULTIPLIER) as usize).clamp(10, 50);

        // If we have enough POIs in database, use them
        if db_pois.len() >= limit.min(min_poi_count) {
            tracing::debug!(
                "Found {} POIs in database within {}km (min threshold: {})",
                db_pois.len(),
                radius_km,
                min_poi_count
            );
            return Ok(db_pois.into_iter().take(limit).collect());
        }

        // Otherwise, try to fetch from Overpass API
        tracing::info!(
            "Fetching POIs from Overpass API for {:?} (radius: {}km)",
            center,
            radius_km
        );

        let categories_to_fetch = categories
            .map(|c| c.to_vec())
            .unwrap_or_else(Self::default_categories);

        // Strategy: Try single query first, fallback to batched on timeout
        let single_query_result = self
            .overpass_client
            .query_pois(center, radius_meters, &categories_to_fetch)
            .await;

        match single_query_result {
            Ok(overpass_pois) => {
                // Success! Store and return
                // Use transaction for batch inserts to ensure atomicity
                let mut transaction = match self.db_pool.begin().await {
                    Ok(tx) => tx,
                    Err(e) => {
                        tracing::warn!("Failed to begin transaction for POI inserts: {}", e);
                        // Continue without transaction - best effort insertion
                        for poi in &overpass_pois {
                            if let Err(e) = queries::insert_poi(&self.db_pool, poi).await {
                                tracing::debug!("Failed to insert POI {}: {}", poi.name, e);
                            }
                        }
                        tracing::info!("Fetched {} POIs from Overpass API (single query)", overpass_pois.len());
                        return Ok(overpass_pois.into_iter().take(limit).collect());
                    }
                };

                let mut inserted_count = 0;
                for poi in &overpass_pois {
                    // Use transaction instead of pool
                    let result = sqlx::query(
                        r#"
                        INSERT INTO pois (id, name, category, location, popularity_score, description, estimated_visit_duration_minutes, osm_id)
                        VALUES ($1, $2, $3, ST_GeogFromText($4), $5, $6, $7, $8)
                        ON CONFLICT (osm_id) DO NOTHING
                        "#
                    )
                    .bind(poi.id)
                    .bind(&poi.name)
                    .bind(poi.category.to_string())
                    .bind(format!("POINT({} {})", poi.coordinates.lng, poi.coordinates.lat))
                    .bind(poi.popularity_score)
                    .bind(&poi.description)
                    .bind(poi.estimated_visit_duration_minutes.map(|d| d as i32))
                    .bind(poi.osm_id)
                    .execute(&mut *transaction)
                    .await;

                    match result {
                        Ok(_) => inserted_count += 1,
                        Err(e) => tracing::debug!("Failed to insert POI {}: {}", poi.name, e),
                    }
                }

                if let Err(e) = transaction.commit().await {
                    tracing::warn!("Failed to commit POI transaction: {}", e);
                } else {
                    tracing::debug!("Inserted {} POIs in transaction", inserted_count);
                }

                tracing::info!(
                    "Fetched {} POIs from Overpass API (single query)",
                    overpass_pois.len()
                );
                Ok(overpass_pois.into_iter().take(limit).collect())
            }
            Err(e) => {
                let error_str = e.to_string();
                let is_timeout = error_str.contains("timed out") || error_str.contains("timeout");

                // On timeout, try batched parallel queries for resilience
                if is_timeout && categories_to_fetch.len() > 3 {
                    tracing::warn!(
                        "Single query timed out, falling back to batched parallel queries"
                    );

                    match self
                        .overpass_client
                        .query_pois_batched(center, radius_meters, &categories_to_fetch)
                        .await
                    {
                        Ok(batched_pois) => {
                            // Store batched results using transaction
                            if let Ok(mut tx) = self.db_pool.begin().await {
                                let mut inserted = 0;
                                for poi in &batched_pois {
                                    let result = sqlx::query(
                                        r#"
                                        INSERT INTO pois (id, name, category, location, popularity_score, description, estimated_visit_duration_minutes, osm_id)
                                        VALUES ($1, $2, $3, ST_GeogFromText($4), $5, $6, $7, $8)
                                        ON CONFLICT (osm_id) DO NOTHING
                                        "#
                                    )
                                    .bind(poi.id)
                                    .bind(&poi.name)
                                    .bind(poi.category.to_string())
                                    .bind(format!("POINT({} {})", poi.coordinates.lng, poi.coordinates.lat))
                                    .bind(poi.popularity_score)
                                    .bind(&poi.description)
                                    .bind(poi.estimated_visit_duration_minutes.map(|d| d as i32))
                                    .bind(poi.osm_id)
                                    .execute(&mut *tx)
                                    .await;

                                    if result.is_ok() {
                                        inserted += 1;
                                    }
                                }
                                let _ = tx.commit().await;
                                tracing::debug!("Inserted {} POIs from batched query", inserted);
                            }

                            tracing::info!(
                                "Fetched {} POIs from Overpass API (batched queries)",
                                batched_pois.len()
                            );
                            return Ok(batched_pois.into_iter().take(limit).collect());
                        }
                        Err(batch_error) => {
                            tracing::warn!(
                                "Batched queries also failed ({}), using database fallback",
                                batch_error
                            );
                        }
                    }
                }

                // All Overpass attempts failed - fall back to database POIs
                tracing::warn!(
                    "Overpass API failed ({}), falling back to {} database POIs",
                    e,
                    db_pois.len()
                );

                if db_pois.is_empty() {
                    return Err(e);
                }

                Ok(db_pois.into_iter().take(limit).collect())
            }
        }
    }

    /// Score and filter POIs based on preferences
    pub fn score_and_filter_pois(
        &self,
        pois: Vec<Poi>,
        hidden_gems: bool,
        max_count: usize,
    ) -> Vec<Poi> {
        let mut scored_pois: Vec<(f32, Poi)> = pois
            .into_iter()
            .map(|poi| {
                let score = poi.quality_score(hidden_gems);
                (score, poi)
            })
            .collect();

        // Sort by score descending
        scored_pois.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored_pois
            .into_iter()
            .take(max_count)
            .map(|(_, poi)| poi)
            .collect()
    }

    fn default_categories() -> Vec<PoiCategory> {
        vec![
            // Original high-value categories
            PoiCategory::Monument,
            PoiCategory::Viewpoint,
            PoiCategory::Park,
            PoiCategory::Museum,
            PoiCategory::Historic,
            PoiCategory::Cultural,
            // Most commonly useful new categories
            PoiCategory::Church,
            PoiCategory::Castle,
            PoiCategory::Plaza,
        ]
    }
}

#[cfg(test)]
mod tests {
    // Tests removed for now - need async test setup
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_score_and_filter_basic() {
        // Test the scoring logic without database
        let poi1 = Poi::new(
            "Popular".to_string(),
            PoiCategory::Monument,
            Coordinates::new(48.8566, 2.3522).unwrap(),
            90.0,
        );

        assert_eq!(poi1.quality_score(false), 90.0);
        assert_eq!(poi1.quality_score(true), 10.0);
    }
}
