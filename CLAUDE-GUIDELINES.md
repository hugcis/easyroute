# CLAUDE-GUIDELINES.md

Code quality guidelines, development patterns, and security considerations for EasyRoute.

These guidelines ensure AI-friendly, maintainable code and are especially important when working with AI coding assistants.

---

## Code Quality Guidelines

### 1. Semantic Types (Newtype Pattern)

**Always use semantic newtype wrappers instead of primitives.** This pushes business intent into the type system.

#### Good vs Bad

```rust
// ✅ GOOD: Semantic types
pub struct DistanceKm(f64);
pub struct RadiusMeters(f64);
pub struct Coordinates { lat: f64, lng: f64 }

fn generate_route(
    start: Coordinates,
    distance: DistanceKm,
    radius: RadiusMeters
) -> Result<Route>

// ❌ BAD: Primitive types
fn generate_route(
    lat: f64,
    lng: f64,
    distance: f64,
    radius: f64
) -> Result<Route>
```

#### Benefits

- **Type Safety**: Can't pass a `DistanceKm` where `RadiusMeters` is expected
- **Self-Documenting**: Function signatures clearly express intent
- **Validation**: Enforce constraints at construction time
- **AI-Friendly**: Makes AI-generated code more type-safe

#### Examples in This Codebase

- `src/models/distance.rs`: `DistanceKm`, `DistanceMeters`, `RadiusMeters`
- `src/models/coordinates.rs`: `Coordinates` with validation
- **Recommended**: Consider `PoiId(Uuid)`, `RouteId(Uuid)` instead of raw `Uuid`

---

### 2. File Organization & Naming

**Treat directory structure as a user interface for AI tools.**

#### Guidelines

✅ **Use descriptive, specific module paths**
- `services/route_generator.rs` not `utils/helpers.rs`
- `models/coordinates.rs` not `types.rs`
- `routes/loop_route.rs` not `handlers.rs`

✅ **One primary type/service per file**
- `poi.rs` contains `Poi` struct and `PoiCategory` enum
- `route_generator.rs` contains `RouteGenerator` service
- Avoid "dumping ground" modules with unrelated code

✅ **Group related functionality in clear modules**
```
src/
├── models/          # Data types (coordinates, distance, poi, route)
├── services/        # Business logic (route_generator, poi_service, mapbox)
├── routes/          # API endpoints (loop_route, pois, debug)
├── db/              # Database layer (queries)
└── cache/           # Caching logic (redis)
```

#### Current Structure (Maintain This Pattern)

- Models: Pure data structures with validation
- Services: Business logic, external API clients
- Routes: Axum handlers, request/response
- DB: Database queries and migrations
- Cache: Redis operations

---

### 3. File Size Limits

**Soft limit: 500 lines per file. Hard limit: 800 lines.**

#### Rationale

- **AI Context Windows**: Smaller files fit entirely without truncation
- **Code Review**: Easier to review and understand
- **Testing**: Focused files are easier to test
- **Single Responsibility**: Forces better separation of concerns

#### When Approaching Limits

1. **Extract helper functions** to separate modules
   ```rust
   // route_generator.rs getting large?
   // Extract to:
   // - route_generator/mod.rs
   // - route_generator/waypoint_selection.rs
   // - route_generator/scoring.rs
   ```

2. **Split services** into sub-modules
   ```rust
   // overpass.rs (681 lines) could become:
   // - overpass/mod.rs
   // - overpass/query_builder.rs
   // - overpass/retry_logic.rs
   ```

3. **Consider if doing too much** (Single Responsibility Principle)

#### Current Status

- ✅ Most files: 100-300 lines
- ⚠️ `route_generator.rs`: 727 lines (approaching limit)
- ⚠️ `overpass.rs`: 681 lines (approaching limit)

**Action**: Consider refactoring these when making major changes.

---

### 4. Fast Test Execution

**Target: Test suite completes in <60 seconds for fast iteration.**

