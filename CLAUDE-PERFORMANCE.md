# CLAUDE-PERFORMANCE.md

Performance optimization, caching strategies, spatial queries, and monitoring for EasyRoute.

---

## Caching Strategy (Critical for Cost Control)

### Why Caching Matters

**Mapbox Free Tier**: 100,000 requests/month

- Each user route request generates **3-5 Mapbox API calls**
- Without caching: 20,000-33,000 user requests max/month
- With 60-70% cache hit rate: **100,000+ user requests/month**

**Cost Impact**: Caching is the difference between a free service and a $50-500/month bill.

---

### Three-Tier Caching System

#### 1. Route Cache (Redis, 24h TTL)

**Purpose**: Cache complete route responses to avoid regenerating identical routes.

**Key Format**: `route:loop:{hash}`

**Hash Generation** (`RoutePreferencesHash`):
```rust
// Coordinates bucketed to ~100m precision
let lat_bucket = (lat * 1000.0).round() / 1000.0;  // 3 decimal places
let lng_bucket = (lng * 1000.0).round() / 1000.0;

// Distance bucketed to 0.5km increments
let distance_bucket = (distance_km * 2.0).round() / 2.0;

// Mode: walking | cycling
let mode_str = mode.to_string();

// Categories: sorted for consistent hashing
let mut categories = poi_categories.clone();
categories.sort();

// Hidden gems flag
let gems_flag = if hidden_gems { "gems" } else { "pop" };

// Final hash
let hash = format!(
    "{:.3}:{:.3}:{:.1}:{}:{}:{}",
    lat_bucket, lng_bucket, distance_bucket,
    mode_str, categories.join(","), gems_flag
);
```

**Impact**:
- Reduces Mapbox calls by 60-70%
- Handles "nearby" requests as cache hits (±100m, ±250m)
- TTL ensures routes don't become stale (24 hours)

**Graceful Degradation**: If Redis is unavailable, application continues without caching (logs warning).

#### 2. POI Region Cache (Redis, 7 day TTL)

**Purpose**: Cache POI queries to reduce Overpass API calls.

**Key Format**: `poi:region:{lat}:{lng}:{radius_km}`

**Hash Generation**:
```rust
// Coordinates rounded to ~1km precision
let lat_bucket = (lat * 100.0).round() / 100.0;  // 2 decimal places
let lng_bucket = (lng * 100.0).round() / 100.0;

// Radius rounded to nearest km
let radius_bucket = radius_km.round();

let key = format!("poi:region:{:.2}:{:.2}:{:.0}",
    lat_bucket, lng_bucket, radius_bucket);
```

**Impact**:
- Reduces Overpass calls by 80-90%
- Longer TTL (7 days) because POIs change infrequently
- Helps avoid Overpass timeout issues in dense areas

#### 3. POI Database (PostgreSQL, Permanent)

**Purpose**: Primary POI source, eliminating most Overpass API calls entirely.

**Implementation**: OSM data imported via `osm2pgsql`

**Benefits**:
- **Instant queries**: No API timeouts, no rate limits
- **Reliable**: No dependency on external API availability
- **Complete**: 800k+ POIs for France, 500+ for Monaco
- **Fresh**: Weekly incremental updates via `osm/update_osm.sh`

**Fallback**: Overpass API only used if database has insufficient POIs for a query.

---

### Cache Performance Monitoring

**Metrics to Track**:
```rust
// In cache service
struct CacheStats {
    hits: u64,
    misses: u64,
    errors: u64,
    hit_rate: f64,  // hits / (hits + misses)
}
```

**Target Hit Rates**:
- Route cache: > 50%
- POI region cache: > 70%

**Logging**:
```rust
tracing::info!(
    cache_key = %key,
    cache_hit = true,
    ttl_remaining_secs = ttl,
    "Route cache hit"
);
```

---

## Spatial Queries with PostGIS

### Geography vs Geometry

EasyRoute uses **Geography** type (not Geometry):
- Accounts for Earth's curvature
- Distances in meters (not degrees)
- More accurate for route planning

```sql
-- GEOGRAPHY(POINT, 4326): lat/lng with accurate distances
CREATE TABLE pois (
    location GEOGRAPHY(POINT, 4326)
);

-- GEOMETRY would use planar math (inaccurate for long distances)
```

### Critical: Coordinate Order

**PostGIS uses (longitude, latitude) order**, NOT (lat, lng)!

```sql
-- ✅ CORRECT: (lng, lat)
ST_GeogFromText('POINT(2.3522 48.8566)')  -- Paris

-- ❌ WRONG: (lat, lng) - will place point in ocean!
ST_GeogFromText('POINT(48.8566 2.3522)')
```

