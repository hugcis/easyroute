use crate::db::PoiRepository;
use crate::models::route::{RoutePoi, SnappedPoi};
use crate::models::{BoundingBox, Coordinates, PoiCategory};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Clone)]
pub struct SnappingService {
    repo: Arc<dyn PoiRepository>,
}

impl SnappingService {
    pub fn new(repo: Arc<dyn PoiRepository>) -> Self {
        SnappingService { repo }
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
        let bbox = BoundingBox::from_path_with_buffer(route_path, snap_radius_m);
        debug!(
            "Calculated bbox: lat [{}, {}], lng [{}, {}]",
            bbox.min_lat, bbox.max_lat, bbox.min_lng, bbox.max_lng
        );

        // Step 2: Query POIs in bounding box
        let nearby_pois = self
            .repo
            .find_in_bbox(
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
}
