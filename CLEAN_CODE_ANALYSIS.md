# Clean Code Best Practices Review - EasyRoute Rust API

## Executive Summary
This Rust codebase shows good foundational structure but has several areas where clean code principles could be improved. Key issues include excessive duplication, magic numbers throughout, overly long functions doing multiple things, and inconsistent error handling patterns.

---

## 1. FUNCTION DESIGN ISSUES

### 1.1 Overly Long Function: `try_generate_loop` (route_generator.rs:124-206)
**Severity**: High | **Lines**: 83 lines | **Issue**: Single Responsibility Principle violation

The function does too much:
- Distance tolerance validation
- POI waypoint selection
- Waypoint sequencing
- API calls to Mapbox
- Distance validation and retry logic
- Logging at multiple stages

**Current Code** (lines 124-206):
```rust
async fn try_generate_loop(
    &self,
    start: &Coordinates,
    target_distance_km: f64,
    distance_tolerance: f64,
    mode: &TransportMode,
    candidate_pois: &[Poi],
    variation: usize,
    preferences: &RoutePreferences,
) -> Result<Route> {
    // MAX_RETRIES constant buried inside function
    const MAX_RETRIES: usize = 5;
    
    // Distance calc repeated
    let min_distance = target_distance_km - distance_tolerance;
    let max_distance = target_distance_km + distance_tolerance;
    
    // Multiple responsibilities: waypoint selection, routing, validation
    for retry in 0..MAX_RETRIES {
        // Complex distance adjustment logic (lines 144-152)
        // POI selection
        // API call
        // Validation
        // Error handling with detailed logs
    }
}
```

**Recommendation**: Break into smaller functions:
- `fn adjusted_target_distance(&self, target: f64, retry: usize) -> f64`
- `fn validate_distance(&self, actual: f64, target: f64, tolerance: f64) -> bool`
- `fn build_loop_waypoints(&self, ...)->Result<Vec<Coordinates>>`

---

### 1.2 Duplicated Retry Logic (overpass.rs:95-184 vs 340-436)
**Severity**: High | **Lines**: `execute_query_with_retry_extended` (90 lines) and `execute_query_with_retry` (97 lines)

Almost identical retry logic with only slight differences:

**Lines 95-184** (`execute_query_with_retry_extended`):
```rust
async fn execute_query_with_retry_extended(&self, query: String) -> Result<Vec<Poi>> {
    let max_retries = 2; // 3 total attempts
    let mut retry_count = 0;
    
    loop {
        let endpoint = self.get_next_endpoint();
        
        let response_result = self.client
            .post(&endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("data={}", urlencoding::encode(&query)))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;
        
        // 40+ lines of nearly identical error handling...
    }
}
```

**Lines 340-436** (`execute_query_with_retry`):
```rust
async fn execute_query_with_retry(&self, query: String) -> Result<Vec<Poi>> {
    let max_retries = 1; // 2 total attempts
    let mut retry_count = 0;
    
    loop {
        let endpoint = self.get_next_endpoint();
        
        let response_result = self.client
            .post(&endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("data={}", urlencoding::encode(&query)))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;
        
        // 40+ lines of nearly identical error handling...
    }
}
```

**Recommendation**: Extract common retry logic into a parameterized function:
```rust
async fn execute_query_with_retry(
    &self,
    query: String,
    max_retries: usize,
) -> Result<Vec<Poi>> {
    // Single implementation
}
```

---

### 1.3 Cascading Category Inference (overpass.rs:532-649)
**Severity**: Medium | **Lines**: 118 lines | **Issue**: Too long, repeated pattern

The `infer_category` function contains 10 separate if-let blocks checking different OSM tags:

