use crate::constants::*;
use std::env;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScoringStrategy {
    Simple, // Distance-only scoring (original)
    #[default]
    Advanced, // Context-aware with quality, clustering, angular diversity
}

impl std::str::FromStr for ScoringStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(ScoringStrategy::Simple),
            "advanced" => Ok(ScoringStrategy::Advanced),
            _ => Err(format!(
                "Invalid scoring strategy: {}. Use 'simple' or 'advanced'",
                s
            )),
        }
    }
}

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
    pub mapbox_base_url: Option<String>,
    pub route_generator: RouteGeneratorConfig,
}

#[derive(Debug, Clone)]
pub struct RouteGeneratorConfig {
    /// Multiplier for POI search radius relative to target distance
    /// For a 5km route with multiplier 0.7, searches within 3.5km radius
    pub poi_search_radius_multiplier: f64,

    /// Multiplier for target waypoint distance from start
    /// For loop geometry, POIs should be ~45% of target distance from start
    pub waypoint_distance_multiplier: f64,

    /// Maximum POI distance as multiplier of target distance
    /// POIs beyond this distance from start are filtered out
    pub max_poi_distance_multiplier: f64,

    /// Minimum distance (km) a POI must be from start point
    pub min_poi_distance_km: f64,

    /// Relaxed tolerance level (as fraction, e.g., 0.3 = ±30%)
    pub tolerance_level_relaxed: f64,

    /// Very relaxed tolerance level (as fraction, e.g., 0.5 = ±50%)
    pub tolerance_level_very_relaxed: f64,

    /// Default distance tolerance as percentage of route distance
    /// e.g., 0.2 = ±20% of target distance
    pub default_distance_tolerance_pct: f64,

    /// Maximum number of retries per route generation attempt
    pub max_route_generation_retries: usize,

    /// Number of waypoints for short routes
    pub waypoints_count_short: usize,

    /// Number of waypoints for medium routes
    pub waypoints_count_medium: usize,

    /// Number of waypoints for long routes
    pub waypoints_count_long: usize,

    /// Distance threshold (km) above which routes are considered "long"
    pub long_route_threshold_km: f64,

    /// POI count threshold for using more waypoints
    pub poi_count_threshold_long: usize,

    /// Distance multiplier for 2-waypoint routes
    pub waypoint_distance_multiplier_2wp: f64,

    /// Distance multiplier for 3-waypoint routes
    pub waypoint_distance_multiplier_3wp: f64,

    /// Distance multiplier for 4-waypoint routes
    pub waypoint_distance_multiplier_4wp: f64,

    // --- POI Scoring Strategy Configuration ---
    /// Strategy for scoring POIs during waypoint selection
    pub poi_scoring_strategy: ScoringStrategy,

    /// Minimum separation distance (km) between selected POIs to avoid clustering
    pub poi_min_separation_km: f64,

    /// Scoring weight for distance from ideal waypoint position (0.0-1.0)
    pub poi_score_weight_distance: f32,

    /// Scoring weight for POI quality/popularity (0.0-1.0)
    pub poi_score_weight_quality: f32,

    /// Scoring weight for angular diversity around start point (0.0-1.0)
    pub poi_score_weight_angular: f32,

    /// Scoring weight penalty for POI clustering (0.0-1.0)
    pub poi_score_weight_clustering: f32,

    /// Scoring weight for variation randomization (0.0-1.0)
    pub poi_score_weight_variation: f32,

    /// Distance threshold (meters) for detecting path overlap (streets walked twice)
    pub metrics_overlap_threshold_m: f64,

    /// Scoring version: 1 = original, 2 = shape-aware (includes circularity/convexity/overlap)
    pub scoring_version: u32,
}

impl Default for RouteGeneratorConfig {
    fn default() -> Self {
        Self {
            poi_search_radius_multiplier: 1.0,
            waypoint_distance_multiplier: 0.35,
            max_poi_distance_multiplier: 0.6,
            min_poi_distance_km: 0.3,
            tolerance_level_relaxed: 0.3,
            tolerance_level_very_relaxed: 0.5,
            default_distance_tolerance_pct: 0.2,
            max_route_generation_retries: 4,
            waypoints_count_short: 2,
            waypoints_count_medium: 3,
            waypoints_count_long: 4,
            long_route_threshold_km: 8.0,
            poi_count_threshold_long: 3,
            waypoint_distance_multiplier_2wp: 0.50,
            waypoint_distance_multiplier_3wp: 0.35,
            waypoint_distance_multiplier_4wp: 0.28,
            // POI Scoring defaults - balanced for Advanced strategy
            poi_scoring_strategy: ScoringStrategy::default(),
            poi_min_separation_km: 0.3,
            poi_score_weight_distance: 0.45, // Still dominant, but leaves room for spatial signals
            poi_score_weight_quality: 0.15,
            poi_score_weight_angular: 0.35, // Key factor for avoiding clustered waypoints
            poi_score_weight_clustering: 0.05,
            poi_score_weight_variation: 0.05,
            metrics_overlap_threshold_m: 25.0,
            scoring_version: 1,
        }
    }
}

