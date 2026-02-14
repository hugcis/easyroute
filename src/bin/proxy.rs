use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
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

        Ok(Self {
            mapbox_api_key,
            api_keys,
            rate_limit,
            port,
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

// ── App state ───────────────────────────────────────────

struct AppState {
    config: ProxyConfig,
    http: Client,
    limiter: Mutex<RateLimiter>,
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

    tracing::info!(
        port = config.port,
        keys = config.api_keys.len(),
        rate_limit = config.rate_limit,
        "Starting Mapbox proxy"
    );

    let state = Arc::new(AppState {
        config,
        http: Client::new(),
        limiter: Mutex::new(RateLimiter::default()),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/directions/{profile}/{coordinates}", get(directions))
        .route("/v1/telemetry", post(telemetry))
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
}
