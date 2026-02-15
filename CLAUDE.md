# CLAUDE.md

Rust REST API + iOS app that generates personalized walking/cycling loop routes with POI waypoints. Given a start point and target distance, discovers nearby POIs, selects waypoints forming a loop, generates turn-by-turn routes via Mapbox, and scores/ranks alternatives.

**Tech Stack**: Rust (stable) | Axum 0.8 | PostgreSQL 18 + PostGIS 3.6 | SQLite (on-device) | Redis 8.4 | Mapbox Directions API | SwiftUI (iOS)

## Build & Development Commands

```bash
# Prerequisites: docker-compose up -d postgres redis

# Run server (PostgreSQL backend)
cargo run --bin easyroute

# Run on-device server (SQLite backend)
cargo run --bin ondevice -- --region=regions/monaco.db --open

# Build SQLite region DB from OSM PBF
cargo run --bin build_region -- --input=osm/data/monaco-latest.osm.pbf --output=regions/monaco.db

# Run Mapbox proxy (for mobile clients)
cargo run --bin proxy

# Run evaluation harness
cargo run --bin evaluate -- --scenario=monaco --runs=5

# Fast tests (skips Mapbox/external API calls)
SKIP_REAL_API_TESTS=true cargo test

# Full tests (requires MAPBOX_API_KEY in .env)
cargo test

# Single test / integration tests only
cargo test test_waypoint_selection
cargo test --test '*'

# Lint & format
cargo fmt && cargo clippy -- -D warnings

# Quick compile check
cargo check
```

Database tests use `easyroute_test` (via `TEST_DATABASE_URL`), run serially via `serial_test`. Test utilities in `tests/common/mod.rs`.

## Project Structure

```
src/
├── main.rs                    # Axum server entry point (PostgreSQL mode)
├── lib.rs                     # Library exports
├── config.rs                  # RouteGeneratorConfig, parse_env! macro, ROUTE_* env vars
├── constants.rs               # Application-wide constants
├── error.rs                   # thiserror Error enum
├── ffi.rs                     # C FFI: easyroute_start/stop for iOS embedding
├── mobile.rs                  # On-device Axum server (SQLite + embedded web UI)
│
├── bin/
│   ├── evaluate.rs            # Evaluation harness CLI
│   ├── ondevice.rs            # Standalone on-device server CLI
│   ├── build_region.rs        # OSM PBF -> SQLite region DB builder
│   └── proxy.rs               # Mapbox API proxy with auth + rate limiting
│
├── services/
│   ├── route_generator/       # Core algorithm (strategy pattern)
│   │   ├── mod.rs             # Orchestrator: POI discovery, generation loop, caching
│   │   ├── waypoint_selection.rs  # 2-4 POI waypoint selection by distance/angle
│   │   ├── scoring_strategy.rs    # Simple vs Advanced scoring strategies
│   │   ├── tolerance_strategy.rs  # Adaptive tolerance: ±20% -> ±30% -> ±50%
│   │   ├── geometric_loop.rs      # Fallback: 4 geometric circle waypoints
│   │   ├── route_scoring.rs       # V1/V2 route scoring
│   │   ├── route_metrics.rs       # 7 quality metrics (circularity, convexity, etc.)
│   │   └── geometry.rs            # Shared: convex_hull, shoelace_area, angle_from_start
│   ├── poi_service.rs         # POI queries via PoiRepository trait
│   ├── mapbox.rs              # Mapbox Directions API client
│   └── snapping_service.rs   # Snap POIs to route path (within 100m)
│
├── models/                    # Data types with validation
│   ├── coordinates.rs         # Coordinates (lat/lng with bounds)
│   ├── distance.rs            # DistanceKm, DistanceMeters, RadiusMeters newtypes
│   ├── poi.rs                 # Poi, PoiCategory (27 categories)
│   ├── route.rs               # Route with GeoJSON, score, metrics
│   ├── geo.rs                 # BoundingBox, LineString helpers
│   └── evaluation.rs          # Evaluation/rating models
│
├── db/
│   ├── poi_repository.rs      # PoiRepository trait + PgPoiRepository
│   ├── poi_queries.rs         # PostGIS spatial queries
│   ├── evaluation_queries.rs  # Evaluation/rating queries
│   ├── sqlite_repo.rs         # SqlitePoiRepository (R-tree spatial index)
│   └── sqlite_repo_tests.rs
│
├── cache/
│   ├── mod.rs                 # RouteCache trait, cache key generation
│   ├── redis.rs               # RedisCacheService (24h TTL)
│   └── memory.rs              # MemoryCacheService (for on-device)
│
├── evaluation/                # Evaluation harness
│   ├── mod.rs                 # Scenario runner, metric aggregation
│   ├── scenarios.rs           # Test scenarios (dense/sparse/geometric)
│   └── baseline.rs            # Baseline comparison with regression detection
│
├── osm/                       # OSM tag -> POI mapping
│   └── mod.rs                 # determine_category, calculate_popularity, etc.
│
└── routes/                    # Axum API handlers
    ├── loop_route.rs          # POST /api/v1/routes/loop
    ├── pois.rs                # GET /api/v1/pois
    ├── debug.rs               # GET /api/v1/debug/health
    └── evaluation.rs          # /api/v1/evaluations/* endpoints

ios/EasyRoute/                 # Native iOS SwiftUI app
                               # Embeds Rust server via C FFI (ffi.rs)

osm/                           # OSM import scripts
├── download_osm.sh            # Download Geofabrik extracts
├── import_osm.sh              # Import via osm2pgsql (PostgreSQL)
└── osm_poi_style.lua          # POI extraction rules (27 categories)
```

