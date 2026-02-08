# CLAUDE-SETUP.md

Development setup, commands, and operational procedures for EasyRoute.

## Running the Application

```bash
# Start dependencies (PostgreSQL + Redis)
docker-compose up -d postgres redis

# Run the API server
cargo run

# Run in development with auto-reload (requires cargo-watch)
cargo watch -x run

# Stop services
docker-compose down
```

## Testing

### Quick Reference

```bash
# Fast: Unit tests + mocked APIs (~10-20s)
SKIP_REAL_API_TESTS=true cargo test

# Full: All tests with real API calls (~60-120s)
cargo test

# Run integration tests only
cargo test --test '*'

# Run specific test
cargo test test_waypoint_selection

# Run with debug logging
RUST_LOG=debug cargo test

# Run database tests (requires PostgreSQL running)
cargo test --test database_tests
```

### Database Test Isolation

Tests use a separate database (`easyroute_test`) to avoid interfering with development data.

**First-time setup:**
```bash
# Restart docker-compose to create test database
docker-compose down
docker-compose up -d

# Verify test database exists
docker-compose exec postgres psql -U easyroute_user -l | grep easyroute_test
```

**Manual test database reset:**
```bash
# Drop and recreate test database
docker-compose exec postgres psql -U easyroute_user -c "DROP DATABASE IF EXISTS easyroute_test;"
docker-compose exec postgres psql -U easyroute_user -c "CREATE DATABASE easyroute_test;"
docker-compose exec postgres psql -U easyroute_user -d easyroute_test -c "CREATE EXTENSION IF NOT EXISTS postgis;"
```

**How it works:**
- Tests connect to `easyroute_test` database (via `TEST_DATABASE_URL` env variable)
- Dev database (`easyroute`) remains untouched during test runs
- Init script (`docker/init-test-db.sh`) auto-creates test database on container startup
- Tests truncate data after each run (test database only)

### Test Coverage (939 lines)

- **api_tests.rs** - Endpoint validation, request/response serialization
- **database_tests.rs** - PostGIS spatial queries, migration validation
- **mapbox_tests.rs** - Mapbox API client integration
- **overpass_tests.rs** - Overpass API with retry logic
- **route_generation_tests.rs** - End-to-end route generation
- **common/mod.rs** - Test utilities (serial execution, DB setup/cleanup)

### Testing Best Practices

- Use `SKIP_REAL_API_TESTS=true` for fast iteration during development
- Database tests run serially to avoid conflicts (using `serial_test` crate)
- Test fixtures in `tests/common/mod.rs` for reusable setup
- Target: Test suite completes in <60 seconds for AI-friendly iteration

## Database Operations

### Migrations

```bash
# Run all pending migrations (also auto-runs on app startup)
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Create new migration
sqlx migrate add <migration_name>
```

### Database Schema

The application uses PostgreSQL with PostGIS extension. Key tables:

- **`pois`** - Points of interest with spatial indexing
- **`cached_routes`** - Optional route storage for analytics
- **`osm2pgsql_properties`** - OSM import state for incremental updates

Migrations are auto-applied on startup via SQLx.

## Build & Deployment

### Local Development

```bash
# Check code without building (fast)
cargo check

# Build debug version
cargo build

# Build optimized release version
cargo build --release

# Format code (auto-fix)
cargo fmt

# Run linter
cargo clippy

# Run linter with warnings as errors
cargo clippy -- -D warnings

# Verify reproducible builds
cargo build --locked
```

### Build Reproducibility

- **Rust toolchain**: Pinned to `stable` via `rust-toolchain.toml`
- **Dependencies**: `Cargo.lock` committed to repository
- **Docker images**: Specific versions (PostgreSQL 18, PostGIS 3.6, Redis 8.4)

This ensures consistent builds across development, CI, and production environments.

## OSM Data Import

The application uses OpenStreetMap data for POIs. You can use the local database or fall back to the Overpass API.

### Quick Start (Monaco - Small Dataset)

```bash
# Download Monaco extract (~2MB, ~500 POIs)
./osm/download_osm.sh monaco

# Import into PostgreSQL
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf
```

### Production Setup (France)

```bash
# Download France extract (~3.8GB, ~800k POIs)
./osm/download_osm.sh europe/france

# Import (takes 10-30 minutes depending on hardware)
./osm/import_osm.sh ./osm/data/france-latest.osm.pbf

# Verify import
psql $DATABASE_URL -c "SELECT COUNT(*) FROM pois;"
```

### Weekly Updates (Automated)

```bash
# Update existing OSM data (incremental)
./osm/update_osm.sh france

# Recommended: Set up cron job for weekly updates
# 0 3 * * 0 /path/to/easyroute/osm/update_osm.sh france
```

### Regional Extracts Available

The download script supports any Geofabrik extract:

