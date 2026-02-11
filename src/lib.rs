// Library exports for testing and reusability

pub mod cache;
pub mod config;
pub mod constants;
pub mod db;
pub mod error;
pub mod evaluation;
pub mod models;
pub mod routes;
pub mod services;

// Re-export commonly used types
pub use cache::{CacheService, CacheStats, RoutePreferencesHash};
pub use error::{AppError, Result};

// App state for sharing across the application
use services::route_generator::RouteGenerator;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub db_pool: PgPool,
    pub route_generator: RouteGenerator,
    /// Optional cache service - None if Redis is not configured
    pub cache: Option<Arc<RwLock<CacheService>>>,
}
