// Library exports for testing and reusability

pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;
pub mod cache;

// Re-export commonly used types
pub use error::{AppError, Result};

// App state for sharing across the application
use sqlx::PgPool;
use services::route_generator::RouteGenerator;

pub struct AppState {
    pub db_pool: PgPool,
    pub route_generator: RouteGenerator,
}