```rust
// In Rust code: always (lat, lng) in structs
pub struct Coordinates {
    pub lat: f64,
    pub lng: f64,
}

// But swap when passing to PostGIS!
let query = format!(
    "ST_GeogFromText('POINT({} {})')",
    coords.lng,  // Longitude first!
    coords.lat   // Latitude second!
);
```

### Common Spatial Queries

#### Find POIs Within Radius

```sql
-- Most common query in the application
SELECT
    id,
    name,
    category,
    ST_AsGeoJSON(location)::json AS location_json,
    popularity,
    ST_Distance(
        location,
        ST_GeogFromText('POINT($1 $2)')  -- lng, lat
    ) AS distance_meters
FROM pois
WHERE ST_DWithin(
    location,
    ST_GeogFromText('POINT($1 $2)'),  -- lng, lat
    $3  -- radius in meters
)
AND category = ANY($4)
ORDER BY distance_meters
LIMIT 50;
```

**Performance**:
- Uses GIST spatial index automatically
- Sub-millisecond queries for most cases
- Scales well to millions of POIs

**Index**:
```sql
CREATE INDEX idx_pois_location ON pois USING GIST(location);
```

#### Calculate Distance Between Points

```sql
-- Returns distance in meters
SELECT ST_Distance(
    ST_GeogFromText('POINT(2.3522 48.8566)'),  -- Paris
    ST_GeogFromText('POINT(2.2945 48.8584)')   -- La Défense
) AS distance_meters;
-- Result: ~4,200 meters
```

#### Find POIs Along Route Path

Used by snapping service:

```sql
-- Find POIs within 100m of a route LineString
SELECT id, name, category
FROM pois
WHERE ST_DWithin(
    location,
    ST_GeogFromText($1),  -- Route geometry (LineString)
    100  -- Snap radius in meters
)
AND id != ALL($2);  -- Exclude waypoint POIs
```

#### Bounding Box Query (Optimization)

```sql
-- Use bounding box for initial filtering (faster than ST_DWithin)
-- Then refine with accurate distance
SELECT id, name
FROM pois
WHERE location && ST_MakeEnvelope(
    $1, $2,  -- min_lng, min_lat
    $3, $4,  -- max_lng, max_lat
    4326
)::geography
AND ST_DWithin(
    location,
    ST_GeogFromText('POINT($5 $6)'),
    $7
);
```

### PostGIS Performance Tips

✅ **Use spatial indexes** (GIST)
- Always create GIST index on geography/geometry columns
- Queries are 100-1000x faster with indexes

✅ **Limit result sets**
- Always use `LIMIT` to prevent huge result sets
- Current limit: 50 POIs per query

✅ **Use bounding boxes for large areas**
- `&&` operator (bounding box overlap) is faster than `ST_DWithin`
- Use for initial filtering, then refine

✅ **Avoid SELECT ***
- Only select needed columns
- `ST_AsGeoJSON()` can be expensive for large geometries

❌ **Don't use ST_Distance in WHERE clause without index**
```sql
-- ❌ SLOW: Full table scan
WHERE ST_Distance(location, point) < 1000

-- ✅ FAST: Uses index
WHERE ST_DWithin(location, point, 1000)
```

---

## Performance Requirements

### Response Time Targets

| Endpoint | Target | Acceptable | Unacceptable |
|----------|--------|------------|--------------|
| `POST /routes/loop` | < 1s | < 3s | > 5s |
| `GET /pois` | < 100ms | < 500ms | > 1s |
| `GET /health` | < 50ms | < 200ms | > 500ms |

### Component Performance

**Route Generation**:
- POI query (DB): < 50ms
- POI query (Overpass fallback): 2-10s
- Waypoint selection: < 100ms
- Mapbox API call: 300-800ms per route
- Total (cache miss): 1-3s

**Cache Operations**:
- Redis GET: < 5ms
- Redis SET: < 10ms
- Cache serialization/deserialization: < 10ms

**Database Queries**:
- POI spatial query: < 50ms (with index)
- Health check query: < 20ms

### Bottlenecks

**Known Slow Operations**:
1. **Overpass API**: 2-10s per query (timeouts possible)
   - Mitigation: PostgreSQL database as primary source
2. **Mapbox API**: 300-800ms per route
   - Mitigation: Redis caching, generate 3-5 alternatives in parallel
3. **Route generation retries**: Can try 3-5 combinations
   - Mitigation: Adaptive tolerance, early exit on success

### Optimization Strategies

**Parallel Mapbox Calls**:
```rust
// Generate multiple routes concurrently
let futures: Vec<_> = waypoint_combinations
    .iter()
    .map(|waypoints| self.mapbox.get_directions(waypoints))
    .collect();

let results = futures::future::join_all(futures).await;
```

