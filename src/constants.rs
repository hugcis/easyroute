// Application-wide constants
// This module centralizes all magic numbers and hardcoded values for better maintainability

// Server defaults
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_PORT: &str = "3000";

// Cache TTL values (in seconds)
pub const DEFAULT_ROUTE_CACHE_TTL_SECONDS: u64 = 86_400; // 24 hours
pub const DEFAULT_POI_REGION_CACHE_TTL_SECONDS: u64 = 604_800; // 7 days

// POI and route configuration
pub const DEFAULT_SNAP_RADIUS_METERS: f64 = 100.0;
pub const MIN_POI_DISTANCE_KM: f64 = 0.2; // Minimum distance from start for POI selection
pub const MAX_DISTANCE_RATIO: f64 = 1.5; // Maximum ratio for POI distance from start

// Route generation retry configuration
pub const MAX_ROUTE_GENERATION_RETRIES: usize = 5;
pub const MIN_ALTERNATIVES_FOR_SUCCESS: u32 = 3; // Generate at least 3 alternatives
pub const MAX_ALTERNATIVES_CLAMP: u32 = 5;

// Adaptive tolerance for sparse POI areas
pub const TOLERANCE_LEVEL_RELAXED: f64 = 0.2; // ±20%
pub const TOLERANCE_LEVEL_VERY_RELAXED: f64 = 0.3; // ±30%

// Geometric loop fallback configuration
pub const GEOMETRIC_LOOP_NUM_WAYPOINTS: usize = 6; // Number of points around the circle
pub const GEOMETRIC_LOOP_RADIUS_DIVISOR: f64 = std::f64::consts::TAU; // 2*PI for circle circumference

// Distance calculation multipliers
pub const DISTANCE_ADJUSTMENT_INITIAL_MULTIPLIER: f64 = 0.8;
pub const DISTANCE_ADJUSTMENT_INITIAL_STEP: f64 = 0.2;
pub const DISTANCE_ADJUSTMENT_AGGRESSIVE_MULTIPLIER: f64 = 0.6;
pub const DISTANCE_ADJUSTMENT_AGGRESSIVE_STEP: f64 = 0.15;

// Waypoint selection thresholds
pub const WAYPOINTS_LONG_ROUTE_DISTANCE_KM: f64 = 10.0;
pub const WAYPOINTS_MEDIUM_ROUTE_DISTANCE_KM: f64 = 5.0;
pub const WAYPOINTS_LONG_ROUTE_MIN_POIS: usize = 6;
pub const WAYPOINTS_MEDIUM_ROUTE_MIN_POIS: usize = 4;
pub const WAYPOINTS_COUNT_LONG_ROUTE: usize = 3;
pub const WAYPOINTS_COUNT_SHORT_ROUTE: usize = 2;

// Spatial distribution
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_TWO_POIS_RAD: f64 = 1.0; // ~57 degrees
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_THREE_POIS_RAD: f64 = 1.047; // 60 degrees

// Variation calculation magic numbers
pub const VARIATION_MULTIPLIER: usize = 3;
pub const VARIATION_OFFSET_BASE: usize = 11;
pub const VARIATION_MOD: usize = 100;
pub const VARIATION_SCORE_FACTOR: f32 = 0.05;

// Overpass API configuration
pub const OVERPASS_QUERY_TIMEOUT_SECONDS: u64 = 30; // Reduced from 60s to fail faster and use batching
pub const OVERPASS_RETRY_MAX_ATTEMPTS: usize = 2; // Total of 3 attempts (0, 1, 2)
pub const OVERPASS_RETRY_EXTENDED_MAX_ATTEMPTS: usize = 2; // For single queries
pub const OVERPASS_HTTP_TOO_MANY_REQUESTS: u16 = 429;
pub const OVERPASS_HTTP_GATEWAY_TIMEOUT: u16 = 504;
