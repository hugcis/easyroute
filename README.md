# EasyRoute - Route Discovery API

A REST API service that generates personalized walking and biking routes with points of interest (POIs).

## Quick Start

### Prerequisites
- Rust 1.70+
- Docker & Docker Compose
- Mapbox API key (free tier: 100k requests/month)

### Setup

1. **Clone and configure**
   ```bash
   cp .env.example .env
   # Edit .env and add your MAPBOX_API_KEY
   ```

2. **Start dependencies**
   ```bash
   docker-compose up -d
   ```

3. **Run migrations**
   ```bash
   cargo install sqlx-cli --no-default-features --features postgres
   sqlx migrate run
   ```

4. **Run the server**
   ```bash
   cargo run
   ```

The API will be available at `http://localhost:3000`

## Development

### Running Tests
```bash
cargo test
```

### Database Migrations
```bash
# Create new migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

## API Documentation

See [route-api-architecture.md](./route-api-architecture.md) for detailed architecture and API specifications.

## License

MIT
