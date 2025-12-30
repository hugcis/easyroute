# CLAUDE-ARCHITECTURE.md

System architecture, components, data models, and technical stack for EasyRoute.

## Technology Stack

### Core Technologies

- **Language**: Rust 1.70+ (Edition 2021)
  - Pinned to `stable` toolchain via `rust-toolchain.toml`
  - Strong type safety, zero-cost abstractions, memory safety

- **Web Framework**: Axum 0.8
  - Built on Tokio async runtime
  - Tower middleware ecosystem
  - Type-safe routing and extractors

- **Database**: PostgreSQL 18 with PostGIS 3.6
  - Spatial indexing with GIST
  - Geography types for lat/lng calculations
  - JSONB for flexible metadata

- **Cache**: Redis 8.4
  - Route caching (24h TTL)
  - POI region caching (7 day TTL)
  - Optional - graceful degradation if unavailable

- **HTTP Client**: Reqwest 0.12
  - Async HTTP client for external APIs
  - JSON serialization/deserialization

### External Services

#### Mapbox Directions API
- **Purpose**: Turn-by-turn route generation
- **Profiles**: `walking`, `cycling`
- **Free Tier**: 100,000 requests/month
- **Rate Limiting**: Critical - each user request generates 3-5 Mapbox calls
- **Caching**: Essential to stay within free tier

#### OpenStreetMap Data
- **Primary**: Local PostgreSQL import via `osm2pgsql`
  - Instant queries, no timeouts
  - Weekly updates from Geofabrik extracts
  - 27 POI categories

- **Fallback**: Overpass API
  - Free but has timeout issues in dense areas
  - Progressive radius reduction (100% → 75% → 50%)
  - Exponential backoff retry logic
  - Only used if database has insufficient POIs

## High-Level Request Flow

```
Client Request
    ↓
API Gateway (Axum Router)
    ↓
Request Validation (Coordinates, Distance)
    ↓
Check Redis Route Cache
    ├─ Cache Hit → Return cached routes
    └─ Cache Miss ↓
         ↓
    POI Service
         ↓
    Query PostgreSQL/PostGIS (primary)
         ├─ Sufficient POIs → Continue
         └─ Insufficient POIs ↓
              ↓
         Overpass API Fallback
              ├─ 100% radius attempt
              ├─ 75% radius (if timeout)
              └─ 50% radius (if timeout)
         ↓
    Filter & Score POIs
         ├─ By category preferences
         ├─ By popularity/hidden gems
         └─ By distance from start
         ↓
    Waypoint Selection Algorithm
         ├─ Select 2-3 POIs
         ├─ Validate spatial distribution
         ├─ Normal tolerance (user-specified)
         ├─ Relaxed tolerance (±20%)
         └─ Very Relaxed tolerance (±30%)
         ↓
    Mapbox Client
         ├─ Generate routes for each POI combination
         └─ Return GeoJSON geometry + metadata
         ↓
    Snapping Service
         └─ Find additional POIs within 100m of route path
         ↓
    Route Scoring
         ├─ Distance accuracy (0-3 points)
         ├─ POI count (0-3 points)
         ├─ POI quality (0-2 points)
         └─ Category diversity (0-2 points)
         ↓
    Cache in Redis (24h TTL)
         ↓
    Return Top N Alternatives (1-5, default 3)
         ↓
    JSON Response to Client
```

## Core Components

### 1. Route Generator Service
**File**: `src/services/route_generator.rs` (727 lines)

The heart of the application. Orchestrates the entire route generation pipeline.

**Responsibilities**:
- POI discovery within radius (target_distance_km / 2)
- Waypoint selection (2-3 POIs forming a loop)
- Spatial distribution validation (minimum angles between POIs)
- Route generation via Mapbox API
- Adaptive tolerance system (normal → relaxed → very relaxed)
- Alternative generation (1-5 routes)
- POI snapping to generated routes
- Route scoring and ranking

**Key Algorithm**:
1. Query POIs from POI Service
2. Try combinations of 2-3 POIs
3. For each combination:
   - Call Mapbox to generate route
   - Check if distance is within tolerance
   - Validate spatial distribution
