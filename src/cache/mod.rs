use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, PoiCategory, Route};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cache service for route and POI caching using Redis
#[derive(Clone)]
pub struct CacheService {
    connection: ConnectionManager,
    route_cache_ttl: u64,
    poi_region_cache_ttl: u64,
}

impl CacheService {
    /// Create a new cache service with Redis connection
    pub async fn new(
        redis_url: &str,
        route_cache_ttl: u64,
        poi_region_cache_ttl: u64,
    ) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AppError::Cache(format!("Failed to create Redis client: {}", e)))?;

        let connection = ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::Cache(format!("Failed to connect to Redis: {}", e)))?;

        tracing::info!("Redis cache connection established");

        Ok(CacheService {
            connection,
            route_cache_ttl,
            poi_region_cache_ttl,
        })
    }

    /// Generate a cache key for loop routes
    /// Key includes: coordinates (3 decimal precision), distance (0.5km buckets), mode, preferences
    pub fn loop_route_cache_key(
        start: &Coordinates,
        distance_km: f64,
        mode: &str,
        preferences: &RoutePreferencesHash,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Round coordinates to 3 decimal places (~100m precision)
        let lat = (start.lat * 1000.0).round() as i64;
        let lng = (start.lng * 1000.0).round() as i64;

        // Round distance to 0.5km buckets
        let distance_bucket = (distance_km * 2.0).round() as i64;

        lat.hash(&mut hasher);
        lng.hash(&mut hasher);
        distance_bucket.hash(&mut hasher);
        mode.hash(&mut hasher);
        preferences.hash(&mut hasher);

        format!("route:loop:{:x}", hasher.finish())
    }

    /// Generate a cache key for POI region queries
    /// Key includes: center coordinates (2 decimal precision), radius (1km buckets), categories
    pub fn poi_region_cache_key(
        center: &Coordinates,
        radius_km: f64,
        categories: Option<&[PoiCategory]>,
    ) -> String {
        let mut hasher = DefaultHasher::new();

        // Round coordinates to 2 decimal places (~1km precision)
        let lat = (center.lat * 100.0).round() as i64;
        let lng = (center.lng * 100.0).round() as i64;

        // Round radius to 1km buckets
        let radius_bucket = radius_km.ceil() as i64;

        lat.hash(&mut hasher);
        lng.hash(&mut hasher);
        radius_bucket.hash(&mut hasher);

        // Hash categories if present
        if let Some(cats) = categories {
            let mut cat_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();
            cat_strs.sort(); // Ensure consistent ordering
            cat_strs.hash(&mut hasher);
        }

        format!("poi:region:{:x}", hasher.finish())
    }

    /// Get cached routes for a loop route request
    pub async fn get_cached_routes(&mut self, cache_key: &str) -> Option<Vec<Route>> {
        let result: redis::RedisResult<Option<String>> = self.connection.get(cache_key).await;

        match result {
            Ok(Some(json)) => match serde_json::from_str(&json) {
                Ok(routes) => {
                    tracing::debug!("Cache hit for route: {}", cache_key);
                    Some(routes)
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize cached routes: {}", e);
                    None
                }
            },
            Ok(None) => {
                tracing::debug!("Cache miss for route: {}", cache_key);
                None
            }
            Err(e) => {
                tracing::warn!("Redis error getting routes: {}", e);
                None
            }
        }
    }

    /// Cache routes for a loop route request
    pub async fn cache_routes(&mut self, cache_key: &str, routes: &[Route]) {
        let json = match serde_json::to_string(routes) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("Failed to serialize routes for cache: {}", e);
                return;
            }
        };

        let result: redis::RedisResult<()> = self
            .connection
            .set_ex(cache_key, json, self.route_cache_ttl)
            .await;

        match result {
            Ok(()) => {
                tracing::debug!(
                    "Cached {} routes with TTL {}s: {}",
                    routes.len(),
                    self.route_cache_ttl,
                    cache_key
                );
            }
            Err(e) => {
                tracing::warn!("Failed to cache routes: {}", e);
            }
        }
    }

    /// Get cached POIs for a region query
    pub async fn get_cached_pois(&mut self, cache_key: &str) -> Option<Vec<Poi>> {
        let result: redis::RedisResult<Option<String>> = self.connection.get(cache_key).await;

        match result {
            Ok(Some(json)) => match serde_json::from_str(&json) {
                Ok(pois) => {
                    tracing::debug!("Cache hit for POI region: {}", cache_key);
                    Some(pois)
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize cached POIs: {}", e);
                    None
                }
            },
            Ok(None) => {
                tracing::debug!("Cache miss for POI region: {}", cache_key);
                None
            }
            Err(e) => {
                tracing::warn!("Redis error getting POIs: {}", e);
                None
            }
        }
    }

    /// Cache POIs for a region query
    pub async fn cache_pois(&mut self, cache_key: &str, pois: &[Poi]) {
        let json = match serde_json::to_string(pois) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("Failed to serialize POIs for cache: {}", e);
                return;
            }
        };

        let result: redis::RedisResult<()> = self
            .connection
            .set_ex(cache_key, json, self.poi_region_cache_ttl)
            .await;

        match result {
            Ok(()) => {
                tracing::debug!(
                    "Cached {} POIs with TTL {}s: {}",
                    pois.len(),
                    self.poi_region_cache_ttl,
                    cache_key
                );
            }
            Err(e) => {
                tracing::warn!("Failed to cache POIs: {}", e);
            }
        }
    }

    /// Get cache statistics for monitoring
    pub async fn get_stats(&mut self) -> CacheStats {
        let info: redis::RedisResult<String> = redis::cmd("INFO")
            .arg("stats")
            .query_async(&mut self.connection)
            .await;

        match info {
            Ok(info_str) => {
                let hits = Self::parse_info_value(&info_str, "keyspace_hits");
                let misses = Self::parse_info_value(&info_str, "keyspace_misses");
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

    fn parse_info_value(info: &str, key: &str) -> u64 {
        info.lines()
            .find(|line| line.starts_with(key))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|val| val.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Check if cache is healthy
    pub async fn health_check(&mut self) -> bool {
        let result: redis::RedisResult<String> = redis::cmd("PING")
            .query_async(&mut self.connection)
            .await;
        result.is_ok()
    }
}

/// Hash-friendly representation of route preferences for cache key generation
#[derive(Hash, Debug, Clone, Serialize, Deserialize)]
pub struct RoutePreferencesHash {
    pub categories: Vec<String>,
    pub hidden_gems: bool,
}

impl RoutePreferencesHash {
    pub fn new(categories: Option<&[PoiCategory]>, hidden_gems: bool) -> Self {
        let mut cat_strs: Vec<String> = categories
            .map(|cats| cats.iter().map(|c| c.to_string()).collect())
            .unwrap_or_default();
        cat_strs.sort(); // Ensure consistent ordering

        RoutePreferencesHash {
            categories: cat_strs,
            hidden_gems,
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub connected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_route_cache_key_consistency() {
        let coord1 = Coordinates::new(48.8566, 2.3522).unwrap();
        let prefs = RoutePreferencesHash::new(Some(&[PoiCategory::Monument]), false);

        let key1 = CacheService::loop_route_cache_key(&coord1, 5.0, "walking", &prefs);
        let key2 = CacheService::loop_route_cache_key(&coord1, 5.0, "walking", &prefs);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_loop_route_cache_key_coordinate_precision() {
        // Small coordinate differences (within 100m) should produce same key
        let coord1 = Coordinates::new(48.8566, 2.3522).unwrap();
        let coord2 = Coordinates::new(48.8567, 2.3523).unwrap(); // ~11m difference

        let prefs = RoutePreferencesHash::new(None, false);

        let key1 = CacheService::loop_route_cache_key(&coord1, 5.0, "walking", &prefs);
        let key2 = CacheService::loop_route_cache_key(&coord2, 5.0, "walking", &prefs);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_loop_route_cache_key_distance_buckets() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();
        let prefs = RoutePreferencesHash::new(None, false);

        // 4.8km and 5.2km should be in same bucket (5.0)
        let key1 = CacheService::loop_route_cache_key(&coord, 4.8, "walking", &prefs);
        let key2 = CacheService::loop_route_cache_key(&coord, 5.2, "walking", &prefs);

        assert_eq!(key1, key2);

        // 5.5km should be in different bucket (5.5)
        let key3 = CacheService::loop_route_cache_key(&coord, 5.5, "walking", &prefs);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_poi_region_cache_key_consistency() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();

        let key1 =
            CacheService::poi_region_cache_key(&coord, 5.0, Some(&[PoiCategory::Monument]));
        let key2 =
            CacheService::poi_region_cache_key(&coord, 5.0, Some(&[PoiCategory::Monument]));

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_poi_region_cache_key_category_order_independence() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();

        // Different order, same categories
        let key1 = CacheService::poi_region_cache_key(
            &coord,
            5.0,
            Some(&[PoiCategory::Monument, PoiCategory::Park]),
        );
        let key2 = CacheService::poi_region_cache_key(
            &coord,
            5.0,
            Some(&[PoiCategory::Park, PoiCategory::Monument]),
        );

        assert_eq!(key1, key2);
    }
}
