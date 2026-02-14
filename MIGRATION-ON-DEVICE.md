# Migration Plan: On-Device EasyRoute

This document describes the technical migration from the current server-based architecture (PostgreSQL + PostGIS + Redis + Mapbox) to an on-device architecture with a thin server proxy.

## Target Architecture

```
┌─────────────────────────────────────────┐
│  On Device (Tauri / CLI)                │
│                                         │
│  ┌──────────────┐  ┌────────────────┐   │
│  │ SQLite + R-tree │ Route Engine  │   │
│  │ (region .db) │  │ (scoring,      │   │
│  │              │  │  waypoints,    │   │
│  │              │  │  metrics)      │   │
│  └──────┬───────┘  └───────┬────────┘   │
│         │                  │            │
│  ┌──────┴──────┐   ┌──────┴─────────┐  │
│  │ PoiRepo     │   │ In-memory      │  │
│  │ (trait)     │   │ cache (moka)   │  │
│  └─────────────┘   └────────────────┘  │
│                                         │
└─────────────────┬───────────────────────┘
                  │ HTTPS
                  ▼
   ┌──────────────────────────────────┐
   │  Thin Server (fly.io / CF Worker)│
   │                                  │
   │  - Mapbox directions proxy       │
   │  - API key / rate limiting       │
   │  - Opt-in telemetry sink         │
   │  - Region DB downloads           │
   └──────────────────────────────────┘
```

### What stays on device
- POI database (SQLite with R-tree spatial index)
- Route generation engine (waypoint selection, scoring, metrics, geometry)
- Route/POI caching (in-memory LRU with TTL)

### What stays on the server
- Mapbox API key (cannot ship in client binary)
- Rate limiting / auth (monetization chokepoint)
- Anonymized telemetry collection
- Region database hosting (download server)

---

## Current External Dependencies

| Component | Current | On-Device Replacement |
|-----------|---------|----------------------|
| PostgreSQL + PostGIS | `PgPool` via `sqlx`, spatial queries | SQLite + R-tree via `sqlx` (sqlite feature) |
| Redis | `ConnectionManager`, GET/SET with TTL | `moka` in-memory cache |
| Mapbox API | Direct HTTPS calls from server | Proxied through thin server |
| OSM import | `osm2pgsql` → PostgreSQL | `osmpbf` crate → SQLite build tool |

### PostGIS Functions Actually Used (Hot Path)

Only two spatial query patterns are used in route generation:

1. **`find_pois_within_radius`** — `ST_DWithin` + `ST_Distance` (radius search)
2. **`find_pois_in_bbox`** — `ST_Y`/`ST_X` BETWEEN (bounding box search)

Both are trivially replaceable with R-tree + haversine in Rust.

The complex PostGIS usage (`ST_ClusterDBSCAN`, `ST_ConcaveHull`, `ST_Buffer`, `ST_Union`, `ST_AsGeoJSON`) is only in `get_poi_coverage()`, which powers the debug/health endpoint — not the route generation path. This can be dropped or reimplemented in Rust using the existing `convex_hull` in `geometry.rs`.

### Files That DON'T Change

The core route engine is untouched by this migration:

- `src/services/route_generator/waypoint_selection.rs`
- `src/services/route_generator/tolerance_strategy.rs`
- `src/services/route_generator/scoring_strategy.rs`
- `src/services/route_generator/route_scoring.rs`
- `src/services/route_generator/route_metrics.rs`
- `src/services/route_generator/geometry.rs`
- `src/services/route_generator/geometric_loop.rs`
- `src/models/` (all model types)
- `src/config.rs`
- `src/constants.rs`

---

## Phase 1: Abstract the Data Layer ✅ DONE

**Goal:** Decouple business logic from PostgreSQL so backends are swappable.

**Status:** Completed. All POI queries in the route generation hot path now go through `Arc<dyn PoiRepository>`. The existing PostgreSQL stack continues to work unchanged via `PgPoiRepository`.

**New file:** `src/db/poi_repository.rs` (~90 lines)

```rust
use async_trait::async_trait;
use crate::models::{Coordinates, Poi, PoiCategory};
use crate::error::Result;

#[async_trait]
pub trait PoiRepository: Send + Sync {
    async fn find_within_radius(
        &self, center: &Coordinates, radius_meters: f64,
        categories: Option<&[PoiCategory]>, limit: i64,
    ) -> Result<Vec<Poi>>;

    async fn find_in_bbox(
        &self, min_lat: f64, max_lat: f64, min_lng: f64, max_lng: f64,
        categories: Option<&[PoiCategory]>, limit: i64,
    ) -> Result<Vec<Poi>>;

    async fn insert(&self, poi: &Poi) -> Result<uuid::Uuid>;

    async fn count(&self) -> Result<i64>;
}
```

`PgPoiRepository` wraps `sqlx::PgPool` and delegates to the existing free functions in `poi_queries.rs` (which remain unchanged). It also exposes a `pool()` accessor for PostgreSQL-specific callers (debug coverage endpoint).

**Files created:**

