# Testing Guide

## Test Structure

The project has comprehensive test coverage including:

- **Unit tests**: Located in `src/` alongside the code (`#[cfg(test)] mod tests`)
- **Integration tests**: Located in `tests/` directory

## Running Tests

### Run All Tests
```bash
cargo test
```

### Run Unit Tests Only
```bash
cargo test --lib
```

### Run Integration Tests
```bash
# Run all integration tests
cargo test --test '*'

# Run specific integration test file
cargo test --test database_tests
cargo test --test mapbox_tests
cargo test --test api_tests
```

### Run Tests with Output
```bash
cargo test -- --nocapture
```

### Run Tests Sequentially (for database tests)
```bash
cargo test --test database_tests -- --test-threads=1
```

## Integration Test Categories

### 1. Database Tests (`database_tests.rs`)
Tests PostgreSQL + PostGIS integration:
- Inserting and querying POIs
- Spatial queries with radius search
- Category filtering
- Distance ordering
- Duplicate OSM ID handling

**Requirements:**
- PostgreSQL with PostGIS running
- Database migrations applied

```bash
docker-compose up -d postgres
sqlx migrate run
cargo test --test database_tests
```

### 2. Mapbox API Tests (`mapbox_tests.rs`)
Tests Mapbox Directions API integration:
- Walking directions
- Biking directions
- Loop routes
- Coordinate validation
- Response conversions

**Requirements:**
- Valid `MAPBOX_API_KEY` environment variable
- Internet connection

**Skip real API tests:**
```bash
SKIP_REAL_API_TESTS=1 cargo test --test mapbox_tests
```

### 3. Overpass API Tests (`overpass_tests.rs`)
Tests OpenStreetMap/Overpass API integration:
- POI queries by location
- Multiple category queries
- OSM ID extraction
- Coordinate validity

**Requirements:**
- Internet connection
- Overpass API availability

**Skip real API tests:**
```bash
SKIP_REAL_API_TESTS=1 cargo test --test overpass_tests
```

### 4. Route Generation Tests (`route_generation_tests.rs`)
End-to-end tests for route generation:
- Loop route creation
- Distance validation
- POI ordering
- Route scoring with preferences

**Requirements:**
- PostgreSQL running
- Valid Mapbox API key
- Internet connection

### 5. API Endpoint Tests (`api_tests.rs`)
Tests HTTP API endpoints:
- Health check endpoint
- Request validation
- Request/response serialization
- Default values

**Requirements:**
- PostgreSQL running

## Test Data

### Test Database
Integration tests use the same database as development but clean up after each test:

```rust
let pool = common::setup_test_db().await;
common::cleanup_test_db(&pool).await; // Cleans up after test
```

### Test POIs
Helper function to create test POIs:

```rust
let poi = common::create_test_poi(
    "Test POI",
    PoiCategory::Monument,
    48.8566, // lat
    2.3522,  // lng
);
```

## Environment Variables

### Required for Full Test Suite
```bash
# Database
DATABASE_URL=postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute

# Mapbox API (for API tests)
MAPBOX_API_KEY=pk.eyJ...your_key_here

# Optional: Skip real API tests
SKIP_REAL_API_TESTS=1
```

### .env.test Example
Create a `.env.test` file:

```bash
DATABASE_URL=postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute
MAPBOX_API_KEY=pk.your_test_key_here
RUST_LOG=debug
```

Then load it:
```bash
source .env.test
cargo test
```

## Running Tests in CI/CD

### GitHub Actions Example
```yaml
- name: Setup database
  run: |
    docker-compose up -d postgres
    sleep 5
    sqlx migrate run

- name: Run unit tests
  run: cargo test --lib

- name: Run integration tests (no real APIs)
  run: SKIP_REAL_API_TESTS=1 cargo test --test '*'
  env:
    DATABASE_URL: postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute
```

## Test Coverage Summary

### Unit Tests (13 tests)
- ✅ Coordinate validation and distance calculations
- ✅ POI category parsing and quality scores
- ✅ Route request validation
- ✅ Transport mode conversions
- ✅ Mapbox response conversions
- ✅ Overpass query building
- ✅ Route scoring logic

### Integration Tests

**Database (4 tests):**
- ✅ Insert and find POIs
- ✅ Find POIs by category
- ✅ Spatial distance ordering
- ✅ Duplicate OSM ID handling

**Mapbox API (5 tests):**
- Walking directions
- Loop routes
- Bike mode
- Invalid coordinates
- Response conversions

**Overpass API (4 tests):**
- Query POIs
- Multiple categories
- OSM ID extraction
- Coordinate validity

**Route Generation (4 tests):**
- Route generation with database POIs
- Distance validation
- POI ordering
- Route scoring with preferences

**API Endpoints (6 tests):**
- Health check
- Request validation
- Request deserialization
- Default values
- Coordinates validation
- Preferences serialization

## Troubleshooting

### "Database connection failed"
```bash
# Make sure PostgreSQL is running
docker-compose ps
docker-compose up -d postgres

# Check connection
psql postgres://easyroute_user:easyroute_pass@localhost:5432/easyroute
```

### "Mapbox API error"
```bash
# Check your API key is set
echo $MAPBOX_API_KEY

# Or skip real API tests
SKIP_REAL_API_TESTS=1 cargo test
```

### "Tests fail randomly"
Database tests might conflict if run in parallel. Run sequentially:
```bash
cargo test --test database_tests -- --test-threads=1
```

### "Migration version mismatch"
Reset the database:
```bash
docker-compose down -v
docker-compose up -d
sqlx migrate run
```

## Writing New Tests

### Unit Test Template
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Arrange
        let input = ...;

        // Act
        let result = function(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Integration Test Template
```rust
mod common;

#[tokio::test]
async fn test_async_operation() {
    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Test code here

    common::cleanup_test_db(&pool).await;
}
```

## Best Practices

1. **Always clean up**: Call `cleanup_test_db()` after database tests
2. **Use --test-threads=1**: For database tests that might conflict
3. **Skip real APIs in CI**: Use `SKIP_REAL_API_TESTS=1`
4. **Descriptive assertions**: Include error messages
5. **Test isolation**: Each test should be independent

## Performance

Typical test run times:
- Unit tests: < 1 second
- Database tests: ~2 seconds
- API tests (with real APIs): ~10-30 seconds
- API tests (mocked): ~1 second

## Future Improvements

- [ ] Add mock servers for Mapbox/Overpass APIs
- [ ] Add load testing
- [ ] Add benchmark tests
- [ ] Increase test coverage to >80%
- [ ] Add property-based testing
- [ ] Add E2E tests with real HTTP requests
