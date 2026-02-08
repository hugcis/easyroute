# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust REST API that generates personalized walking/cycling loop routes with POI waypoints (monuments, viewpoints, parks, museums). Given a start point and target distance, it discovers nearby POIs, selects waypoints forming a loop, generates turn-by-turn routes via Mapbox, and scores/ranks alternatives.

**Tech Stack**: Rust (stable) | Axum 0.8 | PostgreSQL 18 + PostGIS 3.6 | Redis 8.4 | Mapbox Directions API

## Build & Development Commands

```bash
# Prerequisites: docker-compose up -d postgres redis

# Run server
cargo run

# Fast tests (skips Mapbox/external API calls)
SKIP_REAL_API_TESTS=true cargo test

# Full tests (requires MAPBOX_API_KEY in .env)
cargo test

# Single test
cargo test test_waypoint_selection

# Integration tests only
cargo test --test '*'

# Lint & format
cargo fmt && cargo clippy -- -D warnings

# Quick compile check
cargo check
```

Database tests use a separate `easyroute_test` database (via `TEST_DATABASE_URL`). Tests run serially via `serial_test` crate to avoid conflicts. Test utilities are in `tests/common/mod.rs`.

## Architecture

### Request Flow

```
POST /api/v1/routes/loop
  -> Check Redis route cache (bucketed by ~100m coords, ~0.5km distance)
  -> Query POIs from PostgreSQL/PostGIS within search radius
  -> Select 2-4 waypoints (based on route length) with spatial distribution checks
  -> Generate routes via Mapbox Directions API for waypoint combinations
  -> Adaptive tolerance: normal (±20%) -> relaxed (±30%) -> very relaxed (±50%)
  -> Snap additional nearby POIs to route path (within 100m)
  -> Score and rank alternatives
  -> Cache result in Redis (24h TTL)
```

### Route Generator (Strategy Pattern)

The route generator (`src/services/route_generator/`) is the core component, split into sub-modules:

- `mod.rs` - Orchestrator: POI discovery, route generation loop, caching integration
- `waypoint_selection.rs` - Selects 2-4 POIs as waypoints based on distance/angle from start
- `scoring_strategy.rs` - Two strategies: `Simple` (distance-only) and `Advanced` (quality + clustering + angular diversity)
- `tolerance_strategy.rs` - Adaptive tolerance levels for distance matching
- `geometric_loop.rs` - Fallback: generates geometric circle waypoints when insufficient POIs
- `route_scoring.rs` - Final route scoring (distance accuracy, POI count, quality, diversity)

The scoring strategy is configured via `ROUTE_POI_SCORING_STRATEGY` env var (`simple` or `advanced`). Default is `simple`. All route generator parameters are configurable via env vars with `ROUTE_` prefix (see `src/config.rs` for the full list with defaults).

### Key Services

- **POI Service** (`src/services/poi_service.rs`): PostgreSQL/PostGIS-only POI queries (Overpass API fallback was removed)
- **Mapbox Client** (`src/services/mapbox.rs`): Wraps Mapbox Directions API for walking/cycling profiles
- **Snapping Service** (`src/services/snapping_service.rs`): Finds additional POIs within snap radius of generated route path
- **Cache** (`src/cache/mod.rs`): Redis caching with bucketed keys; graceful degradation if Redis unavailable

### Data Model

- `Coordinates` - validated lat/lng with bounds checking
- `DistanceKm`, `DistanceMeters`, `RadiusMeters` - newtype wrappers (never use raw f64 for distances)
- `Poi` with `PoiCategory` enum (27 categories: Monument, Viewpoint, Park, Museum, etc.)
- `Route` with GeoJSON geometry, distance, duration, POIs, and score (0-10)

## Important Patterns

**Semantic types**: Always use newtype wrappers (`DistanceKm`, `Coordinates`) instead of primitive `f64`. Validation happens at construction time.

**Error handling**: `thiserror`-based `Error` enum in `src/error.rs`. All services return `Result<T, Error>`. No `.unwrap()` in production code.

**PostGIS coordinate order**: PostGIS uses `(longitude, latitude)` order, while the Rust `Coordinates` struct uses `(lat, lng)`. Always swap when building PostGIS queries: `ST_GeogFromText('POINT({lng} {lat})')`.

**Async**: Tokio runtime. All I/O is async.

**File size limit**: Soft limit 500 lines, hard limit 800 lines per file.

## API Endpoints

- `POST /api/v1/routes/loop` - Generate loop routes (main endpoint)
- `GET /api/v1/pois` - Query POIs by location/category
- `GET /api/v1/debug/health` - Health check (DB, PostGIS, Redis, POI count)

## Constraints

- **Mapbox Free Tier**: 100k requests/month. Each route request makes 3-5 Mapbox calls. Caching is essential.
- **Response Time**: Target < 3s for route generation
- **Cache Hit Rate**: Target > 50% (route cache 24h TTL, POI region cache 7d TTL)

## OSM Data Import

POIs come from OpenStreetMap via `osm2pgsql` import (scripts in `osm/`):
```bash
./osm/download_osm.sh monaco          # Small test dataset
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf
```

## Supplementary Documentation

- [CLAUDE-SETUP.md](CLAUDE-SETUP.md) - Environment variables, database ops, Docker services, troubleshooting
- [CLAUDE-ARCHITECTURE.md](CLAUDE-ARCHITECTURE.md) - Detailed component descriptions, data models, request flow
- [CLAUDE-GUIDELINES.md](CLAUDE-GUIDELINES.md) - Code quality guidelines, development patterns
- [CLAUDE-PERFORMANCE.md](CLAUDE-PERFORMANCE.md) - Caching strategy details, PostGIS query patterns, performance targets
