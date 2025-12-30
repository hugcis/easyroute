# CLAUDE.md

This file provides quick reference guidance for Claude Code. **For detailed information, see the linked documentation files below.**

## Project Overview

REST API service in Rust that generates personalized walking/biking routes with POIs (monuments, viewpoints, parks, museums). Creates loop routes from a single point, incorporating interesting waypoints.

**Tech Stack**: Rust (stable) • Axum 0.8 • PostgreSQL 18 + PostGIS 3.6 • Redis 8.4 • Mapbox API

## Quick Start

```bash
# Start services
docker-compose up -d postgres redis

# Run API server
cargo run

# Run tests (skip external APIs for speed)
SKIP_REAL_API_TESTS=true cargo test

# Format & lint
cargo fmt && cargo clippy
```

## Current Status (Phase 3)

- ✅ Loop route generation with adaptive tolerance
- ✅ Redis caching (24h routes, 7d POI regions)
- ✅ PostgreSQL/PostGIS with OSM import system
- ✅ POI service (DB + Overpass fallback)
- ✅ Health check endpoint
- ❌ Point-to-point routes (planned Phase 4)
- ❌ API docs, rate limiting, metrics (TODO)

## API Endpoints

- `POST /api/v1/routes/loop` - Generate loop routes
- `GET /api/v1/pois` - Query POIs by location/category
- `GET /api/v1/debug/health` - System health check

## Project Structure

```
src/
├── models/          # Data types (coordinates, distance, poi, route)
├── services/        # Business logic (route_generator, poi_service, mapbox, overpass)
├── routes/          # API endpoints (loop_route, pois, debug)
├── db/              # PostGIS spatial queries
└── cache/           # Redis caching

migrations/          # SQLx auto-migrations
tests/              # Integration tests (939 lines)
osm/                # OSM import scripts
```

**Total**: 4,394 lines of Rust + 939 lines of tests

## Key Services

- **Route Generator** (`route_generator.rs`, 727 lines): Core waypoint selection algorithm with adaptive tolerance
- **POI Service** (`poi_service.rs`, 272 lines): PostgreSQL/PostGIS + Overpass fallback
- **Mapbox Client** (`mapbox.rs`): Turn-by-turn route generation
- **Snapping Service** (`snapping_service.rs`): Find POIs along route paths

## Important Patterns

**Semantic Types**: Use newtype wrappers (`DistanceKm`, `RadiusMeters`, `Coordinates`) not primitives

**Error Handling**: All services return `Result<T, Error>` with `thiserror`

**Async**: Tokio runtime for all I/O operations

**Testing**: Fast iteration with `SKIP_REAL_API_TESTS=true` (~10-20s vs 60-120s)

## Critical Constraints

- **File Size**: Soft limit 500 lines, hard limit 800 lines (for AI context)
- **Mapbox Free Tier**: 100k requests/month (3-5 calls per route request)
- **Response Time**: < 3 seconds for route generation
- **Cache Hit Rate**: Target > 50%

## Documentation Index

📖 **[CLAUDE-SETUP.md](CLAUDE-SETUP.md)** - Development commands, testing, database, deployment, OSM import

🏗️ **[CLAUDE-ARCHITECTURE.md](CLAUDE-ARCHITECTURE.md)** - Tech stack, components, request flow, data models, dependencies

📋 **[CLAUDE-GUIDELINES.md](CLAUDE-GUIDELINES.md)** - Code quality guidelines, development patterns, security

⚡ **[CLAUDE-PERFORMANCE.md](CLAUDE-PERFORMANCE.md)** - Caching strategy, PostGIS queries, performance, monitoring

---

**For detailed architectural decisions, component descriptions, and implementation details, refer to the documentation files above.**
