# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a REST API service built in Rust that generates personalized walking and biking routes with points of interest (POIs). The service helps users discover new places by creating loop routes from a single point or routes between two points, incorporating interesting POIs like monuments, viewpoints, parks, and museums.

## Development Commands

### Running the Application
```bash
# Start dependencies (PostgreSQL + Redis)
docker-compose up -d postgres redis

# Run the API server
cargo run

# Run in development with auto-reload
cargo watch -x run
```

### Testing
```bash
# Run all tests
cargo test

# Run integration tests only
cargo test --test '*'

# Run specific test
cargo test test_waypoint_selection

# Run with logging
RUST_LOG=debug cargo test
```

### Database
```bash
# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Create new migration
sqlx migrate add <migration_name>
```

### Build & Deployment
```bash
# Build for production
cargo build --release

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy linter
cargo clippy
```

### OSM Data Import (Recommended for Production)
```bash
# Quick test with Monaco (~2MB, ~500 POIs)
./osm/download_osm.sh monaco
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf

# Production: Import France region (~3.8GB, ~800k POIs)
./osm/download_osm.sh europe/france
./osm/import_osm.sh ./osm/data/france-latest.osm.pbf

# Weekly updates (automated with cron)
./osm/update_osm.sh france

# See osm/QUICKSTART.md for detailed instructions
```

## Core Architecture

### Technology Stack
- **Language**: Rust 1.70+
- **Web Framework**: Axum 0.7 (built on Tokio)
- **Database**: PostgreSQL 15+ with PostGIS 3.3+
- **Cache**: Redis 7+
- **HTTP Client**: Reqwest for external APIs

### Key External Services
1. **Mapbox Directions API**: Generates turn-by-turn routes between waypoints
   - Free tier: 100,000 requests/month
   - Profiles: `walking` and `cycling`
2. **OpenStreetMap Data**: POI source
   - **Recommended**: Local OSM import via `osm2pgsql` (eliminates API timeouts)
   - **Fallback**: Overpass API (free but has timeout issues in dense areas)

### High-Level Request Flow

```
User Request → API Gateway (Axum) → Route Generator Service
                                          ↓
                             POI Service ← Check Redis Cache
                                          ↓
                             Query PostgreSQL/PostGIS for POIs
                                          ↓
                             Waypoint Selection Algorithm
                                          ↓
                             Mapbox Client → Generate actual route
                                          ↓
                             Score & Rank Routes → Return alternatives
```

## Core Components & Responsibilities

### Route Generator Service (`src/services/route_generator.rs`)
The heart of the application. Implements the waypoint selection algorithm:
1. **POI Discovery**: Query POIs within radius (target_distance_km / 2)
2. **Waypoint Selection**: Select 2-3 POIs that form a loop/path
3. **Route Generation**: Call Mapbox with waypoints
4. **Distance Validation**: Ensure ±10% tolerance
5. **Alternative Generation**: Create 2-3 different combinations
6. **Scoring**: Rank routes by quality (0-10 scale)

**Critical**: The algorithm must balance distance accuracy, POI quality, and spatial distribution.

### POI Service (`src/services/poi_service.rs`)
Manages POI data with two-tier architecture:
- **Primary**: PostgreSQL/PostGIS database with OSM-imported POIs
  - Instant queries using spatial indexes
  - No API timeouts or rate limits
  - Weekly updates from Geofabrik OSM extracts
- **Fallback**: Overpass API (only used if database is empty)
- Filter by category and preferences
- Calculate popularity scores (based on OSM tags: Wikipedia, UNESCO heritage, etc.)
- **Import System**: See `osm/` directory for import scripts and documentation

### API Layer (`src/routes/`)
Three main endpoint groups:
1. **Loop Routes** (`POST /routes/loop`): Single starting point, returns to origin
2. **Point-to-Point** (`POST /routes/point-to-point`): Start to end with POIs
3. **POI Queries** (`GET /pois`): Direct POI lookups

### Database Layer (`src/db/`)
PostgreSQL with PostGIS extension for spatial queries:
- `pois` table with GEOGRAPHY(POINT, 4326) type
- Spatial index using GIST: `CREATE INDEX idx_pois_location ON pois USING GIST(location)`
- Key query: `ST_DWithin()` for radius searches

## Important Data Models

### Route Preferences
```rust
pub struct RoutePreferences {
    pub poi_categories: Option<Vec<PoiCategory>>,
    pub hidden_gems: bool,  // Prefer lower popularity scores
    pub max_alternatives: u32,  // Default: 3
}
```

### Route Scoring Algorithm
Routes are scored 0-10 based on:
- **Distance accuracy** (0-3 points): Closer to target = better
- **POI count** (0-3 points): More POIs = better (up to 3)
- **POI quality** (0-2 points): Popularity or hidden gems based on preference
- **Category diversity** (0-2 points): Variety of POI types

## Caching Strategy (Critical for Cost Control)