```rust
fn infer_category(&self, tags: &HashMap<String, String>) -> Option<PoiCategory> {
    // 534-543: Check tourism tags (10 lines with match statement)
    if let Some(tourism_type) = tags.get("tourism") {
        return match tourism_type.as_str() { ... };
    }
    
    // 546-551: Check historic tags (6 lines with match statement)
    if let Some(historic_type) = tags.get("historic") {
        return match historic_type.as_str() { ... };
    }
    
    // 554-566: Check amenity tags (13 lines)
    if let Some(amenity_type) = tags.get("amenity") {
        return match amenity_type.as_str() { ... };
    }
    
    // ... repeats 7 more times for leisure, natural, waterway, man_made, craft, building, place, shop, boundary
    
    None
}
```

**Recommendation**: Create a category mapping table:
```rust
fn infer_category(&self, tags: &HashMap<String, String>) -> Option<PoiCategory> {
    let rules = vec![
        ("tourism", ["monument", "memorial"], PoiCategory::Monument),
        // ... more rules
    ];
    
    for (tag_key, tag_values, category) in rules {
        if let Some(val) = tags.get(tag_key) {
            if tag_values.contains(&val.as_str()) {
                return Some(category);
            }
        }
    }
    None
}
```

---

### 1.4 Duplicated Database Queries (db/queries.rs:7-79 and 83-148)
**Severity**: High | **Lines**: Two nearly identical functions

`find_pois_within_radius` and `find_pois_in_bbox` share almost identical structure:

**Lines 7-79**:
```rust
pub async fn find_pois_within_radius(
    pool: &PgPool,
    center: &Coordinates,
    radius_meters: f64,
    categories: Option<&[PoiCategory]>,
    limit: i64,
) -> Result<Vec<Poi>, sqlx::Error> {
    // ... identical if-let pattern ...
    
    let query = if let Some(cats) = categories {
        let category_strs: Vec<String> = cats.iter().map(|c| c.to_string()).collect();
        
        sqlx::query_as::<_, PoiRow>(
            r#"SELECT ... WHERE ST_DWithin(...) AND category = ANY($3)"#
        )
        // ... binding parameters ...
    } else {
        sqlx::query_as::<_, PoiRow>(
            r#"SELECT ... WHERE ST_DWithin(...)"#
        )
        // ... binding parameters ...
    };
}
```

**Lines 83-148**: Same pattern for `find_pois_in_bbox`

**Recommendation**: Create a helper to build parameterized queries and handle optional filters.

---

### 1.5 Repeated Health Check Logic (debug.rs:7-52)
**Severity**: Medium | **Lines**: 46 lines

Three almost identical blocks checking different services:

```rust
pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut status = json!({"status": "ok", "checks": {}});
    
    // Block 1: Database check (lines 14-22)
    match sqlx::query("SELECT 1").fetch_one(&state.db_pool).await {
        Ok(_) => {
            status["checks"]["database"] = json!("ok");
        }
        Err(e) => {
            status["checks"]["database"] = json!({"error": e.to_string()});
            status["status"] = json!("error");
        }
    }
    
    // Block 2: PostGIS check (lines 24-36) - identical pattern
    match sqlx::query("SELECT PostGIS_Version()").fetch_one(&state.db_pool).await {
        Ok(_) => {
            status["checks"]["postgis"] = json!("ok");
        }
        Err(e) => {
            status["checks"]["postgis"] = json!({"error": e.to_string()});
            status["status"] = json!("error");
        }
    }
    
    // Block 3: POI count (lines 39-49) - same pattern
    match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pois").fetch_one(&state.db_pool).await {
        Ok(count) => {
            status["checks"]["poi_count"] = json!(count);
        }
        Err(e) => {
            status["checks"]["poi_count"] = json!({"error": e.to_string()});
        }
    }
}
```

**Recommendation**: Extract into a helper function:
```rust
async fn check_service<T>(
    name: &str,
    check_fn: impl Future<Output = Result<T, sqlx::Error>>,
) -> (String, Value) {
    match check_fn.await {
        Ok(_) => (name.to_string(), json!("ok")),
        Err(e) => (name.to_string(), json!({"error": e.to_string()})),
    }
}
```

