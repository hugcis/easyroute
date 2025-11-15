# Justfile for EasyRoute project commands
# Install just: cargo install just

# List all available commands
default:
    @just --list

# Start database and Redis containers
services-up:
    docker-compose up -d
    @echo "Waiting for services to be ready..."
    sleep 3

# Stop database and Redis containers
services-down:
    docker-compose down

# Run database migrations
migrate:
    sqlx migrate run

# Create a new migration
migrate-create name:
    sqlx migrate add {{name}}

# Revert last migration
migrate-revert:
    sqlx migrate revert

# Build the project
build:
    cargo build

# Run the server (requires services to be running)
run: services-up
    cargo run

# Run tests
test:
    cargo test

# Run tests with logging
test-verbose:
    RUST_LOG=debug cargo test -- --nocapture

# Format code
fmt:
    cargo fmt

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean

# Full setup: start services, run migrations, build
setup: services-up
    @echo "Installing SQLx CLI..."
    cargo install sqlx-cli --no-default-features --features postgres
    @echo "Running migrations..."
    sqlx migrate run
    @echo "Building project..."
    cargo build
    @echo "Setup complete! Run 'just run' to start the server"

# Run a complete check (lint, test, build)
check: fmt lint test build
    @echo "All checks passed!"