/// Parse an environment variable with a default value, returning a descriptive error on failure.
macro_rules! parse_env {
    ($env:literal, $default:expr) => {
        env::var($env)
            .unwrap_or_else(|_| $default.to_string())
            .parse()
            .map_err(|_| concat!("Invalid ", $env))?
    };
}

impl RouteGeneratorConfig {
    pub fn from_env() -> Result<Self, String> {
        let d = Self::default();

        Ok(Self {
            poi_search_radius_multiplier: parse_env!(
                "ROUTE_POI_SEARCH_RADIUS_MULTIPLIER",
                d.poi_search_radius_multiplier
            ),
            waypoint_distance_multiplier: parse_env!(
                "ROUTE_WAYPOINT_DISTANCE_MULTIPLIER",
                d.waypoint_distance_multiplier
            ),
            max_poi_distance_multiplier: parse_env!(
                "ROUTE_MAX_POI_DISTANCE_MULTIPLIER",
                d.max_poi_distance_multiplier
            ),
            min_poi_distance_km: parse_env!("ROUTE_MIN_POI_DISTANCE_KM", d.min_poi_distance_km),
            tolerance_level_relaxed: parse_env!(
                "ROUTE_TOLERANCE_LEVEL_RELAXED",
                d.tolerance_level_relaxed
            ),
            tolerance_level_very_relaxed: parse_env!(
                "ROUTE_TOLERANCE_LEVEL_VERY_RELAXED",
                d.tolerance_level_very_relaxed
            ),
            default_distance_tolerance_pct: parse_env!(
                "ROUTE_DEFAULT_DISTANCE_TOLERANCE_PCT",
                d.default_distance_tolerance_pct
            ),
            max_route_generation_retries: parse_env!(
                "ROUTE_MAX_GENERATION_RETRIES",
                d.max_route_generation_retries
            ),
            waypoints_count_short: parse_env!(
                "ROUTE_WAYPOINTS_COUNT_SHORT",
                d.waypoints_count_short
            ),
            waypoints_count_medium: parse_env!(
                "ROUTE_WAYPOINTS_COUNT_MEDIUM",
                d.waypoints_count_medium
            ),
            waypoints_count_long: parse_env!("ROUTE_WAYPOINTS_COUNT_LONG", d.waypoints_count_long),
            long_route_threshold_km: parse_env!(
                "ROUTE_LONG_ROUTE_THRESHOLD_KM",
                d.long_route_threshold_km
            ),
            poi_count_threshold_long: parse_env!(
                "ROUTE_POI_COUNT_THRESHOLD_LONG",
                d.poi_count_threshold_long
            ),
            waypoint_distance_multiplier_2wp: parse_env!(
                "ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_2WP",
                d.waypoint_distance_multiplier_2wp
            ),
            waypoint_distance_multiplier_3wp: parse_env!(
                "ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_3WP",
                d.waypoint_distance_multiplier_3wp
            ),
            waypoint_distance_multiplier_4wp: parse_env!(
                "ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_4WP",
                d.waypoint_distance_multiplier_4wp
            ),
            poi_scoring_strategy: env::var("ROUTE_POI_SCORING_STRATEGY")
                .unwrap_or_else(|_| "advanced".to_string())
                .parse()?,
            poi_min_separation_km: parse_env!(
                "ROUTE_POI_MIN_SEPARATION_KM",
                d.poi_min_separation_km
            ),
            poi_score_weight_distance: parse_env!(
                "ROUTE_POI_SCORE_WEIGHT_DISTANCE",
                d.poi_score_weight_distance
            ),
            poi_score_weight_quality: parse_env!(
                "ROUTE_POI_SCORE_WEIGHT_QUALITY",
                d.poi_score_weight_quality
            ),
            poi_score_weight_angular: parse_env!(
                "ROUTE_POI_SCORE_WEIGHT_ANGULAR",
                d.poi_score_weight_angular
            ),
            poi_score_weight_clustering: parse_env!(
                "ROUTE_POI_SCORE_WEIGHT_CLUSTERING",
                d.poi_score_weight_clustering
            ),
            poi_score_weight_variation: parse_env!(
                "ROUTE_POI_SCORE_WEIGHT_VARIATION",
                d.poi_score_weight_variation
            ),
            metrics_overlap_threshold_m: parse_env!(
                "ROUTE_METRICS_OVERLAP_THRESHOLD_M",
                d.metrics_overlap_threshold_m
            ),
            scoring_version: parse_env!("ROUTE_SCORING_VERSION", d.scoring_version),
        })
    }
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
            mapbox_base_url: env::var("MAPBOX_BASE_URL").ok(),
            route_generator: RouteGeneratorConfig::from_env()?,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // --- ScoringStrategy::from_str (no env vars needed) ---

