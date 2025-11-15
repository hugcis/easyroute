use easyroute::config::Config;
use easyroute::models::{Coordinates, Poi, PoiCategory};
use sqlx::PgPool;
use uuid::Uuid;

/// Setup test database connection
#[allow(dead_code)]
pub async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute".to_string()
    });

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Clean up test database - remove all test data
#[allow(dead_code)]
pub async fn cleanup_test_db(pool: &PgPool) {
    sqlx::query("TRUNCATE TABLE pois CASCADE")
        .execute(pool)
        .await
        .expect("Failed to clean up test database");
}

/// Create a test POI
#[allow(dead_code)]
pub fn create_test_poi(name: &str, category: PoiCategory, lat: f64, lng: f64) -> Poi {
    Poi {
        id: Uuid::new_v4(),
        name: name.to_string(),
        category,
        coordinates: Coordinates::new(lat, lng).unwrap(),
        popularity_score: 50.0,
        description: Some(format!("Test POI: {}", name)),
        estimated_visit_duration_minutes: Some(30),
        osm_id: None,
    }
}

/// Get test configuration
#[allow(dead_code)]
pub fn get_test_config() -> Config {
    Config {
        host: "0.0.0.0".to_string(),
        port: 3000,
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute".to_string()
        }),
        redis_url: Some(
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        ),
        mapbox_api_key: std::env::var("MAPBOX_API_KEY").unwrap_or_else(|_| "test_key".to_string()),
        route_cache_ttl: 3600,
        poi_region_cache_ttl: 86400,
        snap_radius_m: 100.0,
    }
}

/// Check if we should skip real API tests
#[allow(dead_code)]
pub fn should_skip_real_api_tests() -> bool {
    std::env::var("SKIP_REAL_API_TESTS").is_ok()
}
