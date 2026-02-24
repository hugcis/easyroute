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

    /// Score a route (public delegation for geometric fallback paths).
    pub fn score_route(
        &self,
        route: &Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> f32 {
        self.route_scorer
            .calculate_route_score(route, target_distance_km, preferences)
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
            return routes;
        }

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
        routes
    }

    /// Select waypoints, order them, verify loop shape, and call Mapbox.
    /// Returns `Ok(Some((directions, ordered_pois)))` on success, `Ok(None)` if shape is bad,
    /// or `Err` on Mapbox failure.
    async fn build_waypoint_route(
        &self,
        params: &LoopRouteParams<'_>,
        corrected_target: f64,
        retry: usize,
    ) -> Result<Option<(crate::services::mapbox::DirectionsResponse, Vec<Poi>)>> {
        let selected_pois = self.waypoint_selector.select_loop_waypoints(
            params.start,
            corrected_target,
            params.candidate_pois,
            params.attempt_seed * self.config.max_route_generation_retries + retry,
            params.preferences,
        )?;

        let ordered_pois = WaypointSelector::order_pois_clockwise(params.start, &selected_pois);

        if !WaypointSelector::verify_loop_shape(params.start, &ordered_pois, retry) {
            tracing::info!(
                retry = retry + 1,
                waypoint_count = ordered_pois.len(),
                "Retry {}: skipped — poor loop shape with {} waypoints",
                retry + 1,
                ordered_pois.len()
            );
            return Ok(None);
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

        Ok(Some((directions, ordered_pois)))
    }

    /// Check if the route distance is within tolerance. If so, build and return the route.
    /// Otherwise, update the distance correction and return `None`.
    #[allow(clippy::too_many_arguments)]
    async fn evaluate_route_distance(
        &self,
        params: &LoopRouteParams<'_>,
        directions: crate::services::mapbox::DirectionsResponse,
        ordered_pois: Vec<Poi>,
        min_distance: f64,
        max_distance: f64,
        distance_correction: &mut f64,
        retry: usize,
    ) -> Result<Option<Route>> {
        let distance_km = directions.distance_km();

        if Self::is_distance_within_tolerance(distance_km, min_distance, max_distance) {
            tracing::info!(
                "Found valid route on attempt {} ({}km, target: {}km ± {}km, correction: {:.2})",
                retry + 1,
                distance_km,
                params.target_distance_km,
                params.distance_tolerance,
                *distance_correction
            );
            let route = self
                .route_scorer
                .build_route(
                    directions,
                    ordered_pois,
                    params.preferences,
                    params.candidate_pois.len(),
                )
                .await?;
            return Ok(Some(route));
        }

        // Feedback correction: adjust based on how far off we were
        let ratio = params.target_distance_km / distance_km;
        *distance_correction *= ratio.powf(DISTANCE_CORRECTION_DAMPING);
        *distance_correction =
            distance_correction.clamp(DISTANCE_CORRECTION_MIN, DISTANCE_CORRECTION_MAX);

        let error_pct =
            (distance_km - params.target_distance_km).abs() / params.target_distance_km * 100.0;
        tracing::info!(
            retry = retry + 1,
            achieved_km = %format!("{:.2}", distance_km),
            target_km = %format!("{:.1}", params.target_distance_km),
            tolerance_range = %format!("{:.2}-{:.2}", min_distance, max_distance),
            error_pct = %format!("{:.1}", error_pct),
            correction = %format!("{:.3}", *distance_correction),
            "Retry {}: {:.2}km outside tolerance {:.2}-{:.2}km ({:.1}% off, next correction: {:.3})",
            retry + 1, distance_km, min_distance, max_distance, error_pct, *distance_correction
        );

        Ok(None)
    }

    /// Try to generate a single loop route with selected waypoints.
    /// Uses feedback-based distance correction: after each out-of-tolerance result,
    /// adjusts the target distance passed to waypoint selection based on the ratio
    /// of target to achieved distance (damped to prevent oscillation).
    pub async fn try_generate_loop(&self, params: LoopRouteParams<'_>) -> Result<Route> {
        let min_distance = params.target_distance_km - params.distance_tolerance;
        let max_distance = params.target_distance_km + params.distance_tolerance;
        let mut distance_correction: f64 = 1.0;

        for retry in 0..self.config.max_route_generation_retries {
            let corrected_target = params.target_distance_km * distance_correction;

            let Some((directions, ordered_pois)) = self
                .build_waypoint_route(&params, corrected_target, retry)
                .await?
            else {
                continue;
            };

            if let Some(route) = self
                .evaluate_route_distance(
                    &params,
                    directions,
                    ordered_pois,
                    min_distance,
                    max_distance,
                    &mut distance_correction,
                    retry,
                )
                .await?
            {
                return Ok(route);
            }
        }

        Err(AppError::RouteGeneration(format!(
            "Could not achieve target distance after {} attempts with {} candidate POIs (wanted {}km ± {}km)",
            self.config.max_route_generation_retries, params.candidate_pois.len(), params.target_distance_km, params.distance_tolerance
        )))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_correction_undershoot() {
        // Simulate: target 5km, achieved 3km → correction should increase
        let target: f64 = 5.0;
        let achieved: f64 = 3.0;
        let ratio = target / achieved; // 1.667
        let correction = ratio.powf(DISTANCE_CORRECTION_DAMPING); // ~1.44
        assert!(
            correction > 1.3 && correction < 1.6,
            "Undershoot correction should push target up: got {:.3}",
            correction
        );
    }

    #[test]
    fn test_feedback_correction_overshoot() {
        // Simulate: target 5km, achieved 7km → correction should decrease
        let target: f64 = 5.0;
        let achieved: f64 = 7.0;
        let ratio = target / achieved; // 0.714
        let correction = ratio.powf(DISTANCE_CORRECTION_DAMPING); // ~0.78
        assert!(
            correction > 0.7 && correction < 0.9,
            "Overshoot correction should push target down: got {:.3}",
            correction
        );
    }

    #[test]
    fn test_feedback_correction_clamping() {
        // Extreme undershoot: target 10km, achieved 1km → should clamp to max
        let target: f64 = 10.0;
        let achieved: f64 = 1.0;
        let ratio = target / achieved; // 10.0
        let correction = ratio
            .powf(DISTANCE_CORRECTION_DAMPING)
            .clamp(DISTANCE_CORRECTION_MIN, DISTANCE_CORRECTION_MAX);
        assert_eq!(
            correction, DISTANCE_CORRECTION_MAX,
            "Extreme undershoot should clamp to max: got {:.3}",
            correction
        );

        // Extreme overshoot: target 1km, achieved 10km → should clamp to min
        let ratio_over = 1.0_f64 / 10.0;
        let correction_over = ratio_over
            .powf(DISTANCE_CORRECTION_DAMPING)
            .clamp(DISTANCE_CORRECTION_MIN, DISTANCE_CORRECTION_MAX);
        assert_eq!(
            correction_over, DISTANCE_CORRECTION_MIN,
            "Extreme overshoot should clamp to min: got {:.3}",
            correction_over
        );
    }

    #[test]
    fn test_feedback_correction_accumulates() {
        // Two consecutive undershoots: correction should compound
        let target: f64 = 5.0;
        let mut correction: f64 = 1.0;

        // First attempt: achieved 3.5km
        let ratio1 = target / 3.5;
        correction *= ratio1.powf(DISTANCE_CORRECTION_DAMPING);
        correction = correction.clamp(DISTANCE_CORRECTION_MIN, DISTANCE_CORRECTION_MAX);
        let after_first = correction;

        // Second attempt: still short at 4.0km (but closer)
        let ratio2 = target / 4.0;
        correction *= ratio2.powf(DISTANCE_CORRECTION_DAMPING);
        correction = correction.clamp(DISTANCE_CORRECTION_MIN, DISTANCE_CORRECTION_MAX);

        assert!(
            correction > after_first,
            "Correction should accumulate: first={:.3}, second={:.3}",
            after_first,
            correction
        );
    }
}
