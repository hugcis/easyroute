use super::route_scoring::RouteScorer;
use super::waypoint_selection::WaypointSelector;
use crate::config::RouteGeneratorConfig;
use crate::constants::*;
use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, Route, RoutePreferences, TransportMode};
use crate::services::mapbox::MapboxClient;

/// Handles adaptive tolerance and retry strategies for route generation
pub struct ToleranceStrategy {
    config: RouteGeneratorConfig,
    mapbox_client: MapboxClient,
    waypoint_selector: WaypointSelector,
    route_scorer: RouteScorer,
}

/// Parameters for loop route generation attempt
pub struct LoopRouteParams<'a> {
    pub start: &'a Coordinates,
    pub target_distance_km: f64,
    pub distance_tolerance: f64,
    pub mode: &'a TransportMode,
    pub candidate_pois: &'a [Poi],
    pub attempt_seed: usize,
    pub preferences: &'a RoutePreferences,
}

impl ToleranceStrategy {
    pub fn new(
        config: RouteGeneratorConfig,
        mapbox_client: MapboxClient,
        waypoint_selector: WaypointSelector,
        route_scorer: RouteScorer,
    ) -> Self {
        Self {
            config,
            mapbox_client,
            waypoint_selector,
            route_scorer,
        }
    }

    /// Try to generate routes with a specific tolerance level
    #[allow(clippy::too_many_arguments)]
    pub async fn try_generate_routes_with_tolerance(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        distance_tolerance: f64,
        mode: &TransportMode,
        candidate_pois: &[Poi],
        preferences: &RoutePreferences,
        seed_offset: usize,
    ) -> Vec<Route> {
        let max_alternatives = preferences
            .max_alternatives
            .clamp(MIN_ALTERNATIVES_FOR_SUCCESS, MAX_ALTERNATIVES_CLAMP)
            as usize;
        let mut routes = Vec::new();

        for attempt in 0..max_alternatives {
            let params = LoopRouteParams {
                start,
                target_distance_km,
                distance_tolerance,
                mode,
                candidate_pois,
                attempt_seed: attempt + seed_offset,
                preferences,
            };

            match self.try_generate_loop(params).await {
                Ok(route) => routes.push(route),
                Err(e) => {
                    tracing::debug!(
                        "Failed to generate route alternative {}: {}",
                        attempt + 1,
                        e
                    );
                }
            }
        }

        if routes.is_empty() {
            tracing::warn!(
                tolerance_km = %format!("{:.2}", distance_tolerance),
                attempts = max_alternatives,
                "Tolerance level exhausted: 0/{} attempts produced valid routes (target: {:.1}km ± {:.2}km)",
                max_alternatives, target_distance_km, distance_tolerance
            );
        }

        // Score and rank routes if we got any
        if !routes.is_empty() {
            for route in &mut routes {
                route.score =
                    self.route_scorer
                        .calculate_route_score(route, target_distance_km, preferences);
            }

            routes.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            tracing::info!("Generated {} route alternatives", routes.len());
        }

        routes
    }

