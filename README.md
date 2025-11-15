# EasyRoute 🗺️

A high-performance REST API service built in Rust that generates personalized walking and biking routes with points of interest (POIs). Discover new places, explore hidden gems, and create unique journeys that match your preferences and desired distance.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **🔄 Loop Routes**: Start and end at the same point with a circular route
- **🎯 Point-to-Point Routes**: Navigate from point A to B with interesting stops
- **🏛️ Rich POI Integration**: Monuments, viewpoints, parks, museums, and more
- **⚡ Smart Caching**: Redis + PostgreSQL for optimal performance and cost efficiency
- **🚶 Multi-Modal**: Support for both walking and cycling routes
- **🎨 Customizable Preferences**: Filter by POI categories, discover hidden gems
- **📊 Route Alternatives**: Get multiple route options with quality scores
- **🗺️ PostGIS Integration**: Advanced geospatial queries for accurate routing

## Technology Stack

- **Language**: Rust 1.70+ (2021 Edition)
- **Web Framework**: Axum 0.8 (built on Tokio async runtime)
- **Database**: PostgreSQL 15+ with PostGIS 3.3+ extension
- **Cache**: Redis 7+ with async connection management
- **Geospatial**: PostGIS for spatial queries, geo & geojson crates
- **HTTP Client**: Reqwest for external API integration
- **Observability**: Tracing with structured logging

### External Services

- **Mapbox Directions API**: Turn-by-turn routing (100k free requests/month)
- **OpenStreetMap Overpass API**: POI data source (free, aggressively cached)

## Quick Start

### Prerequisites

- Rust 1.70 or later ([Install Rust](https://rustup.rs/))
- Docker & Docker Compose
- Mapbox API key ([Get free key](https://account.mapbox.com/))

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/yourusername/easyroute.git
   cd easyroute
   ```

2. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env and add your MAPBOX_API_KEY
   ```

3. **Start dependencies (PostgreSQL + Redis)**
   ```bash
   docker-compose up -d
   ```

4. **Install SQLx CLI and run migrations**
   ```bash
   cargo install sqlx-cli --no-default-features --features postgres
   sqlx migrate run
   ```

5. **Run the server**
   ```bash
   cargo run
   ```

The API will be available at `http://localhost:3000` 🚀

## Usage Examples

### Generate a Loop Route

```bash
curl -X POST http://localhost:3000/routes/loop \
  -H "Content-Type: application/json" \
  -d '{
    "start_lat": 48.8566,
    "start_lng": 2.3522,
    "target_distance_km": 5.0,
    "mode": "walking",
    "preferences": {
      "poi_categories": ["monument", "viewpoint", "park"],
      "hidden_gems": true,
      "max_alternatives": 3
    }
  }'
```

### Generate Point-to-Point Route

```bash
curl -X POST http://localhost:3000/routes/point-to-point \
  -H "Content-Type: application/json" \
  -d '{
    "start_lat": 48.8566,
    "start_lng": 2.3522,
    "end_lat": 48.8606,
    "end_lng": 2.3376,
    "mode": "cycling",
    "preferences": {
      "poi_categories": ["museum", "monument"]
    }
  }'
```

### Query POIs in Area

```bash
curl "http://localhost:3000/pois?lat=48.8566&lng=2.3522&radius_km=2&categories=monument,viewpoint"
```

## Architecture Overview

```
User Request → Axum Router → Route Generator Service
                                      ↓
                         POI Service ← Redis Cache Check
                                      ↓
                         PostGIS Spatial Query (PostgreSQL)
                                      ↓
                         Waypoint Selection Algorithm
                                      ↓
                         Mapbox API → Generate Route Geometry
                                      ↓
                         Score & Rank → Return Alternatives
```

### Key Components

- **Route Generator** (`src/services/route_generator.rs`): Core waypoint selection algorithm
- **POI Service** (`src/services/poi_service.rs`): POI discovery with multi-tier caching
- **Mapbox Client** (`src/services/mapbox.rs`): Route geometry generation
- **PostGIS Queries** (`src/db/queries.rs`): Spatial database operations

For detailed architecture, see [route-api-architecture.md](./route-api-architecture.md).

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test '*'

# Run with logging
RUST_LOG=debug cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint with Clippy
cargo clippy

# Check without building
cargo check
```

### Database Operations

```bash
# Create new migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

### Development Mode

```bash
# Auto-reload on changes (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run
```

## Documentation

- **[Route API Architecture](./route-api-architecture.md)**: Detailed system design and API specs
- **[Development Guide](./DEVELOPMENT.md)**: Development workflow and best practices
- **[POI Categories](./POI_CATEGORIES.md)**: Available POI types and filters
- **[Testing Guide](./TESTING.md)**: Test strategy and running tests
- **[Claude Code Guide](./CLAUDE.md)**: AI assistant development guidelines

## Performance & Cost

### Performance Targets

- **Response Time**: < 3 seconds for route generation
- **Distance Accuracy**: Within ±10% of target distance
- **POI Density**: 2+ relevant POIs per route
- **Cache Hit Rate**: > 50%

### Cost Optimization

EasyRoute is designed to operate at **zero cost** under 100k requests/month:

- ✅ Stays within Mapbox free tier (100k requests/month)
- ✅ Uses free Overpass API with aggressive caching
- ✅ Self-hosted PostgreSQL and Redis
- 📊 3-tier caching reduces external API calls by 80-90%

## Project Status

**Current Phase**: Phase 1 - MVP Development

- [x] Core route generation algorithm
- [x] PostGIS spatial queries
- [x] Mapbox integration
- [x] Basic caching (Redis)
- [ ] Loop route API endpoint
- [ ] Point-to-point routing
- [ ] Route scoring and alternatives
- [ ] Preference filters

### Roadmap

- **Phase 2** (Weeks 5-7): Performance optimization, enhanced caching
- **Phase 3** (Weeks 8-10): User preferences, bike support, API documentation
- **Phase 4** (Weeks 11-12): Point-to-point routes with POIs
- **Phase 5** (Weeks 13-16): User accounts, saved routes, personalization

## Contributing

Contributions are welcome! Please follow these guidelines:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests (`cargo test`)
4. Format code (`cargo fmt`)
5. Run Clippy (`cargo clippy`)
6. Commit changes (`git commit -m 'Add amazing feature'`)
7. Push to branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

- Follow Rust conventions and idioms
- Use meaningful variable names
- Add doc comments for public APIs
- Write tests for new features
- Keep functions focused and composable

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- **OpenStreetMap Contributors**: POI data source
- **Mapbox**: Routing engine
- **PostGIS Community**: Geospatial database capabilities
- **Rust Community**: Amazing ecosystem and tooling

## Support

For questions, issues, or feature requests:
- Open an [issue](https://github.com/yourusername/easyroute/issues)
- Check existing [documentation](./route-api-architecture.md)
- Review [development guide](./DEVELOPMENT.md)

---

Built with ❤️ in Rust
