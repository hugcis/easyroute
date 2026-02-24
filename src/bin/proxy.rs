use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use reqwest::Client;
use rusqlite::OpenFlags;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const MAPBOX_API_BASE: &str = "https://api.mapbox.com/directions/v5/mapbox";
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

// ── Config ──────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ProxyConfig {
    mapbox_api_key: String,
    api_keys: Vec<String>,
    rate_limit: usize,
    port: u16,
    regions_dir: PathBuf,
}

impl ProxyConfig {
    fn from_env() -> Result<Self, String> {
        let mapbox_api_key =
            std::env::var("MAPBOX_API_KEY").map_err(|_| "MAPBOX_API_KEY must be set")?;
        let api_keys_str =
            std::env::var("PROXY_API_KEYS").map_err(|_| "PROXY_API_KEYS must be set")?;
        let api_keys: Vec<String> = api_keys_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if api_keys.is_empty() {
            return Err("PROXY_API_KEYS must contain at least one key".to_string());
        }

        let rate_limit: usize = std::env::var("PROXY_RATE_LIMIT")
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .map_err(|_| "Invalid PROXY_RATE_LIMIT")?;
        let port: u16 = std::env::var("PROXY_PORT")
            .unwrap_or_else(|_| "4000".to_string())
            .parse()
            .map_err(|_| "Invalid PROXY_PORT")?;

        let regions_dir: PathBuf = std::env::var("PROXY_REGIONS_DIR")
            .unwrap_or_else(|_| "./regions".to_string())
            .into();

        Ok(Self {
            mapbox_api_key,
            api_keys,
            rate_limit,
            port,
            regions_dir,
        })
    }
}

// ── Rate limiter ────────────────────────────────────────

#[derive(Default)]
struct RateLimiter {
    windows: HashMap<String, Vec<Instant>>,
}

impl RateLimiter {
    fn check(&mut self, key: &str, limit: usize) -> bool {
        let now = Instant::now();
        let cutoff = now - RATE_LIMIT_WINDOW;

        let entries = self.windows.entry(key.to_string()).or_default();
        entries.retain(|t| *t > cutoff);

        if entries.len() >= limit {
            return false;
        }
        entries.push(now);
        true
    }
}

// ── Region catalog ─────────────────────────────────────

#[derive(Clone, Serialize)]
struct RegionInfo {
    id: String,
    name: String,
    size_bytes: u64,
    poi_count: u64,
    build_date: String,
}

fn scan_regions(dir: &std::path::Path) -> Vec<RegionInfo> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(path = %dir.display(), error = %e, "Cannot read regions directory");
            return Vec::new();
        }
    };

    let mut regions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }

        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        let conn = match rusqlite::Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "Cannot open region DB");
                continue;
            }
        };

        let mut meta = HashMap::new();
        if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM region_meta") {
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .into_iter()
                .flatten();
            for row in rows.flatten() {
                meta.insert(row.0, row.1);
            }
        }

        regions.push(RegionInfo {
            id,
            name: meta
                .get("region_name")
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            size_bytes,
            poi_count: meta
                .get("poi_count")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            build_date: meta
                .get("build_date")
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
        });
    }

    regions.sort_by(|a, b| a.id.cmp(&b.id));
    regions
}

// ── App state ───────────────────────────────────────────

struct AppState {
    config: ProxyConfig,
    http: Client,
    limiter: Mutex<RateLimiter>,
    regions: Vec<RegionInfo>,
}

// ── Auth helpers ────────────────────────────────────────

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn validate_api_key(token: &str, valid_keys: &[String]) -> bool {
    valid_keys.iter().any(|k| k == token)
}

// ── Handlers ────────────────────────────────────────────

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "keys_configured": state.config.api_keys.len(),
        "rate_limit": state.config.rate_limit,
    }))
}

async fn directions(
    State(state): State<Arc<AppState>>,
    Path((profile, coordinates)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 1. Extract and validate bearer token
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing or invalid Authorization header"})),
            )
                .into_response()
        }
    };

    if !validate_api_key(token, &state.config.api_keys) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid API key"})),
        )
            .into_response();
    }

    // 2. Rate limit check
    {
        let mut limiter = state.limiter.lock().await;
        if !limiter.check(token, state.config.rate_limit) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error": "Rate limit exceeded"})),
            )
                .into_response();
        }
    }

    // 3. Forward to Mapbox
    let url = format!("{}/{}/{}", MAPBOX_API_BASE, profile, coordinates);

    let mut query_params: Vec<(String, String)> = params.into_iter().collect();
    query_params.push((
        "access_token".to_string(),
        state.config.mapbox_api_key.clone(),
    ));

    tracing::info!(profile = %profile, "Proxying directions request");

    let result = state.http.get(&url).query(&query_params).send().await;

    match result {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = resp.bytes().await.unwrap_or_default();
            (status, body).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Mapbox request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "Upstream request failed"})),
            )
                .into_response()
        }
    }
}

async fn telemetry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing or invalid Authorization header"})),
            )
        }
    };

    if !validate_api_key(token, &state.config.api_keys) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid API key"})),
        );
    }

    tracing::info!(payload = %payload, "Telemetry received");
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn list_regions(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing or invalid Authorization header"})),
            )
                .into_response()
        }
    };

    if !validate_api_key(token, &state.config.api_keys) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid API key"})),
        )
            .into_response();
    }

    Json(json!({ "regions": state.regions })).into_response()
}

