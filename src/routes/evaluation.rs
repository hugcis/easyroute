use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::queries;
use crate::error::AppError;
use crate::models::evaluation::{EvaluationStats, RatingRequest};

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

/// GET /api/v1/evaluations - List evaluated routes
pub async fn list_evaluations(
    State(pool): State<PgPool>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = params.limit.clamp(1, 100);
    let routes = queries::list_evaluated_routes(&pool, limit, params.offset).await?;

    Ok(Json(serde_json::json!({
        "routes": routes,
        "limit": limit,
        "offset": params.offset,
    })))
}

/// GET /api/v1/evaluations/:id - Get route detail with metrics and ratings
pub async fn get_evaluation(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let route = queries::get_evaluated_route(&pool, id).await?;

    match route {
        Some(r) => Ok(Json(serde_json::json!(r))),
        None => Err(AppError::NotFound(format!(
            "Evaluated route {} not found",
            id
        ))),
    }
}

/// POST /api/v1/evaluations/:id/ratings - Submit a rating
pub async fn submit_rating(
    State(pool): State<PgPool>,
    Path(route_id): Path<Uuid>,
    Json(req): Json<RatingRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    req.validate().map_err(AppError::InvalidRequest)?;

    // Verify route exists
    let route = queries::get_evaluated_route(&pool, route_id).await?;
    if route.is_none() {
        return Err(AppError::NotFound(format!(
            "Evaluated route {} not found",
            route_id
        )));
    }

    let rating_id = queries::insert_route_rating(
        &pool,
        route_id,
        req.overall_rating,
        req.shape_rating,
        req.scenicness_rating,
        req.variety_rating,
        req.comment.as_deref(),
        req.rater_id.as_deref(),
    )
    .await?;

    Ok(Json(serde_json::json!({
        "id": rating_id,
        "route_id": route_id,
    })))
}

/// GET /api/v1/evaluations/stats - Correlation stats
pub async fn evaluation_stats(
    State(pool): State<PgPool>,
) -> Result<Json<EvaluationStats>, AppError> {
    let (total_routes, total_ratings) = queries::get_evaluation_counts(&pool).await?;
    let correlations = queries::get_correlation_data(&pool).await?;

    Ok(Json(EvaluationStats {
        total_routes,
        total_ratings,
        correlations,
    }))
}
