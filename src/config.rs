use crate::constants::*;
use std::env;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScoringStrategy {
    #[default]
    Simple, // Distance-only scoring (original), default until Advanced waypoint scaling fixed
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
            max_route_generation_retries: 5,
            waypoints_count_short: 2,
            waypoints_count_medium: 3,
            waypoints_count_long: 4,
            long_route_threshold_km: 8.0,
            poi_count_threshold_long: 3,
            waypoint_distance_multiplier_2wp: 0.40,
            waypoint_distance_multiplier_3wp: 0.25,
            waypoint_distance_multiplier_4wp: 0.28,
            // POI Scoring defaults - Distance is most important for route length accuracy
            poi_scoring_strategy: ScoringStrategy::default(),
            poi_min_separation_km: 0.3,
            poi_score_weight_distance: 0.6, // Increased from 0.4 - distance is critical
            poi_score_weight_quality: 0.2,  // Decreased from 0.3
            poi_score_weight_angular: 0.1,  // Decreased from 0.15
            poi_score_weight_clustering: 0.05, // Decreased from 0.1
            poi_score_weight_variation: 0.05, // Same
        }
    }
}

impl RouteGeneratorConfig {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        Ok(Self {
            poi_search_radius_multiplier: env::var("ROUTE_POI_SEARCH_RADIUS_MULTIPLIER")
                .unwrap_or_else(|_| defaults.poi_search_radius_multiplier.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SEARCH_RADIUS_MULTIPLIER")?,

            waypoint_distance_multiplier: env::var("ROUTE_WAYPOINT_DISTANCE_MULTIPLIER")
                .unwrap_or_else(|_| defaults.waypoint_distance_multiplier.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINT_DISTANCE_MULTIPLIER")?,

            max_poi_distance_multiplier: env::var("ROUTE_MAX_POI_DISTANCE_MULTIPLIER")
                .unwrap_or_else(|_| defaults.max_poi_distance_multiplier.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_MAX_POI_DISTANCE_MULTIPLIER")?,

            min_poi_distance_km: env::var("ROUTE_MIN_POI_DISTANCE_KM")
                .unwrap_or_else(|_| defaults.min_poi_distance_km.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_MIN_POI_DISTANCE_KM")?,

            tolerance_level_relaxed: env::var("ROUTE_TOLERANCE_LEVEL_RELAXED")
                .unwrap_or_else(|_| defaults.tolerance_level_relaxed.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_TOLERANCE_LEVEL_RELAXED")?,

            tolerance_level_very_relaxed: env::var("ROUTE_TOLERANCE_LEVEL_VERY_RELAXED")
                .unwrap_or_else(|_| defaults.tolerance_level_very_relaxed.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_TOLERANCE_LEVEL_VERY_RELAXED")?,

            default_distance_tolerance_pct: env::var("ROUTE_DEFAULT_DISTANCE_TOLERANCE_PCT")
                .unwrap_or_else(|_| defaults.default_distance_tolerance_pct.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_DEFAULT_DISTANCE_TOLERANCE_PCT")?,

            max_route_generation_retries: env::var("ROUTE_MAX_GENERATION_RETRIES")
                .unwrap_or_else(|_| defaults.max_route_generation_retries.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_MAX_GENERATION_RETRIES")?,

            waypoints_count_short: env::var("ROUTE_WAYPOINTS_COUNT_SHORT")
                .unwrap_or_else(|_| defaults.waypoints_count_short.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINTS_COUNT_SHORT")?,

            waypoints_count_medium: env::var("ROUTE_WAYPOINTS_COUNT_MEDIUM")
                .unwrap_or_else(|_| defaults.waypoints_count_medium.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINTS_COUNT_MEDIUM")?,

            waypoints_count_long: env::var("ROUTE_WAYPOINTS_COUNT_LONG")
                .unwrap_or_else(|_| defaults.waypoints_count_long.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINTS_COUNT_LONG")?,

            long_route_threshold_km: env::var("ROUTE_LONG_ROUTE_THRESHOLD_KM")
                .unwrap_or_else(|_| defaults.long_route_threshold_km.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_LONG_ROUTE_THRESHOLD_KM")?,

            poi_count_threshold_long: env::var("ROUTE_POI_COUNT_THRESHOLD_LONG")
                .unwrap_or_else(|_| defaults.poi_count_threshold_long.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_COUNT_THRESHOLD_LONG")?,

            waypoint_distance_multiplier_2wp: env::var("ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_2WP")
                .unwrap_or_else(|_| defaults.waypoint_distance_multiplier_2wp.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_2WP")?,

            waypoint_distance_multiplier_3wp: env::var("ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_3WP")
                .unwrap_or_else(|_| defaults.waypoint_distance_multiplier_3wp.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_3WP")?,

            waypoint_distance_multiplier_4wp: env::var("ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_4WP")
                .unwrap_or_else(|_| defaults.waypoint_distance_multiplier_4wp.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_WAYPOINT_DISTANCE_MULTIPLIER_4WP")?,

            // POI Scoring configuration
            poi_scoring_strategy: env::var("ROUTE_POI_SCORING_STRATEGY")
                .unwrap_or_else(|_| "simple".to_string())
                .parse()?,

            poi_min_separation_km: env::var("ROUTE_POI_MIN_SEPARATION_KM")
                .unwrap_or_else(|_| defaults.poi_min_separation_km.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_MIN_SEPARATION_KM")?,

            poi_score_weight_distance: env::var("ROUTE_POI_SCORE_WEIGHT_DISTANCE")
                .unwrap_or_else(|_| defaults.poi_score_weight_distance.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SCORE_WEIGHT_DISTANCE")?,

            poi_score_weight_quality: env::var("ROUTE_POI_SCORE_WEIGHT_QUALITY")
                .unwrap_or_else(|_| defaults.poi_score_weight_quality.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SCORE_WEIGHT_QUALITY")?,

            poi_score_weight_angular: env::var("ROUTE_POI_SCORE_WEIGHT_ANGULAR")
                .unwrap_or_else(|_| defaults.poi_score_weight_angular.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SCORE_WEIGHT_ANGULAR")?,

            poi_score_weight_clustering: env::var("ROUTE_POI_SCORE_WEIGHT_CLUSTERING")
                .unwrap_or_else(|_| defaults.poi_score_weight_clustering.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SCORE_WEIGHT_CLUSTERING")?,

            poi_score_weight_variation: env::var("ROUTE_POI_SCORE_WEIGHT_VARIATION")
                .unwrap_or_else(|_| defaults.poi_score_weight_variation.to_string())
                .parse()
                .map_err(|_| "Invalid ROUTE_POI_SCORE_WEIGHT_VARIATION")?,
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
            route_generator: RouteGeneratorConfig::from_env()?,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
