pub mod debug;
pub mod loop_route;
pub mod pois;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/routes/loop", post(loop_route::create_loop_route))
        .route("/pois", get(pois::query_pois))
        .route("/debug/health", get(debug::health_check))
        .route("/debug/coverage", get(debug::data_coverage))
        .with_state(state)
}
