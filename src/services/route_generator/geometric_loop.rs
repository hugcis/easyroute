use crate::error::Result;
use crate::models::{Coordinates, Route, TransportMode};
use crate::services::mapbox::MapboxClient;

/// Number of waypoints for geometric loop (reduced from 6 to prevent over-constraining Mapbox)
const GEOMETRIC_LOOP_NUM_WAYPOINTS: usize = 4;

/// Radius jitter range (±15%)
const RADIUS_JITTER_RANGE: f64 = 0.15;

/// Rotation jitter range in radians (~20 degrees)
const ROTATION_JITTER_RAD: f64 = 0.35;

/// Handles generation of geometric loop routes when POIs are unavailable
pub struct GeometricLoopGenerator {
    mapbox_client: MapboxClient,
}

impl GeometricLoopGenerator {
    pub fn new(mapbox_client: MapboxClient) -> Self {
        Self { mapbox_client }
    }

    /// Generate a geometric loop when POIs are unavailable
    /// Creates a circular route using evenly distributed waypoints with slight randomization
    pub async fn generate_geometric_loop(
        &self,
        start: Coordinates,
        target_distance_km: f64,
        mode: &TransportMode,
    ) -> Result<Route> {
        tracing::info!(
            "Generating geometric loop (no POIs) for {}km route",
            target_distance_km
        );

        // Calculate base radius: circumference = 2*pi*r, so r = target / (2*pi)
        let base_radius_km = target_distance_km / std::f64::consts::TAU;
        let base_radius_deg = base_radius_km / 111.0;

        // Use a deterministic seed based on start coordinates and target distance
        // for reproducible but varied results
        let seed = ((start.lat * 1000.0).abs() as u64)
            .wrapping_mul(31)
            .wrapping_add((start.lng * 1000.0).abs() as u64)
            .wrapping_mul(37)
            .wrapping_add((target_distance_km * 100.0) as u64);

        // Rotation jitter: slight offset to circle orientation
        let rotation_offset =
            pseudo_random_f64(seed, 0) * ROTATION_JITTER_RAD * 2.0 - ROTATION_JITTER_RAD;

        let num_waypoints = GEOMETRIC_LOOP_NUM_WAYPOINTS;
        let mut waypoints = vec![start];

        for i in 0..num_waypoints {
            let base_angle = (i as f64 / num_waypoints as f64) * std::f64::consts::TAU;
            let angle = base_angle + rotation_offset;

            // Per-waypoint radius jitter: ±15% of base radius
            let jitter =
                pseudo_random_f64(seed, i + 1) * RADIUS_JITTER_RANGE * 2.0 - RADIUS_JITTER_RANGE;
            let radius_deg = base_radius_deg * (1.0 + jitter);

            let lat_offset = radius_deg * angle.cos();
            let lng_offset = radius_deg * angle.sin() / start.lat.to_radians().cos();

            let waypoint_lat = start.lat + lat_offset;
            let waypoint_lng = start.lng + lng_offset;

            match Coordinates::new(waypoint_lat, waypoint_lng) {
                Ok(waypoint) => waypoints.push(waypoint),
                Err(_) => {
                    tracing::warn!(
                        index = i,
                        lat = waypoint_lat,
                        lng = waypoint_lng,
                        "Geometric loop: invalid waypoint {} coordinates ({}, {}), skipping",
                        i,
                        waypoint_lat,
                        waypoint_lng
                    );
                }
            }
        }

        waypoints.push(start); // Return to start

        tracing::debug!(
            "Generated geometric loop with {} waypoints (base radius: {:.2}km)",
            waypoints.len() - 2, // Exclude start/end duplicates
            base_radius_km
        );

        // Get directions from Mapbox to snap to actual roads
        let directions = self.mapbox_client.get_directions(&waypoints, mode).await?;

        tracing::info!(
            "Geometric loop generated: {:.2}km (target: {}km)",
            directions.distance_km(),
            target_distance_km
        );

        let path = directions.to_coordinates();
        Ok(Route::new(
            directions.distance_km(),
            directions.duration_minutes(),
            path,
            vec![],
        ))
    }
}

/// Simple deterministic pseudo-random number generator
/// Returns a value in [0.0, 1.0)
fn pseudo_random_f64(seed: u64, index: usize) -> f64 {
    let mut x = seed
        .wrapping_add(index as u64)
        .wrapping_mul(6364136223846793005);
    x = x.wrapping_add(1442695040888963407);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}
