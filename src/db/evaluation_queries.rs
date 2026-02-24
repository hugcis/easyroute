use crate::models::evaluation::{EvaluatedRoute, MetricCorrelation, RouteRating};
use sqlx::PgPool;
use uuid::Uuid;

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

/// Fetches an evaluated route by ID, including all associated ratings.
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

/// Compute Pearson correlations between each route metric and average human rating.
pub async fn get_correlation_data(pool: &PgPool) -> Result<Vec<MetricCorrelation>, sqlx::Error> {
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

    type MetricField = (&'static str, fn(&CorrelationRow) -> Option<f32>);
    let metrics: [MetricField; 6] = [
        ("circularity", |r| r.circularity),
        ("convexity", |r| r.convexity),
        ("path_overlap_pct", |r| r.path_overlap_pct),
        ("poi_density_per_km", |r| r.poi_density_per_km),
        ("category_entropy", |r| r.category_entropy),
        ("landmark_coverage", |r| r.landmark_coverage),
    ];

    let correlations = metrics
        .into_iter()
        .filter_map(|(name, extract)| {
            let pairs: Vec<(f64, f64)> = rows
                .iter()
                .zip(ratings.iter())
                .filter_map(|(row, &rating)| extract(row).map(|m| (m as f64, rating)))
                .collect();

            (pairs.len() >= 2).then(|| MetricCorrelation {
                metric_name: name.to_string(),
                pearson_r: pearson_correlation(&pairs),
                sample_count: pairs.len(),
            })
        })
        .collect();

    Ok(correlations)
}

pub async fn get_evaluation_counts(pool: &PgPool) -> Result<(i64, i64), sqlx::Error> {
    let route_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evaluated_routes")
        .fetch_one(pool)
        .await?;
    let rating_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM route_ratings")
        .fetch_one(pool)
        .await?;
    Ok((route_count, rating_count))
}

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