**Early Exit**:
```rust
// Stop trying once we have enough valid routes
if valid_routes.len() >= max_alternatives {
    break;
}
```

**Database Connection Pooling**:
```rust
// SQLx automatically pools connections
let pool = PgPoolOptions::new()
    .max_connections(10)
    .connect(&database_url)
    .await?;
```

---

## Monitoring & Observability

### Currently Implemented

#### Structured Logging (tracing)

```rust
// Request logging
tracing::info!(
    method = %req.method(),
    uri = %req.uri(),
    "HTTP request received"
);

// Service logging
tracing::debug!(
    poi_count = pois.len(),
    radius_km = radius,
    "POIs queried from database"
);

// Error logging
tracing::error!(
    error = %e,
    retry_count = attempts,
    "Mapbox API call failed"
);
```

**Log Levels** (via `RUST_LOG` env var):
- `error`: Errors that need investigation
- `warn`: Unexpected but handled (Redis down, Overpass timeout)
- `info`: High-level operations (route generated, cache hit/miss)
- `debug`: Detailed flow (POI filtering, waypoint selection)
- `trace`: Very verbose (SQL queries, API payloads)

**Configuration**:
```bash
# Development: Verbose
RUST_LOG=debug,easyroute=trace,sqlx=debug

# Production: Quieter
RUST_LOG=info,easyroute=info

# Debugging specific issue
RUST_LOG=info,easyroute::services::mapbox=debug
```

#### Health Check Endpoint

`GET /api/v1/debug/health`

Returns:
```json
{
  "status": "healthy",
  "database": "connected",
  "postgis_version": "3.6.0",
  "redis": "connected",
  "poi_count": 850000,
  "uptime_seconds": 3600
}
```

**Checks**:
- Database connectivity (try query)
- PostGIS extension availability
- Redis connectivity (optional)
- POI count (verifies data loaded)

**Use Cases**:
- Load balancer health checks
- Monitoring alerts
- Deployment verification

#### Cache Statistics

```rust
// Track in cache service
pub struct CacheService {
    hits: AtomicU64,
    misses: AtomicU64,
    // ...
}

impl CacheService {
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        CacheStats {
            hits,
            misses,
            hit_rate: hits as f64 / (hits + misses) as f64,
        }
    }
}
```

### Recommended for Production (Not Yet Implemented)

#### Prometheus Metrics

**Metrics to Export**:

**Request Metrics**:
- `http_requests_total{method, endpoint, status}`
- `http_request_duration_seconds{method, endpoint}`
- `http_requests_in_flight{method, endpoint}`

**Cache Metrics**:
- `cache_hits_total{cache_type}` (route vs poi_region)
- `cache_misses_total{cache_type}`
- `cache_hit_rate{cache_type}`
- `cache_operation_duration_seconds{operation}` (get, set)

**External API Metrics**:
- `mapbox_requests_total{status}` ⚠️ **CRITICAL FOR COST**
- `mapbox_request_duration_seconds`
- `overpass_requests_total{status}`
- `overpass_timeout_total`

**Database Metrics**:
- `db_query_duration_seconds{query_type}`
- `db_connection_pool_size`
- `db_connection_pool_idle`

**Route Generation Metrics**:
- `route_generation_duration_seconds`
- `route_generation_total{success, fallback_level}`
- `poi_count_per_route{percentile}` (p50, p95, p99)

**Implementation** (using `prometheus` crate):
```rust
use prometheus::{Counter, Histogram, Registry};

lazy_static! {
    static ref HTTP_REQUESTS: Counter = register_counter!(
        "http_requests_total",
        "Total HTTP requests"
    ).unwrap();

    static ref MAPBOX_CALLS: Counter = register_counter!(
        "mapbox_requests_total",
        "Total Mapbox API calls"
    ).unwrap();
}

// In route handler
HTTP_REQUESTS.inc();

// In Mapbox client
MAPBOX_CALLS.inc();
```

**Expose Metrics**:
```rust
// Add endpoint
app.route("/metrics", get(metrics_handler))

async fn metrics_handler() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode_to_string(&metric_families).unwrap()
}
```

#### Grafana Dashboards

**Recommended Panels**:

1. **Request Rate & Latency**
   - Requests per second (by endpoint)
   - p50, p95, p99 latency
   - Error rate

2. **Mapbox Usage** ⚠️
   - Calls per hour/day
   - Projection to monthly limit
   - Cost estimate
   - Alert if >80% of quota

3. **Cache Performance**
   - Hit rate (route cache, POI cache)
   - Cache size (memory usage)
   - Eviction rate

4. **Database Performance**
   - Query latency
   - Connection pool utilization
   - Slow queries (>100ms)

