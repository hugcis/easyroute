use crate::db::queries;
use crate::models::route::{RoutePoi, SnappedPoi};
use crate::models::{Coordinates, PoiCategory};
use sqlx::PgPool;
use std::collections::HashSet;
use tracing::{debug, instrument};

#[derive(Clone)]
pub struct SnappingService {
    db_pool: PgPool,
}

impl SnappingService {
    pub fn new(db_pool: PgPool) -> Self {
        SnappingService { db_pool }
    }

    /// Find POIs that are near the route path but not used as waypoints
    #[instrument(skip(self, route_path, waypoint_pois))]
    pub async fn find_snapped_pois(
        &self,
        route_path: &[Coordinates],
        waypoint_pois: &[RoutePoi],
        snap_radius_m: f64,
        categories: Option<&[PoiCategory]>,
    ) -> Result<Vec<SnappedPoi>, Box<dyn std::error::Error>> {
        if route_path.len() < 2 {
            debug!("Route path too short for snapping");
            return Ok(Vec::new());
        }

        // Step 1: Calculate bounding box of the route with buffer
        let bbox = self.calculate_bbox_with_buffer(route_path, snap_radius_m);
        debug!(
            "Calculated bbox: lat [{}, {}], lng [{}, {}]",
            bbox.min_lat, bbox.max_lat, bbox.min_lng, bbox.max_lng
        );

        // Step 2: Query POIs in bounding box
        let nearby_pois = queries::find_pois_in_bbox(
            &self.db_pool,
            bbox.min_lat,
            bbox.max_lat,
            bbox.min_lng,
            bbox.max_lng,
            categories,
            500, // Reasonable limit for POIs in bbox
        )
        .await?;

        debug!("Found {} POIs in bounding box", nearby_pois.len());

        // Step 3: Create set of waypoint POI IDs for deduplication
        let waypoint_ids: HashSet<_> = waypoint_pois.iter().map(|rp| rp.poi.id).collect();

        // Step 4: Filter POIs by distance to path
        let mut snapped = Vec::new();
        let nearby_pois_count = nearby_pois.len();

        for poi in nearby_pois {
            // Skip if this POI is already a waypoint
            if waypoint_ids.contains(&poi.id) {
                continue;
            }

            // Calculate distance from POI to route path
            if let Some((dist_km, _segment, dist_along_km)) =
                poi.coordinates.distance_to_linestring(route_path)
            {
                let dist_m = dist_km * 1000.0;

                if dist_m <= snap_radius_m {
                    snapped.push(SnappedPoi::new(poi, dist_along_km, dist_m as f32));
                }
            }
        }

        debug!(
            "Snapped {} POIs to route (from {} candidates)",
            snapped.len(),
            nearby_pois_count
        );

        // Step 5: Sort by distance along path
        snapped.sort_by(|a, b| {
            a.distance_from_start_km
                .partial_cmp(&b.distance_from_start_km)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(snapped)
    }

    /// Calculate bounding box with buffer for the route path
    fn calculate_bbox_with_buffer(&self, path: &[Coordinates], buffer_m: f64) -> BoundingBox {
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lng = f64::INFINITY;
        let mut max_lng = f64::NEG_INFINITY;

        for coord in path {
            min_lat = min_lat.min(coord.lat);
            max_lat = max_lat.max(coord.lat);
            min_lng = min_lng.min(coord.lng);
            max_lng = max_lng.max(coord.lng);
        }

        // Add buffer (approximate: 1 degree latitude ≈ 111km, longitude varies)
        // Using conservative estimate for longitude at mid latitudes
        let lat_buffer = buffer_m / 111_000.0; // meters to degrees latitude
        let mid_lat = (min_lat + max_lat) / 2.0;

        // Clamp mid_lat to avoid extreme values near poles (±85° is safe limit)
        // At extreme latitudes, use a conservative fixed buffer instead
        let lng_buffer = if mid_lat.abs() > 85.0 {
            // Near poles: use same buffer as latitude (conservative)
            lat_buffer
        } else {
            // Normal case: adjust for latitude
            buffer_m / (111_000.0 * mid_lat.to_radians().cos())
        };

        BoundingBox {
            min_lat: min_lat - lat_buffer,
            max_lat: max_lat + lat_buffer,
            min_lng: min_lng - lng_buffer,
            max_lng: max_lng + lng_buffer,
        }
    }
}

struct BoundingBox {
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
}

#[cfg(test)]
mod tests {
    // Note: Tests requiring database connection are in integration tests
}
