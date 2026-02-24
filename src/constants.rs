//! Stable application-wide constants.
//!
//! Values here are structural invariants, algorithm coefficients, and default
//! fallbacks for env-var-based configuration. They should rarely change.
//! For quality-critical tuning knobs that benefit from runtime experimentation,
//! see [`RouteGeneratorConfig`](crate::config::RouteGeneratorConfig) instead.

// --- Server defaults (used when HOST / PORT env vars are absent) ---

/// Default bind address for the HTTP server.
pub const DEFAULT_HOST: &str = "0.0.0.0";
/// Default port for the HTTP server.
pub const DEFAULT_PORT: &str = "3000";

// --- Cache TTL defaults (seconds, used when env vars are absent) ---

/// Default route cache TTL: 24 hours. Overridden by `ROUTE_CACHE_TTL`.
pub const DEFAULT_ROUTE_CACHE_TTL_SECONDS: u64 = 86_400;
/// Default POI-region cache TTL: 7 days. Overridden by `POI_REGION_CACHE_TTL`.
pub const DEFAULT_POI_REGION_CACHE_TTL_SECONDS: u64 = 604_800;

// --- Route generation structural limits ---

/// Default snap radius (meters) for associating nearby POIs with a route path.
/// Overridden by `SNAP_RADIUS_M` env var (validated 0..1000).
pub const DEFAULT_SNAP_RADIUS_METERS: f64 = 100.0;
/// Minimum number of alternative routes the generator must produce before
/// returning results. Requests that yield fewer alternatives fall through to
/// the next tolerance level or geometric fallback.
pub const MIN_ALTERNATIVES_FOR_SUCCESS: u32 = 3;
/// Hard upper bound on alternative routes returned, regardless of user request.
pub const MAX_ALTERNATIVES_CLAMP: u32 = 5;

// --- Spatial distribution angle thresholds (radians) ---
// Used by `WaypointSelector::verify_loop_shape()` to reject waypoint
// configurations where POIs are too close together angularly (from the start
// point's perspective). Ensures the route loop fans out in distinct directions.

/// Minimum angular separation (radians, ~57 deg) between 2 waypoints.
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_TWO_POIS_RAD: f64 = 1.0;
/// Minimum angular separation (radians, 60 deg) between any pair of 3 waypoints.
pub const SPATIAL_DISTRIBUTION_MIN_ANGLE_THREE_POIS_RAD: f64 = 1.047;

// --- Variation / pseudo-random scoring coefficients ---
// Used by `WaypointSelector` to introduce deterministic variation across
// alternative route generations so each attempt produces distinct waypoint sets.

/// Multiplied with the attempt seed to spread hash values.
pub const VARIATION_MULTIPLIER: usize = 3;
/// Added to the seed-derived value before modular reduction.
pub const VARIATION_OFFSET_BASE: usize = 11;
/// Modulus for the pseudo-random variation hash.
pub const VARIATION_MOD: usize = 100;
/// Scaling factor converting the variation hash (0..100) to a score bonus (0.0..0.05).
pub const VARIATION_SCORE_FACTOR: f32 = 0.05;

// --- In-memory cache defaults ---

/// Maximum entries for the on-device in-memory route cache (LRU eviction).
pub const DEFAULT_MEMORY_CACHE_MAX_ENTRIES: u64 = 1_000;

// --- Distance correction feedback loop ---
// After each Mapbox route response, the generator adjusts the waypoint distance
// multiplier based on how far the actual route distance was from the target.
// These coefficients control the feedback loop's aggressiveness and bounds.

/// Damping factor applied to each distance correction step.
/// 0.85 means the correction is 85% of the raw error, preventing oscillation.
pub const DISTANCE_CORRECTION_DAMPING: f64 = 0.85;
/// Lower bound on the cumulative distance correction multiplier.
/// Prevents the generator from under-shooting waypoint distances too aggressively.
pub const DISTANCE_CORRECTION_MIN: f64 = 0.5;
/// Upper bound on the cumulative distance correction multiplier.
/// Prevents runaway expansion of waypoint distances.
pub const DISTANCE_CORRECTION_MAX: f64 = 2.5;

// --- Distance-stratified candidate selection ---
// For long routes in dense areas, POIs are bucketed into concentric distance
// rings to prevent the closest-first DB limit from filling the candidate pool
// entirely with nearby POIs. These are structural algorithm parameters.

/// Number of concentric distance rings for stratified candidate selection.
pub const STRATIFIED_RING_COUNT: usize = 4;
/// Outermost ring distance as a fraction of target route distance.
/// E.g., for a 10 km target, rings extend out to 6 km from the start.
pub const STRATIFIED_MAX_RING_DISTANCE_FACTOR: f64 = 0.6;
