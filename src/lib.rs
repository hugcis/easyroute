// Library exports for testing and reusability

pub mod cache;
pub mod config;
pub mod constants;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

// Re-export commonly used types
pub use error::{AppError, Result};

// App state for sharing across the application
use services::route_generator::RouteGenerator;
use sqlx::PgPool;

pub struct AppState {
    pub db_pool: PgPool,
    pub route_generator: RouteGenerator,
}