---

## 2. MAGIC NUMBERS AND HARDCODED VALUES

### 2.1 Configuration Defaults (config.rs)
**Severity**: High | **Lines**: 20, 22, 29, 33, 37

```rust
host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),           // Line 20
port: env::var("PORT")
    .unwrap_or_else(|_| "3000".to_string())                                 // Line 22
    .parse()
    .map_err(|_| "Invalid PORT")?,

route_cache_ttl: env::var("ROUTE_CACHE_TTL")
    .unwrap_or_else(|_| "86400".to_string())                                // Line 29 (86400 = 24 hours)
    .parse()
    .map_err(|_| "Invalid ROUTE_CACHE_TTL")?,

poi_region_cache_ttl: env::var("POI_REGION_CACHE_TTL")
    .unwrap_or_else(|_| "604800".to_string())                               // Line 33 (604800 = 7 days)
    .parse()
    .map_err(|_| "Invalid POI_REGION_CACHE_TTL")?,

snap_radius_m: env::var("SNAP_RADIUS_M")
    .unwrap_or_else(|_| "100.0".to_string())                                // Line 37
    .parse()
    .map_err(|_| "Invalid SNAP_RADIUS_M")?,
```

**Recommendation**: Define constants at module top:
```rust
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "3000";
const DEFAULT_ROUTE_CACHE_TTL_SECS: u64 = 86400;  // 24 hours
const DEFAULT_POI_REGION_CACHE_TTL_SECS: u64 = 604800;  // 7 days
const DEFAULT_SNAP_RADIUS_M: f64 = 100.0;
```

---

### 2.2 Route Generation Constants (route_generator.rs)
**Severity**: Medium | **Various lines**

Scattered throughout the file:

```rust
// Line 135: MAX_RETRIES
const MAX_RETRIES: usize = 5;

// Lines 225-231: Hardcoded distance thresholds
let num_waypoints = if (target_distance_km > 10.0 && pois.len() >= 6)
    || (target_distance_km > 5.0 && pois.len() >= 4) { 3 } else { 2 };

// Line 245: Minimum distance
if dist < 0.2 { return None; }

// Line 253: Distance ratio
let max_reasonable_dist = target_distance_km / 1.5;

// Lines 260-266: Scoring multipliers
let distance_score = if dist < target_waypoint_distance {
    (dist / target_waypoint_distance) as f32 * 0.8 + 0.2
} else {
    let excess_ratio = (dist - target_waypoint_distance) / target_waypoint_distance;
    (1.0 - (excess_ratio * 0.5).min(0.8)) as f32
};

// Line 272: Variation calculation
let variation_offset = ((idx * 3 + variation * 11) % 100) as f32 * 0.05;

// Line 365: Angle thresholds
let min_angle_diff = if pois.len() == 2 { 1.0 } else { 1.047 };

// Lines 446-450: Route scoring weights
score += 3.0 * (1.0 - distance_error_ratio.min(1.0)) as f32;  // Distance
score += poi_count_score;  // POI count (max 3.0)
score += 2.0 * avg_poi_quality;  // POI quality
score += 2.0 * diversity_score;  // Category diversity
```

**Recommendation**: Group into constants at module level:
```rust
// Route generation tuning parameters
const MAX_ROUTE_GENERATION_RETRIES: usize = 5;
const MIN_POI_DISTANCE_KM: f64 = 0.2;
const MAX_REASONABLE_DISTANCE_RATIO: f64 = 1.5;

// Waypoint selection parameters
const MIN_3_WAYPOINT_DISTANCE_KM: f64 = 10.0;
const MIN_3_WAYPOINT_POI_COUNT: usize = 6;
const MIN_2_WAYPOINT_DISTANCE_KM: f64 = 5.0;
const MIN_2_WAYPOINT_POI_COUNT: usize = 4;

// Scoring weights (0-10 scale)
const DISTANCE_ACCURACY_WEIGHT: f32 = 3.0;
const POI_COUNT_WEIGHT: f32 = 3.0;
const POI_QUALITY_WEIGHT: f32 = 2.0;
const CATEGORY_DIVERSITY_WEIGHT: f32 = 2.0;

// Distance scoring parameters
const CLOSER_THAN_IDEAL_MULTIPLIER: f32 = 0.8;
const CLOSER_THAN_IDEAL_OFFSET: f32 = 0.2;
const FARTHER_THAN_IDEAL_PENALTY: f32 = 0.5;
```

