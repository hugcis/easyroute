use crate::cache::{CacheStats, RouteCache};
use crate::models::Route;
use async_trait::async_trait;
use moka::future::Cache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// In-memory cache backed by moka with TTL and bounded capacity.
/// All methods are `&self` — no locking needed.
pub struct MemoryCacheService {
    routes: Cache<String, Arc<Vec<Route>>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl MemoryCacheService {
    pub fn new(route_ttl_seconds: u64, max_capacity: u64) -> Self {
        let routes = Cache::builder()
            .time_to_live(Duration::from_secs(route_ttl_seconds))
            .max_capacity(max_capacity)
            .build();

        MemoryCacheService {
            routes,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl RouteCache for MemoryCacheService {
    async fn get_cached_routes(&self, key: &str) -> Option<Vec<Route>> {
        match self.routes.get(key).await {
            Some(arc_routes) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                tracing::debug!("Memory cache hit for route: {}", key);
                Some((*arc_routes).clone())
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                tracing::debug!("Memory cache miss for route: {}", key);
                None
            }
        }
    }

    async fn cache_routes(&self, key: &str, routes: &[Route]) {
        let arc_routes = Arc::new(routes.to_vec());
        self.routes.insert(key.to_string(), arc_routes).await;
        tracing::debug!("Memory cached {} routes: {}", routes.len(), key);
    }

    async fn get_stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
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

    async fn health_check(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_test_route(distance_km: f64) -> Route {
        Route {
            id: Uuid::new_v4(),
            distance_km,
            estimated_duration_minutes: 30,
            elevation_gain_m: None,
            path: vec![],
            pois: vec![],
            snapped_pois: vec![],
            score: 7.0,
            metrics: None,
        }
    }

    #[tokio::test]
    async fn cache_miss() {
        let cache = MemoryCacheService::new(3600, 100);
        assert!(cache.get_cached_routes("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn roundtrip() {
        let cache = MemoryCacheService::new(3600, 100);
        let routes = vec![make_test_route(5.0), make_test_route(3.0)];

        cache.cache_routes("key1", &routes).await;
        let cached = cache.get_cached_routes("key1").await.unwrap();

        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].distance_km, 5.0);
        assert_eq!(cached[1].distance_km, 3.0);
    }

    #[tokio::test]
    async fn stats_tracking() {
        let cache = MemoryCacheService::new(3600, 100);
        let routes = vec![make_test_route(5.0)];
        cache.cache_routes("key1", &routes).await;

        // 1 miss
        cache.get_cached_routes("missing").await;
        // 2 hits
        cache.get_cached_routes("key1").await;
        cache.get_cached_routes("key1").await;

        let stats = cache.get_stats().await;
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 66.666).abs() < 1.0);
    }

    #[tokio::test]
    async fn health_always_true() {
        let cache = MemoryCacheService::new(3600, 100);
        assert!(cache.health_check().await);
    }

    #[tokio::test]
    async fn backend_name_is_memory() {
        let cache = MemoryCacheService::new(3600, 100);
        assert_eq!(cache.backend_name(), "memory");
    }

    #[tokio::test]
    async fn ttl_expiry() {
        let cache = MemoryCacheService::new(1, 100); // 1 second TTL
        let routes = vec![make_test_route(5.0)];
        cache.cache_routes("key1", &routes).await;

        assert!(cache.get_cached_routes("key1").await.is_some());

        tokio::time::sleep(Duration::from_secs(2)).await;

        assert!(cache.get_cached_routes("key1").await.is_none());
    }
}
