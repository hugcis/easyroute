# Route Discovery API - Architectural Design Document

## Project Overview

A REST API service that generates personalized walking and biking routes with points of interest (POIs), helping users discover new places or explore familiar areas differently.

**Version:** 1.0  
**Date:** November 2, 2025  
**Status:** Initial Design

---

## Table of Contents

1. [Goals & Objectives](#goals--objectives)
2. [System Architecture](#system-architecture)
3. [Technology Stack](#technology-stack)
4. [Core Components](#core-components)
5. [Data Models](#data-models)
6. [API Endpoints](#api-endpoints)
7. [Route Generation Algorithm](#route-generation-algorithm)
8. [External Services](#external-services)
9. [Caching Strategy](#caching-strategy)
10. [Database Schema](#database-schema)
11. [Development Phases](#development-phases)
12. [Cost Analysis](#cost-analysis)
13. [Future Enhancements](#future-enhancements)

---

## Goals & Objectives

### Primary Goals
- Generate walking and biking routes that loop from a single starting point
- Generate routes between two specified points
- Incorporate interesting POIs (monuments, viewpoints, parks, etc.)
- Support customizable route distances with tolerance ranges
- Provide multiple route alternatives when possible

### Success Criteria
- Routes match requested distance within ±10% tolerance
- API response time under 3 seconds for route generation
- High POI density in generated routes
- Zero cost operation under 100k requests/month

---

## System Architecture

### High-Level Architecture

```
┌─────────────┐
│   Frontend  │
│  (Separate) │
└──────┬──────┘
       │ HTTPS/REST
       ▼
┌─────────────────────────────────────────┐
│          API Gateway (Axum)             │
│  ┌────────────────────────────────┐    │
│  │  Route Endpoints               │    │
│  │  - POST /routes/loop           │    │
│  │  - POST /routes/point-to-point │    │
│  │  - GET  /routes/{id}           │    │
│  └────────────────────────────────┘    │
└───────┬─────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────┐
│       Application Layer (Rust)          │
│  ┌────────────┐  ┌──────────────┐      │
│  │   Route    │  │     POI      │      │
│  │ Generator  │  │   Service    │      │
│  └────────────┘  └──────────────┘      │
│  ┌────────────┐  ┌──────────────┐      │
│  │  Mapbox    │  │  Overpass    │      │
│  │  Client    │  │   Client     │      │
│  └────────────┘  └──────────────┘      │
└───┬─────────────────────┬───────────────┘
    │                     │
    ▼                     ▼
┌─────────┐         ┌──────────┐
│  Redis  │         │PostgreSQL│
│ (Cache) │         │ + PostGIS│
└─────────┘         └──────────┘
    │                     │
    └──────────┬──────────┘
               ▼
    ┌────────────────────┐
    │  External Services │
    │  - Mapbox API      │
    │  - Overpass API    │
    └────────────────────┘
```

### Architecture Principles
- **Stateless API**: Each request contains all necessary information
- **Async-First**: All I/O operations use Tokio async runtime
- **Cache-Heavy**: Minimize external API calls through aggressive caching
- **Type-Safe**: Leverage Rust's type system for correctness
- **Geographic-Aware**: PostGIS for efficient spatial queries

---

## Technology Stack

### Core Technologies

| Component | Technology | Justification |
|-----------|-----------|---------------|
| **Language** | Rust 1.70+ | Performance, type safety, memory efficiency |
| **Web Framework** | Axum 0.7 | Modern, ergonomic, built on Tokio |
| **Async Runtime** | Tokio | Industry standard for async Rust |
| **Database** | PostgreSQL 15+ | Robust, excellent PostGIS support |
| **Spatial Extension** | PostGIS 3.3+ | Industry-leading geospatial features |
| **Cache** | Redis 7+ | Fast in-memory caching |
| **HTTP Client** | Reqwest 0.11 | Simple async HTTP client |

### Key Rust Crates

```toml
[dependencies]
# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "macros", "uuid", "json"] }

# HTTP client
reqwest = { version = "0.11", features = ["json"] }

# Geospatial
geo = "0.28"
geojson = "0.24"

# Caching
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Utilities
uuid = { version = "1", features = ["serde", "v4"] }
dotenv = "0.15"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## Core Components

### 1. API Layer (`src/routes/`)
Handles HTTP requests and responses using Axum framework.

**Responsibilities:**
- Request validation
- Authentication (future)
- Response formatting
- Error handling

**Key Files:**
- `mod.rs` - Route registration
- `loop_route.rs` - Loop route endpoints
- `point_to_point.rs` - Point-to-point endpoints
- `pois.rs` - POI query endpoints

### 2. Route Generator Service (`src/services/route_generator.rs`)
Core business logic for route generation.

**Responsibilities:**
- Waypoint selection algorithm
- Route optimization
- Alternative route generation
- Distance validation

**Key Methods:**
```rust
pub async fn generate_loop_route(
    start: Coordinates,
    distance_km: f64,
    preferences: RoutePreferences,
) -> Result<Vec<Route>>;

pub async fn generate_point_to_point_route(
    start: Coordinates,
    end: Coordinates,
    preferences: RoutePreferences,
) -> Result<Vec<Route>>;
```

### 3. POI Service (`src/services/poi_service.rs`)
Manages POI data retrieval and caching.

**Responsibilities:**
- Query POIs within radius
- Filter by category and preferences
- Calculate POI popularity scores
- Cache POI data

### 4. External API Clients

#### Mapbox Client (`src/services/mapbox.rs`)
```rust
pub async fn get_directions(
    waypoints: Vec<Coordinates>,
    mode: TransportMode,
) -> Result<DirectionsResponse>;
```

#### Overpass Client (`src/services/overpass.rs`)
```rust
pub async fn query_pois(
    center: Coordinates,
    radius_meters: f64,
    categories: Vec<PoiCategory>,
) -> Result<Vec<Poi>>;
```

### 5. Database Layer (`src/db/`)
PostgreSQL with PostGIS for spatial data.

**Responsibilities:**
- POI storage and retrieval
- Spatial queries
- User data (future)
- Route caching

### 6. Cache Layer (`src/cache/`)
Redis for high-speed caching.

**Cached Data:**
- Generated routes (keyed by parameters hash)
- POI data for regions
- Popular route alternatives

---

## Data Models

### Core Types (`src/models/`)

```rust
// Coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub lat: f64,
    pub lng: f64,
}

// Transport Mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    Walk,
    Bike,
}

// POI Category
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoiCategory {
    Monument,
    Viewpoint,
    Park,
    Museum,
    Restaurant,
    Cafe,
    Historic,
    Cultural,
}

// POI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poi {
    pub id: Uuid,
    pub name: String,
    pub category: PoiCategory,
    pub coordinates: Coordinates,
    pub popularity_score: f32,  // 0-100
    pub description: Option<String>,
    pub estimated_visit_duration_minutes: Option<u32>,
}

// Route Preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePreferences {
    pub poi_categories: Option<Vec<PoiCategory>>,
    pub hidden_gems: bool,  // Prefer lower popularity scores
    pub max_alternatives: u32,  // Default: 3
}

// Route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: Uuid,
    pub distance_km: f64,
    pub estimated_duration_minutes: u32,
    pub elevation_gain_m: Option<f32>,
    pub path: Vec<Coordinates>,  // GeoJSON LineString
    pub pois: Vec<RoutePoi>,
    pub score: f32,  // Route quality score (0-10)
}

// POI within a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePoi {
    #[serde(flatten)]
    pub poi: Poi,
    pub order_in_route: u32,
    pub distance_from_start_km: f64,
}
```

---

## API Endpoints

### Base URL
```
https://api.yourdomain.com/api/v1
```

### 1. Generate Loop Route

**Endpoint:** `POST /routes/loop`

**Request Body:**
```json
{
  "start_point": {
    "lat": 48.8566,
    "lng": 2.3522
  },
  "distance_km": 5.0,
  "distance_tolerance": 0.5,
  "mode": "walk",
  "preferences": {
    "poi_categories": ["monument", "viewpoint", "park"],
    "hidden_gems": false,
    "max_alternatives": 3
  }
}
```

**Response:** `200 OK`
```json
{
  "routes": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "distance_km": 5.2,
      "estimated_duration_minutes": 78,
      "elevation_gain_m": 45,
      "path": [
        {"lat": 48.8566, "lng": 2.3522},
        {"lat": 48.8606, "lng": 2.3376},
        ...
      ],
      "pois": [
        {
          "id": "...",
          "name": "Eiffel Tower",
          "category": "monument",
          "coordinates": {"lat": 48.8584, "lng": 2.2945},
          "popularity_score": 95,
          "order_in_route": 1,
          "distance_from_start_km": 1.2,
          "estimated_visit_duration_minutes": 30
        }
      ],
      "score": 8.5
    }
  ]
}
```

### 2. Generate Point-to-Point Route

**Endpoint:** `POST /routes/point-to-point`

**Request Body:**
```json
{
  "start_point": {"lat": 48.8566, "lng": 2.3522},
  "end_point": {"lat": 48.8738, "lng": 2.2950},
  "mode": "walk",
  "preferences": {
    "poi_categories": ["park", "cafe"],
    "hidden_gems": true,
    "max_alternatives": 2
  }
}
```

**Response:** Same structure as loop route

### 3. Get Route by ID

**Endpoint:** `GET /routes/{route_id}`

**Response:** Single route object

### 4. Query POIs

**Endpoint:** `GET /pois?lat={lat}&lng={lng}&radius_km={radius}&categories={cat1,cat2}`

**Response:**
```json
{
  "pois": [
    {
      "id": "...",
      "name": "Local Art Gallery",
      "category": "museum",
      "coordinates": {"lat": 48.8566, "lng": 2.3522},
      "popularity_score": 42,
      "description": "Small contemporary art gallery"
    }
  ]
}
```

### Error Responses

**400 Bad Request**
```json
{
  "error": "Invalid request",
  "message": "distance_km must be between 0.5 and 50"
}
```

**500 Internal Server Error**
```json
{
  "error": "Internal server error",
  "message": "Failed to generate route"
}
```

---

## Route Generation Algorithm

### Hybrid Waypoint Strategy

The core algorithm uses a waypoint-based approach that leverages existing routing services while optimizing for POI inclusion.

#### Algorithm Steps

1. **POI Discovery**
   ```
   - Calculate search radius: target_distance_km / 2
   - Query POIs within radius from start point
   - Filter by user preferences (categories, hidden gems)
   - Score POIs based on:
     * Distance from start
     * Popularity (inverse if hidden_gems=true)
     * Category match
   ```

2. **Waypoint Selection**
   ```
   - For loop routes:
     * Select 2-3 POIs that form a polygon
     * Target perimeter: ~target_distance_km
     * Ensure POIs are spatially distributed (avoid clustering)
     * Prefer different categories for variety
   
   - For point-to-point routes:
     * Select 1-2 POIs between start and end
     * Ensure they don't deviate too far from direct path
   ```

3. **Route Generation**
   ```
   - Build waypoint sequence: Start → POI1 → POI2 → POI3 → Start
   - Call Mapbox Directions API with waypoints
   - Receive turn-by-turn path with distance
   ```

4. **Distance Validation**
   ```
   - Check if distance is within tolerance range
   - If too short/long:
     * Adjust POI selection
     * Try different POI combinations
     * Maximum 3 retry attempts
   ```

5. **Alternative Generation**
   ```
   - Generate 2-3 different POI combinations
   - Score each route:
     * POI count and quality
     * Distance accuracy
     * Elevation profile
     * Path diversity (avoid overlap with other alternatives)
   ```

6. **Ranking & Return**
   ```
   - Sort routes by score
   - Return top N alternatives (max_alternatives parameter)
   - Cache results for similar future requests
   ```

#### Scoring Function

```rust
fn score_route(route: &Route, preferences: &RoutePreferences) -> f32 {
    let mut score = 0.0;
    
    // Distance accuracy (0-3 points)
    let distance_error = (route.distance_km - target_distance).abs();
    score += 3.0 * (1.0 - distance_error / target_distance);
    
    // POI count (0-3 points)
    score += (route.pois.len() as f32).min(3.0);
    
    // POI quality (0-2 points)
    let avg_poi_score = route.pois.iter()
        .map(|p| p.poi.popularity_score / 100.0)
        .sum::<f32>() / route.pois.len() as f32;
    score += 2.0 * if preferences.hidden_gems {
        1.0 - avg_poi_score  // Prefer lower popularity
    } else {
        avg_poi_score  // Prefer higher popularity
    };
    
    // Category diversity (0-2 points)
    let unique_categories = route.pois.iter()
        .map(|p| &p.poi.category)
        .collect::<HashSet<_>>()
        .len();
    score += 2.0 * (unique_categories as f32 / 3.0).min(1.0);
    
    score  // Max: 10.0
}
```

#### Complexity Analysis

- **POI Query**: O(1) - spatial index lookup
- **Waypoint Selection**: O(n²) - where n = POI count (typically < 50)
- **Mapbox API Calls**: 4-5 calls per request
- **Total Time**: ~1-3 seconds per request

---

## External Services

### 1. Mapbox Directions API

**Purpose:** Generate turn-by-turn routes between waypoints

**Endpoint:** `https://api.mapbox.com/directions/v5/mapbox/{profile}/{coordinates}`

**Profiles:**
- `walking` - For pedestrian routes
- `cycling` - For bike routes

**Request Example:**
```
GET /directions/v5/mapbox/walking/2.3522,48.8566;2.2945,48.8584;2.3522,48.8566
?geometries=geojson
&overview=full
&steps=true
```

**Rate Limits:**
- Free tier: 100,000 requests/month
- Our usage: ~40,000-50,000 requests per 10,000 user requests

**Cost:** $0 (within free tier for MVP)

### 2. Overpass API (OpenStreetMap)

**Purpose:** Query POI data from OpenStreetMap

**Endpoint:** `https://overpass-api.de/api/interpreter`

**Query Example:**
```
[out:json];
(
  node["tourism"="viewpoint"](around:2500,48.8566,2.3522);
  node["historic"](around:2500,48.8566,2.3522);
  node["leisure"="park"](around:2500,48.8566,2.3522);
);
out body;
```

**POI Categories Mapping:**
```rust
Monument    → tourism=monument, historic=*
Viewpoint   → tourism=viewpoint
Park        → leisure=park
Museum      → tourism=museum
Restaurant  → amenity=restaurant
Cafe        → amenity=cafe
```

**Rate Limits:** Fair use policy (cache aggressively)

**Cost:** $0 (free, open source)

---

## Caching Strategy

### Three-Tier Caching

#### 1. Route Cache (Redis)
**TTL:** 24 hours

**Key Structure:**
```
route:loop:{hash}
route:p2p:{hash}
```

**Hash Includes:**
- Start coordinates (rounded to 3 decimals)
- Distance (rounded to 0.5km)
- Mode
- Preferences

**Benefits:**
- Instant response for repeated queries
- Reduces Mapbox API calls by ~60-70%

#### 2. POI Region Cache (Redis)
**TTL:** 7 days

**Key Structure:**
```
poi:region:{lat_rounded}:{lng_rounded}:{radius_km}
```

**Benefits:**
- Reduces Overpass API calls by ~80-90%
- Faster POI lookups

#### 3. POI Database (PostgreSQL)
**Permanent storage**

**Update Strategy:**
- Initial load: Import OSM extracts for target cities
- Incremental updates: Weekly sync from Overpass
- User contributions: Direct inserts (future feature)

**Benefits:**
- Eliminates Overpass dependency for most queries
- Enables complex spatial queries
- Supports custom POI metadata

### Cache Invalidation

**Manual Invalidation:**
- POI data changes (admin action)
- Algorithm updates

**Automatic Invalidation:**
- TTL expiration
- LRU eviction when Redis memory full

---

## Database Schema

### PostgreSQL with PostGIS

#### Tables

```sql
-- Enable PostGIS extension
CREATE EXTENSION IF NOT EXISTS postgis;

-- POIs table
CREATE TABLE pois (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    category VARCHAR(50) NOT NULL,
    location GEOGRAPHY(POINT, 4326) NOT NULL,  -- PostGIS geography type
    popularity_score FLOAT DEFAULT 50.0,
    description TEXT,
    estimated_visit_duration_minutes INTEGER,
    osm_id BIGINT UNIQUE,  -- OpenStreetMap ID
    metadata JSONB,  -- Flexible additional data
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Spatial index for fast queries
CREATE INDEX idx_pois_location ON pois USING GIST(location);
CREATE INDEX idx_pois_category ON pois(category);
CREATE INDEX idx_pois_popularity ON pois(popularity_score);

-- Routes cache table (optional, can use Redis only)
CREATE TABLE cached_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cache_key VARCHAR(255) UNIQUE NOT NULL,
    route_data JSONB NOT NULL,  -- Serialized route object
    created_at TIMESTAMP DEFAULT NOW(),
    access_count INTEGER DEFAULT 0,
    last_accessed_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_cached_routes_key ON cached_routes(cache_key);
CREATE INDEX idx_cached_routes_last_accessed ON cached_routes(last_accessed_at);

-- User POI interactions (future)
CREATE TABLE user_seen_pois (
    user_id UUID NOT NULL,
    poi_id UUID NOT NULL REFERENCES pois(id),
    seen_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (user_id, poi_id)
);

-- User custom POIs (future)
CREATE TABLE user_custom_pois (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    category VARCHAR(50) NOT NULL,
    location GEOGRAPHY(POINT, 4326) NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_user_custom_pois_location ON user_custom_pois USING GIST(location);
```

#### Key Queries

**Find POIs within radius:**
```sql
SELECT 
    id, name, category,
    ST_AsGeoJSON(location)::json as coordinates,
    popularity_score,
    ST_Distance(location, ST_GeogFromText('POINT(2.3522 48.8566)')) as distance_meters
FROM pois
WHERE ST_DWithin(
    location,
    ST_GeogFromText('POINT(2.3522 48.8566)'),
    2500  -- radius in meters
)
AND category = ANY($1)  -- array of categories
ORDER BY distance_meters
LIMIT 50;
```

**Count POIs by category in region:**
```sql
SELECT category, COUNT(*) as count
FROM pois
WHERE ST_DWithin(
    location,
    ST_GeogFromText('POINT(2.3522 48.8566)'),
    5000
)
GROUP BY category;
```

---

## Development Phases

### Phase 1: MVP Core (Weeks 1-4)

**Goal:** Working API with basic loop route generation

**Deliverables:**
- ✅ Project setup with Rust + Axum
- ✅ Single endpoint: `POST /routes/loop`
- ✅ Mapbox integration for routing
- ✅ Overpass API integration for POIs (direct queries)
- ✅ Basic waypoint selection algorithm
- ✅ Return single best route
- ✅ Basic error handling

**Testing:**
- Manual testing with Postman/curl
- Integration tests for Mapbox/Overpass clients
- Unit tests for waypoint selection

**Success Metric:** Generate valid 5km loop with 2+ POIs in Paris

---

### Phase 2: Optimization & Storage (Weeks 5-7)

**Goal:** Improve performance and add persistence

**Deliverables:**
- ✅ PostgreSQL + PostGIS setup
- ✅ POI database with initial Paris data
- ✅ Redis caching layer
- ✅ Return 2-3 alternative routes
- ✅ Improved scoring algorithm
- ✅ Logging and monitoring (tracing)

**Data Migration:**
- Import OSM data for Paris
- Calculate popularity scores
- Build spatial indexes

**Success Metric:** 
- Response time < 3 seconds
- Cache hit rate > 50%
- 3 diverse route alternatives

---

### Phase 3: Feature Expansion (Weeks 8-10)

**Goal:** Add preferences and bike support

**Deliverables:**
- ✅ POI category filtering
- ✅ Hidden gems vs. popular toggle
- ✅ Bike route support
- ✅ `GET /pois` endpoint
- ✅ Better error messages
- ✅ API documentation (Swagger/OpenAPI)

**Success Metric:** 
- Support both walk and bike modes
- Effective filtering by preferences
- Complete API documentation

---

### Phase 4: Advanced Routes (Weeks 11-12)

**Goal:** Point-to-point routes

**Deliverables:**
- ✅ `POST /routes/point-to-point` endpoint
- ✅ Modified algorithm for P2P routes
- ✅ Integration tests

**Success Metric:** Generate valid P2P routes with POIs

---

### Phase 5: User Features (Weeks 13-16)

**Goal:** User accounts and personalization

**Deliverables:**
- ✅ User authentication (JWT)
- ✅ User "seen POIs" tracking
- ✅ Custom POI creation
- ✅ Personalized route suggestions
- ✅ `GET /routes/{id}` endpoint

**Database Changes:**
- Add users table
- Add user_seen_pois table
- Add user_custom_pois table

**Success Metric:** Users can mark POIs as seen and affect future routes

---

## Cost Analysis

### Infrastructure Costs (Monthly)

**Budget Tier:**

| Service | Cost | Notes |
|---------|------|-------|
| **Mapbox API** | $0 | Free tier: 100k requests/month |
| **Overpass API** | $0 | Free, open source |
| **VPS (Hetzner/DigitalOcean)** | $20-40 | 4GB RAM, 2 vCPU |
| **PostgreSQL** | $0 | Self-hosted on VPS |
| **Redis** | $0 | Self-hosted on VPS |
| **Domain + SSL** | $2-5 | Let's Encrypt free SSL |
| **Total** | **$22-45/month** | |

### Scaling Projections

**10,000 user requests/month:**
- ~40,000 Mapbox API calls
- **Cost: $0** (within free tier)

**100,000 user requests/month:**
- ~400,000 Mapbox API calls
- **Cost: $0** (at free tier limit)

**250,000 user requests/month:**
- ~1M Mapbox API calls
- Mapbox overage: ~900k × $0.40/1000 = $360
- Need larger VPS: $80/month
- **Total Cost: ~$440/month**

### Cost Optimization Strategies

1. **Aggressive Caching**: Reduce Mapbox calls by 60-70%
2. **Route Pre-generation**: Cache popular routes offline
3. **Regional Batching**: Group nearby requests
4. **User Throttling**: Rate limit per user
5. **Upgrade to Paid Tier**: Better rates at scale

---

## Future Enhancements

### Short-term (3-6 months)

**Multi-city Support:**
- Import OSM data for top 20 European cities
- City-specific popularity algorithms
- Local recommendations

**Public Transit Integration:**
- Integrate transit APIs (Google Transit, local providers)
- Mixed-mode routes (walk + metro)
- Time-based routing (avoid closed stations)

**Advanced Preferences:**
- Difficulty levels (easy/moderate/hard)
- Accessibility features (wheelchair-friendly)
- Time of day considerations (opening hours)
- Weather-aware routing

**Social Features:**
- Share routes with friends
- Community-curated POIs
- Route ratings and reviews

### Medium-term (6-12 months)

**Mobile SDK:**
- Native iOS/Android SDKs
- Offline route caching
- GPS-based progress tracking

**Premium Features:**
- Guided audio tours
- Augmented reality POI markers
- Personalized AI recommendations
- Historical route analytics

**Partner Integrations:**
- Tourism boards
- Local businesses
- Event organizers
- Hotel concierge systems

### Long-term (12+ months)

**Machine Learning:**
- Learn user preferences over time
- Predict optimal routes
- Anomaly detection (traffic, closures)
- Seasonal recommendations

**Global Expansion:**
- Worldwide coverage
- Multi-language support
- Regional cultural customization

**Monetization:**
- Freemium model (advanced features paid)
- B2B API access
- Sponsored POIs (ethical advertising)

---

## Security Considerations

### API Security

**Rate Limiting:**
```rust
// Per IP: 100 requests/hour
// Per user: 500 requests/day (authenticated)
```

**Input Validation:**
- Coordinate bounds checking
- Distance limits (0.5km - 50km)
- SQL injection prevention (parameterized queries)
- XSS prevention (sanitize user inputs)

**Authentication (Future):**
- JWT tokens
- API key for partner access
- OAuth2 for third-party apps

### Data Privacy

**User Data:**
- GDPR compliance
- User data export functionality
- Right to deletion
- Encrypted passwords (bcrypt)

**Location Privacy:**
- Don't store precise user locations
- Aggregate analytics only
- Anonymize cached data

### Infrastructure Security

**Network:**
- HTTPS only (TLS 1.3)
- CORS policy enforcement
- DDoS protection (Cloudflare)

**Database:**
- Encrypted connections
- Principle of least privilege
- Regular backups
- Prepared statements only

---

## Monitoring & Observability

### Metrics to Track

**Performance:**
- API response times (p50, p95, p99)
- Route generation duration
- Cache hit rates
- Database query times

**Business:**
- Daily active users
- Routes generated per day
- Most popular POIs
- Average route distance

**Errors:**
- Error rate by endpoint
- External API failures
- Database connection issues

### Tools

**Logging:** `tracing` + `tracing-subscriber`
```rust
tracing::info!(
    route_id = %route.id,
    distance_km = route.distance_km,
    poi_count = route.pois.len(),
    "Route generated successfully"
);
```

**Metrics:** Prometheus + Grafana
- Custom metrics for route quality scores
- External API latency
- Cache performance

**Alerting:**
- Error rate > 5%
- Response time > 5 seconds
- External API down
- Database connection pool exhausted

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_waypoint_selection() {
        // Test POI selection algorithm
    }
    
    #[test]
    fn test_route_scoring() {
        // Test scoring function
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_generate_loop_route_integration() {
    // Test full flow with test database
    // Mock external APIs
}
```

### Load Tests
- Use `k6` or `vegeta`
- Simulate 100 concurrent users
- Target: 1000 requests in 60 seconds

### API Contract Tests
- OpenAPI specification
- Automated validation
- Backward compatibility checks

---

## Deployment

### Development Environment
```bash
# Docker Compose setup
docker-compose up -d postgres redis
cargo run
```

### Production Environment

**Option 1: Single VPS**
- Hetzner CPX31 (4GB RAM, 2 vCPU): €12/month
- Run all services on one machine
- Good for MVP and early growth

**Option 2: Managed Services**
- Fly.io / Railway for Rust app
- Managed PostgreSQL (Supabase/Neon)
- Managed Redis (Upstash)
- Higher cost but easier management

### CI/CD Pipeline

**GitHub Actions:**
```yaml
1. Run tests
2. Build Docker image
3. Push to registry
4. Deploy to production
5. Run smoke tests
```

---

## Project Structure

```
route-api/
├── Cargo.toml
├── Cargo.lock
├── .env.example
├── .gitignore
├── README.md
├── docker-compose.yml
│
├── migrations/
│   ├── 001_create_pois_table.sql
│   ├── 002_create_cached_routes_table.sql
│   └── 003_create_user_tables.sql
│
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   │
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── loop_route.rs
│   │   ├── point_to_point.rs
│   │   └── pois.rs
│   │
│   ├── services/
│   │   ├── mod.rs
│   │   ├── route_generator.rs
│   │   ├── poi_service.rs
│   │   ├── mapbox.rs
│   │   └── overpass.rs
│   │
│   ├── models/
│   │   ├── mod.rs
│   │   ├── route.rs
│   │   ├── poi.rs
│   │   └── user.rs
│   │
│   ├── db/
│   │   ├── mod.rs
│   │   └── queries.rs
│   │
│   └── cache/
│       ├── mod.rs
│       └── redis.rs
│
└── tests/
    ├── integration/
    │   ├── loop_route_tests.rs
    │   └── poi_tests.rs
    └── common/
        └── mod.rs
```

---

## Success Criteria

### MVP Success (Phase 1-2)
- ✅ Generate valid loop routes in < 3 seconds
- ✅ Include 2+ relevant POIs per route
- ✅ Distance accuracy within ±10%
- ✅ Zero cost operation under 10k requests/month
- ✅ API uptime > 99%

### Product-Market Fit (Phase 3-5)
- 1,000+ monthly active users
- Average 5+ routes generated per user
- Positive user feedback (NPS > 30)
- Retention rate > 40% (users return within 30 days)

### Scale Readiness (Future)
- Support 100k+ routes/month
- Multi-city coverage (20+ cities)
- Response time < 2 seconds at p95
- Cost per route < $0.01

---

## Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| **Mapbox free tier exhausted** | High | Medium | Aggressive caching, route pre-generation |
| **Poor route quality** | High | Medium | Iterative algorithm improvement, user feedback |
| **Overpass API downtime** | Medium | Low | Local POI database, redundant API instances |
| **Slow database queries** | Medium | Medium | Proper indexing, query optimization |
| **Insufficient POI data** | Medium | High | Multiple data sources, user contributions |
| **Scaling costs** | High | High | Monitor usage closely, optimize early |

---

## Conclusion

This architecture provides a solid foundation for a route discovery API that:

1. **Starts simple** with a budget-friendly MVP
2. **Scales incrementally** as user base grows
3. **Leverages free/open-source** services to minimize costs
4. **Uses Rust** for performance and reliability
5. **Provides clear extensibility** for future features

The phased approach allows for rapid iteration based on user feedback while maintaining technical quality and cost efficiency.

---

## Appendix

### Useful Resources

**Rust & Web Development:**
- [Axum Documentation](https://docs.rs/axum/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [SQLx Documentation](https://docs.rs/sqlx/)

**Geospatial:**
- [PostGIS Documentation](https://postgis.net/docs/)
- [Geo Crate](https://docs.rs/geo/)
- [GeoJSON Spec](https://geojson.org/)

**External APIs:**
- [Mapbox Directions API](https://docs.mapbox.com/api/navigation/directions/)
- [Overpass API](https://wiki.openstreetmap.org/wiki/Overpass_API)
- [OpenStreetMap Wiki](https://wiki.openstreetmap.org/)

**Architecture:**
- [12 Factor App](https://12factor.net/)
- [API Design Best Practices](https://swagger.io/resources/articles/best-practices-in-api-design/)

### Contact & Contribution

**Project Owner:** [Your Name]  
**Repository:** [GitHub URL]  
**Documentation:** [Docs URL]  
**License:** [License Type]

---

*Document Version: 1.0*  
*Last Updated: November 2, 2025*
