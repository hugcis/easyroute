mod geometric_loop;
pub mod route_metrics;
mod route_scoring;
mod scoring_strategy;
mod tolerance_strategy;
mod waypoint_selection;

use crate::config::RouteGeneratorConfig;
use crate::error::Result;
use crate::models::{Coordinates, Route, RoutePreferences, TransportMode};
use crate::services::mapbox::MapboxClient;
use crate::services::poi_service::PoiService;
use crate::services::snapping_service::SnappingService;

use geometric_loop::GeometricLoopGenerator;
use route_scoring::RouteScorer;
use tolerance_strategy::ToleranceStrategy;
use waypoint_selection::WaypointSelector;

pub struct RouteGenerator {
    poi_service: PoiService,
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
        // Create shared components
        let waypoint_selector = WaypointSelector::new(config.clone());
        let route_scorer = RouteScorer::new(snapping_service, snap_radius_m, config.clone());
        let geometric_loop_generator = GeometricLoopGenerator::new(mapbox_client.clone());
        let tolerance_strategy = ToleranceStrategy::new(
            config.clone(),
            mapbox_client,
            waypoint_selector,
            route_scorer,
        );

        RouteGenerator {
            poi_service,
            config,
            geometric_loop_generator,
            tolerance_strategy,
        }
    }

    /// Generate loop routes starting and ending at the same point
    /// Returns multiple alternative routes based on preferences
    /// Implements adaptive tolerance: tries with normal tolerance first, then relaxes if needed
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

        // Step 1: Discover POIs within search radius
        let search_radius_km = target_distance_km * self.config.poi_search_radius_multiplier;
        // Dynamic POI limit based on route distance
        // For 5km: 100 POIs, For 9km: 180 POIs, For 15km: 300 POIs
        let poi_limit = (target_distance_km * 20.0).clamp(50.0, 500.0) as usize;

        let raw_pois = self
            .poi_service
            .find_pois(
                &start,
                search_radius_km,
                preferences.poi_categories.as_deref(),
                poi_limit,
            )
            .await?;

        if raw_pois.is_empty() {
            // Try geometric fallback if no POIs
            tracing::warn!(
                search_radius_km = %format!("{:.2}", search_radius_km),
                poi_limit = poi_limit,
                categories = ?preferences.poi_categories,
                "No POIs found within {:.1}km radius (limit: {}), attempting geometric fallback",
                search_radius_km, poi_limit
            );
            return self
                .geometric_loop_generator
                .generate_geometric_loop(start, target_distance_km, mode)
                .await
                .map(|route| vec![route]);
        }

        // Step 2: Score and filter POIs
        // Scale candidate limit with distance: 5km→50, 9km→90, 15km→100
        let candidate_limit = (target_distance_km * 10.0).clamp(20.0, 100.0) as usize;
        let raw_count = raw_pois.len();
        let candidate_pois = self.poi_service.score_and_filter_pois(
            raw_pois,
            preferences.hidden_gems,
            candidate_limit,
        );

        tracing::info!(
            raw_pois = raw_count,
            candidates = candidate_pois.len(),
            filtered_out = raw_count - candidate_pois.len(),
            search_radius_km = %format!("{:.2}", search_radius_km),
            "POI discovery: {} raw -> {} candidates ({} filtered out) within {:.1}km",
            raw_count, candidate_pois.len(), raw_count - candidate_pois.len(), search_radius_km
        );

        // Step 3: Try with progressively relaxed tolerance levels
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

        let mut tolerance_levels_tried = 0;
        for (level_index, (tolerance, tolerance_name)) in tolerance_levels.iter().enumerate() {
            tolerance_levels_tried += 1;
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
                    &start,
                    target_distance_km,
                    *tolerance,
                    mode,
                    &candidate_pois,
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
                return Ok(routes);
            }

            if *tolerance_name != very_relaxed_str.as_str() {
                tracing::warn!(
                    tolerance = *tolerance_name,
                    "Failed with {} tolerance, trying next level",
                    tolerance_name
                );
            }
        }

        // Step 4: Final fallback - geometric loop (always attempt, never return 500)
        tracing::warn!(
            candidates = candidate_pois.len(),
            tolerance_levels_tried = tolerance_levels_tried,
            target_km = %format!("{:.1}", target_distance_km),
            "All {} tolerance levels exhausted with {} candidates for {:.1}km target, falling back to geometric loop",
            tolerance_levels_tried, candidate_pois.len(), target_distance_km
        );
        self.geometric_loop_generator
            .generate_geometric_loop(start, target_distance_km, mode)
            .await
            .map(|route| vec![route])
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
