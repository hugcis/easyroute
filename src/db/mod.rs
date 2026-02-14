use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

mod evaluation_queries;
mod poi_queries;
pub mod poi_repository;
#[cfg(feature = "sqlite")]
pub mod sqlite_repo;

/// Re-export all query functions under `queries` for backwards compatibility
pub mod queries {
    pub use super::evaluation_queries::*;
    pub use super::poi_queries::*;
}

pub use poi_repository::{PgPoiRepository, PoiRepository};
#[cfg(feature = "sqlite")]
pub use sqlite_repo::SqlitePoiRepository;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}