#### Why This Matters

- AI agents can run tests multiple times per task
- Faster feedback loops during development
- Encourages test-driven development
- Prevents "waiting on tests" bottleneck

#### Strategies

**Mock External APIs by Default**
```bash
# Fast: Unit tests + mocked integration tests (~10-20s)
SKIP_REAL_API_TESTS=true cargo test

# Slow: Full integration tests with real APIs (~60-120s)
cargo test
```

**Use Test Fixtures**
- Centralize setup in `tests/common/mod.rs`
- Reuse database connections
- Pre-create test data

**Run Expensive Tests Separately**
```bash
# Quick smoke test during development
cargo test --lib

# Full suite before commit
cargo test
```

**In-Memory Databases When Possible**
- Consider SQLite in-memory for unit tests
- Use PostgreSQL for integration tests only

#### Current Setup

```rust
// In test files:
#[tokio::test]
async fn test_route_generation() {
    if std::env::var("SKIP_REAL_API_TESTS").is_ok() {
        return; // Skip test
    }
    // ... real API test
}
```

---

### 5. Regression Detection for Routing Changes

**Any change to routing logic or config parameters must be validated against the evaluation baseline.**

This applies to changes in:
- `src/services/route_generator/` (waypoint selection, scoring, tolerance, geometric fallback)
- `src/services/route_generator/route_metrics.rs` (metric computation)
- `src/services/route_generator/route_scoring.rs` (scoring weights/formulas)
- `src/config.rs` (route generator config defaults or new parameters)
- `src/services/snapping_service.rs` (POI snapping logic)

#### Workflow

```bash
# 1. Before making changes, ensure a baseline exists
#    (skip if evaluation/baseline.json already exists and is current)
just evaluate-baseline --runs=3

# 2. Make your changes to routing logic / config

# 3. Check for regressions against the saved baseline
just evaluate-check --runs=3

# 4. If metrics improved and you want to update the baseline
just evaluate-baseline --runs=5
```

#### What Gets Checked

The evaluation harness runs 10 scenarios (dense/moderate/sparse/geometric, various distances and modes) and compares each metric against the baseline with a 15% regression threshold:
- **Higher-is-better**: circularity, convexity, POI density, category entropy, landmark coverage, success rate
- **Lower-is-better**: path overlap percentage

A regression is flagged when a metric worsens beyond the threshold. `--check` exits with code 1 if any regressions are detected.

#### Key Commands

```bash
just evaluate-check --runs=3              # Quick regression check
just evaluate-check --runs=3 --json       # Machine-readable output
just evaluate-baseline --runs=5           # Save new baseline after improvements
cargo run --bin evaluate -- --help        # See all flags
```

---

### 6. Type Safety Across Boundaries

**Leverage Rust's type system at all boundaries.**

#### Database (SQLx)

✅ **Use compile-time checked queries**
```rust
// Compile-time verification of SQL and column types
let pois = sqlx::query_as!(
    Poi,
    r#"
    SELECT id, name, category, location
    FROM pois
    WHERE ST_DWithin(location, $1, $2)
    "#,
    location,
    radius_meters
)
.fetch_all(&pool)
.await?;
```

✅ **Leverage PostgreSQL constraints**
- `CHECK (popularity >= 0 AND popularity <= 100)`
- `UNIQUE (osm_id)`
- `NOT NULL` where appropriate

✅ **Type JSONB metadata**
```rust
// Don't leave JSONB untyped
#[derive(Deserialize)]
struct PoiMetadata {
    wikipedia_url: Option<String>,
    unesco_site: bool,
}

let metadata: PoiMetadata = serde_json::from_value(row.metadata)?;
```

#### APIs (Axum)

✅ **Strongly-typed request/response models**
```rust
#[derive(Deserialize)]
pub struct LoopRouteRequest {
    pub start: Coordinates,  // Not (f64, f64)
    pub distance_km: DistanceKm,  // Not f64
    pub mode: TravelMode,  // Not String
    pub preferences: Option<RoutePreferences>,
}
```