---

### 2.3 POI Service Constants (poi_service.rs)
**Severity**: Medium | **Line**: 44

```rust
let min_poi_count = ((radius_km * 2.5) as usize).clamp(10, 50);
// Magic: 2.5 multiplier, 10 minimum, 50 maximum
```

Should be:
```rust
const MIN_POI_THRESHOLD_RATIO: f64 = 2.5;
const MIN_POI_THRESHOLD_ABSOLUTE: usize = 10;
const MAX_POI_THRESHOLD_ABSOLUTE: usize = 50;

let min_poi_count = ((radius_km * MIN_POI_THRESHOLD_RATIO) as usize)
    .clamp(MIN_POI_THRESHOLD_ABSOLUTE, MAX_POI_THRESHOLD_ABSOLUTE);
```

---

### 2.4 Overpass API Constants (overpass.rs)
**Severity**: Medium | **Lines**: 107, 156, 407, and others

```rust
// Line 107: Hardcoded timeout
.timeout(std::time::Duration::from_secs(60))

// Lines 156, 407: Hardcoded status codes
let is_retryable = status == 429 || status == 504;

// Lines 96, 342: Hardcoded retry counts
let max_retries = 2;  // vs let max_retries = 1;
```

Should be:
```rust
const OVERPASS_REQUEST_TIMEOUT_SECS: u64 = 60;
const RETRYABLE_STATUS_TOO_MANY_REQUESTS: u16 = 429;
const RETRYABLE_STATUS_GATEWAY_TIMEOUT: u16 = 504;
const SINGLE_QUERY_MAX_RETRIES: usize = 2;
const BATCH_QUERY_MAX_RETRIES: usize = 1;
```

---

### 2.5 Coordinate Conversion Constants (snapping_service.rs)
**Severity**: Low | **Line**: 110-112

```rust
let lat_buffer = buffer_m / 111_000.0;  // Line 110 - magic number!
let mid_lat = (min_lat + max_lat) / 2.0;
let lng_buffer = buffer_m / (111_000.0 * mid_lat.to_radians().cos());  // Line 112 - duplicated magic!
```

Should be:
```rust
const METERS_PER_DEGREE_LAT: f64 = 111_000.0;  // Approximate at equator

let lat_buffer = buffer_m / METERS_PER_DEGREE_LAT;
let mid_lat = (min_lat + max_lat) / 2.0;
let lng_buffer = buffer_m / (METERS_PER_DEGREE_LAT * mid_lat.to_radians().cos());
```

---

## 3. ERROR HANDLING INCONSISTENCIES

### 3.1 Silent Category Parsing Failures (db/queries.rs)
**Severity**: Medium | **Lines**: 196, 229

```rust
// Line 196 in PoiRow::from implementation
category: row.category.parse().unwrap_or(PoiCategory::Historic),

// Line 229 in PoiRowSimple::from implementation
category: row.category.parse().unwrap_or(PoiCategory::Historic),
```

**Issue**: Invalid categories silently default to `Historic`. Should log and handle explicitly:

```rust
category: row.category.parse()
    .unwrap_or_else(|e| {
        tracing::warn!("Failed to parse category '{}': {}, defaulting to Historic", 
                       row.category, e);
        PoiCategory::Historic
    }),
```

---

### 3.2 Inconsistent Error Type in SnappingService (snapping_service.rs)
**Severity**: Low | **Line**: 26

