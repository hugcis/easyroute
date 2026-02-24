mod geometric_loop;
pub mod geometry;
pub mod route_metrics;
mod route_scoring;
mod scoring_strategy;
mod tolerance_strategy;
mod waypoint_selection;

use crate::config::RouteGeneratorConfig;
use crate::error::Result;
use crate::models::{Coordinates, Poi, Route, RoutePreferences, TransportMode};
use crate::services::mapbox::MapboxClient;
use crate::services::poi_service::PoiService;
use crate::services::snapping_service::SnappingService;

use geometric_loop::GeometricLoopGenerator;
use route_metrics::RouteMetrics;
use route_scoring::RouteScorer;
use tolerance_strategy::ToleranceStrategy;
use waypoint_selection::WaypointSelector;

pub struct RouteGenerator {
    poi_service: PoiService,
    snapping_service: SnappingService,
    snap_radius_m: f64,
    config: RouteGeneratorConfig,
    geometric_loop_generator: GeometricLoopGenerator,
    tolerance_strategy: ToleranceStrategy,
}

impl RouteGenerator {
    pub fn new(
        mapbox_client: MapboxClient,
        poi_service: PoiService,
        snapping_service: SnappingService,
        snap_radius_m: f64,
        config: RouteGeneratorConfig,
    ) -> Self {
        let waypoint_selector = WaypointSelector::new(config.clone());
        let route_scorer =
            RouteScorer::new(snapping_service.clone(), snap_radius_m, config.clone());
        let geometric_loop_generator = GeometricLoopGenerator::new(mapbox_client.clone());
        let tolerance_strategy = ToleranceStrategy::new(
            config.clone(),
            mapbox_client,
            waypoint_selector,
            route_scorer,
        );

        RouteGenerator {
            poi_service,
            snapping_service,
            snap_radius_m,
            config,
            geometric_loop_generator,
            tolerance_strategy,
        }
    }

    /// Enhance a geometric fallback route with snapped POIs and quality metrics.
    /// Snapping failure is non-fatal — the route is always returned.
    async fn enhance_geometric_route(
        &self,
        mut route: Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
        area_poi_count: usize,
    ) -> Route {
        match self
            .snapping_service
            .find_snapped_pois(
                &route.path,
                &[],
                self.snap_radius_m,
                preferences.poi_categories.as_deref(),
            )
            .await
        {
            Ok(snapped_pois) => {
                tracing::debug!(
                    count = snapped_pois.len(),
                    "Added snapped POIs to geometric fallback route"
                );
                route = route.with_snapped_pois(snapped_pois);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to snap POIs to geometric fallback route, continuing without"
                );
            }
        }

        let metrics = RouteMetrics::compute_with_threshold(
            &route,
            area_poi_count,
            self.config.metrics_overlap_threshold_m,
        );
        route.metrics = Some(metrics);

        route.score = self
            .tolerance_strategy
            .score_route(&route, target_distance_km, preferences);