4. If no valid routes, relax tolerance and retry
5. Snap additional POIs to route paths
6. Score and rank all valid routes
7. Return top alternatives

**Critical Constants** (from `src/constants.rs`):
- `MAX_ROUTE_GENERATION_RETRIES: 5`
- `TOLERANCE_LEVEL_RELAXED: 0.2` (±20%)
- `TOLERANCE_LEVEL_VERY_RELAXED: 0.3` (±30%)
- `MIN_POI_DISTANCE_KM: 0.2`

### 2. POI Service
**File**: `src/services/poi_service.rs` (272 lines)

Two-tier POI data architecture with database primary and API fallback.

**Responsibilities**:
- Query PostgreSQL/PostGIS for POIs (primary)
- Fall back to Overpass API if insufficient results
- Filter by category and preferences
- Calculate popularity scores (0-100)
- Handle progressive radius reduction for Overpass timeouts

**Database Query**:
```rust
// Fast spatial query using GIST index
SELECT id, name, category, ST_AsGeoJSON(location)::json, popularity
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

**Fallback Strategy**:
- Check POI count threshold (desired_count * 4)
- If insufficient, try Overpass with 100% radius
- If timeout, retry with 75% radius
- If timeout again, retry with 50% radius
- Cache successful results in Redis (7 day TTL)

### 3. Mapbox Client
**File**: `src/services/mapbox.rs` (172 lines)

Wrapper around Mapbox Directions API with type-safe request/response models.

**Features**:
- Supports `walking` and `cycling` profiles
- Handles up to 25 waypoints
- Returns GeoJSON geometry, distance, duration
- Error handling with retry logic
- Request rate tracking (for monitoring Mapbox quota)

**Request Format**:
```
GET /directions/v5/mapbox/{profile}/{coordinates}
Parameters:
  - geometries=geojson
  - overview=full
  - steps=true (optional)
