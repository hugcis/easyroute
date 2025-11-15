use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub mapbox_api_key: String,
    pub route_cache_ttl: u64,
    pub poi_region_cache_ttl: u64,
    pub snap_radius_m: f64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenv::dotenv().ok();

        Ok(Config {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| "Invalid PORT")?,
            database_url: env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set")?,
            redis_url: env::var("REDIS_URL")
                .map_err(|_| "REDIS_URL must be set")?,
            mapbox_api_key: env::var("MAPBOX_API_KEY")
                .map_err(|_| "MAPBOX_API_KEY must be set")?,
            route_cache_ttl: env::var("ROUTE_CACHE_TTL")
                .unwrap_or_else(|_| "86400".to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_CACHE_TTL")?,
            poi_region_cache_ttl: env::var("POI_REGION_CACHE_TTL")
                .unwrap_or_else(|_| "604800".to_string())
                .parse()
                .map_err(|_| "Invalid POI_REGION_CACHE_TTL")?,
            snap_radius_m: env::var("SNAP_RADIUS_M")
                .unwrap_or_else(|_| "100.0".to_string())
                .parse()
                .map_err(|_| "Invalid SNAP_RADIUS_M")?,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