```rust
pub async fn find_snapped_pois(
    &self,
    route_path: &[Coordinates],
    waypoint_pois: &[RoutePoi],
    snap_radius_m: f64,
    categories: Option<&[PoiCategory]>,
) -> Result<Vec<SnappedPoi>, Box<dyn std::error::Error>> {  // Generic error type!
```

**Issue**: Uses generic `Box<dyn std::error::Error>` instead of application's `AppError`.

**Recommendation**:
```rust
pub async fn find_snapped_pois(
    ...
) -> crate::Result<Vec<SnappedPoi>> {  // Use app's Result type
```

---

### 3.3 Silent Coordinate Validation Failures (overpass.rs)
**Severity**: Low | **Line**: 506

```rust
let coords = Coordinates::new(lat, lon).ok()?;
// Silently drops POIs with invalid coordinates without logging
```

**Recommendation**:
```rust
let coords = match Coordinates::new(lat, lon) {
    Ok(c) => c,
    Err(e) => {
        tracing::debug!("Invalid coordinates for POI {}: {}", elem.id, e);
        return None;
    }
};
```

---

### 3.4 Unhandled Async Operation Failures (poi_service.rs)
**Severity**: Medium | **Lines**: 78-80, 106-109

```rust
// Lines 78-80: Silent insertion failures
for poi in &overpass_pois {
    if let Err(e) = queries::insert_poi(&self.db_pool, poi).await {
        tracing::debug!("Failed to insert POI {}: {}", poi.name, e);
    }
}

// Lines 106-109: Same pattern repeated
for poi in &batched_pois {
    if let Err(e) = queries::insert_poi(&self.db_pool, poi).await {
        tracing::debug!("Failed to insert POI {}: {}", poi.name, e);
    }
}
```

**Issue**: Code continues execution even if POI insertion fails. Should at least count failures:

```rust
let mut insertion_failures = 0;
for poi in &overpass_pois {
    if let Err(e) = queries::insert_poi(&self.db_pool, poi).await {
        insertion_failures += 1;
        tracing::debug!("Failed to insert POI {}: {}", poi.name, e);
    }
}
if insertion_failures > 0 {
    tracing::warn!("Failed to insert {} POIs from Overpass API", insertion_failures);
}
```

---

## 4. COMPLEX CONDITIONALS

### 4.1 Nested Distance Selection Logic (route_generator.rs)
**Severity**: Medium | **Lines**: 225-231

```rust
let num_waypoints = if (target_distance_km > 10.0 && pois.len() >= 6)
    || (target_distance_km > 5.0 && pois.len() >= 4)
{
    3
} else {
    2
};
```

**Recommendation**: Extract into a named function:
```rust
fn determine_waypoint_count(target_distance_km: f64, available_pois: usize) -> usize {
    match (target_distance_km, available_pois) {
        (d, p) if d > 10.0 && p >= 6 => 3,
        (d, p) if d > 5.0 && p >= 4 => 3,
        _ => 2,
    }
}
```

---

### 4.2 Complex Distance Adjustment (route_generator.rs)
**Severity**: Medium | **Lines**: 144-152

```rust
let adjusted_target = if retry == 0 {
    target_distance_km
} else if retry <= 2 {
    // Try variations around the target
    target_distance_km * (0.8 + (retry as f64 * 0.2))
} else {
    // More aggressive adjustments for later retries
    target_distance_km * (0.6 + (retry as f64 * 0.15))
};
```

**Recommendation**: Extract into a strategy function:
```rust
fn calculate_adjusted_distance(
    target_distance_km: f64,
    retry_attempt: usize,
) -> f64 {
    match retry_attempt {
        0 => target_distance_km,
        1..=2 => target_distance_km * (0.8 + (retry_attempt as f64 * 0.2)),
        _ => target_distance_km * (0.6 + (retry_attempt as f64 * 0.15)),
    }
}
```

