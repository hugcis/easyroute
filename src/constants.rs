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

// Distance-stratified candidate selection (for long routes in dense areas)
/// Number of concentric distance rings for stratified selection
pub const STRATIFIED_RING_COUNT: usize = 4;
/// Max ring distance as fraction of target distance
pub const STRATIFIED_MAX_RING_DISTANCE_FACTOR: f64 = 0.6;
