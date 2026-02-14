use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Mapbox API error: {0}")]
    MapboxApi(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Route generation failed: {0}")]
    RouteGeneration(String),

    #[error("No POIs found in database: {0}")]
    NoPoisFound(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

// Convert AppError into HTTP responses
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(ref e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal database error")
            }
            AppError::MapboxApi(ref e) => {
                tracing::error!("Mapbox API error: {}", e);
                (StatusCode::BAD_GATEWAY, "Routing service error")
            }
            AppError::Cache(ref e) => {
                tracing::warn!("Cache error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Cache error")
            }
            AppError::InvalidRequest(ref e) => (StatusCode::BAD_REQUEST, e.as_str()),
            AppError::RouteGeneration(ref e) => {
                tracing::warn!("Route generation failed: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.as_str())
            }
            AppError::NoPoisFound(ref e) => {
                tracing::info!("No POIs found in database: {}", e);
                (StatusCode::NOT_FOUND, e.as_str())
            }
            AppError::NotFound(ref e) => (StatusCode::NOT_FOUND, e.as_str()),
            AppError::Internal(ref e) => {
                tracing::error!("Internal error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({
            "error": status.canonical_reason().unwrap_or("Unknown error"),
            "message": error_message,
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