| File | Description |
|------|-------------|
| `src/db/poi_repository.rs` | `PoiRepository` trait + `PgPoiRepository` implementation |

**Files changed:**

| File | Change |
|------|--------|
| `Cargo.toml` | Added `async-trait = "0.1"` |
| `src/db/mod.rs` | Added `pub mod poi_repository;`, re-exports `PoiRepository` and `PgPoiRepository` |
| `src/lib.rs` | `AppState` gained `poi_repo: Arc<dyn db::PoiRepository>` field |
| `src/services/poi_service.rs` | `PgPool` → `Arc<dyn PoiRepository>`, calls `self.repo.find_within_radius()` |
| `src/services/snapping_service.rs` | `PgPool` → `Arc<dyn PoiRepository>`, calls `self.repo.find_in_bbox()` |
| `src/routes/pois.rs` | Uses `state.poi_repo.find_within_radius()` instead of `queries::` |
| `src/routes/debug.rs` | POI count check uses `state.poi_repo.count()` instead of raw SQL |
| `src/main.rs` | Creates `PgPoiRepository`, passes `Arc` to services and `AppState` |
| `src/bin/evaluate.rs` | Same wiring as `main.rs` |

**Files NOT changed (as planned):**

- `src/db/poi_queries.rs` — free functions stay as-is, `PgPoiRepository` delegates to them
- `src/db/evaluation_queries.rs` — stays PostgreSQL-only (server development tool)
- `src/routes/evaluation.rs` — keeps using `state.db_pool` directly
- All route generator sub-modules — internal change is transparent
- `src/models/*`, `src/config.rs`, `src/error.rs`, `src/cache/mod.rs`

