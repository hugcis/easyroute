# Development Guide

## Initial Setup

1. **Install dependencies:**
   ```bash
   # Install Rust (if not already installed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install SQLx CLI for migrations
   cargo install sqlx-cli --no-default-features --features postgres

   # Optional: Install just for task running
   cargo install just
   ```

2. **Configure environment:**
   ```bash
   # Copy example env file
   cp .env.example .env

   # Edit .env and add your Mapbox API key
   # Get a free key at: https://account.mapbox.com/
   ```

3. **Start services:**
   ```bash
   # Using just (recommended)
   just setup

   # Or manually:
   docker-compose up -d
   sqlx migrate run
   cargo build
   ```

4. **Run the server:**
   ```bash
   just run
   # Or: cargo run
   ```

## Development Workflow

### Running the Server
```bash
# Development mode with auto-reload (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run

# Normal run
cargo run
```

### Testing
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Database Operations
```bash
# Run migrations
sqlx migrate run

# Create new migration
sqlx migrate add create_something

# Revert last migration
sqlx migrate revert
```

### Code Quality
```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run all checks
just check
```

## Testing the API

### Example Request
```bash
curl -X POST http://localhost:3000/api/v1/routes/loop \
  -H "Content-Type: application/json" \
  -d @examples/test_request.json
```

### With httpie
```bash
http POST localhost:3000/api/v1/routes/loop < examples/test_request.json
```

## Project Structure

```
easyroute/
├── src/
│   ├── main.rs              # Application entry point
│   ├── config.rs            # Configuration management
│   ├── error.rs             # Error types
│   ├── models/              # Data models
│   ├── routes/              # API route handlers
│   ├── services/            # Business logic
│   │   ├── mapbox.rs        # Mapbox API client
│   │   ├── overpass.rs      # Overpass API client
│   │   ├── poi_service.rs   # POI management
│   │   └── route_generator.rs # Core route generation
│   ├── db/                  # Database layer
│   └── cache/               # Caching (Phase 2)
├── migrations/              # Database migrations
├── tests/                   # Integration tests
└── examples/                # Example requests
```

## Common Issues

### Database Connection Failed
```bash
# Make sure PostgreSQL is running
docker-compose ps

# Check connection
psql postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute
```

### Mapbox API Errors
- Ensure your `MAPBOX_API_KEY` is set in `.env`
- Check you haven't exceeded the free tier limit (100k requests/month)
- Verify the API key is valid at https://account.mapbox.com/

### No POIs Found
- First run will query Overpass API (may be slow)
- Subsequent runs use the database cache
- Try a different location or larger radius if no POIs found

## Phase 1 MVP Checklist

- [x] Project structure set up
- [x] Database migrations created
- [x] Data models defined
- [x] Mapbox client implemented
- [x] Overpass client implemented
- [x] POI service created
- [x] Route generator with waypoint selection
- [x] Route scoring function
- [x] POST /routes/loop endpoint
- [x] Axum server with CORS and tracing
- [ ] Unit tests
- [ ] Integration tests
- [ ] Manual end-to-end testing
- [ ] Performance validation (<3s response time)

## Next Steps (Phase 2)

- [ ] Redis caching layer
- [ ] Multiple route alternatives
- [ ] Improved scoring algorithm
- [ ] GET /routes/{id} endpoint
- [ ] API documentation (Swagger/OpenAPI)
- [ ] Performance optimization