## Architecture

### Dual Backend: Server vs On-Device

**Server mode** (`cargo run --bin easyroute`): PostgreSQL/PostGIS + Redis. Full-featured with spatial indexes and route caching.

**On-device mode** (`cargo run --bin ondevice` / iOS app via FFI): SQLite with R-tree spatial index + in-memory cache. Same route generation logic, portable `.db` region files built from OSM PBF via `build_region`.

Both modes share the same `PoiRepository` trait (`src/db/poi_repository.rs`) — `PgPoiRepository` for server, `SqlitePoiRepository` for on-device.

### Request Flow

```
POST /api/v1/routes/loop
  -> Check route cache (Redis or in-memory, bucketed by ~100m coords + ~0.5km distance)
  -> Query POIs from PoiRepository within search radius
  -> Select 2-4 waypoints with spatial distribution checks
  -> Generate routes via Mapbox Directions API for waypoint combinations
  -> Adaptive tolerance: normal (±20%) -> relaxed (±30%) -> very relaxed (±50%)
  -> Snap additional nearby POIs to route path (within 100m)
  -> Score and rank alternatives
  -> Cache result (24h TTL)
```

### Route Generator (Strategy Pattern)

The route generator (`src/services/route_generator/`) is the core component:

- `mod.rs` - Orchestrator: POI discovery, route generation loop, caching
- `waypoint_selection.rs` - Selects 2-4 POIs as waypoints; `Advanced` strategy rewards candidates that expand convex hull area
- `scoring_strategy.rs` - `Simple` (distance-only) vs `Advanced` (quality + clustering + angular diversity + shape prediction)
- `tolerance_strategy.rs` - Adaptive tolerance; `verify_loop_shape()` rejects bad configurations before Mapbox calls
- `geometric_loop.rs` - Fallback: 4 geometric circle waypoints (±15% radius jitter, ~20° rotation jitter)
- `route_scoring.rs` - V1 (distance accuracy, POI count, quality, diversity) / V2 (adds circularity, convexity, path overlap)
- `route_metrics.rs` - 7 quality metrics auto-computed and attached to every route
- `geometry.rs` - Shared geometric functions (convex hull, shoelace area, angles)

Config: `ROUTE_POI_SCORING_STRATEGY` (`simple`/`advanced`), `ROUTE_SCORING_VERSION` (`1`/`2`). All params use `ROUTE_` env var prefix (see `src/config.rs`).

### Mapbox Proxy

`src/bin/proxy.rs` — Rate-limited proxy for mobile clients. Authenticates via Bearer tokens (`PROXY_API_KEYS`), forwards to Mapbox with the server's `MAPBOX_API_KEY`. Env vars: `PROXY_API_KEYS`, `PROXY_RATE_LIMIT` (default 20/min), `PROXY_PORT` (default 4000).

## Important Patterns