---

### 4.3 Cascading Fallback Logic (poi_service.rs)
**Severity**: Medium | **Lines**: 69-140

The `find_pois` function has deeply nested error handling:

```rust
// First try single query
let single_query_result = self.overpass_client.query_pois(...).await;

match single_query_result {
    Ok(overpass_pois) => {
        // Success path: insert and return
    }
    Err(e) => {
        let is_timeout = error_str.contains("timed out");
        
        // Timeout-specific retry with batched queries
        if is_timeout && categories_to_fetch.len() > 3 {
            match self.overpass_client.query_pois_batched(...).await {
                Ok(batched_pois) => {
                    // Success with batches
                }
                Err(batch_error) => {
                    // Batch failed, fall back to db
                }
            }
        }
        
        // All Overpass failed, use database
        if db_pois.is_empty() {
            return Err(e);
        }
        Ok(db_pois.into_iter().take(limit).collect())
    }
}
```

**Recommendation**: Create a strategy enum to handle fallback more clearly:
```rust
enum PoiQueryStrategy {
    DirectQuery,
    BatchedQuery,
    DatabaseOnly,
}

impl PoiService {
    async fn try_query_with_fallback(&self, center, radius, categories) -> Result<Vec<Poi>> {
        // Try each strategy in order
    }
}
```

---

## 5. MODULE ORGANIZATION ISSUES

### 5.1 Private Struct in Services (snapping_service.rs)
**Severity**: Low | **Lines**: 123-128

```rust
struct BoundingBox {
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
}
```

**Issue**: Should be in `models/` module for reuse.

**Recommendation**: Move to `/src/models/bounding_box.rs` and expose from models module.

---

### 5.2 Placeholder Implementation (cache/mod.rs)
**Severity**: Low | **Lines**: 1-15

```rust
// Redis cache module - to be implemented in Phase 2
// For Phase 1 MVP, we're not using Redis caching yet

#[allow(dead_code)]
pub struct CacheService {
    // redis_client: redis::Client,
}

#[allow(dead_code)]
impl CacheService {
    pub fn new(_redis_url: &str) -> Self {
        CacheService {}
    }
}
```

**Issue**: Dead code attribute suppresses compiler warnings. Should either:
1. Remove if not needed
2. Complete implementation if planned
3. Create issue/TODO comment referencing phase 2

---

### 5.3 Highly Duplicated Query Row Types (db/queries.rs)
**Severity**: Medium | **Lines**: 176-242

```rust
// Line 177-189: PoiRow with distance
#[derive(sqlx::FromRow)]
struct PoiRow {
    id: Uuid,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f32,
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
    #[allow(dead_code)]
    distance_meters: f64,
}

// Line 212-221: PoiRowSimple without distance
#[derive(sqlx::FromRow)]
struct PoiRowSimple {
    id: Uuid,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f32,
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
}
```

**Issue**: Almost identical structs. The `#[allow(dead_code)]` on distance_meters is a red flag.

**Recommendation**: Use a single struct or make distance optional:
```rust
#[derive(sqlx::FromRow)]
struct PoiRow {
    id: Uuid,
    name: String,
    category: String,
    lat: f64,
    lng: f64,
    popularity_score: f32,
    description: Option<String>,
    estimated_visit_duration_minutes: Option<i32>,
    #[sqlx(default)]  // Optional field
    distance_meters: Option<f64>,
}
```

---

## 6. NAMING AND CLARITY ISSUES

### 6.1 Unclear Parameter Name (route_generator.rs)
**Severity**: Low | **Line**: 131

```rust
async fn try_generate_loop(
    ...
    variation: usize,  // What does this mean?
    ...
)
```

**Issue**: `variation` is cryptic. Used for POI selection rotation and random seeding.

**Recommendation**: Rename to clarify intent:
```rust
attempt_seed: usize,  // or poi_selection_rotation
```

---

