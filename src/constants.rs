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

// In-memory cache defaults
pub const DEFAULT_MEMORY_CACHE_MAX_ENTRIES: u64 = 1_000;

// Distance correction feedback loop
pub const DISTANCE_CORRECTION_DAMPING: f64 = 0.85;
pub const DISTANCE_CORRECTION_MIN: f64 = 0.5;
pub const DISTANCE_CORRECTION_MAX: f64 = 2.5;

// POI discovery limits
/// Linear scaling factor for short-route POI limit: limit = distance_km × factor
pub const POI_LIMIT_SHORT_ROUTE_FACTOR: f64 = 20.0;
pub const POI_LIMIT_SHORT_ROUTE_MIN: f64 = 50.0;
pub const POI_LIMIT_SHORT_ROUTE_MAX: f64 = 500.0;
/// Area-based density factor for long-route POI limit: limit = π × r² × factor
pub const POI_LIMIT_LONG_ROUTE_DENSITY: f64 = 200.0;
pub const POI_LIMIT_LONG_ROUTE_MIN: f64 = 1_000.0;
pub const POI_LIMIT_LONG_ROUTE_MAX: f64 = 15_000.0;
/// Waypoint distance as fraction of target distance for area calculation
pub const POI_LIMIT_LONG_ROUTE_WP_DIST_FACTOR: f64 = 0.45;

// Candidate selection limits
pub const CANDIDATE_LIMIT_FACTOR: f64 = 10.0;
pub const CANDIDATE_LIMIT_MIN: f64 = 20.0;
pub const CANDIDATE_LIMIT_LONG: f64 = 500.0;
pub const CANDIDATE_LIMIT_MEDIUM: f64 = 300.0;
pub const CANDIDATE_LIMIT_SHORT: f64 = 100.0;
/// Distance threshold for "medium" candidate pool (routes > 5km get more candidates)
pub const CANDIDATE_MEDIUM_THRESHOLD_KM: f64 = 5.0;

// Distance-stratified candidate selection (for long routes in dense areas)
/// Number of concentric distance rings for stratified selection
pub const STRATIFIED_RING_COUNT: usize = 4;
/// Max ring distance as fraction of target distance
pub const STRATIFIED_MAX_RING_DISTANCE_FACTOR: f64 = 0.6;
