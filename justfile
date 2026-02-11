# Justfile for EasyRoute
# Install just: cargo install just

set dotenv-load

# List all available commands
[private]
default:
    @just --list

# ─── Environment ──────────────────────────────────────────

# Copy .env.example to .env if missing
[group('setup')]
env:
    @[ -f .env ] && echo ".env already exists" || (cp .env.example .env && echo "Created .env from .env.example")

# Private helper: ensure .env exists before running
[private]
_ensure-env:
    @[ -f .env ] || (cp .env.example .env && echo "Created .env from .env.example — edit it with your MAPBOX_API_KEY")

# Full setup: install tools, start services, run migrations, build
[group('setup')]
setup: services-up
    @echo "Installing cargo-watch..."
    cargo install cargo-watch
    @echo "Installing SQLx CLI..."
    cargo install sqlx-cli --no-default-features --features postgres
    @just env
    @echo "Running migrations..."
    sqlx migrate run
    @echo "Building project..."
    cargo build
    @echo "Setup complete! Run 'just serve' to start the server with auto-reload"

# ─── Services ─────────────────────────────────────────────

# Start Postgres and Redis containers
[group('services')]
services-up:
    docker-compose up -d
    @echo "Waiting for services to be ready..."
    sleep 3

# Stop Postgres and Redis containers
[group('services')]
services-down:
    docker-compose down

# Flush the Redis route/POI cache
[group('services')]
flush-cache:
    docker exec easyroute_redis redis-cli FLUSHDB
    @echo "Redis cache flushed."

# ─── Build & Run ──────────────────────────────────────────

# Build the project (debug)
[group('dev')]
build:
    cargo build

# Run the server
[group('dev')]
run: _ensure-env services-up
    cargo run --bin easyroute

# Start server with auto-reload + open visualizer
[group('dev')]
serve: _ensure-env services-up
    @echo "Starting server with auto-reload..."
    @echo "Opening visualizer in browser..."
    @sleep 1 && open scripts/visualize.html &
    cargo watch -x 'run --bin easyroute'

# Run cargo-watch for fast compilation feedback
[group('dev')]
watch:
    cargo watch -x check

# Quick compile check (no codegen)
[group('dev')]
check-compile:
    cargo check

# Open the route visualizer in the browser
[group('dev')]
open:
    open scripts/visualize.html

# ─── Testing ──────────────────────────────────────────────

# Run all tests
[group('test')]
test:
    cargo test

# Run tests skipping Mapbox API calls
[group('test')]
test-fast:
    SKIP_REAL_API_TESTS=true cargo test

# Run tests with RUST_LOG=debug output
[group('test')]
test-verbose:
    RUST_LOG=debug cargo test -- --nocapture

# Run a single test by name
[group('test')]
test-one NAME:
    cargo test {{NAME}}

# Run the evaluation harness (e.g. just evaluate --scenario=monaco --runs=5)
[group('test')]
evaluate *ARGS: _ensure-env
    cargo run --bin evaluate -- {{ARGS}}

# ─── Code Quality ─────────────────────────────────────────

# Format code with rustfmt
[group('quality')]
fmt:
    cargo fmt

# Run clippy linter (warnings = errors)
[group('quality')]
lint:
    cargo clippy -- -D warnings

# Run full check: fmt, lint, test, build
[group('quality')]
check: fmt lint test build
    @echo "All checks passed!"

# Remove build artifacts
[group('quality')]
clean:
    cargo clean

# ─── Database ─────────────────────────────────────────────

# Run pending database migrations
[group('database')]
migrate: _ensure-env
    sqlx migrate run

# Create a new migration file
[group('database')]
migrate-create name:
    sqlx migrate add {{name}}

# Revert the last migration
[group('database')]
migrate-revert:
    sqlx migrate revert

# Open a psql shell to the database
[group('database')]
db:
    psql "$DATABASE_URL"

# Drop, recreate, and re-migrate the database
[group('database')]
reset-db:
    @echo "Dropping and recreating database..."
    sqlx database drop -y
    sqlx database create
    sqlx migrate run
    @echo "Database reset complete."

# ─── OSM Data ─────────────────────────────────────────────

# Download and import OSM POIs for a region (e.g. just import monaco)
[group('database')]
import REGION:
    ./osm/download_osm.sh {{REGION}}
    ./osm/import_osm.sh ./osm/data/{{REGION}}-latest.osm.pbf

# ─── API Shortcuts ────────────────────────────────────────

# Hit the /health endpoint
[group('api')]
health:
    @curl -s http://localhost:3000/api/v1/debug/health | python3 -m json.tool

# Send the example loop request to the API
[group('api')]
try:
    @curl -s -X POST http://localhost:3000/api/v1/routes/loop \
        -H "Content-Type: application/json" \
        -d @examples/test_request.json | python3 -m json.tool