✅ **Validate in constructors**
```rust
impl Coordinates {
    pub fn new(lat: f64, lng: f64) -> Result<Self, Error> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(Error::InvalidLatitude(lat));
        }
        if !(-180.0..=180.0).contains(&lng) {
            return Err(Error::InvalidLongitude(lng));
        }
        Ok(Self { lat, lng })
    }
}
```

✅ **Return typed errors**
```rust
// Not anyhow::Error or Box<dyn Error>
pub enum Error {
    InvalidCoordinates,
    MapboxApiError(String),
    DatabaseError(sqlx::Error),
    // ...
}
```

#### External APIs

✅ **Wrap third-party clients**
```rust
pub struct MapboxClient {
    client: reqwest::Client,
    api_key: String,
}

impl MapboxClient {
    pub async fn get_directions(
        &self,
        waypoints: &[Coordinates],  // Typed input
        profile: TravelMode,
    ) -> Result<MapboxResponse, Error> {  // Typed output
        // ...
    }
}
```

---

### 7. Automated Code Quality

**All code must pass automated quality checks before commit.**

#### Required Checks

```bash
# 1. Format code (auto-fix)
cargo fmt

# 2. Check for common mistakes (must pass)
cargo clippy -- -D warnings

# 3. Run tests
cargo test
```

#### Recommended Clippy Configuration

Add to `Cargo.toml`:

```toml
[lints.clippy]
# Warn on all default lints
all = "warn"

# Enable pedantic lints (opinionated but useful)
pedantic = "warn"

# Specific lints to enforce strictly
unwrap_used = "deny"           # Force explicit error handling
expect_used = "warn"            # Discourage expect, prefer proper errors
panic = "deny"                  # No panics in production code (except tests)
todo = "warn"                   # Track incomplete code
unimplemented = "warn"          # Track unimplemented features

# Allow some pedantic lints that are too noisy
must_use_candidate = "allow"
missing_errors_doc = "allow"
module_name_repetitions = "allow"
```

#### Why These Lints Matter

**`unwrap_used = "deny"`**
```rust
// ❌ BAD: Can panic
let poi = pois.get(0).unwrap();

// ✅ GOOD: Explicit error handling
let poi = pois.first().ok_or(Error::NoPoisFound)?;
```

**`expect_used = "warn"`**
```rust
// ⚠️ DISCOURAGED: Still can panic
let api_key = env::var("MAPBOX_API_KEY")
    .expect("MAPBOX_API_KEY must be set");

// ✅ BETTER: Return error
let api_key = env::var("MAPBOX_API_KEY")
    .map_err(|_| Error::MissingApiKey)?;
```

**`panic = "deny"`**
```rust
// ❌ BAD: Panics in production
if pois.is_empty() {
    panic!("No POIs found");
}

// ✅ GOOD: Return error
if pois.is_empty() {
    return Err(Error::InsufficientPois);
}

// ✅ OK: Panic in tests
#[test]
fn test_poi_service() {
    let result = get_pois().unwrap();  // OK in tests
    assert!(!result.is_empty());
}
```

#### Enforcement

**During Development**
```bash
# Auto-fix what can be fixed
cargo clippy --fix

# Apply formatting
cargo fmt
```

**Pre-Commit** (Recommended)
Create `.git/hooks/pre-commit`:
```bash
#!/bin/bash
cargo fmt --check || exit 1
cargo clippy -- -D warnings || exit 1
```

**CI/CD** (Future)
```yaml
# GitHub Actions
- run: cargo fmt --check
- run: cargo clippy -- -D warnings
- run: cargo test
```

---

### 8. Clear, Focused Documentation

**Every public module, struct, and function should have doc comments.**

#### Documentation Guidelines

