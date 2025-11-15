use crate::models::{Coordinates, Poi, PoiCategory};
use sqlx::PgPool;
use uuid::Uuid;

/// Find POIs within a radius from a center point
/// Uses PostGIS ST_DWithin for efficient spatial queries
pub async fn find_pois_within_radius(
    pool: &PgPool,
    center: &Coordinates,
    radius_meters: f64,
    categories: Option<&[PoiCategory]>,
    limit: i64,
) -> Result<Vec<Poi>, sqlx::Error> {
    let point_wkt = format!("POINT({} {})", center.lng, center.lat);

    let query = if let Some(cats) = categories {
        let category_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();

        sqlx::query_as::<_, PoiRow>(
            r#"
            SELECT
                id,
                name,
                category,
                ST_Y(location::geometry) as lat,
                ST_X(location::geometry) as lng,
                popularity_score,
                description,
                estimated_visit_duration_minutes,
                osm_id,
                ST_Distance(location, ST_GeogFromText($1)) as distance_meters
            FROM pois
            WHERE ST_DWithin(
                location,
                ST_GeogFromText($1),
                $2
            )
            AND category = ANY($3)
            ORDER BY distance_meters
            LIMIT $4
            "#,
        )
        .bind(&point_wkt)
        .bind(radius_meters)
        .bind(&category_strs)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, PoiRow>(
            r#"
            SELECT
                id,
                name,
                category,
                ST_Y(location::geometry) as lat,
                ST_X(location::geometry) as lng,
                popularity_score,
                description,
                estimated_visit_duration_minutes,
                osm_id,
                ST_Distance(location, ST_GeogFromText($1)) as distance_meters
            FROM pois
            WHERE ST_DWithin(
                location,
                ST_GeogFromText($1),
                $2
            )
            ORDER BY distance_meters
            LIMIT $3
            "#,
        )
        .bind(&point_wkt)
        .bind(radius_meters)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(query.into_iter().map(|row| row.into()).collect())
}

/// Find POIs within a bounding box
/// More efficient than radius query for linear route paths
pub async fn find_pois_in_bbox(
    pool: &PgPool,
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    categories: Option<&[PoiCategory]>,
    limit: i64,
) -> Result<Vec<Poi>, sqlx::Error> {
    let query = if let Some(cats) = categories {
        let category_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();

        sqlx::query_as::<_, PoiRowSimple>(
            r#"
            SELECT
                id,
                name,
                category,
                ST_Y(location::geometry) as lat,
                ST_X(location::geometry) as lng,
                popularity_score,
                description,
                estimated_visit_duration_minutes,
                osm_id
            FROM pois
            WHERE ST_Y(location::geometry) BETWEEN $1 AND $2
            AND ST_X(location::geometry) BETWEEN $3 AND $4
            AND category = ANY($5)
            LIMIT $6
            "#,
        )
        .bind(min_lat)
        .bind(max_lat)
        .bind(min_lng)
        .bind(max_lng)
        .bind(&category_strs)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, PoiRowSimple>(
            r#"
            SELECT
                id,
                name,
                category,
                ST_Y(location::geometry) as lat,
                ST_X(location::geometry) as lng,
                popularity_score,
                description,
                estimated_visit_duration_minutes,
                osm_id
            FROM pois
            WHERE ST_Y(location::geometry) BETWEEN $1 AND $2
            AND ST_X(location::geometry) BETWEEN $3 AND $4
            LIMIT $5
            "#,
        )
        .bind(min_lat)
        .bind(max_lat)
        .bind(min_lng)
        .bind(max_lng)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(query.into_iter().map(|row| row.into()).collect())
}

/// Insert a POI into the database
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

// Helper struct for deserializing POI rows from database
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
    distance_meters: f64,
}

impl From<PoiRow> for Poi {
    fn from(row: PoiRow) -> Self {
        // Parse category with warning on failure
        let category = row.category.parse().unwrap_or_else(|_| {
            tracing::warn!(
                "Invalid POI category '{}' for POI '{}' (id: {}), defaulting to Historic",
                row.category,
                row.name,
                row.id
            );
            PoiCategory::Historic
        });

        // Construct coordinates safely - they should always be valid from DB
        // but we validate anyway for defense in depth
        let coordinates = Coordinates::new(row.lat, row.lng).unwrap_or_else(|e| {
            tracing::error!(
                "Invalid coordinates for POI '{}' (id: {}): {}. Using fallback.",
                row.name,
                row.id,
                e
            );
            // Fallback to null island (should never happen with proper DB constraints)
            Coordinates { lat: 0.0, lng: 0.0 }
        });

        // Safely convert visit duration, rejecting negative values
        let estimated_visit_duration_minutes = row.estimated_visit_duration_minutes
            .and_then(|d| if d >= 0 { Some(d as u32) } else {
                tracing::warn!(
                    "Negative visit duration {} for POI '{}', ignoring",
                    d,
                    row.name
                );
                None
            });

        Poi {
            id: row.id,
            name: row.name,
            category,
            coordinates,
            popularity_score: row.popularity_score,
            description: row.description,
            estimated_visit_duration_minutes,
            osm_id: row.osm_id,
        }
    }
}

// Helper struct for deserializing POI rows (without distance)
#[derive(sqlx::FromRow)]
struct PoiRowSimple {
    id: Uuid,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f32,
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
    osm_id: Option<i64>,
}

impl From<PoiRowSimple> for Poi {
    fn from(row: PoiRowSimple) -> Self {
        // Parse category with warning on failure
        let category = row.category.parse().unwrap_or_else(|_| {
            tracing::warn!(
                "Invalid POI category '{}' for POI '{}' (id: {}), defaulting to Historic",
                row.category,
                row.name,
                row.id
            );
            PoiCategory::Historic
        });

        // Construct coordinates safely
        let coordinates = Coordinates::new(row.lat, row.lng).unwrap_or_else(|e| {
            tracing::error!(
                "Invalid coordinates for POI '{}' (id: {}): {}. Using fallback.",
                row.name,
                row.id,
                e
            );
            Coordinates { lat: 0.0, lng: 0.0 }
        });

        // Safely convert visit duration
        let estimated_visit_duration_minutes = row.estimated_visit_duration_minutes
            .and_then(|d| if d >= 0 { Some(d as u32) } else {
                tracing::warn!(
                    "Negative visit duration {} for POI '{}', ignoring",
                    d,
                    row.name
                );
                None
            });

        Poi {
            id: row.id,
            name: row.name,
            category,
            coordinates,
            popularity_score: row.popularity_score,
            description: row.description,
            estimated_visit_duration_minutes,
            osm_id: row.osm_id,
        }
    }
}