    /// Try to generate a single loop route with selected waypoints
    pub async fn try_generate_loop(&self, params: LoopRouteParams<'_>) -> Result<Route> {
        let min_distance = params.target_distance_km - params.distance_tolerance;
        let max_distance = params.target_distance_km + params.distance_tolerance;

        for retry in 0..self.config.max_route_generation_retries {
            let adjusted_target =
                Self::calculate_adjusted_target_distance(params.target_distance_km, retry);

            // Select POIs with different variation seed for each retry
            let selected_pois = self.waypoint_selector.select_loop_waypoints(
                params.start,
                adjusted_target,
                params.candidate_pois,
                params.attempt_seed * self.config.max_route_generation_retries + retry,
                params.preferences,
            )?;

            // Order POIs geographically before building waypoints to prevent backtracking
            let ordered_pois = WaypointSelector::order_pois_clockwise(params.start, &selected_pois);

            // Verify loop shape before calling Mapbox (saves API calls on bad configurations)
            if !WaypointSelector::verify_loop_shape(params.start, &ordered_pois, retry) {
                tracing::info!(
                    retry = retry + 1,
                    waypoint_count = ordered_pois.len(),
                    "Retry {}: skipped — poor loop shape with {} waypoints",
                    retry + 1,
                    ordered_pois.len()
                );
                continue;
            }

            let waypoints = Self::build_loop_waypoints(params.start, &ordered_pois);
            let directions = match self
                .mapbox_client
                .get_directions(&waypoints, params.mode)
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(
                        retry = retry + 1,
                        error = %e,
                        waypoint_count = waypoints.len(),
                        "Mapbox API call failed on retry {}: {}",
                        retry + 1, e
                    );
                    return Err(e);
                }
            };
            let distance_km = directions.distance_km();

            // Check if distance is within tolerance
            if Self::is_distance_within_tolerance(distance_km, min_distance, max_distance) {
                tracing::info!(
                    "Found valid route on attempt {} ({}km, target: {}km ± {}km)",
                    retry + 1,
                    distance_km,
                    params.target_distance_km,
                    params.distance_tolerance
                );
                return self
                    .route_scorer
                    .build_route(
                        directions,
                        ordered_pois,
                        params.preferences,
                        params.candidate_pois.len(),
                    )
                    .await;
            }

            // If not within tolerance, log and retry with different POIs
            let error_pct =
                (distance_km - params.target_distance_km).abs() / params.target_distance_km * 100.0;
            tracing::info!(
                retry = retry + 1,
                achieved_km = %format!("{:.2}", distance_km),
                target_km = %format!("{:.1}", params.target_distance_km),
                tolerance_range = %format!("{:.2}-{:.2}", min_distance, max_distance),
                error_pct = %format!("{:.1}", error_pct),
                "Retry {}: {:.2}km outside tolerance {:.2}-{:.2}km ({:.1}% off target)",
                retry + 1, distance_km, min_distance, max_distance, error_pct
            );
        }

        // All retries exhausted
        Err(AppError::RouteGeneration(format!(
            "Could not achieve target distance after {} attempts with {} candidate POIs (wanted {}km ± {}km)",
            self.config.max_route_generation_retries, params.candidate_pois.len(), params.target_distance_km, params.distance_tolerance
        )))
    }

    /// Calculate adjusted target distance based on retry attempt
    /// Uses progressive adjustment strategy to find viable routes
    fn calculate_adjusted_target_distance(target_distance_km: f64, retry: usize) -> f64 {
        if retry == 0 {
            target_distance_km
        } else if retry <= 2 {
            // Try variations around the target
            target_distance_km
                * (DISTANCE_ADJUSTMENT_INITIAL_MULTIPLIER
                    + (retry as f64 * DISTANCE_ADJUSTMENT_INITIAL_STEP))
        } else {
            // More aggressive adjustments for later retries
            target_distance_km
                * (DISTANCE_ADJUSTMENT_AGGRESSIVE_MULTIPLIER
                    + (retry as f64 * DISTANCE_ADJUSTMENT_AGGRESSIVE_STEP))
        }
    }

    /// Check if a distance is within the acceptable tolerance range
    fn is_distance_within_tolerance(
        distance_km: f64,
        min_distance: f64,
        max_distance: f64,
    ) -> bool {
        (min_distance..=max_distance).contains(&distance_km)
    }

    /// Build the complete waypoint sequence for a loop route
    /// Returns: Start → POI1 → POI2 → [POI3] → Start
    fn build_loop_waypoints(start: &Coordinates, selected_pois: &[Poi]) -> Vec<Coordinates> {
        let mut waypoints = vec![*start];
        waypoints.extend(selected_pois.iter().map(|p| p.coordinates));
        waypoints.push(*start); // Return to start
        waypoints
    }
}