✅ **Use `///` for public API documentation**
```rust
/// Generates loop routes from a starting point with POI waypoints.
///
/// This is the core route generation algorithm. It orchestrates:
/// 1. POI discovery within radius
/// 2. Waypoint selection (2-3 POIs with good spatial distribution)
/// 3. Route generation via Mapbox API
/// 4. Adaptive tolerance if no valid routes found
///
/// # Arguments
/// * `start` - Starting coordinates
/// * `distance` - Target total distance
/// * `preferences` - Route preferences (categories, hidden gems, etc.)
///
/// # Returns
/// Vector of alternative routes, sorted by score (best first)
///
/// # Errors
/// Returns `Error::InsufficientPois` if fewer than 2 POIs are available.
/// Returns `Error::MapboxApi` if route generation fails repeatedly.
///
/// # Example
/// ```no_run
/// let routes = generator.generate_loop_route(
///     start,
///     DistanceKm(5.0),
///     Some(preferences)
/// ).await?;
/// ```
pub async fn generate_loop_route(
    &self,
    start: Coordinates,
    distance: DistanceKm,
    preferences: Option<RoutePreferences>,
) -> Result<Vec<Route>, Error> {
    // ...
}
```

✅ **Document invariants and edge cases**
```rust
/// Returns POIs within the specified radius.
///
/// # Important
/// - PostGIS uses (longitude, latitude) order, NOT (lat, lng)!
/// - Radius is in meters
/// - Results are sorted by distance from center
/// - Maximum 50 results to prevent overwhelming the API
```

✅ **Keep CLAUDE.md files updated**
- Document architectural decisions here
- Update when adding new services or patterns
- Link to specific files for details

#### What to Document

**Public APIs**: Always
- All public functions
- All public structs and enums
- Module-level docs

**Private helpers**: Sometimes
- Complex algorithms
- Non-obvious behavior
- Edge cases and gotchas

**Tests**: Rarely
- Test names should be self-documenting
- Add comments for non-obvious test setup

---

## Common Development Patterns

### Error Handling

✅ **Use `thiserror` for custom errors**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid coordinates: lat={0}, lng={1}")]
    InvalidCoordinates(f64, f64),

    #[error("Mapbox API error: {0}")]
    MapboxApi(String),

    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Insufficient POIs (found {found}, need at least {needed})")]
    InsufficientPois { found: usize, needed: usize },
}
```

✅ **All service methods return `Result<T, Error>`**
```rust
// Not pub async fn generate_route(...) -> Vec<Route>
pub async fn generate_route(...) -> Result<Vec<Route>, Error>
```

✅ **Propagate errors with `?` operator**
```rust
let pois = self.poi_service.query_pois(start, radius).await?;
let route = self.mapbox.get_directions(&waypoints).await?;
```

### Async Operations

✅ **Use Tokio async runtime for all I/O**
```rust
pub async fn generate_route(...) -> Result<Route, Error> {
    // All external API calls, DB queries use .await
    let pois = query_database().await?;
    let route = call_mapbox_api().await?;
    cache.set(&key, &route).await?;
    Ok(route)
}
```

✅ **Prefer async/await over raw futures**
```rust
// ✅ GOOD: Clear async/await
async fn process() -> Result<()> {
    let data = fetch_data().await?;
    save_data(data).await?;
    Ok(())
}

// ❌ BAD: Manual future combinators
fn process() -> impl Future<Output = Result<()>> {
    fetch_data().and_then(|data| save_data(data))
}
```

### Logging

✅ **Use `tracing` crate with structured logging**
```rust
use tracing::{info, warn, error, debug};

// Structured fields
tracing::info!(
    route_id = %route.id,
    distance_km = route.distance_km,
    poi_count = route.pois.len(),
    score = route.score,
    "Route generated successfully"
);

// Error logging with context
tracing::error!(
    error = %e,
    coordinates = ?start,
    "Failed to generate route"
);
```

