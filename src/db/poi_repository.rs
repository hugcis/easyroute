use crate::error::Result;
use crate::models::{Coordinates, Poi, PoiCategory};
use async_trait::async_trait;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared row-to-Poi conversion helpers (used by both Pg and SQLite repos)
// ---------------------------------------------------------------------------

/// Raw POI fields extracted from a database row, before validation.
/// Both PostgreSQL and SQLite implementations populate this struct,
/// then call `into_poi()` for shared validation logic.
pub(super) struct RawPoiRow {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub lat: f64,
    pub lng: f64,
    pub popularity_score: f32,
    pub description: Option<String>,
    pub estimated_visit_duration_minutes: Option<i32>,
    pub osm_id: Option<i64>,
}

impl RawPoiRow {
    pub fn into_poi(self) -> Poi {
        let category = self.category.parse().unwrap_or_else(|_| {
            tracing::warn!(
                "Invalid POI category '{}' for POI '{}' (id: {}), defaulting to Historic",
                self.category,
                self.name,
                self.id
            );
            PoiCategory::Historic
        });

        let coordinates = Coordinates::new(self.lat, self.lng).unwrap_or_else(|e| {
            tracing::error!(
                "Invalid coordinates for POI '{}' (id: {}): {}. Using fallback.",
                self.name,
                self.id,
                e
            );
            Coordinates { lat: 0.0, lng: 0.0 }
        });

        let estimated_visit_duration_minutes =
            self.estimated_visit_duration_minutes.and_then(|d| {
                if d >= 0 {
                    Some(d as u32)
                } else {
                    tracing::warn!(
                        "Negative visit duration {} for POI '{}', ignoring",
                        d,
                        self.name
                    );
                    None
                }
            });

        Poi {
            id: self.id,
            name: self.name,
            category,
            coordinates,
            popularity_score: self.popularity_score,
            description: self.description,
            estimated_visit_duration_minutes,
            osm_id: self.osm_id,
        }
    }
}

#[async_trait]
pub trait PoiRepository: Send + Sync {
    async fn find_within_radius(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: Option<&[PoiCategory]>,
        limit: i64,
    ) -> Result<Vec<Poi>>;

    async fn find_in_bbox(
        &self,
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
        categories: Option<&[PoiCategory]>,
        limit: i64,
    ) -> Result<Vec<Poi>>;

    async fn insert(&self, poi: &Poi) -> Result<Uuid>;

    async fn count(&self) -> Result<i64>;
}

pub struct PgPoiRepository {
    pool: sqlx::PgPool,
}

impl PgPoiRepository {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

#[async_trait]
impl PoiRepository for PgPoiRepository {
    async fn find_within_radius(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: Option<&[PoiCategory]>,
        limit: i64,
    ) -> Result<Vec<Poi>> {
        Ok(super::poi_queries::find_pois_within_radius(
            &self.pool,
            center,
            radius_meters,
            categories,
            limit,
        )
        .await?)
    }

    async fn find_in_bbox(
        &self,
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
        categories: Option<&[PoiCategory]>,
        limit: i64,
    ) -> Result<Vec<Poi>> {
        Ok(super::poi_queries::find_pois_in_bbox(
            &self.pool, min_lat, max_lat, min_lng, max_lng, categories, limit,
        )
        .await?)
    }

    async fn insert(&self, poi: &Poi) -> Result<Uuid> {
        Ok(super::poi_queries::insert_poi(&self.pool, poi).await?)
    }

    async fn count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pois")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}
