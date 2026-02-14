use easyroute::cache::MemoryCacheService;
use easyroute::config::RouteGeneratorConfig;
use easyroute::constants::DEFAULT_MEMORY_CACHE_MAX_ENTRIES;
use easyroute::db::SqlitePoiRepository;
use easyroute::services::mapbox::{AuthMode, MapboxClient};
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use easyroute::AppState;

use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_CACHE_TTL: u64 = 86_400;
const DEFAULT_SNAP_RADIUS_M: f64 = 100.0;

fn print_help() {
    eprintln!(
        "\
Usage: ondevice [OPTIONS]

On-device EasyRoute server — needs only a .db region file and optionally a proxy URL.

Options:
  --region=PATH     Path to SQLite region DB (required)
  --port=PORT       Port to listen on (default: {DEFAULT_PORT})
  --open            Open browser after starting
  --help            Show this help message

Environment variables:
  MAPBOX_API_KEY    Mapbox access token (required)
  MAPBOX_BASE_URL   Proxy URL (optional — uses direct Mapbox if unset)"
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env if present
    dotenv::dotenv().ok();

    // Parse CLI args
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help") {
        print_help();
        return Ok(());
    }

    let region_path = args
        .iter()
        .find_map(|a| a.strip_prefix("--region="))
        .ok_or("--region=PATH is required. Run with --help for usage.")?
        .to_string();

    let port: u16 = args
        .iter()
        .find_map(|a| a.strip_prefix("--port="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let open_browser = args.iter().any(|a| a == "--open");

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "easyroute=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Open SQLite pool with read-performance pragmas
    tracing::info!("Opening region database: {}", region_path);
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", region_path))?
        .create_if_missing(false)
        .pragma("journal_mode", "WAL")
        .pragma("mmap_size", "268435456") // 256 MB
        .pragma("cache_size", "-16384"); // 16 MB (negative = KiB)

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .map_err(|e| format!("Failed to open region DB '{}': {}", region_path, e))?;

    // Ensure schema exists (idempotent)
    SqlitePoiRepository::create_schema(&pool).await?;

    // Log region metadata
    let poi_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pois")
        .fetch_one(&pool)
        .await?;
    let region_name: Option<String> =
        sqlx::query_scalar("SELECT value FROM region_meta WHERE key = 'region_name'")
            .fetch_optional(&pool)
            .await?;

    tracing::info!(
        "Region: {} ({} POIs)",
        region_name.as_deref().unwrap_or("unknown"),
        poi_count
    );

    if poi_count == 0 {
        tracing::warn!("Region DB has 0 POIs — route generation will use geometric fallback only");
    }

    // Initialize cache (in-memory)
    let cache = Arc::new(MemoryCacheService::new(
        DEFAULT_CACHE_TTL,
        DEFAULT_MEMORY_CACHE_MAX_ENTRIES,
    ));

    // Initialize Mapbox client
    let mapbox_api_key = env::var("MAPBOX_API_KEY")
        .map_err(|_| "MAPBOX_API_KEY must be set in .env or environment")?;

    let mapbox_client = if let Ok(base_url) = env::var("MAPBOX_BASE_URL") {
        tracing::info!("Using Mapbox proxy: {}", base_url);
        MapboxClient::with_config(mapbox_api_key, base_url, AuthMode::BearerHeader)
    } else {
        tracing::info!("Using direct Mapbox API");
        MapboxClient::new(mapbox_api_key)
    };

    // Initialize services
    let snap_radius_m: f64 = env::var("SNAP_RADIUS_M")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SNAP_RADIUS_M);

    let route_generator_config = RouteGeneratorConfig::from_env()
        .map_err(|e| format!("Route generator config error: {}", e))?;

    let poi_repo: Arc<dyn easyroute::db::PoiRepository> = Arc::new(SqlitePoiRepository::new(pool));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        snap_radius_m,
        route_generator_config,
    );

    // Create application state
    let state = Arc::new(AppState {
        poi_repo,
        route_generator,
        cache: Some(cache),
    });

    // Build router: API routes + static file fallback for web UI
    let app = Router::new()
        .nest("/api/v1", easyroute::routes::create_router(state))
        .fallback_service(ServeDir::new("app"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    let addr = format!("127.0.0.1:{}", port);
    tracing::info!("Server listening on http://{}", addr);

    if open_browser {
        let url = format!("http://localhost:{}", port);
        tracing::info!("Opening browser: {}", url);
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