        route
    }

    /// Discover POIs within the search radius and score/filter them to candidate set.
    /// Returns `None` if no POIs found at all (caller should use geometric fallback).
    async fn discover_and_filter_pois(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> Result<Option<Vec<Poi>>> {
        let search_radius_km = target_distance_km * self.config.poi_search_radius_multiplier;
        let poi_limit = (target_distance_km * 20.0).clamp(50.0, 500.0) as usize;

        let raw_pois = self
            .poi_service
            .find_pois(
                start,
                search_radius_km,
                preferences.poi_categories.as_deref(),
                poi_limit,
            )
            .await?;

        if raw_pois.is_empty() {
            tracing::warn!(
                search_radius_km = %format!("{:.2}", search_radius_km),
                poi_limit = poi_limit,
                categories = ?preferences.poi_categories,
                "No POIs found within {:.1}km radius (limit: {}), attempting geometric fallback",
                search_radius_km, poi_limit
            );
            return Ok(None);
        }

        let max_candidates = if target_distance_km > 12.0 {
            300.0
        } else {
            100.0
        };
        let candidate_limit = (target_distance_km * 10.0).clamp(20.0, max_candidates) as usize;
        let raw_count = raw_pois.len();
        let candidate_pois =
            self.poi_service
                .select_top_pois(raw_pois, preferences.hidden_gems, candidate_limit);

        tracing::info!(
            raw_pois = raw_count,
            candidates = candidate_pois.len(),
            filtered_out = raw_count - candidate_pois.len(),
            search_radius_km = %format!("{:.2}", search_radius_km),
            "POI discovery: {} raw -> {} candidates ({} filtered out) within {:.1}km",
            raw_count, candidate_pois.len(), raw_count - candidate_pois.len(), search_radius_km
        );

        Ok(Some(candidate_pois))
    }

    /// Try generating routes at progressively relaxed tolerance levels.
    /// Returns routes on success, or empty vec if all levels exhausted.
    async fn try_tolerance_levels(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        distance_tolerance: f64,
        mode: &TransportMode,
        candidate_pois: &[Poi],
        preferences: &RoutePreferences,
    ) -> Vec<Route> {
        let relaxed_str = format!(
            "relaxed (±{}%)",
            (self.config.tolerance_level_relaxed * 100.0) as i32
        );
        let very_relaxed_str = format!(
            "very relaxed (±{}%)",
            (self.config.tolerance_level_very_relaxed * 100.0) as i32
        );

        let tolerance_levels = [
            (distance_tolerance, "normal"),
            (
                target_distance_km * self.config.tolerance_level_relaxed,
                relaxed_str.as_str(),
            ),
            (
                target_distance_km * self.config.tolerance_level_very_relaxed,
                very_relaxed_str.as_str(),
            ),
        ];

        let max_alternatives = preferences.max_alternatives.clamp(
            crate::constants::MIN_ALTERNATIVES_FOR_SUCCESS,
            crate::constants::MAX_ALTERNATIVES_CLAMP,
        ) as usize;

        for (level_index, (tolerance, tolerance_name)) in tolerance_levels.iter().enumerate() {
            tracing::info!(
                tolerance_name = tolerance_name,
                tolerance_km = %format!("{:.2}", tolerance),
                target_km = %format!("{:.1}", target_distance_km),
                candidates = candidate_pois.len(),
                "Trying {} tolerance: {:.1}km ± {:.2}km",
                tolerance_name, target_distance_km, tolerance
            );

            let seed_offset = level_index * max_alternatives;
            let routes = self
                .tolerance_strategy
                .try_generate_routes_with_tolerance(
                    start,
                    target_distance_km,
                    *tolerance,
                    mode,
                    candidate_pois,
                    preferences,
                    seed_offset,
                )
                .await;

            if !routes.is_empty() {
                if *tolerance_name != "normal" {
                    tracing::info!(
                        "Successfully generated routes with {} tolerance",
                        tolerance_name
                    );
                }
                return routes;
            }

            if *tolerance_name != very_relaxed_str.as_str() {
                tracing::warn!(
                    tolerance = *tolerance_name,
                    "Failed with {} tolerance, trying next level",
                    tolerance_name
                );
            }
        }

        Vec::new()
    }

    /// Last-resort POI-based attempt: accept any route within ±100% of target distance.
    async fn try_extreme_tolerance(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        mode: &TransportMode,
        candidate_pois: &[Poi],
        preferences: &RoutePreferences,
        seed_offset: usize,
    ) -> Vec<Route> {
        let extreme_tolerance = target_distance_km; // ±100%
        tracing::warn!(
            tolerance_km = %format!("{:.2}", extreme_tolerance),
            target_km = %format!("{:.1}", target_distance_km),
            candidates = candidate_pois.len(),
            "All tolerance levels exhausted, trying extreme tolerance (±100%): {:.1}km ± {:.2}km",
            target_distance_km, extreme_tolerance
        );

        let routes = self
            .tolerance_strategy
            .try_generate_routes_with_tolerance(
                start,
                target_distance_km,
                extreme_tolerance,
                mode,
                candidate_pois,
                preferences,
                seed_offset,
            )
            .await;

        if !routes.is_empty() {
            tracing::info!(
                count = routes.len(),
                "Generated {} routes with extreme tolerance (±100%)",
                routes.len()
            );
        }

        routes
    }

    /// Generate loop routes starting and ending at the same point.
    /// Returns multiple alternative routes based on preferences.
    /// Implements adaptive tolerance: tries normal -> relaxed -> very relaxed -> extreme -> geometric fallback.
    pub async fn generate_loop_route(
        &self,
        start: Coordinates,
        target_distance_km: f64,
        distance_tolerance: f64,
        mode: &TransportMode,
        preferences: &RoutePreferences,
    ) -> Result<Vec<Route>> {
        tracing::info!(
            "Generating loop route from {:?}, target: {}km",
            start,
            target_distance_km
        );

        // Step 1: Discover and filter POIs
        let candidate_pois = match self
            .discover_and_filter_pois(&start, target_distance_km, preferences)
            .await?
        {
            Some(pois) => pois,
            None => {
                let route = self
                    .geometric_loop_generator
                    .generate_geometric_loop(start, target_distance_km, mode)
                    .await?;
                let route = self
                    .enhance_geometric_route(route, target_distance_km, preferences, 0)
                    .await;
                return Ok(vec![route]);
            }
        };

        // Step 2: Try progressively relaxed tolerance levels
        let routes = self
            .try_tolerance_levels(
                &start,
                target_distance_km,
                distance_tolerance,
                mode,
                &candidate_pois,
                preferences,
            )
            .await;
        if !routes.is_empty() {
            return Ok(routes);
        }

        // Step 3: Extreme tolerance (±100%)
        let max_alternatives = preferences.max_alternatives.clamp(
            crate::constants::MIN_ALTERNATIVES_FOR_SUCCESS,
            crate::constants::MAX_ALTERNATIVES_CLAMP,
        ) as usize;
        let seed_offset = 3 * max_alternatives; // After 3 normal tolerance levels
        let routes = self
            .try_extreme_tolerance(
                &start,
                target_distance_km,
                mode,
                &candidate_pois,
                preferences,
                seed_offset,
            )
            .await;
        if !routes.is_empty() {
            return Ok(routes);
        }

        // Step 4: Final fallback — geometric loop
        tracing::warn!(
            candidates = candidate_pois.len(),
            target_km = %format!("{:.1}", target_distance_km),
            "All tolerance levels exhausted with {} candidates for {:.1}km target, falling back to geometric loop",
            candidate_pois.len(), target_distance_km
        );
        let route = self
            .geometric_loop_generator
            .generate_geometric_loop(start, target_distance_km, mode)
            .await?;
        let route = self
            .enhance_geometric_route(route, target_distance_km, preferences, candidate_pois.len())
            .await;
        Ok(vec![route])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PoiCategory;

    // Spatial distribution test - simplified without needing full RouteGenerator
    #[test]
    fn test_angle_calculation() {
        let start = Coordinates::new(48.8566, 2.3522).unwrap();
        let north = Coordinates::new(48.8666, 2.3522).unwrap();
        let east = Coordinates::new(48.8566, 2.3722).unwrap();

        // Just verify coordinates are different
        assert_ne!(start.lat, north.lat);
        assert_ne!(start.lng, east.lng);
    }

    #[test]
    fn test_route_scoring_logic() {
        use crate::models::{Poi, Route, RoutePoi};

        let route = Route {
            id: uuid::Uuid::new_v4(),
            distance_km: 5.1, // Close to target of 5.0
            estimated_duration_minutes: 75,
            elevation_gain_m: None,
            path: vec![],
            pois: vec![
                RoutePoi::new(
                    Poi::new(
                        "Test 1".to_string(),
                        PoiCategory::Monument,
                        Coordinates::new(48.8566, 2.3522).unwrap(),
                        80.0,
                    ),
                    1,
                    1.7,
                ),
                RoutePoi::new(
                    Poi::new(
                        "Test 2".to_string(),
                        PoiCategory::Park,
                        Coordinates::new(48.8570, 2.3530).unwrap(),
                        70.0,
                    ),
                    2,
                    3.4,
                ),
            ],
            snapped_pois: vec![],
            score: 0.0,
            metrics: None,
        };

        // Test that route has expected properties
        assert_eq!(route.pois.len(), 2);
        assert!(route.distance_km > 5.0);
        assert!(route.distance_km < 5.2);
    }
}