```

### 4. Overpass Client
**File**: `src/services/overpass.rs` (681 lines)

Resilient client for Overpass API with batching and retry logic.

**Features**:
- OSM tag to POI category mapping (see `overpass_tags.rs`)
- Batched queries to avoid large result sets
- 30-second timeout per query
- Exponential backoff retry (2 attempts)
- Progressive radius reduction on timeouts
- Popularity scoring from OSM tags

**Popularity Scoring**:
- Wikipedia link: +30 points
- UNESCO heritage: +25 points
- Wikidata: +20 points
- Historic tag: +15 points
- Named feature: +10 points
- Base score for category

### 5. Snapping Service
**File**: `src/services/snapping_service.rs` (142 lines)

Finds additional POIs along generated route paths.

**Purpose**: Enrich routes with POIs that weren't used as waypoints but are near the path.

**Algorithm**:
1. Extract route geometry (LineString from GeoJSON)
2. Calculate bounding box around route
3. Query POIs within snap radius (default 100m) of route
4. Filter out POIs already used as waypoints
5. Return enriched POI list

**Configuration**: `SNAP_RADIUS_M` environment variable (0-1000m)

### 6. Cache Service
**File**: `src/cache/mod.rs` (360 lines)

Redis-based caching with smart key generation.

**Route Cache** (24h TTL):
- Key: `route:loop:{hash}`
- Hash includes:
  - Coordinates (bucketed to 3 decimals ≈ 100m)
  - Distance (bucketed to 0.5km)
  - Mode (walking/cycling)
  - POI categories (sorted)
  - Hidden gems flag
- Reduces Mapbox calls by 60-70%

**POI Region Cache** (7 day TTL):
- Key: `poi:region:{lat}:{lng}:{radius_km}`
- Coordinates rounded to 2 decimals (≈ 1km)
- Radius rounded to nearest km
- Reduces Overpass calls by 80-90%

**Graceful Degradation**: If Redis is unavailable, application continues without caching (logs warning).

### 7. Database Layer
**File**: `src/db/queries.rs` (306 lines)

PostgreSQL/PostGIS spatial queries with SQLx compile-time checking.

**Tables**:
- **`pois`**: POI data with spatial indexing
  - `id` (UUID, primary key)
  - `name` (TEXT)
  - `category` (TEXT)
  - `location` (GEOGRAPHY(POINT, 4326))
  - `popularity` (INTEGER 0-100)
  - `osm_id` (BIGINT, unique)
  - `metadata` (JSONB)
  - `created_at`, `updated_at` (TIMESTAMPTZ)

- **`cached_routes`**: Optional route analytics

- **`osm2pgsql_properties`**: OSM import state

**Indexes**:
- GIST index on `location` for fast spatial queries
- Unique index on `osm_id` for deduplication

### 8. API Layer
**Files**: `src/routes/*.rs`

Axum router with type-safe handlers.

**Endpoints**:

#### POST `/api/v1/routes/loop`
Generate loop routes from a starting point.

Request:
```json
{
  "start": {"lat": 48.8566, "lng": 2.3522},
  "distance_km": 5.0,
  "mode": "walking",
  "preferences": {
    "poi_categories": ["Monument", "Museum", "Park"],
    "hidden_gems": false,
    "max_alternatives": 3
  }
}
```

Response: Array of routes with GeoJSON geometry, POIs, scores.

#### GET `/api/v1/pois`
Query POIs by location and filters.

Query parameters:
- `lat`, `lng`: Center point
- `radius_km`: Search radius
- `categories`: Comma-separated list

#### GET `/api/v1/debug/health`
System health check.

Returns:
```json
{
  "status": "healthy",
  "database": "connected",
  "postgis": "3.6.0",
  "redis": "connected",
  "poi_count": 850000
}
```

## Data Models

### Core Types

**Coordinates** (`src/models/coordinates.rs`, 192 lines):
```rust
pub struct Coordinates {
    pub lat: f64,  // -90 to 90
    pub lng: f64,  // -180 to 180
}
```
- Validation at construction
- Bounds checking
- Serialization to/from JSON

**Distance Types** (`src/models/distance.rs`, 196 lines):
```rust
pub struct DistanceKm(f64);      // Kilometers
pub struct DistanceMeters(f64);  // Meters
pub struct RadiusMeters(f64);    // Radius in meters
```
- Newtype pattern for type safety
- Conversion methods between units
- Validation (non-negative, within bounds)

**POI** (`src/models/poi.rs`, 194 lines):
```rust
pub struct Poi {
    pub id: Uuid,
    pub name: String,
    pub category: PoiCategory,
    pub location: Coordinates,
    pub popularity: u8,  // 0-100
    pub metadata: Option<serde_json::Value>,
}

pub enum PoiCategory {
    Monument, Viewpoint, Park, Museum, Restaurant, Cafe,
    Historic, Cultural, Waterfront, Waterfall, NatureReserve,
    Church, Castle, Bridge, Tower, Plaza, Fountain, Market,
    Artwork, Lighthouse, Winery, Brewery, Theatre, Library,
    // 27 total categories
}
```

**Route** (`src/models/route.rs`, 263 lines):
```rust
pub struct Route {
    pub geometry: LineString,  // GeoJSON geometry
    pub distance_km: DistanceKm,
    pub duration_seconds: u32,
    pub pois: Vec<Poi>,
    pub score: f64,  // 0-10 scale
}
```

### Route Preferences

```rust
pub struct RoutePreferences {
    pub poi_categories: Option<Vec<PoiCategory>>,  // Filter POIs
    pub hidden_gems: bool,  // Prefer lower popularity scores
    pub max_alternatives: u32,  // 1-5, default 3
}
```

### Route Scoring

Routes scored 0-10 based on:
- **Distance accuracy** (0-3 points): `3 * (1 - abs(actual - target) / target)`
- **POI count** (0-3 points): `min(poi_count, 3)` points
- **POI quality** (0-2 points): Average popularity (or inverse if hidden_gems)
- **Category diversity** (0-2 points): `min(unique_categories, 2)` points

## Project Structure

```
easyroute/
├── src/
│   ├── main.rs              # Entry point, Axum setup, auto-migrations (106 lines)
│   ├── lib.rs               # Library exports for testing (27 lines)
│   ├── config.rs            # Environment config (54 lines)
│   ├── constants.rs         # Application constants (59 lines)
│   ├── error.rs             # Custom error types with thiserror (84 lines)
│   │
│   ├── routes/              # API endpoints
│   │   ├── loop_route.rs    # Loop route generation (69 lines)
│   │   ├── pois.rs          # POI queries (271 lines)
│   │   └── debug.rs         # Health check endpoint (72 lines)
│   │
│   ├── services/            # Business logic
│   │   ├── route_generator.rs    # Core algorithm (727 lines)
│   │   ├── poi_service.rs        # DB + Overpass fallback (272 lines)
│   │   ├── mapbox.rs             # Mapbox API client (172 lines)
│   │   ├── overpass.rs           # Overpass API with retries (681 lines)
│   │   ├── overpass_tags.rs      # OSM tag mappings (101 lines)
│   │   └── snapping_service.rs   # POI-to-route snapping (142 lines)
│   │
│   ├── models/              # Data structures
│   │   ├── coordinates.rs   # Coordinate validation (192 lines)
│   │   ├── distance.rs      # Distance type wrappers (196 lines)
│   │   ├── poi.rs           # POI models, 27 categories (194 lines)
│   │   └── route.rs         # Route models with scoring (263 lines)
│   │
│   ├── db/                  # Database layer
│   │   ├── mod.rs           # Module exports (12 lines)
│   │   └── queries.rs       # PostGIS spatial queries (306 lines)
│   │
│   └── cache/               # Redis integration
│       └── mod.rs           # Route & POI caching (360 lines)
│
├── migrations/              # SQLx migrations (auto-run on startup)
│   ├── 001_initial_schema.sql
│   ├── 002_add_pois_table.sql
│   └── ...
│
├── tests/                   # Integration tests (939 lines)
│   ├── api_tests.rs
│   ├── database_tests.rs
│   ├── mapbox_tests.rs
│   ├── overpass_tests.rs
│   ├── route_generation_tests.rs
│   └── common/mod.rs
│
├── osm/                     # OSM import system
│   ├── download_osm.sh      # Download Geofabrik extracts
│   ├── import_osm.sh        # Import via osm2pgsql
│   ├── update_osm.sh        # Incremental updates
│   ├── osm_poi_style.lua    # POI extraction logic
│   ├── QUICKSTART.md        # Import guide
│   ├── README.md            # Detailed docs
│   └── data/                # Downloaded OSM files
│
├── docker-compose.yml       # PostgreSQL + Redis + osm2pgsql
├── rust-toolchain.toml      # Pins to stable Rust
├── Cargo.toml              # Dependencies
└── Cargo.lock              # Locked versions (committed)
```

**Total**: 4,394 lines of Rust code + 939 lines of tests

## Key Dependencies

```toml
[dependencies]
# Web Framework
axum = "0.8"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tokio = { version = "1", features = ["full"] }

# Database
sqlx = { version = "0.8", features = [
    "postgres", "runtime-tokio-native-tls",
    "macros", "uuid", "json", "time"
] }

# HTTP Client & External APIs
reqwest = { version = "0.12", features = ["json"] }

# Caching
redis = { version = "1.0", features = ["tokio-comp"] }

# Geospatial
geo = "0.32"
geojson = "0.24"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error Handling
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Other
uuid = { version = "1", features = ["v4", "serde"] }
dotenv = "0.15"
```

## Development Phases

### Phase 1: MVP ✅ COMPLETED
- Basic loop route generation
- Mapbox integration
- Simple POI queries from Overpass
- In-memory caching

### Phase 2: Optimization ✅ COMPLETED
- PostgreSQL/PostGIS database
- Redis caching
- OSM import system
- Performance tuning

### Phase 3: Features 🔄 IN PROGRESS
- ✅ Route preferences (categories, hidden gems)
- ✅ Walking and cycling modes
- ✅ Health check endpoint
- ✅ Adaptive tolerance system
- ❌ OpenAPI/Swagger documentation (TODO)
- ❌ Rate limiting (TODO)
- ❌ Metrics/observability (TODO)

### Phase 4: Point-to-Point 📋 PLANNED
- Routes between two points
- Multi-waypoint routes
- Route optimization

### Phase 5: Personalization 📋 PLANNED
- User accounts
- Saved routes
- Personalized recommendations
- Usage analytics