    #[test]
    fn scoring_strategy_simple() {
        assert_eq!(
            "simple".parse::<ScoringStrategy>().unwrap(),
            ScoringStrategy::Simple
        );
    }

    #[test]
    fn scoring_strategy_advanced() {
        assert_eq!(
            "advanced".parse::<ScoringStrategy>().unwrap(),
            ScoringStrategy::Advanced
        );
    }

    #[test]
    fn scoring_strategy_case_insensitive() {
        assert_eq!(
            "SIMPLE".parse::<ScoringStrategy>().unwrap(),
            ScoringStrategy::Simple
        );
        assert_eq!(
            "Advanced".parse::<ScoringStrategy>().unwrap(),
            ScoringStrategy::Advanced
        );
    }

    #[test]
    fn scoring_strategy_invalid() {
        assert!("invalid".parse::<ScoringStrategy>().is_err());
    }

    // --- RouteGeneratorConfig defaults ---

    #[test]
    fn route_generator_config_defaults() {
        let d = RouteGeneratorConfig::default();
        assert_eq!(d.poi_search_radius_multiplier, 1.0);
        assert_eq!(d.waypoint_distance_multiplier, 0.35);
        assert_eq!(d.default_distance_tolerance_pct, 0.2);
        assert_eq!(d.max_route_generation_retries, 4);
        assert_eq!(d.waypoints_count_short, 2);
        assert_eq!(d.waypoints_count_medium, 3);
        assert_eq!(d.waypoints_count_long, 4);
        assert_eq!(d.long_route_threshold_km, 8.0);
        assert_eq!(d.scoring_version, 1);
        assert_eq!(d.poi_scoring_strategy, ScoringStrategy::Advanced);
    }

    // --- RouteGeneratorConfig::from_env ---

    #[test]
    #[serial]
    fn route_generator_config_from_env_defaults() {
        // Clear all ROUTE_ env vars to ensure defaults
        let route_keys: Vec<String> = env::vars()
            .filter(|(k, _)| k.starts_with("ROUTE_"))
            .map(|(k, _)| k)
            .collect();
        for key in &route_keys {
            unsafe { env::remove_var(key) };
        }
        let config = RouteGeneratorConfig::from_env().unwrap();
        let d = RouteGeneratorConfig::default();
        assert_eq!(
            config.poi_search_radius_multiplier,
            d.poi_search_radius_multiplier
        );
        assert_eq!(config.scoring_version, d.scoring_version);
    }

    #[test]
    #[serial]
    fn route_generator_config_custom_override() {
        unsafe {
            env::set_var("ROUTE_SCORING_VERSION", "2");
            env::remove_var("ROUTE_POI_SCORING_STRATEGY");
        }
        let config = RouteGeneratorConfig::from_env().unwrap();
        assert_eq!(config.scoring_version, 2);
        unsafe { env::remove_var("ROUTE_SCORING_VERSION") };
    }

    #[test]
    #[serial]
    fn route_generator_config_invalid_value() {
        unsafe { env::set_var("ROUTE_SCORING_VERSION", "not_a_number") };
        let result = RouteGeneratorConfig::from_env();
        assert!(result.is_err());
        unsafe { env::remove_var("ROUTE_SCORING_VERSION") };
    }

    // --- Config::server_address ---

    #[test]
    fn server_address_format() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 8080,
            database_url: String::new(),
            redis_url: None,
            mapbox_api_key: String::new(),
            route_cache_ttl: 0,
            poi_region_cache_ttl: 0,
            snap_radius_m: 100.0,
            mapbox_base_url: None,
            route_generator: RouteGeneratorConfig::default(),
        };
        assert_eq!(config.server_address(), "127.0.0.1:8080");
    }
}