### Three-Tier Caching System
1. **Route Cache** (Redis, 24h TTL)
   - Key: `route:loop:{hash}` or `route:p2p:{hash}`
   - Hash includes: coordinates (3 decimals), distance (0.5km), mode, preferences
   - Reduces Mapbox calls by 60-70%

2. **POI Region Cache** (Redis, 7 day TTL)
   - Key: `poi:region:{lat}:{lng}:{radius}`
   - Reduces Overpass calls by 80-90%

3. **POI Database** (PostgreSQL, permanent)
   - Initial load from OSM extracts
   - Weekly incremental updates

**Why this matters**: Mapbox free tier is 100k requests/month. Without caching, we'd hit limits quickly.

## Spatial Queries with PostGIS

PostGIS is used for all geographic operations. Key patterns:

```sql
-- Find POIs within radius (most common query)
SELECT id, name, category, ST_AsGeoJSON(location)::json
FROM pois
WHERE ST_DWithin(
    location,
    ST_GeogFromText('POINT(lng lat)'),  -- Note: lng, lat order!
    radius_meters
)
AND category = ANY($1)
ORDER BY ST_Distance(location, ST_GeogFromText('POINT(lng lat)'))
LIMIT 50;
```

**Important**: PostGIS uses (longitude, latitude) order, not (lat, lng)!

## Performance Requirements

- **Response Time**: < 3 seconds for route generation
- **Distance Accuracy**: Within ±10% tolerance
- **POI Density**: 2+ relevant POIs per route
- **Cache Hit Rate**: Target > 50%
- **API Uptime**: > 99%

## Cost Optimization

The service is designed to operate at zero cost under 100k requests/month:
- Stay within Mapbox free tier through aggressive caching
- Use free Overpass API with local POI database
- Self-host PostgreSQL and Redis

**Critical**: Monitor Mapbox API usage closely. Each user request generates 4-5 Mapbox calls.

## Development Phases

The project follows a phased approach:
- **Phase 1** (Weeks 1-4): MVP with basic loop routes
- **Phase 2** (Weeks 5-7): Optimization, caching, PostgreSQL/Redis
- **Phase 3** (Weeks 8-10): Preferences, bike support, API docs
- **Phase 4** (Weeks 11-12): Point-to-point routes
- **Phase 5** (Weeks 13-16): User accounts, personalization

Currently starting Phase 1.

## Common Development Patterns

### Error Handling
Use `thiserror` for custom errors. All service methods return `Result<T, Error>`.

### Async Operations
All I/O operations use Tokio async runtime. Key pattern:
```rust
pub async fn generate_route(...) -> Result<Route> {
    // All external API calls, DB queries use .await
}
```

### Logging
Use `tracing` crate with structured logging:
```rust
tracing::info!(
    route_id = %route.id,
    distance_km = route.distance_km,
    poi_count = route.pois.len(),
    "Route generated successfully"
);
```

## Security Considerations

### Input Validation
- Coordinate bounds: Valid lat/lng ranges
- Distance limits: 0.5km - 50km
- Always use parameterized queries with SQLx (prevents SQL injection)

### Rate Limiting
- Per IP: 100 requests/hour
- Per user: 500 requests/day (when auth implemented)

### API Security
- HTTPS only in production
- CORS policy enforcement
- Future: JWT authentication for Phase 5

## Project Structure

```
easyroute/
├── src/
│   ├── main.rs              # Entry point, Axum setup
│   ├── config.rs            # Environment config
│   ├── error.rs             # Custom error types
│   ├── routes/              # API endpoints
│   │   ├── loop_route.rs
│   │   ├── point_to_point.rs
│   │   └── pois.rs
│   ├── services/            # Business logic
│   │   ├── route_generator.rs  # Core algorithm
│   │   ├── poi_service.rs
│   │   ├── mapbox.rs
│   │   └── overpass.rs
│   ├── models/              # Data structures
│   │   ├── route.rs
│   │   ├── poi.rs
│   │   └── user.rs
│   ├── db/                  # Database layer
│   │   └── queries.rs
│   └── cache/               # Redis integration
│       └── redis.rs
├── migrations/              # SQLx migrations
├── tests/                   # Integration tests
└── docker-compose.yml       # Local dev environment
```

## Key Dependencies (from Cargo.toml)

```toml
axum = "0.7"                 # Web framework
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls"] }
reqwest = { version = "0.11", features = ["json"] }
redis = { version = "0.24", features = ["tokio-comp"] }
geo = "0.28"                 # Geospatial types
geojson = "0.24"             # GeoJSON serialization
serde = { version = "1", features = ["derive"] }
tracing = "0.1"              # Logging
```

## Monitoring

Track these metrics:
- API response times (p50, p95, p99)
- Route generation duration
- Cache hit rates (Redis)
- External API failures (Mapbox, Overpass)
- Database query times

Use `tracing` for logging, Prometheus + Grafana for metrics in production.
