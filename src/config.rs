use crate::constants::*;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: Option<String>, // Optional for Phase 1, required in Phase 2
    pub mapbox_api_key: String,
    pub route_cache_ttl: u64,
    pub poi_region_cache_ttl: u64,
    pub snap_radius_m: f64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenv::dotenv().ok();

        // Parse and validate snap_radius_m
        let snap_radius_m: f64 = env::var("SNAP_RADIUS_M")
            .unwrap_or_else(|_| "100.0".to_string())
            .parse()
            .map_err(|_| "Invalid SNAP_RADIUS_M")?;

        if snap_radius_m <= 0.0 || snap_radius_m > 1000.0 {
            return Err("SNAP_RADIUS_M must be between 0 and 1000 meters".to_string());
        }

        Ok(Config {
            host: env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| DEFAULT_PORT.to_string())
                .parse()
                .map_err(|_| "Invalid PORT")?,
            database_url: env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set")?,
            redis_url: env::var("REDIS_URL").ok(), // Optional for now
            mapbox_api_key: env::var("MAPBOX_API_KEY").map_err(|_| "MAPBOX_API_KEY must be set")?,
            route_cache_ttl: env::var("ROUTE_CACHE_TTL")
                .unwrap_or_else(|_| DEFAULT_ROUTE_CACHE_TTL_SECONDS.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_CACHE_TTL")?,
            poi_region_cache_ttl: env::var("POI_REGION_CACHE_TTL")
                .unwrap_or_else(|_| DEFAULT_POI_REGION_CACHE_TTL_SECONDS.to_string())
                .parse()
                .map_err(|_| "Invalid POI_REGION_CACHE_TTL")?,
            snap_radius_m,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
