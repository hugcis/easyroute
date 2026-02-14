use axum::Router;
use easyroute::cache::{MemoryCacheService, RedisCacheService, RouteCache};
use easyroute::config::Config;
use easyroute::constants::DEFAULT_MEMORY_CACHE_MAX_ENTRIES;
use easyroute::db::PgPoiRepository;
use easyroute::services::mapbox::{AuthMode, MapboxClient};
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use easyroute::AppState;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "easyroute=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env().map_err(|e| format!("Failed to load configuration: {}", e))?;

    tracing::info!("Starting EasyRoute API server");
    tracing::info!("Configuration loaded successfully");

    // Create database connection pool
    tracing::info!("Connecting to database...");
    let db_pool = easyroute::db::create_pool(&config.database_url).await?;
    tracing::info!("Database connection established");

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&db_pool).await?;
    tracing::info!("Database migrations completed");

    // Initialize cache: try Redis, fall back to in-memory
    let cache: Arc<dyn RouteCache> = if let Some(ref redis_url) = config.redis_url {
        tracing::info!("Connecting to Redis cache...");
        match RedisCacheService::new(redis_url, config.route_cache_ttl).await {
            Ok(redis_cache) => {
                tracing::info!("Redis cache connection established");
                Arc::new(redis_cache)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to connect to Redis: {}. Falling back to in-memory cache.",
                    e
                );
                Arc::new(MemoryCacheService::new(
                    config.route_cache_ttl,
                    DEFAULT_MEMORY_CACHE_MAX_ENTRIES,
                ))
            }
        }
    } else {
        tracing::info!("Redis URL not configured. Using in-memory cache.");
        Arc::new(MemoryCacheService::new(
            config.route_cache_ttl,
            DEFAULT_MEMORY_CACHE_MAX_ENTRIES,
        ))
    };

    // Initialize services
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(db_pool.clone()));
    let mapbox_client = if let Some(ref base_url) = config.mapbox_base_url {
        MapboxClient::with_config(
            config.mapbox_api_key.clone(),
            base_url.clone(),
            AuthMode::BearerHeader,
        )
    } else {
        MapboxClient::new(config.mapbox_api_key.clone())
    };
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        config.snap_radius_m,
        config.route_generator.clone(),
    );

    // Create application state
    let state = Arc::new(AppState {
        poi_repo,
        route_generator,
        cache: Some(cache),
    });

    // Build router with CORS and tracing
    let app = Router::new()
        .nest(
            "/api/v1",
            easyroute::routes::create_router(state)
                .merge(easyroute::routes::create_pg_router(db_pool)),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = config.server_address();
    tracing::info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
