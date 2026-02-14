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
    use super::*;

    fn c(lat: f64, lng: f64) -> Coordinates {
        Coordinates::new(lat, lng).unwrap()
    }

    fn service() -> SnappingService {
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/fake").unwrap();
        SnappingService::new(pool)
    }

    #[tokio::test]
    async fn bbox_single_segment() {
        let svc = service();
        let path = vec![c(48.85, 2.35), c(48.86, 2.36)];
        let bbox = svc.calculate_bbox_with_buffer(&path, 0.0);
        assert!((bbox.min_lat - 48.85).abs() < 1e-10);
        assert!((bbox.max_lat - 48.86).abs() < 1e-10);
        assert!((bbox.min_lng - 2.35).abs() < 1e-10);
        assert!((bbox.max_lng - 2.36).abs() < 1e-10);
    }

    #[tokio::test]
    async fn bbox_buffer_expansion() {
        let svc = service();
        let path = vec![c(48.85, 2.35), c(48.86, 2.36)];
        let buffer_m = 1000.0; // 1km
        let bbox = svc.calculate_bbox_with_buffer(&path, buffer_m);
        let lat_buffer = buffer_m / 111_000.0;
        assert!((bbox.min_lat - (48.85 - lat_buffer)).abs() < 1e-10);
        assert!((bbox.max_lat - (48.86 + lat_buffer)).abs() < 1e-10);
    }

    #[tokio::test]
    async fn bbox_longitude_buffer_widens_at_higher_latitude() {
        let svc = service();
        let buffer_m = 1000.0;
        let lat_buffer = buffer_m / 111_000.0;

        // Near equator
        let path_eq = vec![c(1.0, 10.0), c(1.0, 10.0)];
        let bbox_eq = svc.calculate_bbox_with_buffer(&path_eq, buffer_m);
        let lng_buf_eq = (bbox_eq.max_lng - 10.0) - 0.0; // extra beyond original

        // At 60° latitude
        let path_60 = vec![c(60.0, 10.0), c(60.0, 10.0)];
        let bbox_60 = svc.calculate_bbox_with_buffer(&path_60, buffer_m);
        let lng_buf_60 = bbox_60.max_lng - 10.0;

        // Longitude buffer should be wider at higher latitude (cos correction)
        assert!(
            lng_buf_60 > lng_buf_eq,
            "lng_buf_60={lng_buf_60}, lng_buf_eq={lng_buf_eq}"
        );
        // Lat buffer should be same regardless
        assert!((bbox_eq.max_lat - (1.0 + lat_buffer)).abs() < 1e-10);
        assert!((bbox_60.max_lat - (60.0 + lat_buffer)).abs() < 1e-10);
    }

    #[tokio::test]
    async fn bbox_near_poles_fallback() {
        let svc = service();
        let path = vec![c(86.0, 10.0), c(86.0, 10.0)];
        let bbox = svc.calculate_bbox_with_buffer(&path, 1000.0);
        let lat_buffer = 1000.0 / 111_000.0;
        // Near poles: lng_buffer should equal lat_buffer (conservative)
        let lng_buffer = bbox.max_lng - 10.0;
        assert!(
            (lng_buffer - lat_buffer).abs() < 1e-10,
            "lng_buffer={lng_buffer}, lat_buffer={lat_buffer}"
        );
    }

    #[tokio::test]
    async fn bbox_multi_point_envelope() {
        let svc = service();
        let path = vec![c(48.85, 2.35), c(48.87, 2.33), c(48.86, 2.38)];
        let bbox = svc.calculate_bbox_with_buffer(&path, 0.0);
        assert!((bbox.min_lat - 48.85).abs() < 1e-10);
        assert!((bbox.max_lat - 48.87).abs() < 1e-10);
        assert!((bbox.min_lng - 2.33).abs() < 1e-10);
        assert!((bbox.max_lng - 2.38).abs() < 1e-10);
    }
}
