use crate::cache::{self, RoutePreferencesHash};
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
        lat = request.start_point.lat,
        lng = request.start_point.lng,
        distance_km = request.distance_km,
        mode = %request.mode.mapbox_profile(),
        tolerance_km = request.distance_tolerance,
        "Loop route request: ({:.4}, {:.4}), {:.1}km, mode={}, tolerance={:.2}km",
        request.start_point.lat, request.start_point.lng,
        request.distance_km, request.mode.mapbox_profile(), request.distance_tolerance
    );

    // Build cache key
    let prefs_hash = RoutePreferencesHash::new(
        request.preferences.poi_categories.as_deref(),
        request.preferences.hidden_gems,
    );
    let cache_key = cache::loop_route_cache_key(
        &request.start_point,
        request.distance_km,
        request.mode.mapbox_profile(),
        &prefs_hash,
    );

    // Check cache first
    if let Some(ref cache) = state.cache {
        if let Some(cached_routes) = cache.get_cached_routes(&cache_key).await {
            tracing::info!(
                "Cache hit for loop route: {} routes returned",
                cached_routes.len()
            );
            return Ok(Json(RouteResponse {
                routes: cached_routes,
            }));
        }
    }

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

    // Cache the results
    if let Some(ref cache) = state.cache {
        cache.cache_routes(&cache_key, &routes).await;
    }

    Ok(Json(RouteResponse { routes }))
}
