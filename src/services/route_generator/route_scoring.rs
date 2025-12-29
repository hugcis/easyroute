use crate::error::Result;
use crate::models::{Poi, Route, RoutePoi, RoutePreferences};
use crate::services::mapbox::DirectionsResponse;
use crate::services::snapping_service::SnappingService;
use std::collections::HashSet;

/// Handles route quality scoring and route object construction
pub struct RouteScorer {
    snapping_service: SnappingService,
    snap_radius_m: f64,
}

impl RouteScorer {
    pub fn new(snapping_service: SnappingService, snap_radius_m: f64) -> Self {
        Self {
            snapping_service,
            snap_radius_m,
        }
    }

    /// Build Route object from directions response and selected POIs
    pub async fn build_route(
        &self,
        directions: DirectionsResponse,
        pois: Vec<Poi>,
        preferences: &RoutePreferences,
    ) -> Result<Route> {
        let path = directions.to_coordinates();

        // Calculate distance from start for each POI (approximate)
        let poi_count = pois.len();
        let route_pois: Vec<RoutePoi> = pois
            .into_iter()
            .enumerate()
            .map(|(idx, poi)| {
                // Rough approximation: distribute POIs along the route
                let distance_fraction = (idx + 1) as f64 / (poi_count + 1) as f64;
                let distance_from_start = directions.distance_km() * distance_fraction;

                RoutePoi::new(poi, idx as u32 + 1, distance_from_start)
            })
            .collect();

        let mut route = Route::new(
            directions.distance_km(),
            directions.duration_minutes(),
            path.clone(),
            route_pois.clone(),
        );

        // Find and add snapped POIs
        match self
            .snapping_service
            .find_snapped_pois(
                &path,
                &route_pois,
                self.snap_radius_m,
                preferences.poi_categories.as_deref(),
            )
            .await
        {
            Ok(snapped_pois) => {
                tracing::debug!("Added {} snapped POIs to route", snapped_pois.len());
                route = route.with_snapped_pois(snapped_pois);
            }
            Err(e) => {
                tracing::warn!("Failed to find snapped POIs: {}", e);
                // Continue without snapped POIs
            }
        }

        Ok(route)
    }

    /// Calculate route quality score (0-10)
    /// Based on: distance accuracy, POI count, POI quality, category diversity
    pub fn calculate_route_score(
        &self,
        route: &Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> f32 {
        let mut score = 0.0;

        // 1. Distance accuracy (0-3 points)
        let distance_error = (route.distance_km - target_distance_km).abs();
        let distance_error_ratio = distance_error / target_distance_km;
        score += 3.0 * (1.0 - distance_error_ratio.min(1.0)) as f32;

        // 2. POI count (0-3 points)
        let poi_count_score = (route.pois.len() as f32).min(3.0);
        score += poi_count_score;

        // 3. POI quality (0-2 points)
        if !route.pois.is_empty() {
            let avg_poi_quality: f32 = route
                .pois
                .iter()
                .map(|rp| rp.poi.quality_score(preferences.hidden_gems) / 100.0)
                .sum::<f32>()
                / route.pois.len() as f32;
            score += 2.0 * avg_poi_quality;
        }

        // 4. Category diversity (0-2 points)
        let unique_categories: HashSet<_> = route.pois.iter().map(|rp| &rp.poi.category).collect();
        let diversity_score = (unique_categories.len() as f32 / 3.0).min(1.0);
        score += 2.0 * diversity_score;

        score.clamp(0.0, 10.0)
    }
}