### 6.2 Unclear Batch Grouping Logic (overpass.rs)
**Severity**: Low | **Lines**: 274-302

```rust
let mut high_value = Vec::new();         // Tourist attractions, monuments
let mut nature_outdoor = Vec::new();     // Parks, waterfronts, nature
let mut urban_cultural = Vec::new();     // Museums, plazas, urban POIs

for category in categories {
    match category {
        // ...categorization logic
    }
}
```

**Recommendation**: Define a category grouping strategy enum:
```rust
enum CategoryGroup {
    HighValueAttraction,
    NatureOutdoor,
    UrbanCultural,
}

impl PoiCategory {
    fn group(&self) -> CategoryGroup {
        match self {
            // ...
        }
    }
}
```

---

### 6.3 Implicit Coordinate Format (mapbox.rs)
**Severity**: Low | **Lines**: 42-46

```rust
let coordinates_str = waypoints
    .iter()
    .map(|c| format!("{},{}", c.lng, c.lat))  // Note: lng BEFORE lat!
    .collect::<Vec<_>>()
    .join(";");
```

**Issue**: Mapbox format (lng,lat) is different from typical GIS (lat,lng). Should be explicit.

**Recommendation**: Extract into a helper function with clear naming:
```rust
fn format_mapbox_coordinate(coord: &Coordinates) -> String {
    // Note: Mapbox uses (longitude, latitude) order, opposite of typical GIS conventions
    format!("{},{}", coord.lng, coord.lat)
}

let coordinates_str = waypoints
    .iter()
    .map(format_mapbox_coordinate)
    .collect::<Vec<_>>()
    .join(";");
```

---

## 7. ADDITIONAL ISSUES

### 7.1 Test Organization (poi_service.rs)
**Severity**: Low | **Lines**: 185-207

```rust
#[cfg(test)]
mod tests {
    // Tests removed for now - need async test setup
}

#[cfg(test)]
mod unit_tests {
    // Actual tests here
}
```

**Issue**: Two test modules, one empty with comment about async tests.

**Recommendation**: Either implement async tests or remove the placeholder.

---

### 7.2 Hardcoded Default Category (db/queries.rs)
**Severity**: Medium | **Lines**: 196, 229

```rust
category: row.category.parse().unwrap_or(PoiCategory::Historic),
```

**Issue**: Why default to `Historic`? This should be configurable or documented.

---

## SUMMARY BY SEVERITY

### High Severity (Address First)
- `try_generate_loop` function too long and does multiple things
- Duplicated retry logic in overpass.rs (95-184 vs 340-436)
- Duplicated database queries (find_pois_within_radius vs find_pois_in_bbox)
- Magic numbers scattered throughout without constants
- Config defaults should use named constants

### Medium Severity (Address Next)
- `infer_category` cascading if-let chains (118 lines)
- Silent category parsing failures
- Complex nested conditionals in waypoint selection
- Complex fallback logic in poi_service
- Duplicated PoiRow structs
- Health check repeated patterns

### Low Severity (Nice to Have)
- BoundingBox struct should be in models
- Parameter naming clarity (variation → attempt_seed)
- Implicit coordinate format in mapbox.rs
- Hardcoded default categories
- Test organization

---

## RECOMMENDED REFACTORING ROADMAP

1. **Phase 1 - High Priority (1-2 days)**
   - Extract constants module with all magic numbers
   - Break down `try_generate_loop` into smaller functions
   - Consolidate duplicate retry logic in overpass.rs
   - Consolidate duplicate query functions in db/queries.rs

2. **Phase 2 - Medium Priority (1-2 days)**
   - Extract cascading category inference logic
   - Simplify nested conditionals
   - Create abstraction for fallback strategy
   - Consolidate row struct definitions

3. **Phase 3 - Low Priority (1 day)**
   - Move BoundingBox to models
   - Rename unclear parameters
   - Extract coordinate formatting helpers
   - Organize tests properly