**Notes for Phase 2:** The trait uses `limit: i64` (matching sqlx's bind types) rather than `usize`. The `SqlitePoiRepository` will implement the same trait. The `count()` method was added to support the health check endpoint without raw SQL.

---

## Phase 2: SQLite + R-tree Backend ✅ DONE

**Goal:** Implement `PoiRepository` backed by SQLite with R-tree spatial index.

**Status:** Completed. `SqlitePoiRepository` implements the `PoiRepository` trait using SQLite R-tree for spatial queries and Haversine post-filtering in Rust. Gated behind the `sqlite` Cargo feature — zero breaking changes to the existing PostgreSQL stack.

### Feature Gating

The `sqlite` feature is purely additive. PostgreSQL remains always-on (no feature gate):

```toml
# Cargo.toml
[features]
default = []
sqlite = ["sqlx/sqlite"]
```

```rust
// src/db/mod.rs
pub mod poi_repository;         // PoiRepository trait (always available)
mod poi_queries;                // existing PgPool free functions (always available)
#[cfg(feature = "sqlite")]
pub mod sqlite_repo;            // SqlitePoiRepository
```

### SQLite Schema

Created programmatically by `SqlitePoiRepository::create_schema()` (idempotent — safe to call multiple times):

```sql
CREATE TABLE IF NOT EXISTS pois (
    rowid INTEGER PRIMARY KEY,
    id TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    lat REAL NOT NULL, lng REAL NOT NULL,
    popularity_score REAL NOT NULL DEFAULT 0.0,
    description TEXT,
    estimated_visit_duration_minutes INTEGER,
    osm_id INTEGER UNIQUE
);
CREATE VIRTUAL TABLE pois_rtree USING rtree(
    id, min_lat, max_lat, min_lng, max_lng
);
CREATE TABLE IF NOT EXISTS region_meta (key TEXT PRIMARY KEY, value TEXT);
CREATE INDEX IF NOT EXISTS idx_pois_category ON pois(category);
```

### Implementation Details

**New file:** `src/db/sqlite_repo.rs` (~310 lines + ~180 lines tests)

Key design decisions:
- **`SqlitePoiRow`** — SQLite-specific row struct (`id` is `String` not `Uuid`, `popularity_score` is `f64` not `f32`)
- **`From<SqlitePoiRow> for Poi`** — defensive conversions matching `poi_queries.rs` patterns (UUID parse fallback, category fallback to `Historic`, coordinate validation)
- **Category filtering in Rust** — SQLite has no `= ANY(array)`. R-tree pre-filter returns candidates, then `HashSet` lookup filters by category. Clean and O(1) per candidate.
- **No SQL LIMIT before Rust filtering** — LIMIT in SQL before category filtering could return fewer results than requested. `.take(limit)` applied after filtering instead.
- **Haversine post-filter** — R-tree bbox is a superset of the true radius circle. `Coordinates::distance_to() * 1000.0 <= radius_meters` rejects bbox-corner false positives.
- **Insert atomicity** — Transaction wraps both `pois` INSERT and `pois_rtree` INSERT. R-tree `id` = `pois.rowid` via `last_insert_rowid()`.
- **R-tree 32-bit precision** — SQLite R-tree stores coordinates as 32-bit floats internally. Tests use relaxed tolerance (`1e-4`) for R-tree value assertions.

### Files Created

| File | Description |
|------|-------------|
| `src/db/sqlite_repo.rs` | `SqlitePoiRepository` struct, `SqlitePoiRow`, `From` impl, `create_schema()`, `PoiRepository` trait impl, 11 tests |

### Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `[features]` section with `sqlite = ["sqlx/sqlite"]` |
| `src/db/mod.rs` | Added `#[cfg(feature = "sqlite")] pub mod sqlite_repo;` + conditional re-export of `SqlitePoiRepository` |

### Files NOT Changed

- `src/db/poi_repository.rs` — trait unchanged
- `src/db/poi_queries.rs` — PostgreSQL queries unchanged
- All route generator sub-modules, models, services, routes, main.rs, evaluate.rs

### Tests (11 tests, in-memory SQLite)

All tests use `sqlite::memory:` — no external database needed.

| Test | Verifies |
|------|----------|
| `create_schema_idempotent` | Calling `create_schema` twice doesn't error |
| `insert_and_count` | Insert 3 POIs, `count()` returns 3 |
| `find_within_radius_basic` | POIs inside radius returned, outside excluded |
| `find_within_radius_category_filter` | Only matching categories returned |
| `find_within_radius_distance_ordering` | Results sorted by distance ascending |
| `find_within_radius_limit` | `.take(limit)` respected |
| `find_within_radius_haversine_rejects_bbox_corners` | POI in bbox corner but outside haversine circle excluded |
| `find_in_bbox_basic` | Correct inclusion/exclusion |
| `find_in_bbox_category_filter` | Category filtering works |
| `insert_rtree_sync` | R-tree entry exists with correct lat/lng after insert |
| `insert_null_optionals` | POI with `None` fields round-trips correctly |

### Verification

```bash
cargo check --features sqlite                        # Compiles with SQLite
cargo clippy --features sqlite -- -D warnings        # No warnings
cargo test --features sqlite --lib                   # 143 tests pass (132 existing + 11 SQLite)
cargo test --lib                                     # 132 existing tests pass (sqlite code not compiled)
SKIP_REAL_API_TESTS=true cargo test --features sqlite # If no DB available
```

### Performance Expectations

| Dataset | POI count | R-tree pre-filter | Haversine post-filter | Total |
|---------|-----------|-------------------|-----------------------|-------|
| Monaco | ~2k | <1ms | <1ms | <1ms |
| Paris | ~30k | ~1ms | ~1ms | ~2ms |
| France | ~200k | ~2ms | ~1ms | ~3ms |

For comparison, PostGIS `ST_DWithin` on the same data: ~10-100ms. SQLite R-tree is faster for these data sizes because everything is in-process with no network hop.

**Notes for Phase 4:** `SqlitePoiRepository::create_schema()` and `insert()` are the building blocks for the OSM-to-SQLite pipeline. The `build_region` binary will create a pool, call `create_schema()`, then stream OSM PBF through `insert()` calls.

---

## Phase 3: In-Memory Cache ✅ DONE

**Goal:** Introduce a `RouteCache` trait with two backends: Redis and moka-based in-memory cache. On-device mode gets caching without Redis; server mode falls back to in-memory when Redis fails.

**Status:** Completed. `RouteCache` trait defined with `&self` methods (no `RwLock` needed). `RedisCacheService` extracted from old `CacheService`, adapted to `&self` via `ConnectionManager::clone()`. `MemoryCacheService` backed by `moka::future::Cache` with `Arc<Vec<Route>>` storage and `AtomicU64` hit/miss counters. `AppState.cache` changed from `Option<Arc<RwLock<CacheService>>>` to `Option<Arc<dyn RouteCache>>`. Dead POI cache methods dropped.

**New files:**

| File | Description |
|------|-------------|
| `src/cache/memory.rs` | `MemoryCacheService` struct, `RouteCache` impl, 6 tests |
| `src/cache/redis.rs` | `RedisCacheService` extracted from old `CacheService`, `RouteCache` impl |

**Files changed:**

| File | Change |
|------|--------|
| `Cargo.toml` | Added `moka = { version = "0.12", features = ["future"] }` |
| `src/constants.rs` | Added `DEFAULT_MEMORY_CACHE_MAX_ENTRIES = 1_000` |
| `src/cache/mod.rs` | `RouteCache` trait, free functions (`loop_route_cache_key`, `poi_region_cache_key`), module declarations, re-exports, updated tests |
| `src/lib.rs` | `AppState.cache` → `Option<Arc<dyn RouteCache>>`, removed `RwLock` import, updated re-exports |
| `src/main.rs` | Cache init tries Redis, falls back to `MemoryCacheService` (always `Some(...)` in production) |
| `src/routes/loop_route.rs` | Calls `cache::loop_route_cache_key()` free function, removed `RwLock` guard pattern |
| `src/routes/debug.rs` | Renamed JSON key `"redis"` → `"cache"`, added `"backend"` sub-field, removed `RwLock` guard pattern |

**Files NOT changed:**

- `src/db/*`, `src/services/*`, `src/models/*`, `src/config.rs`, `src/error.rs`
- `src/bin/evaluate.rs` (doesn't use cache)
- `tests/*` (uses `cache: None`, still compiles)
- All route generator sub-modules

**Key design decisions:**

- **`&self` everywhere, no `RwLock`:** Redis `ConnectionManager` is `Arc`-based — `.clone()` per call is a cheap atomic op. Moka's `Cache` is already `&self`. Eliminated `RwLock` from entire cache path.
- **`Arc<Vec<Route>>` in moka:** Avoids deep-cloning `Vec<Route>` on every cache hit. Inner `Vec` only cloned when crossing trait boundary.
- **In-memory fallback:** Previously no Redis = no cache. Now no Redis = in-memory cache. `Option<Arc<dyn RouteCache>>` stays `Option` for tests only.
- **Dead code removed:** `get_cached_pois()`, `cache_pois()`, `poi_region_cache_ttl` param — never called outside `mod.rs`.

**Verification:**

```bash
cargo check                                    # Compiles
cargo check --features sqlite                  # Compiles with sqlite
cargo clippy -- -D warnings                    # No warnings
cargo clippy --features sqlite -- -D warnings  # No warnings with sqlite
SKIP_REAL_API_TESTS=true cargo test            # 158 tests pass
SKIP_REAL_API_TESTS=true cargo test --features sqlite --lib  # 149 tests pass
```

---

## Phase 4: OSM to SQLite Build Pipeline ✅ DONE

**Goal:** Replace `osm2pgsql` → PostgreSQL with a pure-Rust tool that builds region `.db` files directly from OSM PBF data.

**Status:** Completed. `build_region` binary reads OSM PBF files via the `osmpbf` crate and writes SQLite region databases using `SqlitePoiRepository`. OSM tag mapping logic ported from `osm/osm_poi_style.lua` into `src/osm/mod.rs`. Everything gated behind the `sqlite` Cargo feature.

### Usage

```bash
# All-in-one: download PBF + build SQLite DB
just add-region monaco
just add-region europe/france/ile-de-france

# Or step by step:
just build-region osm/data/monaco-latest.osm.pbf regions/monaco.db

# Or directly with cargo:
cargo run --features sqlite --bin build_region -- \
    --input osm/data/monaco-latest.osm.pbf \
    --output regions/monaco.db

# Inspect output:
sqlite3 regions/monaco.db "SELECT COUNT(*) FROM pois"
sqlite3 regions/monaco.db "SELECT category, COUNT(*) FROM pois GROUP BY category ORDER BY COUNT(*) DESC"
sqlite3 regions/monaco.db "SELECT * FROM region_meta"
```

### Processing Strategy — Single Pass

PBF files from Geofabrik are sorted (nodes first, then ways). In a single `reader.for_each()`:

1. **Nodes/DenseNodes**: Store `(id, lat, lon)` in `HashMap`. If the node has a `name` tag + POI-relevant tags, produce a `Poi` immediately.
2. **Ways**: If has `name` + POI tags + is closed (first ref == last ref), store as `PendingWay` with node refs.
3. **Post-pass**: Resolve each `PendingWay` by looking up node coordinates from the HashMap, compute centroid (average of coordinates), produce `Poi`.

SQLite writes use batched inserts (1000 POIs per transaction) with WAL mode + `synchronous=NORMAL` + 64MB cache for bulk loading speed. Progress logged with phase numbering, element counts, and elapsed time.

All node coordinates are held in a `HashMap` during processing. This is fine for target regions (Monaco ~1MB, Ile-de-France ~280MB) but would need a two-pass optimization for continent-scale files.

### OSM Tag Mapping (src/osm/mod.rs)

Ported from `osm/osm_poi_style.lua` — four functions:

- **`determine_category(tags) -> Option<PoiCategory>`** — Tag priority: tourism > historic > amenity > leisure > natural > man_made > craft > building > place. Handles `place_of_worship` special case (only `religion=christian` maps to Church, all others to Cultural).
- **`calculate_popularity(tags) -> f32`** — Base 50, boosted by wikipedia/wikidata (+20), UNESCO (+15), tourist tag (+10), language names (+5/+10), website (+5), opening_hours (+5), stars (*2), historic importance (+10). Penalty -10 for sparse tags without wikipedia. Clamped 0–100.
- **`estimate_duration(tags, category) -> u32`** — Explicit `duration` tag if present, else category defaults (museum=90, castle=120, monument=30, etc., fallback=30).
- **`build_description(tags) -> Option<String>`** — Joins: description, UNESCO note, architect, artist, elevation.

Helper: `collect_tags(iter) -> HashMap<&str, &str>` to convert osmpbf tag iterators.

### Bulk Insert Methods (src/db/sqlite_repo.rs)

Two new methods on `SqlitePoiRepository`:

- **`insert_batch(&self, pois: &[Poi]) -> Result<usize>`** — Wraps all inserts in a single transaction. Uses `INSERT OR IGNORE` to skip duplicate `osm_id`s. Inserts into both `pois` and `pois_rtree` tables. Returns count of inserted POIs.
- **`set_meta(&self, key: &str, value: &str) -> Result<()>`** — `INSERT OR REPLACE` into `region_meta` table.

### Region Metadata

Written to `region_meta` table after import:

| Key | Example Value |
|-----|---------------|
| `region_name` | `ile-de-france` |
| `build_date` | `2026-02-14T12:00:00Z` |
| `poi_count` | `16371` |
| `source_file` | `osm/data/ile-de-france-latest.osm.pbf` |
| `builder_version` | `0.1.0` |
| `source_file_size_bytes` | `294567890` |

### Region File Sizes (Actual)

| Region | POIs | .db size |
|--------|------|----------|
| Monaco | ~500 | ~100KB |
| Paris (Ile-de-France) | ~16k | ~2MB |

### Files Created

| File | Description |
|------|-------------|
| `src/osm/mod.rs` | OSM tag mapping: `determine_category`, `calculate_popularity`, `estimate_duration`, `build_description`, `collect_tags` + 30 unit tests |
| `src/bin/build_region.rs` | CLI binary: PBF streaming reader → SQLite writer with progress indicators |

### Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `osmpbf = { version = "0.3", optional = true }`, updated `sqlite` feature to include `osmpbf`, added `build_region` binary entry |
| `src/lib.rs` | Added `#[cfg(feature = "sqlite")] pub mod osm;` |
| `src/db/sqlite_repo.rs` | Added `insert_batch()` and `set_meta()` methods + 3 tests |
| `justfile` | Added `build-region` and `add-region` commands |

### Files NOT Changed

- `src/osm/osm_poi_style.lua` — Lua script preserved for PostgreSQL import path
- `src/db/poi_repository.rs` — trait unchanged
- All route generator sub-modules, models, services, routes, main.rs, evaluate.rs

### Verification

```bash
cargo check --features sqlite                        # Compiles
cargo clippy --features sqlite -- -D warnings        # No warnings
SKIP_REAL_API_TESTS=true cargo test --features sqlite --lib  # 181 tests pass
cargo test --lib                                     # 138 existing tests pass (sqlite not compiled)

# Build and inspect a region:
just add-region monaco
sqlite3 regions/monaco.db "SELECT COUNT(*) FROM pois"
sqlite3 regions/monaco.db "SELECT category, COUNT(*) FROM pois GROUP BY category ORDER BY COUNT(*) DESC"
```

---

## Phase 5: Mapbox Proxy Server ✅ DONE

**Goal:** Thin stateless proxy that holds the Mapbox API key, provides rate limiting per client key, and collects opt-in telemetry. Keeps the Mapbox key server-side so it never ships in the client binary.

**Status:** Completed. Two-part change: (A) `MapboxClient` gained a configurable base URL and auth mode so the on-device client can target either Mapbox directly or the proxy, and (B) a standalone `proxy` binary in the same repo implements auth, rate limiting, and pass-through forwarding.

### Why a proxy?

1. **Security:** Cannot embed Mapbox API key in distributed binary (would be extracted immediately)
2. **Monetization:** Rate limiting is the payment chokepoint (free: 10 routes/day, paid: unlimited)
3. **Telemetry:** Anonymized usage data without tracking exact user locations
4. **Cost control:** Single Mapbox API key with server-side budget enforcement

### Client-Side Change — Configurable `MapboxClient`

Added `AuthMode` enum and `base_url` field to `MapboxClient`:

```rust
pub enum AuthMode {
    DirectToken,   // current: access_token query param (direct Mapbox)
    BearerHeader,  // proxy: Authorization: Bearer header
}

pub struct MapboxClient {
    client: Client,
    api_key: String,
    base_url: String,       // NEW (was const)
    auth_mode: AuthMode,    // NEW
}
```

- `MapboxClient::new(api_key)` — unchanged, defaults to direct Mapbox mode (backwards compatible)
- `MapboxClient::with_config(api_key, base_url, auth_mode)` — new constructor for proxy mode
- `get_directions()` — uses `self.base_url` instead of const, conditional auth (query param vs Bearer header)
- `Config` gained `mapbox_base_url: Option<String>` — when set, `main.rs` and `evaluate.rs` create the client in `BearerHeader` mode; when absent, direct Mapbox (zero-impact default)

### Proxy Binary — `src/bin/proxy.rs` (~230 lines)

Standalone Axum binary that does **not** import the `easyroute` library. Uses only existing crate dependencies (axum, reqwest, tokio, serde_json, tracing, tower-http, dotenv).

**Config** (from env vars):
- `MAPBOX_API_KEY` — real Mapbox key (proxy holds this)
- `PROXY_API_KEYS` — comma-separated valid client keys
- `PROXY_RATE_LIMIT` — requests/min per key (default: 20)
- `PROXY_PORT` — port (default: 4000)

**Endpoints:**

```
GET /health
  → { "status": "ok", "keys_configured": 2, "rate_limit": 20 }

GET /v1/directions/{profile}/{coordinates}?geometries=geojson&overview=full&steps=false
  Headers: Authorization: Bearer <client-key>
  → 1. Validate Bearer token against PROXY_API_KEYS
  → 2. Rate limit check (sliding window, 60s)
  → 3. Forward to Mapbox with real access_token
  → 4. Stream raw Mapbox response back (pass-through, no parsing)

POST /v1/telemetry
  Headers: Authorization: Bearer <client-key>
  Body: { ... }
  → Auth check, log payload to tracing (no DB)
```

**Rate limiter:** `HashMap<String, Vec<Instant>>` behind `tokio::sync::Mutex`. Sliding window (60s). Lazily evicts expired entries on each check.

**Request flow:**
```
Client:  GET /v1/directions/walking/2.35,48.86;2.29,48.86?geometries=geojson&overview=full&steps=false
         Authorization: Bearer <client-key>
  ↓
Proxy:   1. Extract Bearer token
         2. Validate against PROXY_API_KEYS
         3. Rate limit check
         4. Forward to: https://api.mapbox.com/directions/v5/mapbox/walking/...?...&access_token=<MAPBOX_KEY>
         5. Stream Mapbox response back (pass-through, no parsing)
```

### Files Created

| File | Description |
|------|-------------|
| `src/bin/proxy.rs` | Standalone Axum proxy binary: config, rate limiter, auth, directions forwarding, telemetry endpoint + 11 unit tests |

### Files Changed

| File | Change |
|------|--------|
| `src/services/mapbox.rs` | Added `AuthMode` enum, `base_url`/`auth_mode` fields, `with_config()` constructor, conditional auth in `get_directions()` + 2 tests |
| `src/config.rs` | Added `mapbox_base_url: Option<String>` to `Config`, parsed from `MAPBOX_BASE_URL` env var, updated test literal |
| `src/main.rs` | Conditional `MapboxClient` creation: `BearerHeader` when `mapbox_base_url` set, `DirectToken` otherwise |
| `src/bin/evaluate.rs` | Same conditional `MapboxClient` wiring as `main.rs` |
| `tests/common/mod.rs` | Added `mapbox_base_url: None` to `get_test_config()` |
| `Cargo.toml` | Added `[[bin]] name = "proxy"` entry |
| `justfile` | Added `proxy` (start server) and `proxy-health` (curl health) commands |

### Files NOT Changed

- `src/db/*` — data layer unchanged
- All route generator sub-modules — transparent change
- `src/models/*`, `src/constants.rs`, `src/error.rs`, `src/cache/*`

### Tests (13 new tests)

**Proxy binary (11 tests):**

| Test | Verifies |
|------|----------|
| `config_valid` | Parses all env vars correctly |
| `config_missing_mapbox_key` | Errors when `MAPBOX_API_KEY` missing |
| `config_missing_api_keys` | Errors when `PROXY_API_KEYS` missing |
| `rate_limiter_within_limit` | Requests within limit allowed |
| `rate_limiter_over_limit` | Requests over limit rejected |
| `rate_limiter_independent_keys` | Different keys have separate budgets |
| `bearer_token_valid` | Extracts token from `Authorization: Bearer` header |
| `bearer_token_missing` | Returns `None` for missing header |
| `bearer_token_wrong_scheme` | Returns `None` for `Basic` scheme |
| `api_key_valid` | Recognizes configured keys |
| `api_key_invalid` | Rejects unknown keys |

**MapboxClient (2 tests):**

| Test | Verifies |
|------|----------|
| `test_new_defaults_to_direct_token` | `new()` uses Mapbox base URL + `DirectToken` |
| `test_with_config_bearer_mode` | `with_config()` stores custom URL + `BearerHeader` |

### Key Design Decisions

- **Proxy in same repo** — reuses existing Cargo.toml deps, no separate project. Does not import the `easyroute` library (self-contained).
- **Pass-through proxy** — forwards raw Mapbox response bytes without parsing. Format-agnostic.
- **`MapboxClient::new()` unchanged** — zero impact on callers that don't set `MAPBOX_BASE_URL`.
- **No new dependencies** — everything already in Cargo.toml (axum, reqwest, tokio, serde_json, tracing, tower-http, dotenv, serial_test).

### Verification

```bash
cargo check                            # Existing code compiles
cargo check --bin proxy                # Proxy compiles
cargo clippy -- -D warnings            # No warnings
SKIP_REAL_API_TESTS=true cargo test    # 171 tests pass (140 lib + 11 proxy + 20 integration)

# Manual test:
MAPBOX_API_KEY=<key> PROXY_API_KEYS=test123 cargo run --bin proxy
curl -H "Authorization: Bearer test123" "http://localhost:4000/health"
curl -H "Authorization: Bearer test123" \
  "http://localhost:4000/v1/directions/walking/2.3522,48.8566;2.2945,48.8584?geometries=geojson&overview=full&steps=false"
```

**Deployment:** fly.io free tier (256MB RAM, shared CPU) is more than sufficient for a stateless proxy.

### Notes for Phase 6

The on-device client needs `MAPBOX_BASE_URL` pointed at the deployed proxy and its own client key in `MAPBOX_API_KEY`. Region download endpoints (`GET /v1/regions`, `GET /v1/regions/{name}/download`) are deferred — they'll be added to the proxy when region hosting is set up.

---

## Phase 6: On-Device Binary + Web UI ✅ DONE

**Goal:** Create a standalone `ondevice` binary that wires together the SQLite backend, in-memory cache, and Mapbox client, serves a web UI, and requires no PostgreSQL, Redis, or Docker. Also cleanly separate PostgreSQL-only routes from backend-agnostic core routes.

**Status:** Completed. `AppState` no longer holds `PgPool` — routes are split into a core router (works with any `PoiRepository` backend) and a PostgreSQL router (evaluation + PostGIS coverage). The `ondevice` binary serves the core API + static web UI from a single process. Web UI extracted from `scripts/visualize.html` into `app/` directory with geolocation support and relative API URLs.

### Key Design: `db_pool` Removed from `AppState`

The `PoiRepository` trait already abstracts the database. The `db_pool: PgPool` field on `AppState` only existed for PostgreSQL-specific endpoints (evaluation routes + PostGIS coverage). Instead of making it `Option<PgPool>`, we removed it entirely and split routes into two groups:

- **Core router** (`create_router`) — uses `Arc<AppState>` with `poi_repo`, `route_generator`, `cache`. Works with any backend.
- **PostgreSQL router** (`create_pg_router`) — uses `PgPool` directly as Axum state. Evaluation + coverage routes. Server-only.

The on-device binary only registers the core router. The server binary merges both. Clean separation, no optionals, no dead code paths.

### Usage

```bash
# Build and run on-device server
just ondevice --region=regions/monaco.db --open

# Or directly:
cargo run --features sqlite --bin ondevice -- --region=regions/monaco.db --port=3000 --open

# Build release binary
just build-ondevice

# Health check
curl http://localhost:3000/api/v1/debug/health
```

### `ondevice` Binary — `src/bin/ondevice.rs` (~160 lines)

**Config** (env vars + CLI args):
- `--region=PATH` (required) — SQLite region DB path
- `--port=PORT` (default: 3000)
- `--open` — open browser after starting
- `MAPBOX_API_KEY` — Mapbox access token (required)
- `MAPBOX_BASE_URL` — proxy URL (optional — uses direct Mapbox if unset)

**Flow:**
```
1. Parse CLI args (--region, --port, --open)
2. dotenv::dotenv().ok()
3. Open SQLite pool with read-perf pragmas (WAL, mmap 256MB, cache 16MB)
4. SqlitePoiRepository::create_schema() (idempotent)
5. Log region metadata (name, POI count)
6. MemoryCacheService::new(86400, 1000)
7. MapboxClient — with_config() if MAPBOX_BASE_URL set, else new()
8. RouteGenerator::new(...)
9. AppState { poi_repo, route_generator, cache }   ← no db_pool!
10. Router: /api/v1 → create_router(state), fallback → ServeDir("app/")
11. Bind 127.0.0.1:PORT (localhost only)
12. Optionally open browser
13. Serve
```

**Static serving:** `tower_http::services::ServeDir::new("app")` as `fallback_service` — API routes take priority, everything else serves from `app/`.

### Web UI — `app/` directory

Split from the existing 886-line `scripts/visualize.html` into three files:

| File | Lines | Description |
|------|-------|-------------|
| `app/index.html` | ~90 | HTML shell — loads CSS + JS, Mapbox GL, map container, form panel |
| `app/style.css` | ~170 | Extracted `<style>` block — all UI styling |
| `app/main.js` | ~400 | Extracted `<script>` block — map logic, route generation, exports |

Key changes from `visualize.html`:
- **API base URL** defaults to `''` (relative to current origin — works on localhost)
- **"Use my location" button** — `navigator.geolocation.getCurrentPosition`, pans map to user location
- **No API endpoint input** — removed (unnecessary when served from same origin)
- **No coverage button** — `data_coverage` is PostgreSQL-only, not available in on-device mode
- **No route type selector** — simplified to loop routes only (point-to-point not yet implemented)
- `scripts/visualize.html` kept untouched (dev tool for server mode with configurable endpoint)

### Files Created

| File | Description |
|------|-------------|
| `src/bin/ondevice.rs` | On-device binary: CLI args, SQLite, memory cache, Mapbox client, static file serving |
| `app/index.html` | Web UI HTML shell |
| `app/style.css` | Web UI styles |
| `app/main.js` | Web UI JavaScript (map, route generation, exports, geolocation) |

### Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `"fs"` to tower-http features (for `ServeDir`), added `ondevice` binary entry with `required-features = ["sqlite"]` |
| `src/lib.rs` | Removed `db_pool: PgPool` field and `use sqlx::PgPool` from `AppState` |
| `src/routes/mod.rs` | Split into `create_router()` (core: loop, pois, health) and `create_pg_router(pool: PgPool)` (evaluation + coverage) |
| `src/routes/debug.rs` | `health_check` uses `poi_repo.count()` instead of raw SQL (works with any backend); `data_coverage` takes `State<PgPool>` directly |
| `src/routes/evaluation.rs` | All 4 handlers: `State(state): State<Arc<AppState>>` → `State(pool): State<PgPool>` |
| `src/main.rs` | Merges both routers under `/api/v1` via `.merge()`, `db_pool` kept as local variable for migrations + pg router |
| `tests/api_tests.rs` | Removed `db_pool` from `AppState` construction |
| `justfile` | Added `ondevice` and `build-ondevice` commands |
| `.github/workflows/ci.yml` | Added `Build ondevice binary` step in `test-sqlite` job |

### Files NOT Changed

- `src/bin/evaluate.rs` — doesn't construct `AppState`, uses `route_generator` directly
- `src/bin/proxy.rs` — standalone, doesn't import `AppState`
- `src/db/*` — data layer unchanged
- All route generator sub-modules — transparent change
- `src/models/*`, `src/config.rs`, `src/constants.rs`, `src/error.rs`, `src/cache/*`
- `scripts/visualize.html` — kept as-is for server development

### Verification

```bash
# Existing code still works
cargo check                                    # Compiles
cargo clippy -- -D warnings                   # No warnings
cargo clippy --all-features -- -D warnings    # No warnings with all features
SKIP_REAL_API_TESTS=true cargo test --lib     # 140 tests pass

# On-device binary compiles and runs
cargo build --features sqlite --bin ondevice
cargo run --features sqlite --bin ondevice -- --help

# End-to-end manual test (requires a region DB + MAPBOX_API_KEY)
cargo run --features sqlite --bin ondevice -- --region=regions/monaco.db --open
# → Browser opens http://localhost:3000
# → Health check: curl http://localhost:3000/api/v1/debug/health
# → Generate a route via the web UI
```

### Future Options

This binary becomes the foundation for:
- **Tauri desktop app** — embed Axum as a background thread, use Tauri IPC or localhost
- **Mobile (iOS/Android)** — Rust FFI via `cargo-ndk` (Android) or static lib (iOS), thin native UI wrapper
- **Hosted web app** — deploy `ondevice` binary with a pre-built region DB behind a reverse proxy

---

## Migration Order and Dependencies

```
Phase 1: PoiRepository trait  ✅ DONE
    │
    ├──→ Phase 2: SQLite backend  ✅ DONE
    │        │
    │        └──→ Phase 4: OSM→SQLite pipeline  ✅ DONE
    │
    └──→ Phase 3: In-memory cache  ✅ DONE
              │
              └──→ Phase 5: Mapbox proxy  ✅ DONE
                       │
                       └──→ Phase 6: On-device binary + Web UI  ✅ DONE
```

All phases complete. The full on-device stack is functional: OSM PBF → SQLite region DB → `ondevice` binary (PoiRepository + in-memory cache + Mapbox client) → web UI. The existing PostgreSQL/Redis server stack continues to work unchanged.

---

## Effort Summary

| Phase | Description | Estimated Effort | Risk |
|-------|-------------|-----------------|------|
| 1 | `PoiRepository` trait abstraction | ~~0.5 day~~ **Done** | Low — mechanical refactor |
| 2 | SQLite + R-tree implementation | ~~1 day~~ **Done** | Low — well-understood pattern |
| 3 | In-memory cache (moka) | ~~0.5 day~~ **Done** | Low — cache is already optional |
| 4 | OSM → SQLite build pipeline | ~~1 day~~ **Done** | Low — faithful Lua port + tests |
| 5 | Mapbox proxy server | ~~1-1.5 days~~ **Done** | Low — simple stateless proxy |
| 6 | On-device binary + Web UI | ~~1-3 days~~ **Done** | Low — Option B (CLI + browser) |
| **Total** | | **All done** | |

---

## Monetization Model

The thin proxy server enables a simple freemium model:

| Tier | Routes/day | Regions | Price |
|------|-----------|---------|-------|
| Free | 10 | 2 bundled | $0 |
| Pro | Unlimited | All | $5/month |
| Region Pack | — | Additional region | $1 one-time |

The proxy is the meter. All payment logic is server-side. The client receives a 429 when the quota is exhausted.

---

## Telemetry (Opt-In)

After each route generation, the client can optionally send anonymized metrics:

```json
{
    "region": "monaco",
    "distance_bucket_km": 5.0,
    "mode": "walking",
    "route_count": 3,
    "avg_score": 7.2,
    "metrics": {
        "circularity": 0.82,
        "poi_density": 0.65,
        "category_entropy": 0.71
    },
    "fallback_used": false,
    "tolerance_level": "normal",
    "client_version": "0.1.0"
}
```

No exact coordinates, no user identity, no tracking. Enough to understand which regions work well, which tolerance levels are needed, and how scoring performs in the wild.

---

## What's Preserved

- **All route generation logic** — zero changes to the core engine
- **Server deployment** — PostgreSQL/Redis stack continues to work via feature flags
- **Evaluation harness** — stays PostgreSQL-only as a development tool
- **API contract** — same request/response format, same endpoints
- **Test suite** — existing tests pass unchanged (they don't test DB layer directly)

## What's Dropped (On-Device Only)

- `get_poi_coverage()` — complex PostGIS clustering, only used in debug endpoint
- Redis `INFO` stats — replaced by in-memory cache stats
- Database connection pooling — SQLite is single-file, no pool needed
- `osm2pgsql` dependency — replaced by Rust build tool
