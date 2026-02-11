pub mod debug;
pub mod evaluation;
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
        .route("/evaluations", get(evaluation::list_evaluations))
        .route("/evaluations/stats", get(evaluation::evaluation_stats))
        .route("/evaluations/{id}", get(evaluation::get_evaluation))
        .route("/evaluations/{id}/ratings", post(evaluation::submit_rating))
        .with_state(state)
}
