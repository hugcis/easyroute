# Justfile for EasyRoute project commands
# Install just: cargo install just

set dotenv-load

# List all available commands
default:
    @just --list

# ─── Environment ──────────────────────────────────────────

# Copy .env.example to .env if missing
env:
    @[ -f .env ] && echo ".env already exists" || (cp .env.example .env && echo "Created .env from .env.example")

# Private helper: ensure .env exists before running
[private]
_ensure-env:
    @[ -f .env ] || (cp .env.example .env && echo "Created .env from .env.example — edit it with your MAPBOX_API_KEY")

# ─── Services ─────────────────────────────────────────────

# Start database and Redis containers
services-up:
    docker-compose up -d
    @echo "Waiting for services to be ready..."
    sleep 3

# Stop database and Redis containers
services-down:
    docker-compose down

# ─── Build & Run ──────────────────────────────────────────

# Build the project
build:
    cargo build

# Run the server (requires services to be running)
run: _ensure-env services-up
    cargo run

# Start server with auto-reload and open visualizer
serve: _ensure-env services-up
    @echo "Starting server with auto-reload..."
    @echo "Opening visualizer in browser..."
    @sleep 1 && open scripts/visualize.html &
    cargo watch -x run

# Run cargo-watch for fast compilation feedback
watch:
    cargo watch -x check

# Open the visualizer HTML in the browser
open:
    open scripts/visualize.html

# Quick compile check
check-compile:
    cargo check

# ─── Testing ──────────────────────────────────────────────

# Run tests
test:
    cargo test

# Run tests skipping real API calls
test-fast:
    SKIP_REAL_API_TESTS=true cargo test

# Run tests with logging
test-verbose:
    RUST_LOG=debug cargo test -- --nocapture

# Run a single test by name
test-one NAME:
    cargo test {{NAME}}

# ─── Code Quality ─────────────────────────────────────────

# Format code
fmt:
    cargo fmt

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Run a complete check (lint, test, build)
check: fmt lint test build
    @echo "All checks passed!"

# Clean build artifacts
clean:
    cargo clean

# ─── Database ─────────────────────────────────────────────

# Run database migrations
migrate: _ensure-env
    sqlx migrate run

# Create a new migration
migrate-create name:
    sqlx migrate add {{name}}

# Revert last migration
migrate-revert:
    sqlx migrate revert

# Open psql connected to the database
db:
    psql "$DATABASE_URL"

# Drop and recreate the database, then run migrations
reset-db:
    @echo "Dropping and recreating database..."
    sqlx database drop -y
    sqlx database create
    sqlx migrate run
    @echo "Database reset complete."

# ─── OSM Data ─────────────────────────────────────────────

# Download and import OSM data for a region (e.g. just import monaco)
import REGION:
    ./osm/download_osm.sh {{REGION}}
    ./osm/import_osm.sh ./osm/data/{{REGION}}-latest.osm.pbf

# ─── API Shortcuts ────────────────────────────────────────

# Curl the health endpoint
health:
    @curl -s http://localhost:3000/api/v1/debug/health | python3 -m json.tool

# Send the example test request to the API
try:
    @curl -s -X POST http://localhost:3000/api/v1/routes/loop \
        -H "Content-Type: application/json" \
        -d @examples/test_request.json | python3 -m json.tool

# ─── Setup ────────────────────────────────────────────────

# Full setup: install tools, start services, run migrations, build
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
