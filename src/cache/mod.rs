pub mod memory;
pub mod redis;

pub use memory::MemoryCacheService;
pub use redis::RedisCacheService;

use crate::models::{Coordinates, PoiCategory, Route};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Trait for route caching backends. All methods take `&self` — no locking needed.
#[async_trait]
pub trait RouteCache: Send + Sync {
    async fn get_cached_routes(&self, key: &str) -> Option<Vec<Route>>;
    async fn cache_routes(&self, key: &str, routes: &[Route]);
    async fn get_stats(&self) -> CacheStats;
    async fn health_check(&self) -> bool;
    fn backend_name(&self) -> &'static str;
}

/// Generate a cache key for loop routes.
/// Key includes: coordinates (3 decimal precision), distance (0.5km buckets), mode, preferences.
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

/// Generate a cache key for POI region queries.
/// Key includes: center coordinates (2 decimal precision), radius (1km buckets), categories.
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

        let key1 = loop_route_cache_key(&coord1, 5.0, "walking", &prefs);
        let key2 = loop_route_cache_key(&coord1, 5.0, "walking", &prefs);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_loop_route_cache_key_coordinate_precision() {
        // Small coordinate differences (within 100m) should produce same key
        let coord1 = Coordinates::new(48.8566, 2.3522).unwrap();
        let coord2 = Coordinates::new(48.8567, 2.3523).unwrap(); // ~11m difference

        let prefs = RoutePreferencesHash::new(None, false);

        let key1 = loop_route_cache_key(&coord1, 5.0, "walking", &prefs);
        let key2 = loop_route_cache_key(&coord2, 5.0, "walking", &prefs);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_loop_route_cache_key_distance_buckets() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();
        let prefs = RoutePreferencesHash::new(None, false);

        // 4.8km and 5.2km should be in same bucket (5.0)
        let key1 = loop_route_cache_key(&coord, 4.8, "walking", &prefs);
        let key2 = loop_route_cache_key(&coord, 5.2, "walking", &prefs);

        assert_eq!(key1, key2);

        // 5.5km should be in different bucket (5.5)
        let key3 = loop_route_cache_key(&coord, 5.5, "walking", &prefs);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_poi_region_cache_key_consistency() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();

        let key1 = poi_region_cache_key(&coord, 5.0, Some(&[PoiCategory::Monument]));
        let key2 = poi_region_cache_key(&coord, 5.0, Some(&[PoiCategory::Monument]));

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_poi_region_cache_key_category_order_independence() {
        let coord = Coordinates::new(48.8566, 2.3522).unwrap();

        // Different order, same categories
        let key1 = poi_region_cache_key(
            &coord,
            5.0,
            Some(&[PoiCategory::Monument, PoiCategory::Park]),
        );
        let key2 = poi_region_cache_key(
            &coord,
            5.0,
            Some(&[PoiCategory::Park, PoiCategory::Monument]),
        );

        assert_eq!(key1, key2);
    }
}
