use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

mod evaluation_queries;
mod poi_queries;

/// Re-export all query functions under `queries` for backwards compatibility
pub mod queries {
    pub use super::evaluation_queries::*;
    pub use super::poi_queries::*;
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}