5. **External API Health**
   - Mapbox success rate
   - Overpass timeout rate
   - Retry counts

#### Distributed Tracing

**For Complex Requests**:
```rust
// Using opentelemetry + Jaeger
use tracing_opentelemetry::OpenTelemetryLayer;

// Trace entire request flow:
// API → Cache Check → POI Query → Overpass Fallback →
// Waypoint Selection → Mapbox Calls → Snapping → Scoring
```

**Benefits**:
- Identify slow operations
- Visualize request flow
- Debug timeouts and errors

#### Alerting Rules

**Critical Alerts** (PagerDuty/Slack):
- API error rate > 5%
- Mapbox quota > 80%
- Database down
- Redis down (warning, not critical)
- p99 latency > 10s

**Warning Alerts**:
- Cache hit rate < 40%
- Overpass timeout rate > 30%
- POI count dropped significantly
- Disk space < 20%

---

## Cost Optimization

### Mapbox API Usage

**Free Tier**: 100,000 requests/month

**Current Usage Pattern**:
- Each user route request: 3-5 Mapbox calls
  - Try 3-5 waypoint combinations
  - Generate route for each combination
- Without caching: 20,000-33,000 user requests/month max

**With Optimization**:
- 60-70% cache hit rate: 100,000+ user requests/month
- Alternative generation in parallel (not sequential)
- Early exit when enough valid routes found

**Monitoring**:
```rust
// Track Mapbox usage
static MAPBOX_CALLS: AtomicU64 = AtomicU64::new(0);

// Alert if approaching limit
if mapbox_calls > 80_000 {
    tracing::warn!("Mapbox quota at 80%");
}
```

**Future Optimizations**:
- Batch waypoint requests (if Mapbox supports)
- Pre-generate popular routes
- User quotas to prevent abuse

### Database Costs

**Current**: Self-hosted PostgreSQL (free)

**Cloud Considerations**:
- Managed PostgreSQL: $20-100/month
- Storage: ~10GB for France POIs
- I/O: Mostly reads, spatial index makes queries cheap

**Optimization**:
- Regular VACUUM to maintain index performance
- Periodic reindexing
- Archive old POIs if needed

### Redis Costs

**Current**: Self-hosted Redis (free)

**Cloud Considerations**:
- Managed Redis: $10-50/month
- Memory: ~500MB for typical usage
- Optional - can operate without it

### OSM Data Updates

**Cost**: Free (Geofabrik provides free extracts)

**Bandwidth**:
- Full France download: ~4GB (once)
- Weekly updates: ~50-200MB

**Storage**:
- OSM file: ~4GB
- PostgreSQL database: ~10GB
- Total: ~15GB

---

## Performance Testing

### Load Testing (Recommended)

**Tools**: `wrk`, `vegeta`, or `k6`

**Scenarios**:
```bash
# 1. Sustained load
wrk -t4 -c100 -d60s http://localhost:3000/api/v1/routes/loop

# 2. Spike test
vegeta attack -duration=30s -rate=0/1s -rate=1000/1s

# 3. Cache warm-up test
# Generate 1000 unique routes, then repeat same routes
```

**Metrics to Measure**:
- Requests per second
- p50, p95, p99 latency
- Error rate
- Cache hit rate during test

### Profiling

**CPU Profiling**:
```bash
cargo install flamegraph
cargo flamegraph --bin easyroute
```

**Memory Profiling**:
```bash
cargo install heaptrack
heaptrack cargo run
```

**Benchmarking**:
```rust
// Use criterion crate
#[bench]
fn bench_waypoint_selection(b: &mut Bencher) {
    b.iter(|| {
        select_waypoints(&pois, 3)
    });
}
```

---

## Scalability Considerations

### Current Limits

- **Single server**: Can handle ~100 req/s with caching
- **Database**: PostgreSQL can scale to 10M+ POIs
- **Redis**: Can handle 10,000+ req/s on single instance

### Horizontal Scaling

**Stateless API Servers**:
- Add load balancer (nginx, HAProxy, AWS ALB)
- Run multiple API server instances
- Shared PostgreSQL and Redis

**Database Scaling**:
- Read replicas for POI queries
- PostGIS works well with replication
- Keep writes on primary (route caching)

**Redis Scaling**:
- Redis Cluster for high availability
- Or run without Redis (graceful degradation)

### Vertical Scaling

**When to Scale Up**:
- Database queries > 100ms: Add more RAM, faster SSD
- Redis evictions: Add more RAM
- API server CPU > 80%: Add more cores

**Current Resource Usage**:
- API server: ~50-100 MB RAM, low CPU
- PostgreSQL: ~500MB-2GB RAM (for France dataset)
- Redis: ~100-500MB RAM
