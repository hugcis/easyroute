use crate::models::evaluation::{EvaluatedRoute, MetricCorrelation, RouteRating};
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

    let rows = if let Some(cats) = categories {
        let category_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();
        execute_radius_query_with_categories(pool, &point_wkt, radius_meters, &category_strs, limit)
            .await?
    } else {
        execute_radius_query_without_categories(pool, &point_wkt, radius_meters, limit).await?
    };

    Ok(rows.into_iter().map(|row| row.into()).collect())
}

/// Execute radius query with category filtering
async fn execute_radius_query_with_categories(
    pool: &PgPool,
    point_wkt: &str,
    radius_meters: f64,
    category_strs: &[String],
    limit: i64,
) -> Result<Vec<PoiRow>, sqlx::Error> {
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
    .bind(point_wkt)
    .bind(radius_meters)
    .bind(category_strs)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Execute radius query without category filtering
async fn execute_radius_query_without_categories(
    pool: &PgPool,
    point_wkt: &str,
    radius_meters: f64,
    limit: i64,
) -> Result<Vec<PoiRow>, sqlx::Error> {
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
    .bind(point_wkt)
    .bind(radius_meters)
    .bind(limit)
    .fetch_all(pool)
    .await
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
    let rows = if let Some(cats) = categories {
        let category_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();
        execute_bbox_query_with_categories(
            pool,
            min_lat,
            max_lat,
            min_lng,
            max_lng,
            &category_strs,
            limit,
        )
        .await?
    } else {
        execute_bbox_query_without_categories(pool, min_lat, max_lat, min_lng, max_lng, limit)
            .await?
    };

    Ok(rows.into_iter().map(|row| row.into()).collect())
}

/// Execute bbox query with category filtering
async fn execute_bbox_query_with_categories(
    pool: &PgPool,
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    category_strs: &[String],
    limit: i64,
) -> Result<Vec<PoiRow>, sqlx::Error> {
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
            NULL::float8 as distance_meters
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
    .bind(category_strs)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Execute bbox query without category filtering
async fn execute_bbox_query_without_categories(
    pool: &PgPool,
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    limit: i64,
) -> Result<Vec<PoiRow>, sqlx::Error> {
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
            NULL::float8 as distance_meters
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
    .await
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

/// Get clustered coverage hulls of all POIs as GeoJSON, plus total POI count.
/// Uses DBSCAN clustering so separate geographic regions appear as distinct polygons
/// rather than one convex blob spanning empty space.
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
    // Distance is optional - only present in radius queries
    // Not used in conversion but needed for deserialization
    #[allow(dead_code)]
    distance_meters: Option<f64>,
}

// --- Evaluation queries ---

/// Insert an evaluated route into the database
pub async fn insert_evaluated_route(
    pool: &PgPool,
    route: &EvaluatedRoute,
) -> Result<Uuid, sqlx::Error> {
    let poi_names_json = serde_json::to_value(&route.poi_names).unwrap_or_default();
    let path_json = serde_json::Value::Null; // Path stored separately or as empty

    let result: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO evaluated_routes (
            id, start_lat, start_lng, target_distance_km, transport_mode,
            actual_distance_km, duration_minutes, path, poi_names,
            poi_count, snapped_poi_count,
            circularity, convexity, path_overlap_pct,
            poi_density_per_km, category_entropy, landmark_coverage,
            system_score, poi_density_context, scoring_strategy
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
        RETURNING id
        "#,
    )
    .bind(route.id)
    .bind(route.start_lat)
    .bind(route.start_lng)
    .bind(route.target_distance_km)
    .bind(&route.transport_mode)
    .bind(route.actual_distance_km)
    .bind(route.duration_minutes)
    .bind(&path_json)
    .bind(&poi_names_json)
    .bind(route.poi_count)
    .bind(route.snapped_poi_count)
    .bind(route.circularity)
    .bind(route.convexity)
    .bind(route.path_overlap_pct)
    .bind(route.poi_density_per_km)
    .bind(route.category_entropy)
    .bind(route.landmark_coverage)
    .bind(route.system_score)
    .bind(&route.poi_density_context)
    .bind(&route.scoring_strategy)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

/// Insert a route rating
#[allow(clippy::too_many_arguments)]
pub async fn insert_route_rating(
    pool: &PgPool,
    route_id: Uuid,
    overall_rating: i16,
    shape_rating: Option<i16>,
    scenicness_rating: Option<i16>,
    variety_rating: Option<i16>,
    comment: Option<&str>,
    rater_id: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let result: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO route_ratings (route_id, overall_rating, shape_rating, scenicness_rating, variety_rating, comment, rater_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(route_id)
    .bind(overall_rating)
    .bind(shape_rating)
    .bind(scenicness_rating)
    .bind(variety_rating)
    .bind(comment)
    .bind(rater_id)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

/// Get an evaluated route by ID with its ratings
pub async fn get_evaluated_route(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<EvaluatedRoute>, sqlx::Error> {
    let row = sqlx::query_as::<_, EvaluatedRouteRow>(
        r#"
        SELECT id, start_lat, start_lng, target_distance_km, transport_mode,
               actual_distance_km, duration_minutes, poi_names, poi_count, snapped_poi_count,
               circularity, convexity, path_overlap_pct,
               poi_density_per_km, category_entropy, landmark_coverage,
               system_score, poi_density_context, scoring_strategy,
               created_at::text as created_at
        FROM evaluated_routes
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let ratings = sqlx::query_as::<_, RouteRatingRow>(
        r#"
        SELECT id, route_id, overall_rating, shape_rating, scenicness_rating,
               variety_rating, comment, rater_id, created_at::text as created_at
        FROM route_ratings
        WHERE route_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let mut route: EvaluatedRoute = row.into();
    route.ratings = Some(ratings.into_iter().map(|r| r.into()).collect());
    Ok(Some(route))
}

/// List evaluated routes with pagination
pub async fn list_evaluated_routes(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<EvaluatedRoute>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EvaluatedRouteRow>(
        r#"
        SELECT id, start_lat, start_lng, target_distance_km, transport_mode,
               actual_distance_km, duration_minutes, poi_names, poi_count, snapped_poi_count,
               circularity, convexity, path_overlap_pct,
               poi_density_per_km, category_entropy, landmark_coverage,
               system_score, poi_density_context, scoring_strategy,
               created_at::text as created_at
        FROM evaluated_routes
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Get correlation data between metrics and average human ratings
pub async fn get_correlation_data(pool: &PgPool) -> Result<Vec<MetricCorrelation>, sqlx::Error> {
    // Fetch routes that have at least one rating
    let rows = sqlx::query_as::<_, CorrelationRow>(
        r#"
        SELECT
            er.circularity,
            er.convexity,
            er.path_overlap_pct,
            er.poi_density_per_km,
            er.category_entropy,
            er.landmark_coverage,
            AVG(rr.overall_rating)::float8 as avg_rating
        FROM evaluated_routes er
        JOIN route_ratings rr ON rr.route_id = er.id
        GROUP BY er.id, er.circularity, er.convexity, er.path_overlap_pct,
                 er.poi_density_per_km, er.category_entropy, er.landmark_coverage
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.len() < 2 {
        return Ok(vec![]);
    }

    let ratings: Vec<f64> = rows.iter().map(|r| r.avg_rating).collect();

    type MetricExtractor = (&'static str, Box<dyn Fn(&CorrelationRow) -> Option<f64>>);
    let metric_extractors: Vec<MetricExtractor> = vec![
        ("circularity", Box::new(|r| r.circularity.map(|v| v as f64))),
        ("convexity", Box::new(|r| r.convexity.map(|v| v as f64))),
        (
            "path_overlap_pct",
            Box::new(|r| r.path_overlap_pct.map(|v| v as f64)),
        ),
        (
            "poi_density_per_km",
            Box::new(|r| r.poi_density_per_km.map(|v| v as f64)),
        ),
        (
            "category_entropy",
            Box::new(|r| r.category_entropy.map(|v| v as f64)),
        ),
        (
            "landmark_coverage",
            Box::new(|r| r.landmark_coverage.map(|v| v as f64)),
        ),
    ];

    let mut correlations = Vec::new();

    for (name, extractor) in &metric_extractors {
        let metric_values: Vec<Option<f64>> = rows.iter().map(extractor).collect();

        // Filter to only rows where both metric and rating are present
        let pairs: Vec<(f64, f64)> = metric_values
            .iter()
            .zip(ratings.iter())
            .filter_map(|(mv, &rv)| mv.map(|m| (m, rv)))
            .collect();

        if pairs.len() >= 2 {
            let r = pearson_correlation(&pairs);
            correlations.push(MetricCorrelation {
                metric_name: name.to_string(),
                pearson_r: r,
                sample_count: pairs.len(),
            });
        }
    }

    Ok(correlations)
}

/// Get total counts for evaluation stats
pub async fn get_evaluation_counts(pool: &PgPool) -> Result<(i64, i64), sqlx::Error> {
    let route_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM evaluated_routes")
        .fetch_one(pool)
        .await?;
    let rating_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM route_ratings")
        .fetch_one(pool)
        .await?;
    Ok((route_count.0, rating_count.0))
}

/// Compute Pearson correlation coefficient
fn pearson_correlation(pairs: &[(f64, f64)]) -> f64 {
    let n = pairs.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let sum_x: f64 = pairs.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = pairs.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = pairs.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = pairs.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = pairs.iter().map(|(_, y)| y * y).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x.powi(2)) * (n * sum_y2 - sum_y.powi(2))).sqrt();

    if denominator.abs() < 1e-10 {
        return 0.0;
    }

    (numerator / denominator).clamp(-1.0, 1.0)
}

// --- Evaluation helper structs ---

#[derive(sqlx::FromRow)]
struct EvaluatedRouteRow {
    id: Uuid,
    start_lat: f64,
    start_lng: f64,
    target_distance_km: f64,
    transport_mode: String,
    actual_distance_km: f64,
    duration_minutes: i32,
    poi_names: serde_json::Value,
    poi_count: i32,
    snapped_poi_count: i32,
    circularity: Option<f32>,
    convexity: Option<f32>,
    path_overlap_pct: Option<f32>,
    poi_density_per_km: Option<f32>,
    category_entropy: Option<f32>,
    landmark_coverage: Option<f32>,
    system_score: f32,
    poi_density_context: Option<String>,
    scoring_strategy: String,
    created_at: Option<String>,
}

impl From<EvaluatedRouteRow> for EvaluatedRoute {
    fn from(row: EvaluatedRouteRow) -> Self {
        let poi_names: Vec<String> = serde_json::from_value(row.poi_names).unwrap_or_default();

        EvaluatedRoute {
            id: row.id,
            start_lat: row.start_lat,
            start_lng: row.start_lng,
            target_distance_km: row.target_distance_km,
            transport_mode: row.transport_mode,
            actual_distance_km: row.actual_distance_km,
            duration_minutes: row.duration_minutes,
            poi_names,
            poi_count: row.poi_count,
            snapped_poi_count: row.snapped_poi_count,
            circularity: row.circularity,
            convexity: row.convexity,
            path_overlap_pct: row.path_overlap_pct,
            poi_density_per_km: row.poi_density_per_km,
            category_entropy: row.category_entropy,
            landmark_coverage: row.landmark_coverage,
            system_score: row.system_score,
            poi_density_context: row.poi_density_context,
            scoring_strategy: row.scoring_strategy,
            created_at: row.created_at,
            ratings: None,
        }
    }
}

#[derive(sqlx::FromRow)]
struct RouteRatingRow {
    id: Uuid,
    route_id: Uuid,
    overall_rating: i16,
    shape_rating: Option<i16>,
    scenicness_rating: Option<i16>,
    variety_rating: Option<i16>,
    comment: Option<String>,
    rater_id: Option<String>,
    created_at: Option<String>,
}

impl From<RouteRatingRow> for RouteRating {
    fn from(row: RouteRatingRow) -> Self {
        RouteRating {
            id: row.id,
            route_id: row.route_id,
            overall_rating: row.overall_rating,
            shape_rating: row.shape_rating,
            scenicness_rating: row.scenicness_rating,
            variety_rating: row.variety_rating,
            comment: row.comment,
            rater_id: row.rater_id,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CorrelationRow {
    circularity: Option<f32>,
    convexity: Option<f32>,
    path_overlap_pct: Option<f32>,
    poi_density_per_km: Option<f32>,
    category_entropy: Option<f32>,
    landmark_coverage: Option<f32>,
    avg_rating: f64,
}

// --- POI helper structs ---

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
        let estimated_visit_duration_minutes = row.estimated_visit_duration_minutes.and_then(|d| {
            if d >= 0 {
                Some(d as u32)
            } else {
                tracing::warn!(
                    "Negative visit duration {} for POI '{}', ignoring",
                    d,
                    row.name
                );
                None
            }
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