**Semantic types**: Always use newtype wrappers (`DistanceKm`, `Coordinates`) instead of primitive `f64`. Validation at construction time.

**Error handling**: `thiserror` `Error` enum in `src/error.rs`. All services return `Result<T, Error>`. No `.unwrap()` in production code.

**PostGIS coordinate order**: PostGIS uses `(longitude, latitude)`, Rust `Coordinates` uses `(lat, lng)`. Always swap: `ST_GeogFromText('POINT({lng} {lat})')`.

**Repository trait**: `PoiRepository` trait (`src/db/poi_repository.rs`) abstracts PostgreSQL vs SQLite. Services depend on `Arc<dyn PoiRepository>`.

**Cache trait**: `RouteCache` trait (`src/cache/mod.rs`) abstracts Redis vs in-memory. Both use bucketed cache keys.

**Config macro**: `parse_env!` macro in `src/config.rs` for concise env var parsing.

**File size limit**: Soft limit 500 lines, hard limit 800 lines per file.

## API Endpoints

- `POST /api/v1/routes/loop` - Generate loop routes (main endpoint)
- `GET /api/v1/pois` - Query POIs by location/category
- `GET /api/v1/debug/health` - Health check (DB, PostGIS/SQLite, cache, POI count)
- `GET /api/v1/evaluations` - List evaluated routes
- `GET /api/v1/evaluations/{id}` - Get evaluation details
- `POST /api/v1/evaluations/{id}/ratings` - Submit human rating
- `GET /api/v1/evaluations/stats/correlation` - Metric-rating Pearson correlation

## Environment Variables

```bash
# Required
DATABASE_URL=postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute
MAPBOX_API_KEY=your_mapbox_key

# Optional
REDIS_URL=redis://localhost:6379          # Omit for in-memory cache fallback
TEST_DATABASE_URL=...easyroute_test       # For database tests
HOST=0.0.0.0                              # Default: 0.0.0.0
PORT=3000                                 # Default: 3000
RUST_LOG=info,easyroute=debug
SNAP_RADIUS_M=100.0                       # POI snap radius (0-1000m)
ROUTE_CACHE_TTL=86400                     # 24h
ROUTE_POI_SCORING_STRATEGY=simple         # simple | advanced
ROUTE_SCORING_VERSION=1                   # 1 | 2 (shape-aware)
# See src/config.rs for full ROUTE_* parameter list
```

## Regression Detection

Any change to route generation logic or config must be validated:

```bash
just evaluate-baseline --runs=3           # Save baseline (if none exists)
# ... make changes ...
just evaluate-check --runs=3              # Check for regressions (exits 1 if found)
just evaluate-baseline --runs=5           # Update baseline after improvements
```

Applies to: `src/services/route_generator/`, `src/config.rs`, `src/services/snapping_service.rs`. Checks 10 scenarios with 15% regression threshold on metrics (circularity, convexity, POI density, etc.).

## Constraints

- **Mapbox Free Tier**: 100k requests/month. Each route request = 3-5 Mapbox calls. Caching essential.
- **Response Time**: Target < 3s for route generation, < 100ms for POI queries
- **Cache Hit Rate**: Target > 50% (route cache 24h TTL)

## OSM Data Import

POIs come from OpenStreetMap. Two import paths:

```bash
# Server (PostgreSQL via osm2pgsql)
./osm/download_osm.sh monaco
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf

# On-device (SQLite via build_region)
cargo run --bin build_region -- --input=osm/data/monaco-latest.osm.pbf --output=regions/monaco.db
```

Geofabrik extracts: `monaco` (test), `europe/france` (production), any region from https://download.geofabrik.de/

## Troubleshooting

- **"Connection refused"**: Ensure `docker-compose up -d postgres redis`, check `DATABASE_URL`
- **Tests failing with Mapbox errors**: Set `MAPBOX_API_KEY` in `.env` or use `SKIP_REAL_API_TESTS=true`
- **PostGIS not found**: Use `postgis/postgis` Docker image, not plain `postgres`
- **Slow tests**: Use `SKIP_REAL_API_TESTS=true` for fast iteration (~10-20s vs ~60-120s)

## Supplementary Documentation

- [CLAUDE-GIT.md](CLAUDE-GIT.md) - Git commit message format, atomic commit guidelines