- `monaco` - Tiny test dataset
- `europe/france` - Full France
- `europe/france/ile-de-france` - Paris region
- `europe/france/bretagne` - Brittany
- `europe/france/pays-de-la-loire` - Western France
- See https://download.geofabrik.de/ for full list

### Import System Details

**Technology**: Uses `osm2pgsql` with custom Lua style (`osm/osm_poi_style.lua`)

**POI Extraction**:
- 27 category mappings (Monument, Viewpoint, Park, Museum, etc.)
- Popularity scoring based on OSM tags (Wikipedia, UNESCO, etc.)
- Automatic deduplication by OSM ID
- GIST spatial index for fast queries

**Performance**:
- Monaco: < 1 minute
- Île-de-France: ~5 minutes
- Full France: 10-30 minutes

**See**: `osm/QUICKSTART.md` and `osm/README.md` for detailed instructions

## Environment Configuration

### Required Environment Variables

Create `.env` file in project root:

```bash
# Server Configuration
HOST=0.0.0.0
PORT=3000

# Database (required)
DATABASE_URL=postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute

# Redis (optional - graceful degradation if unavailable)
REDIS_URL=redis://localhost:6379

# External APIs (required for route generation)
MAPBOX_API_KEY=your_mapbox_api_key_here

# Logging
RUST_LOG=info,easyroute=debug

# Cache TTLs (optional - defaults shown)
ROUTE_CACHE_TTL=86400         # 24 hours
POI_REGION_CACHE_TTL=604800   # 7 days

# POI Snapping (optional)
SNAP_RADIUS_M=100.0           # Default: 100m, Range: 0-1000m
```

### Getting API Keys

**Mapbox Directions API**:
1. Sign up at https://mapbox.com
2. Create access token with `directions:read` scope
3. Free tier: 100,000 requests/month

## Docker Services

The `docker-compose.yml` defines three services:

### PostgreSQL + PostGIS

```yaml
postgis/postgis:18-3.6
Ports: 5432:5432
Volumes: postgres_data:/var/lib/postgresql/data
```

### Redis

```yaml
redis:8.4-alpine
Ports: 6379:6379
```

### osm2pgsql (Tool Container)

Used by import scripts for OSM data processing.

### Managing Services

```bash
# Start all services
docker-compose up -d

# Start specific service
docker-compose up -d postgres

# View logs
docker-compose logs -f postgres

# Stop all services
docker-compose down

# Stop and remove volumes (deletes data!)
docker-compose down -v
```

## Health Checks

### Application Health

```bash
curl http://localhost:3000/api/v1/debug/health
```

Returns JSON with:
- Database connectivity status
- PostGIS extension availability
- Redis connectivity (if configured)
- POI count in database

### Service Status

```bash
# Check PostgreSQL
docker-compose ps postgres

# Check Redis
docker-compose ps redis

# Connect to PostgreSQL
psql $DATABASE_URL

# Connect to Redis
redis-cli
```

## Common Development Tasks

### Adding a New Migration

```bash
# Create migration file
sqlx migrate add add_user_favorites

# Edit migrations/XXXXXX_add_user_favorites.sql
# Then run migration
sqlx migrate run
```

### Resetting Database

```bash
# Stop application
# Drop and recreate database
docker-compose down -v
docker-compose up -d postgres

# Migrations will auto-run on next app startup
cargo run
```

### Clearing Redis Cache

```bash
# Connect to Redis
redis-cli

# Flush all caches
FLUSHALL

# Or delete specific keys
KEYS route:loop:*
DEL route:loop:abc123...
```

### Updating Dependencies

```bash
# Check for outdated dependencies
cargo outdated

# Update within semver constraints
cargo update

# Update to latest (edit Cargo.toml first)
cargo update <package_name>

# Rebuild with locked dependencies
cargo build --locked
```

## Troubleshooting

### "Connection refused" errors

- Ensure PostgreSQL is running: `docker-compose ps`
- Check DATABASE_URL matches docker-compose port
- Verify network connectivity: `psql $DATABASE_URL`

### "PostGIS extension not found"

- Ensure you're using `postgis/postgis` image, not plain `postgres`
- Check extension: `psql $DATABASE_URL -c "SELECT PostGIS_version();"`

### Tests failing with Mapbox errors

- Set MAPBOX_API_KEY in `.env`
- Or skip external API tests: `SKIP_REAL_API_TESTS=true cargo test`

### OSM import hangs or fails

- Check disk space (France extract needs ~10GB during import)
- Increase Docker memory limit (Settings > Resources)
- See logs: `docker-compose logs osm2pgsql`

### Slow test execution

- Use `SKIP_REAL_API_TESTS=true` to mock external APIs
- Run specific tests: `cargo test test_name`
- Ensure PostgreSQL has enough resources
