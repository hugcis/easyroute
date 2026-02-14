use crate::error::Result;
use crate::models::{Coordinates, Poi, PoiCategory};
use async_trait::async_trait;
use uuid::Uuid;

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