✅ **Use appropriate log levels**
- `error!`: Errors that need investigation
- `warn!`: Unexpected but handled (e.g., Redis unavailable, falling back)
- `info!`: High-level operations (route generated, API called)
- `debug!`: Detailed flow (POI filtering, waypoint selection)

✅ **Configure via environment**
```bash
# Development: Verbose logging
RUST_LOG=debug,easyroute=trace

# Production: Quieter
RUST_LOG=info,easyroute=debug
```

---

## Security Considerations

### Input Validation

✅ **Validate at API boundaries**
```rust
// In model constructors
impl Coordinates {
    pub fn new(lat: f64, lng: f64) -> Result<Self, Error> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(Error::InvalidLatitude(lat));
        }
        if !(-180.0..=180.0).contains(&lng) {
            return Err(Error::InvalidLongitude(lng));
        }
        Ok(Self { lat, lng })
    }
}

impl DistanceKm {
    pub fn new(value: f64) -> Result<Self, Error> {
        if !(0.5..=50.0).contains(&value) {
            return Err(Error::InvalidDistance(value));
        }
        Ok(Self(value))
    }
}
```

✅ **Enforce bounds**
- Coordinates: -90 ≤ lat ≤ 90, -180 ≤ lng ≤ 180
- Distance: 0.5 km ≤ distance ≤ 50 km
- Radius: 0 m ≤ radius ≤ 100 km
- Max alternatives: 1 ≤ n ≤ 5

### SQL Injection Prevention

✅ **Always use parameterized queries**
```rust
// ✅ GOOD: Parameters prevent injection
sqlx::query_as!(
    Poi,
    "SELECT * FROM pois WHERE category = $1",
    category
)

// ❌ BAD: String concatenation
let query = format!("SELECT * FROM pois WHERE category = '{}'", category);
sqlx::query(&query)  // VULNERABLE!
```

✅ **Use SQLx compile-time checking**
- `query!` and `query_as!` macros verify SQL at compile time
- Type mismatches caught before runtime
- Invalid column names caught at compile time

### API Security

#### Current (Development)

- CORS enabled for development
- No authentication (public API)
- No rate limiting

#### Planned (Production)

**HTTPS Only**
- Redirect HTTP → HTTPS
- HSTS headers
- TLS 1.2+ only

**Rate Limiting** (Phase 3)
- Per IP: 100 requests/hour
- Per user: 500 requests/day (once auth implemented)
- Exponential backoff on violations

**CORS Policy**
```rust
// Restrict to specific origins in production
CorsLayer::new()
    .allow_origin("https://app.example.com".parse::<HeaderValue>()?)
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
```

**Authentication** (Phase 5)
- JWT tokens
- OAuth 2.0 for third-party apps
- API key rotation

### Secrets Management

✅ **Never commit secrets**
- Use `.env` files (gitignored)
- Use environment variables in production
- Rotate API keys regularly

✅ **Validate API keys at startup**
```rust
// Fail fast if missing
let mapbox_key = env::var("MAPBOX_API_KEY")
    .map_err(|_| Error::MissingApiKey)?;

// Validate format (basic check)
if mapbox_key.len() < 20 {
    return Err(Error::InvalidApiKey);
}
```

❌ **Never log secrets**
```rust
// ❌ BAD
tracing::debug!("API key: {}", api_key);

// ✅ GOOD
tracing::debug!("API key configured: {}", api_key.len() > 0);
```

---

## Additional Best Practices

### Dependency Management

- Keep dependencies up to date (`cargo outdated`)
- Pin major versions, allow patch updates
- Audit dependencies (`cargo audit`)
- Minimize dependency count

### Performance

- Avoid unnecessary clones
- Use references where possible
- Prefer `&str` over `String` in function parameters
- Use connection pooling (SQLx does this)
- Cache expensive operations (Redis)

### Maintainability

- Follow Rust naming conventions (snake_case, CamelCase)
- Keep functions small and focused
- Avoid deeply nested code
- Use early returns to reduce indentation
- Prefer iteration over recursion (stack overflow risk)
