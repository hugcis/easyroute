use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{BoundingBox, Coordinates, Poi, PoiCategory};

use super::poi_repository::{PoiRepository, RawPoiRow};

// ---------------------------------------------------------------------------
// Row type (SQLite-specific)
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SqlitePoiRow {
    id: String,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f64, // SQLite REAL is always f64
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
    osm_id: Option<i64>,
}

impl SqlitePoiRow {
    fn into_poi(self) -> Poi {
        let id = self.id.parse::<Uuid>().unwrap_or_else(|_| {
            tracing::warn!(
                "Invalid UUID '{}' for POI '{}', using nil",
                self.id,
                self.name
            );
            Uuid::nil()
        });

        RawPoiRow {
            id,
            name: self.name,
            category: self.category,
            lat: self.lat,
            lng: self.lng,
            popularity_score: self.popularity_score as f32,
            description: self.description,
            estimated_visit_duration_minutes: self.estimated_visit_duration_minutes,
            osm_id: self.osm_id,
        }
        .into_poi()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_category_filter(categories: Option<&[PoiCategory]>) -> Option<HashSet<PoiCategory>> {
    categories.map(|cats| cats.iter().cloned().collect())
}

fn matches_category_filter(poi: &Poi, filter: &Option<HashSet<PoiCategory>>) -> bool {
    match filter {
        Some(cats) => cats.contains(&poi.category),
        None => true,
    }
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

pub struct SqlitePoiRepository {
    pool: SqlitePool,
}

impl SqlitePoiRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create the SQLite schema (tables + R-tree). Idempotent.
    pub async fn create_schema(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pois (
                rowid INTEGER PRIMARY KEY,
                id TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                category TEXT NOT NULL,
                lat REAL NOT NULL,
                lng REAL NOT NULL,
                popularity_score REAL NOT NULL DEFAULT 0.0,
                description TEXT,
                estimated_visit_duration_minutes INTEGER,
                osm_id INTEGER UNIQUE
            )",
        )
        .execute(pool)
        .await?;

        // R-tree virtual tables don't support IF NOT EXISTS — check sqlite_master.
        let rtree_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='pois_rtree')",
        )
        .fetch_one(pool)
        .await?;

