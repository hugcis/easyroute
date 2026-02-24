use crate::models::{Coordinates, Poi, PoiCategory};
use sqlx::PgPool;
use uuid::Uuid;

use super::poi_repository::RawPoiRow;

pub async fn find_pois_within_radius(
    pool: &PgPool,
    center: &Coordinates,
    radius_meters: f64,
    categories: Option<&[PoiCategory]>,
    limit: i64,
) -> Result<Vec<Poi>, sqlx::Error> {
    let point_wkt = format!("POINT({} {})", center.lng, center.lat);
    let category_strs = categories_to_strings(categories);

    let (category_clause, limit_param) = if category_strs.is_some() {
        ("AND category = ANY($3)", "$4")
    } else {
        ("", "$3")
    };

    let sql = format!(
        "SELECT id, name, category,
                ST_Y(location::geometry) as lat, ST_X(location::geometry) as lng,
                popularity_score, description, estimated_visit_duration_minutes,
                osm_id, ST_Distance(location, ST_GeogFromText($1)) as distance_meters
         FROM pois
         WHERE ST_DWithin(location, ST_GeogFromText($1), $2)
         {category_clause}
         ORDER BY distance_meters
         LIMIT {limit_param}"
    );

    let mut query = sqlx::query_as::<_, PoiRow>(&sql)
        .bind(&point_wkt)
        .bind(radius_meters);

    if let Some(ref cats) = category_strs {
        query = query.bind(cats);
    }

    let rows = query.bind(limit).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| row.into_raw().into_poi())
        .collect())
}

pub async fn find_pois_in_bbox(
    pool: &PgPool,
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    categories: Option<&[PoiCategory]>,
    limit: i64,
) -> Result<Vec<Poi>, sqlx::Error> {
    let category_strs = categories_to_strings(categories);

    let (category_clause, limit_param) = if category_strs.is_some() {
        ("AND category = ANY($5)", "$6")
    } else {
        ("", "$5")
    };

    let sql = format!(
        "SELECT id, name, category,
                ST_Y(location::geometry) as lat, ST_X(location::geometry) as lng,
                popularity_score, description, estimated_visit_duration_minutes,
                osm_id, NULL::float8 as distance_meters
         FROM pois
         WHERE ST_Y(location::geometry) BETWEEN $1 AND $2
         AND ST_X(location::geometry) BETWEEN $3 AND $4
         {category_clause}
         LIMIT {limit_param}"
    );

    let mut query = sqlx::query_as::<_, PoiRow>(&sql)
        .bind(min_lat)
        .bind(max_lat)
        .bind(min_lng)
        .bind(max_lng);

    if let Some(ref cats) = category_strs {
        query = query.bind(cats);
    }

    let rows = query.bind(limit).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|row| row.into_raw().into_poi())
        .collect())
}

fn categories_to_strings(categories: Option<&[PoiCategory]>) -> Option<Vec<String>> {
    categories.map(|cats| cats.iter().map(|c| c.to_string()).collect())
}

pub async fn insert_poi(pool: &PgPool, poi: &Poi) -> Result<Uuid, sqlx::Error> {
    let point_wkt = format!("POINT({} {})", poi.coordinates.lng, poi.coordinates.lat);

    let result: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO pois (id, name, category, location, popularity_score, description, estimated_visit_duration_minutes, osm_id)
        VALUES ($1, $2, $3, ST_GeogFromText($4), $5, $6, $7, $8)
        RETURNING id
        "#
    )
    .bind(poi.id)
    .bind(&poi.name)
    .bind(poi.category.to_string())
    .bind(&point_wkt)
    .bind(poi.popularity_score)
    .bind(&poi.description)
    .bind(poi.estimated_visit_duration_minutes.map(|d| d as i32))
    .bind(poi.osm_id)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

/// Get clustered POI coverage hulls as GeoJSON via DBSCAN clustering.
/// Returns (coverage_geojson, poi_count, cluster_count).
pub async fn get_poi_coverage(pool: &PgPool) -> Result<(Option<String>, i64, i64), sqlx::Error> {
    let row: (Option<String>, i64, i64) = sqlx::query_as(
        r#"
        WITH clusters AS (
            SELECT
                ST_ClusterDBSCAN(location::geometry, eps := 0.05, minpoints := 2) OVER () as cluster_id,
                location::geometry as geom
            FROM pois
        ),
        cluster_hulls AS (
            SELECT
                CASE
                    WHEN COUNT(*) >= 3 THEN ST_ConcaveHull(ST_Collect(geom), 0.3)
                    ELSE ST_Buffer(ST_Collect(geom), 0.001)
                END as hull
            FROM clusters
            WHERE cluster_id IS NOT NULL
            GROUP BY cluster_id

            UNION ALL

            SELECT ST_Buffer(geom, 0.001) as hull
            FROM clusters
            WHERE cluster_id IS NULL
        )
        SELECT
            ST_AsGeoJSON(ST_Union(hull)) as coverage_geojson,
            (SELECT COUNT(*) FROM pois) as poi_count,
            (SELECT COUNT(DISTINCT cluster_id) FROM clusters WHERE cluster_id IS NOT NULL) as cluster_count
        FROM cluster_hulls
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

#[derive(sqlx::FromRow)]
struct PoiRow {
    id: Uuid,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f32,
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
    osm_id: Option<i64>,
    #[allow(dead_code)]
    distance_meters: Option<f64>,
}

impl PoiRow {
    fn into_raw(self) -> RawPoiRow {
        RawPoiRow {
            id: self.id,
            name: self.name,
            category: self.category,
            lat: self.lat,
            lng: self.lng,
            popularity_score: self.popularity_score,
            description: self.description,
            estimated_visit_duration_minutes: self.estimated_visit_duration_minutes,
            osm_id: self.osm_id,
        }
    }
}
