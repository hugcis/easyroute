use crate::db::PoiRepository;
use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, PoiCategory};
use std::sync::Arc;

pub struct PoiService {
    repo: Arc<dyn PoiRepository>,
}

impl PoiService {
    pub fn new(repo: Arc<dyn PoiRepository>) -> Self {
        PoiService { repo }
    }

    /// Find POIs within a radius, with optional category filtering
    /// Uses only the local PostgreSQL/PostGIS database with imported OSM data
    pub async fn find_pois(
        &self,
        center: &Coordinates,
        radius_km: f64,
        categories: Option<&[PoiCategory]>,
        limit: usize,
    ) -> Result<Vec<Poi>> {
        let radius_meters = radius_km * 1000.0;

        // Query database for POIs
        let db_pois = self
            .repo
            .find_within_radius(center, radius_meters, categories, limit as i64)
            .await?;

        // Check if we found any POIs
        if !db_pois.is_empty() {
            tracing::info!(
                "Found {} POIs in database within {:.1}km of ({:.4}, {:.4})",
                db_pois.len(),
                radius_km,
                center.lat,
                center.lng
            );
            return Ok(db_pois.into_iter().take(limit).collect());
        }

        // No POIs found - return clear error
        tracing::warn!(
            "No POIs found in database within {:.1}km of ({:.4}, {:.4})",
            radius_km,
            center.lat,
            center.lng
        );

        Err(AppError::NoPoisFound(format!(
            "No POIs found in database within {:.1}km of coordinates ({:.4}, {:.4}). \
             This area may not be covered by the current OSM import. \
             Try a different location or contact support to request data import for this region.",
            radius_km, center.lat, center.lng
        )))
    }

    /// Score and filter POIs based on preferences
    pub fn select_top_pois(&self, pois: Vec<Poi>, hidden_gems: bool, max_count: usize) -> Vec<Poi> {
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
}

#[cfg(test)]
mod tests {
    // Tests removed for now - need async test setup
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_select_top_pois_basic() {
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