        if !rtree_exists {
            sqlx::query(
                "CREATE VIRTUAL TABLE pois_rtree USING rtree(
                    id, min_lat, max_lat, min_lng, max_lng
                )",
            )
            .execute(pool)
            .await?;
        }

        sqlx::query("CREATE TABLE IF NOT EXISTS region_meta (key TEXT PRIMARY KEY, value TEXT)")
            .execute(pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_pois_category ON pois(category)")
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Insert a batch of POIs in a single transaction.
    ///
    /// Uses `INSERT OR IGNORE` so duplicate `osm_id`s are silently skipped.
    /// Returns the number of POIs actually inserted.
    pub async fn insert_batch(&self, pois: &[Poi]) -> std::result::Result<usize, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = 0usize;

        for poi in pois {
            let result = sqlx::query(
                "INSERT OR IGNORE INTO pois (id, name, category, lat, lng, popularity_score,
                                             description, estimated_visit_duration_minutes, osm_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(poi.id.to_string())
            .bind(&poi.name)
            .bind(poi.category.to_string())
            .bind(poi.coordinates.lat)
            .bind(poi.coordinates.lng)
            .bind(poi.popularity_score as f64)
            .bind(&poi.description)
            .bind(poi.estimated_visit_duration_minutes.map(|d| d as i32))
            .bind(poi.osm_id)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                let rowid: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
                    .fetch_one(&mut *tx)
                    .await?;

                sqlx::query(
                    "INSERT INTO pois_rtree (id, min_lat, max_lat, min_lng, max_lng)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(rowid)
                .bind(poi.coordinates.lat)
                .bind(poi.coordinates.lat)
                .bind(poi.coordinates.lng)
                .bind(poi.coordinates.lng)
                .execute(&mut *tx)
                .await?;

                inserted += 1;
            }
        }

        tx.commit().await?;
        Ok(inserted)
    }

    /// Set a key/value pair in the `region_meta` table.
    pub async fn set_meta(&self, key: &str, value: &str) -> std::result::Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO region_meta (key, value) VALUES (?1, ?2)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl PoiRepository for SqlitePoiRepository {
    async fn find_within_radius(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: Option<&[PoiCategory]>,
        limit: i64,
    ) -> Result<Vec<Poi>> {
        let bbox = BoundingBox::from_center_radius(center, radius_meters);

        // R-tree pre-filter
        let rows: Vec<SqlitePoiRow> = sqlx::query_as(
            "SELECT p.id, p.name, p.category, p.lat, p.lng, p.popularity_score,
                    p.description, p.estimated_visit_duration_minutes, p.osm_id
             FROM pois p
             INNER JOIN pois_rtree r ON p.rowid = r.id
             WHERE r.max_lat >= ?1 AND r.min_lat <= ?2
               AND r.max_lng >= ?3 AND r.min_lng <= ?4",
        )
        .bind(bbox.min_lat)
        .bind(bbox.max_lat)
        .bind(bbox.min_lng)
        .bind(bbox.max_lng)
        .fetch_all(&self.pool)
        .await?;

        let cat_filter = build_category_filter(categories);

        // Haversine post-filter + category filter + sort by distance + limit
        let mut results: Vec<(f64, Poi)> = rows
            .into_iter()
            .map(SqlitePoiRow::into_poi)
            .filter(|poi| matches_category_filter(poi, &cat_filter))
            .filter_map(|poi| {
                let dist_km = center.distance_to(&poi.coordinates);
                let dist_m = dist_km * 1000.0;
                if dist_m <= radius_meters {
                    Some((dist_m, poi))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results
            .into_iter()
            .map(|(_, poi)| poi)
            .take(limit as usize)
            .collect())
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
        let rows: Vec<SqlitePoiRow> = sqlx::query_as(
            "SELECT p.id, p.name, p.category, p.lat, p.lng, p.popularity_score,
                    p.description, p.estimated_visit_duration_minutes, p.osm_id
             FROM pois p
             INNER JOIN pois_rtree r ON p.rowid = r.id
             WHERE r.max_lat >= ?1 AND r.min_lat <= ?2
               AND r.max_lng >= ?3 AND r.min_lng <= ?4",
        )
        .bind(min_lat)
        .bind(max_lat)
        .bind(min_lng)
        .bind(max_lng)
        .fetch_all(&self.pool)
        .await?;

        let cat_filter = build_category_filter(categories);

        let results: Vec<Poi> = rows
            .into_iter()
            .map(SqlitePoiRow::into_poi)
            .filter(|poi| matches_category_filter(poi, &cat_filter))
            .take(limit as usize)
            .collect();

        Ok(results)
    }

    async fn insert(&self, poi: &Poi) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO pois (id, name, category, lat, lng, popularity_score,
                               description, estimated_visit_duration_minutes, osm_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(poi.id.to_string())
        .bind(&poi.name)
        .bind(poi.category.to_string())
        .bind(poi.coordinates.lat)
        .bind(poi.coordinates.lng)
        .bind(poi.popularity_score as f64)
        .bind(&poi.description)
        .bind(poi.estimated_visit_duration_minutes.map(|d| d as i32))
        .bind(poi.osm_id)
        .execute(&mut *tx)
        .await?;

        let rowid: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO pois_rtree (id, min_lat, max_lat, min_lng, max_lng)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(rowid)
        .bind(poi.coordinates.lat)
        .bind(poi.coordinates.lat)
        .bind(poi.coordinates.lng)
        .bind(poi.coordinates.lng)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(poi.id)
    }

    async fn count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pois")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
#[path = "sqlite_repo_tests.rs"]
mod sqlite_repo_tests;
