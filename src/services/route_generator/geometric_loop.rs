use crate::constants::*;
use crate::error::Result;
use crate::models::{Coordinates, Route, TransportMode};
use crate::services::mapbox::MapboxClient;

/// Handles generation of geometric loop routes when POIs are unavailable
pub struct GeometricLoopGenerator {
    mapbox_client: MapboxClient,
}

impl GeometricLoopGenerator {
    pub fn new(mapbox_client: MapboxClient) -> Self {
        Self { mapbox_client }
    }

    /// Generate a geometric loop when POIs are unavailable
    /// Creates a circular route using evenly distributed waypoints
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

        // Calculate radius such that the circle circumference approximates target distance
        // circumference = 2πr, so r = target_distance / (2π)
        let radius_km = target_distance_km / GEOMETRIC_LOOP_RADIUS_DIVISOR;

        // Convert radius to degrees (rough approximation: 1 degree ≈ 111km at equator)
        // This is imprecise but good enough for generating waypoints
        let radius_deg = radius_km / 111.0;

        // Generate waypoints around a circle
        let num_waypoints = GEOMETRIC_LOOP_NUM_WAYPOINTS;
        let mut waypoints = vec![start]; // Start point

        for i in 0..num_waypoints {
            let angle = (i as f64 / num_waypoints as f64) * 2.0 * std::f64::consts::PI;
            let lat_offset = radius_deg * angle.cos();
            let lng_offset = radius_deg * angle.sin() / start.lat.to_radians().cos();

            let waypoint_lat = start.lat + lat_offset;
            let waypoint_lng = start.lng + lng_offset;

            if let Ok(waypoint) = Coordinates::new(waypoint_lat, waypoint_lng) {
                waypoints.push(waypoint);
            }
        }

        waypoints.push(start); // Return to start

        tracing::debug!(
            "Generated geometric loop with {} waypoints (radius: {:.2}km)",
            waypoints.len(),
            radius_km
        );

        // Get directions from Mapbox to snap to actual roads
        let directions = self.mapbox_client.get_directions(&waypoints, mode).await?;

        tracing::info!(
            "Geometric loop generated: {:.2}km (target: {}km)",
            directions.distance_km(),
            target_distance_km
        );

        // Build route without POIs
        let path = directions.to_coordinates();
        Ok(Route::new(
            directions.distance_km(),
            directions.duration_minutes(),
            path,
            vec![], // No POIs
        ))
    }
}
