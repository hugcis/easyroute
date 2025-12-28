use axum::Router;
use easyroute::cache::CacheService;
use easyroute::config::Config;
use easyroute::services::mapbox::MapboxClient;
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use easyroute::AppState;
use std::sync::Arc;
use tokio::sync::RwLock;
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

    // Initialize Redis cache if configured
    let cache = if let Some(ref redis_url) = config.redis_url {
        tracing::info!("Connecting to Redis cache...");
        match CacheService::new(
            redis_url,
            config.route_cache_ttl,
            config.poi_region_cache_ttl,
        )
        .await
        {
            Ok(cache_service) => {
                tracing::info!("Redis cache connection established");
                Some(Arc::new(RwLock::new(cache_service)))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to connect to Redis: {}. Continuing without cache.",
                    e
                );
                None
            }
        }
    } else {
        tracing::info!("Redis URL not configured. Running without cache.");
        None
    };

    // Initialize services
    let mapbox_client = MapboxClient::new(config.mapbox_api_key.clone());
    let poi_service = PoiService::new(db_pool.clone());
    let snapping_service = SnappingService::new(db_pool.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        config.snap_radius_m,
    );

    // Create application state
    let state = Arc::new(AppState {
        db_pool,
        route_generator,
        cache,
    });

    // Build router with CORS and tracing
    let app = Router::new()
        .nest("/api/v1", easyroute::routes::create_router(state))
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