async fn download_region(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing or invalid Authorization header"})),
            )
                .into_response()
        }
    };

    if !validate_api_key(token, &state.config.api_keys) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid API key"})),
        )
            .into_response();
    }

    // Validate id to prevent path traversal
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid region id"})),
        )
            .into_response();
    }

    // Verify the region exists in our catalog
    if !state.regions.iter().any(|r| r.id == id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Region not found"})),
        )
            .into_response();
    }

    let path = state.config.regions_dir.join(format!("{}.db", id));

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "Cannot open region file");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Cannot read region file"})),
            )
                .into_response();
        }
    };

    let file_size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let filename = format!("{}.db", id);
    let headers = [
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
        (header::CONTENT_LENGTH, file_size.to_string()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    (headers, body).into_response()
}

// ── Main ────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proxy=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv::dotenv().ok();
    let config = ProxyConfig::from_env().map_err(|e| format!("Config error: {}", e))?;
    let addr = format!("0.0.0.0:{}", config.port);

    let regions = scan_regions(&config.regions_dir);
    tracing::info!(
        port = config.port,
        keys = config.api_keys.len(),
        rate_limit = config.rate_limit,
        regions = regions.len(),
        "Starting Mapbox proxy"
    );

    let state = Arc::new(AppState {
        config,
        http: Client::new(),
        limiter: Mutex::new(RateLimiter::default()),
        regions,
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/directions/{profile}/{coordinates}", get(directions))
        .route("/v1/telemetry", post(telemetry))
        .route("/v1/regions", get(list_regions))
        .route("/v1/regions/{id}/download", get(download_region))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Proxy listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // --- ProxyConfig ---

    #[test]
    #[serial]
    fn config_valid() {
        unsafe {
            std::env::set_var("MAPBOX_API_KEY", "pk.test");
            std::env::set_var("PROXY_API_KEYS", "key1,key2");
            std::env::set_var("PROXY_RATE_LIMIT", "30");
            std::env::set_var("PROXY_PORT", "5000");
        }
        let cfg = ProxyConfig::from_env().unwrap();
        assert_eq!(cfg.mapbox_api_key, "pk.test");
        assert_eq!(cfg.api_keys, vec!["key1", "key2"]);
        assert_eq!(cfg.rate_limit, 30);
        assert_eq!(cfg.port, 5000);
        assert_eq!(cfg.regions_dir, PathBuf::from("./regions"));
        unsafe {
            std::env::remove_var("PROXY_RATE_LIMIT");
            std::env::remove_var("PROXY_PORT");
        }
    }

    #[test]
    #[serial]
    fn config_missing_mapbox_key() {
        unsafe {
            std::env::remove_var("MAPBOX_API_KEY");
            std::env::set_var("PROXY_API_KEYS", "key1");
        }
        assert!(ProxyConfig::from_env().is_err());
        // Restore for other tests
        unsafe { std::env::set_var("MAPBOX_API_KEY", "pk.test") };
    }

    #[test]
    #[serial]
    fn config_missing_api_keys() {
        unsafe {
            std::env::set_var("MAPBOX_API_KEY", "pk.test");
            std::env::remove_var("PROXY_API_KEYS");
        }
        assert!(ProxyConfig::from_env().is_err());
        unsafe { std::env::set_var("PROXY_API_KEYS", "key1") };
    }

    // --- Rate limiter ---

    #[test]
    fn rate_limiter_within_limit() {
        let mut rl = RateLimiter::default();
        for _ in 0..5 {
            assert!(rl.check("k1", 5));
        }
    }

    #[test]
    fn rate_limiter_over_limit() {
        let mut rl = RateLimiter::default();
        for _ in 0..5 {
            assert!(rl.check("k1", 5));
        }
        assert!(!rl.check("k1", 5));
    }

    #[test]
    fn rate_limiter_independent_keys() {
        let mut rl = RateLimiter::default();
        for _ in 0..5 {
            assert!(rl.check("k1", 5));
        }
        assert!(!rl.check("k1", 5));
        // Different key still has budget
        assert!(rl.check("k2", 5));
    }

    // --- Bearer token extraction ---

    #[test]
    fn bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer mytoken".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("mytoken"));
    }

    #[test]
    fn bearer_token_missing() {
        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Basic abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    // --- API key validation ---

    #[test]
    fn api_key_valid() {
        let keys = vec!["key1".to_string(), "key2".to_string()];
        assert!(validate_api_key("key1", &keys));
        assert!(validate_api_key("key2", &keys));
    }

    #[test]
    fn api_key_invalid() {
        let keys = vec!["key1".to_string()];
        assert!(!validate_api_key("wrong", &keys));
    }

    // --- Region scanning ---

    #[test]
    fn scan_regions_empty_dir() {
        let dir = std::env::temp_dir().join("easyroute_test_empty_regions");
        let _ = std::fs::create_dir_all(&dir);
        let regions = scan_regions(&dir);
        assert!(regions.is_empty());
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn scan_regions_missing_dir() {
        let dir = PathBuf::from("/tmp/easyroute_nonexistent_dir_12345");
        let regions = scan_regions(&dir);
        assert!(regions.is_empty());
    }
}
