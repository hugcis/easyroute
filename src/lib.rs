// Library exports for testing and reusability

pub mod cache;
pub mod config;
pub mod constants;
pub mod db;
pub mod error;
pub mod evaluation;
pub mod models;
#[cfg(feature = "sqlite")]
pub mod osm;
pub mod routes;
pub mod services;

// Re-export commonly used types
pub use cache::{CacheStats, RouteCache, RoutePreferencesHash};
pub use error::{AppError, Result};

// App state for sharing across the application
use services::route_generator::RouteGenerator;
use std::sync::Arc;

pub struct AppState {
    pub poi_repo: Arc<dyn db::PoiRepository>,
    pub route_generator: RouteGenerator,
    /// Optional cache service - None only in tests
    pub cache: Option<Arc<dyn RouteCache>>,
}
