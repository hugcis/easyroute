use crate::config::RouteGeneratorConfig;
use crate::error::Result;
use crate::models::{Poi, Route, RoutePoi, RoutePreferences};
use crate::services::mapbox::DirectionsResponse;
use crate::services::snapping_service::SnappingService;
use std::collections::HashSet;

use super::route_metrics::RouteMetrics;

/// Handles route quality scoring and route object construction
pub struct RouteScorer {
    snapping_service: SnappingService,
    snap_radius_m: f64,
    config: RouteGeneratorConfig,
}

impl RouteScorer {
    pub fn new(
        snapping_service: SnappingService,
        snap_radius_m: f64,
        config: RouteGeneratorConfig,
    ) -> Self {
        Self {
            snapping_service,
            snap_radius_m,
            config,
        }
    }

    /// Build Route object from directions response and selected POIs
    pub async fn build_route(
        &self,
        directions: DirectionsResponse,
        pois: Vec<Poi>,
        preferences: &RoutePreferences,
        area_poi_count: usize,
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
                tracing::warn!(
                    error = %e,
                    path_points = path.len(),
                    snap_radius_m = self.snap_radius_m,
                    "Failed to snap POIs ({} path points, {}m radius): {}",
                    path.len(), self.snap_radius_m, e
                );
                // Continue without snapped POIs
            }
        }

        // Compute route quality metrics
        let metrics = RouteMetrics::compute_with_threshold(
            &route,
            area_poi_count,
            self.config.metrics_overlap_threshold_m,
        );
        route.metrics = Some(metrics);

        Ok(route)
    }

    /// Calculate route quality score (0-10)
    /// V1: distance accuracy, POI count, POI quality, category diversity
    /// V2: adds route shape (circularity + convexity) and path diversity (1 - overlap)
    pub fn calculate_route_score(
        &self,
        route: &Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> f32 {
        if self.config.scoring_version >= 2 {
            self.calculate_route_score_v2(route, target_distance_km, preferences)
        } else {
            self.calculate_route_score_v1(route, target_distance_km, preferences)
        }
    }

    /// V1 scoring: original algorithm (0-10)
    fn calculate_route_score_v1(
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

    /// V2 scoring: shape-aware (0-10)
    /// Distance accuracy: 2.5, POI count: 2.0, POI quality: 1.5,
    /// Category diversity: 1.0, Route shape: 2.0, Path diversity: 1.0
    fn calculate_route_score_v2(
        &self,
        route: &Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> f32 {
        let mut score = 0.0;

        // 1. Distance accuracy (0-2.5 points)
        let distance_error = (route.distance_km - target_distance_km).abs();
        let distance_error_ratio = distance_error / target_distance_km;
        score += 2.5 * (1.0 - distance_error_ratio.min(1.0)) as f32;

        // 2. POI count (0-2 points)
        let poi_count_score = (route.pois.len() as f32 / 3.0).min(1.0);
        score += 2.0 * poi_count_score;

        // 3. POI quality (0-1.5 points)
        if !route.pois.is_empty() {
            let avg_poi_quality: f32 = route
                .pois
                .iter()
                .map(|rp| rp.poi.quality_score(preferences.hidden_gems) / 100.0)
                .sum::<f32>()
                / route.pois.len() as f32;
            score += 1.5 * avg_poi_quality;
        }

        // 4. Category diversity (0-1 point)
        let unique_categories: HashSet<_> = route.pois.iter().map(|rp| &rp.poi.category).collect();
        let diversity_score = (unique_categories.len() as f32 / 3.0).min(1.0);
        score += 1.0 * diversity_score;

        // 5. Route shape (0-2 points) — from metrics if available
        if let Some(ref metrics) = route.metrics {
            let shape_score = (metrics.circularity + metrics.convexity) / 2.0;
            score += 2.0 * shape_score;

            // 6. Path diversity (0-1 point) — penalize overlap
            score += 1.0 * (1.0 - metrics.path_overlap_pct);
        }

        score.clamp(0.0, 10.0)
    }
}
