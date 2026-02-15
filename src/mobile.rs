use crate::cache::MemoryCacheService;
use crate::config::RouteGeneratorConfig;
use crate::constants::DEFAULT_MEMORY_CACHE_MAX_ENTRIES;
use crate::db::SqlitePoiRepository;
use crate::services::mapbox::{AuthMode, MapboxClient};
use crate::services::poi_service::PoiService;
use crate::services::route_generator::RouteGenerator;
use crate::services::snapping_service::SnappingService;
use crate::AppState;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rust_embed::Embed;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

const DEFAULT_CACHE_TTL: u64 = 86_400;
const DEFAULT_SNAP_RADIUS_M: f64 = 100.0;

#[derive(Embed)]
#[folder = "app/"]
struct Assets;

async fn static_handler(req: Request<Body>) -> Response {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for unknown paths
            match Assets::get("index.html") {
                Some(file) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    file.data.to_vec(),
                )
                    .into_response(),
                None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
            }
        }
    }
}

pub struct ServerConfig {
    pub region_db_path: String,
    pub port: u16,
    pub mapbox_api_key: String,
    pub mapbox_base_url: Option<String>,
}

pub struct ServerHandle {
    pub port: u16,
    pub shutdown_tx: oneshot::Sender<()>,
}

pub async fn start_server(
    config: ServerConfig,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    // Open SQLite pool with mobile-tuned pragmas
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", config.region_db_path))?
        .create_if_missing(false)
        .pragma("journal_mode", "WAL")
        .pragma("mmap_size", "33554432") // 32 MB (mobile)
        .pragma("cache_size", "-8192"); // 8 MB

    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .map_err(|e| {
            format!(
                "Failed to open region DB '{}': {}",
                config.region_db_path, e
            )
        })?;

    SqlitePoiRepository::create_schema(&pool).await?;

    // In-memory cache
    let cache = Arc::new(MemoryCacheService::new(
        DEFAULT_CACHE_TTL,
        DEFAULT_MEMORY_CACHE_MAX_ENTRIES,
    ));

    // Mapbox client
    let mapbox_client = if let Some(base_url) = config.mapbox_base_url {
        MapboxClient::with_config(config.mapbox_api_key, base_url, AuthMode::BearerHeader)
    } else {
        MapboxClient::new(config.mapbox_api_key)
    };

    // Services
    let route_generator_config = RouteGeneratorConfig::default();
    let poi_repo: Arc<dyn crate::db::PoiRepository> = Arc::new(SqlitePoiRepository::new(pool));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        DEFAULT_SNAP_RADIUS_M,
        route_generator_config,
    );

    let state = Arc::new(AppState {
        poi_repo,
        route_generator,
        cache: Some(cache),
    });

    // Router: API + embedded static fallback
    let app = Router::new()
        .nest("/api/v1", crate::routes::create_router(state))
        .fallback(static_handler)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Bind listener
    let addr = format!("127.0.0.1:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();

    // Graceful shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Spawn server
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    Ok(ServerHandle {
        port: actual_port,
        shutdown_tx,
    })
}
