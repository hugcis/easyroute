use crate::error::{AppError, Result};
use crate::models::route::{LoopRouteRequest, RouteResponse};
use crate::AppState;
use axum::{extract::State, Json};
use std::sync::Arc;

/// POST /routes/loop
/// Generate loop routes that start and end at the same point
pub async fn create_loop_route(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoopRouteRequest>,
) -> Result<Json<RouteResponse>> {
    // Validate request
    request.validate().map_err(AppError::InvalidRequest)?;

    tracing::info!(
        "Loop route request: {:?}, distance: {}km, mode: {:?}",
        request.start_point,
        request.distance_km,
        request.mode
    );

    // Generate routes
    let routes = state
        .route_generator
        .generate_loop_route(
            request.start_point,
            request.distance_km,
            request.distance_tolerance,
            &request.mode,
            &request.preferences,
        )
        .await?;

    Ok(Json(RouteResponse { routes }))
}
