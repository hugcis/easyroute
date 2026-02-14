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
pub const MIN_ALTERNATIVES_FOR_SUCCESS: u32 = 3; // Generate at least 3 alternatives
pub const MAX_ALTERNATIVES_CLAMP: u32 = 5;

// Waypoint selection thresholds (for medium/long route classification)
pub const WAYPOINTS_LONG_ROUTE_MIN_POIS: usize = 6;
pub const WAYPOINTS_MEDIUM_ROUTE_MIN_POIS: usize = 4;

// Spatial distribution
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_TWO_POIS_RAD: f64 = 1.0; // ~57 degrees
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_THREE_POIS_RAD: f64 = 1.047; // 60 degrees

// Variation calculation magic numbers
pub const VARIATION_MULTIPLIER: usize = 3;
pub const VARIATION_OFFSET_BASE: usize = 11;
pub const VARIATION_MOD: usize = 100;
pub const VARIATION_SCORE_FACTOR: f32 = 0.05;

// Distance correction feedback loop
pub const DISTANCE_CORRECTION_DAMPING: f64 = 0.7;
pub const DISTANCE_CORRECTION_MIN: f64 = 0.5;
pub const DISTANCE_CORRECTION_MAX: f64 = 2.5;
