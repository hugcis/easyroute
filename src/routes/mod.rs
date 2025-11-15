pub mod debug;
pub mod loop_route;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;

use crate::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/routes/loop", post(loop_route::create_loop_route))
        .route("/debug/health", get(debug::health_check))
        .with_state(state)
}
