use crate::cache::{CacheStats, RouteCache};
use crate::error::{AppError, Result};
use crate::models::Route;
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

/// Redis-backed cache service. All methods are `&self` — `ConnectionManager` is
/// `Arc`-based internally, so `.clone()` is a cheap atomic increment.
pub struct RedisCacheService {
    connection: ConnectionManager,
    route_cache_ttl: u64,
}

impl RedisCacheService {
    pub async fn new(redis_url: &str, route_cache_ttl: u64) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AppError::Cache(format!("Failed to create Redis client: {}", e)))?;

        let connection = ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::Cache(format!("Failed to connect to Redis: {}", e)))?;

        tracing::info!("Redis cache connection established");

        Ok(RedisCacheService {
            connection,
            route_cache_ttl,
        })
    }
}

#[async_trait]
impl RouteCache for RedisCacheService {
    async fn get_cached_routes(&self, key: &str) -> Option<Vec<Route>> {
        let mut conn = self.connection.clone();
        let result: redis::RedisResult<Option<String>> = conn.get(key).await;

        match result {
            Ok(Some(json)) => match serde_json::from_str(&json) {
                Ok(routes) => {
                    tracing::debug!("Cache hit for route: {}", key);
                    Some(routes)
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize cached routes: {}", e);
                    None
                }
            },
            Ok(None) => {
                tracing::debug!("Cache miss for route: {}", key);
                None
            }
            Err(e) => {
                tracing::warn!("Redis error getting routes: {}", e);
                None
            }
        }
    }

    async fn cache_routes(&self, key: &str, routes: &[Route]) {
        let json = match serde_json::to_string(routes) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("Failed to serialize routes for cache: {}", e);
                return;
            }
        };

        let mut conn = self.connection.clone();
        let result: redis::RedisResult<()> = conn.set_ex(key, json, self.route_cache_ttl).await;

        match result {
            Ok(()) => {
                tracing::debug!(
                    "Cached {} routes with TTL {}s: {}",
                    routes.len(),
                    self.route_cache_ttl,
                    key
                );
            }
            Err(e) => {
                tracing::warn!("Failed to cache routes: {}", e);
            }
        }
    }

    async fn get_stats(&self) -> CacheStats {
        let mut conn = self.connection.clone();
        let info: redis::RedisResult<String> =
            redis::cmd("INFO").arg("stats").query_async(&mut conn).await;

        match info {
            Ok(info_str) => {
                let hits = parse_info_value(&info_str, "keyspace_hits");
                let misses = parse_info_value(&info_str, "keyspace_misses");
                let hit_rate = if hits + misses > 0 {
                    (hits as f64 / (hits + misses) as f64) * 100.0
                } else {
                    0.0
                };

                CacheStats {
                    hits,
                    misses,
                    hit_rate,
                    connected: true,
                }
            }
            Err(_) => CacheStats {
                hits: 0,
                misses: 0,
                hit_rate: 0.0,
                connected: false,
            },
        }
    }

    async fn health_check(&self) -> bool {
        let mut conn = self.connection.clone();
        let result: redis::RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;
        result.is_ok()
    }

    fn backend_name(&self) -> &'static str {
        "redis"
    }
}

fn parse_info_value(info: &str, key: &str) -> u64 {
    info.lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|val| val.trim().parse().ok())
        .unwrap_or(0)
}
