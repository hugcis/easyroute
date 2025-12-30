# Advanced POI Scoring Strategy - Future Improvements

**Status**: Experimental (not production-ready)
**Current Default**: Simple strategy (proven to work)
**Enable with**: `ROUTE_POI_SCORING_STRATEGY=advanced`

## Current Issues

### 1. Waypoint Count Scaling Problem ⚠️

**Symptom**: Routes are 2-3x longer than target (e.g., 16km for 6km target)

**Root Cause**: The current waypoint count calculation doesn't account for geometric perimeter growth:

```rust
// Current logic (config.rs:117-119)
if target_distance_km > 4.0 && poi_count >= 3 {
    waypoints_count = 4  // Uses 4 POIs
} else {
    waypoints_count = 2  // Uses 2 POIs
}
```

**Why this fails**:
- For 6km route with 4 POIs at ~3.6km from start
- Connecting 4 POIs in different compass directions creates a large perimeter
- Result: 15-20km route instead of 6km

**Proposed Fix**: Geometry-based waypoint calculation

```rust
/// Calculate optimal waypoint count based on target distance and POI radius
/// Formula: For N waypoints at radius R, perimeter ≈ N × 2R × sin(π/N)
/// Solve for N such that perimeter ≈ target_distance
fn calculate_optimal_waypoints(target_distance_km: f64, poi_radius: f64) -> usize {
    // Lookup table based on geometric calculations
    let ratio = target_distance_km / poi_radius;

    match ratio {
        r if r < 2.5 => 2,   // Short routes: 2 waypoints
        r if r < 4.0 => 3,   // Medium routes: 3 waypoints
        r if r < 6.0 => 4,   // Long routes: 4 waypoints
        _ => 5,              // Very long routes: 5 waypoints
    }
}
```

### 2. Distance Filtering Too Permissive

**Current**: `max_dist = target_distance * 0.7` (for routes < 7km)

**Issue**: For 6km route, allows POIs up to 4.2km from start. When combined with 4 waypoints, creates oversized routes.

**Proposed Fix**: Tighter distance constraints

```rust
// Stricter filtering based on waypoint count
let max_reasonable_dist = if num_waypoints == 2 {
    target_distance_km * 0.6   // 2 POIs can be farther
} else if num_waypoints == 3 {
    target_distance_km * 0.5   // 3 POIs need to be closer
} else {
    target_distance_km * 0.4   // 4+ POIs must be very close
};
```

### 3. Scoring Weight Balance

**Current Weights**:
- Distance: 60%
- Quality: 20%
- Angular: 10%
- Clustering: 5%
- Variation: 5%

**Issue**: Distance weight at 60% still isn't enough to prevent far POIs from being selected when they have high quality scores.

**Proposed Fix**: Dynamic weighting based on previous attempts

```rust
// Increase distance weight on retries
let distance_weight = if retry_count == 0 {
    0.6  // First attempt: balanced
} else {
    0.8  // Retries: prioritize distance accuracy
};
```

## Recommended Implementation Order

### Phase 1: Fix Waypoint Scaling (Critical)
1. Implement `calculate_optimal_waypoints()` function
2. Add unit tests for various distance/radius combinations
3. Integrate into `WaypointSelector::calculate_waypoint_count()`

**Expected Impact**: Routes within ±20% of target distance

### Phase 2: Adaptive Distance Filtering
1. Make `max_reasonable_dist` dependent on waypoint count
2. Test with real Bretagne data
3. Tune multipliers based on success rate

**Expected Impact**: Improved first-attempt success rate

### Phase 3: Dynamic Weight Adjustment
1. Add retry counter to `ScoringContext`
2. Adjust weights based on retry count
3. Potentially add "strict distance mode" for final retries

**Expected Impact**: Faster convergence to valid routes

### Phase 4: Iterative Selection Refinement
1. After selecting first POI, recalculate target radius for next POI
2. Ensure subsequent POIs complement the partial route
3. Add "route preview" to estimate distance before Mapbox call

**Expected Impact**: Reduced Mapbox API calls

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_waypoint_scaling() {
    // 3km route should use 2 waypoints
    assert_eq!(calculate_optimal_waypoints(3.0, 1.8), 2);

    // 6km route should use 3 waypoints
    assert_eq!(calculate_optimal_waypoints(6.0, 3.6), 3);

    // 10km route should use 4 waypoints
    assert_eq!(calculate_optimal_waypoints(10.0, 6.0), 4);
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_advanced_strategy_success_rate() {
    let test_cases = vec![
        (paris_coords(), 3.0),  // Short urban
        (paris_coords(), 6.0),  // Medium urban
        (rennes_coords(), 5.0), // Medium suburban
        (rural_coords(), 4.0),  // Short rural
    ];

    let mut successes = 0;
    for (coords, distance) in test_cases {
        if generate_route(coords, distance).is_ok() {
            successes += 1;
        }
    }

    // Target: >80% success rate
    assert!(successes as f32 / test_cases.len() as f32 > 0.8);
}
```

## Configuration Tunables

Once fixed, these env vars control Advanced strategy behavior:

```bash
# Strategy selection
ROUTE_POI_SCORING_STRATEGY=advanced

# Distance constraints
ROUTE_POI_MIN_SEPARATION_KM=0.3          # Min distance between POIs
ROUTE_MAX_POI_DISTANCE_MULTIPLIER=0.85   # Max POI distance (unused in Advanced)

# Scoring weights (must sum to 1.0)
ROUTE_POI_SCORE_WEIGHT_DISTANCE=0.6
ROUTE_POI_SCORE_WEIGHT_QUALITY=0.2
ROUTE_POI_SCORE_WEIGHT_ANGULAR=0.1
ROUTE_POI_SCORE_WEIGHT_CLUSTERING=0.05
ROUTE_POI_SCORE_WEIGHT_VARIATION=0.05
```

## Benefits When Fixed

1. **Better POI Quality**: Routes include interesting landmarks, not just closest points
2. **Category Diversity**: Automatic preference for varied POI types
3. **No Clustering**: POIs guaranteed to be separated by 300m+
4. **Spatial Distribution**: POIs spread around start point for better loops
5. **User Preference Aware**: Respects `hidden_gems` setting

## Current Status

- ✅ Infrastructure complete (Strategy pattern, scoring trait, tests)
- ✅ Simple strategy working well (proven algorithm)
- ⚠️ Advanced strategy needs waypoint scaling fix
- 📋 Estimated effort: 4-6 hours to implement Phase 1 fix

## Related Files

- `src/config.rs` - Strategy enum and configuration
- `src/services/route_generator/scoring_strategy.rs` - Scoring implementations
- `src/services/route_generator/waypoint_selection.rs` - Selection logic
- `src/services/route_generator/tolerance_strategy.rs` - Retry logic
