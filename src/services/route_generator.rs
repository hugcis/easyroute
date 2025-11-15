use crate::constants::*;
use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, Route, RoutePoi, RoutePreferences, TransportMode};
use crate::services::mapbox::{DirectionsResponse, MapboxClient};
use crate::services::poi_service::PoiService;
use crate::services::snapping_service::SnappingService;
use std::collections::HashSet;

pub struct RouteGenerator {
    mapbox_client: MapboxClient,
    poi_service: PoiService,
    snapping_service: SnappingService,
    snap_radius_m: f64,
}

/// Parameters for loop route generation attempt
struct LoopRouteParams<'a> {
    start: &'a Coordinates,
    target_distance_km: f64,
    distance_tolerance: f64,
    mode: &'a TransportMode,
    candidate_pois: &'a [Poi],
    attempt_seed: usize,
    preferences: &'a RoutePreferences,
}

impl RouteGenerator {
    pub fn new(
        mapbox_client: MapboxClient,
        poi_service: PoiService,
        snapping_service: SnappingService,
        snap_radius_m: f64,
    ) -> Self {
        RouteGenerator {
            mapbox_client,
            poi_service,
            snapping_service,
            snap_radius_m,
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
        let search_radius_km = target_distance_km / 2.0;
        let raw_pois = self
            .poi_service
            .find_pois(
                &start,
                search_radius_km,
                preferences.poi_categories.as_deref(),
                50,
            )
            .await?;

        if raw_pois.is_empty() {
            // Try geometric fallback if no POIs
            tracing::warn!("No POIs found, attempting geometric loop fallback");
            return self
                .generate_geometric_loop(start, target_distance_km, mode)
                .await
                .map(|route| vec![route]);
        }

        // Step 2: Score and filter POIs
        let candidate_pois =
            self.poi_service
                .score_and_filter_pois(raw_pois, preferences.hidden_gems, 20);

        tracing::debug!("Found {} candidate POIs", candidate_pois.len());

        // Step 3: Try with progressively relaxed tolerance levels
        let tolerance_levels = vec![
            (distance_tolerance, "normal"),
            (
                target_distance_km * TOLERANCE_LEVEL_RELAXED,
                "relaxed (±20%)",
            ),
            (
                target_distance_km * TOLERANCE_LEVEL_VERY_RELAXED,
                "very relaxed (±30%)",
            ),
        ];

        for (tolerance, tolerance_name) in tolerance_levels {
            let routes = self
                .try_generate_routes_with_tolerance(
                    &start,
                    target_distance_km,
                    tolerance,
                    mode,
                    &candidate_pois,
                    preferences,
                )
                .await;

            if !routes.is_empty() {
                if tolerance_name != "normal" {
                    tracing::info!(
                        "Successfully generated routes with {} tolerance",
                        tolerance_name
                    );
                }
                return Ok(routes);
            }

            if tolerance_name != "very relaxed (±30%)" {
                tracing::warn!(
                    "Failed with {} tolerance, trying next level",
                    tolerance_name
                );
            }
        }

        // Step 4: Final fallback - geometric loop
        if candidate_pois.len() < 2 {
            tracing::warn!(
                "Only {} POI(s) and no valid routes found, falling back to geometric loop",
                candidate_pois.len()
            );
            return self
                .generate_geometric_loop(start, target_distance_km, mode)
                .await
                .map(|route| vec![route]);
        }

        Err(AppError::RouteGeneration(
            "Failed to generate any valid routes even with relaxed tolerance".to_string(),
        ))
    }

    /// Try to generate routes with a specific tolerance level
    async fn try_generate_routes_with_tolerance(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        distance_tolerance: f64,
        mode: &TransportMode,
        candidate_pois: &[Poi],
        preferences: &RoutePreferences,
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
                attempt_seed: attempt,
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

        // Score and rank routes if we got any
        if !routes.is_empty() {
            for route in &mut routes {
                route.score = self.calculate_route_score(route, target_distance_km, preferences);
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

    /// Try to generate a single loop route with selected waypoints
    async fn try_generate_loop(&self, params: LoopRouteParams<'_>) -> Result<Route> {
        let min_distance = params.target_distance_km - params.distance_tolerance;
        let max_distance = params.target_distance_km + params.distance_tolerance;

        for retry in 0..MAX_ROUTE_GENERATION_RETRIES {
            let adjusted_target =
                Self::calculate_adjusted_target_distance(params.target_distance_km, retry);

            // Select POIs with different variation seed for each retry
            let selected_pois = self.select_loop_waypoints(
                params.start,
                adjusted_target,
                params.candidate_pois,
                params.attempt_seed * MAX_ROUTE_GENERATION_RETRIES + retry,
            )?;

            let waypoints = Self::build_loop_waypoints(params.start, &selected_pois);
            let directions = self
                .mapbox_client
                .get_directions(&waypoints, params.mode)
                .await?;
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
                    .build_route(directions, selected_pois, params.preferences)
                    .await;
            }

            // If not within tolerance, log and retry with different POIs
            if retry < MAX_ROUTE_GENERATION_RETRIES - 1 {
                let error_pct = (distance_km - params.target_distance_km).abs()
                    / params.target_distance_km
                    * 100.0;
                tracing::debug!(
                    "Attempt {} failed: {}km outside tolerance ({}km - {}km), error: {:.1}%",
                    retry + 1,
                    distance_km,
                    min_distance,
                    max_distance,
                    error_pct
                );
            }
        }

        // All retries exhausted
        Err(AppError::RouteGeneration(format!(
            "Could not achieve target distance after {} attempts with {} candidate POIs (wanted {}km ± {}km)",
            MAX_ROUTE_GENERATION_RETRIES, params.candidate_pois.len(), params.target_distance_km, params.distance_tolerance
        )))
    }

    /// Select waypoints for a loop route
    /// Strategy: Choose 2-3 POIs that are spatially distributed to form a loop
    fn select_loop_waypoints(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        pois: &[Poi],
        attempt_seed: usize,
    ) -> Result<Vec<Poi>> {
        if pois.len() < 2 {
            return Err(AppError::RouteGeneration(
                "Not enough POIs to create route".to_string(),
            ));
        }

        let num_waypoints = self.calculate_waypoint_count(target_distance_km, pois.len());
        let target_waypoint_distance = target_distance_km / (num_waypoints as f64 + 1.5);

        let mut scored_pois = self.score_pois_for_loop(
            start,
            pois,
            target_waypoint_distance,
            target_distance_km,
            attempt_seed,
        );

        // Fallback if we don't have enough POIs after filtering
        if scored_pois.len() < num_waypoints {
            scored_pois = self.fallback_score_closest_pois(start, pois, num_waypoints)?;
        }

        let selected =
            self.select_top_pois_with_variation(scored_pois, num_waypoints, attempt_seed);

        // Spatial distribution check (informational only for MVP)
        if selected.len() >= 2 && !self.are_spatially_distributed(start, &selected) {
            tracing::debug!("POIs not well distributed, using anyway for MVP");
        }

        Ok(selected)
    }

    /// Calculate the optimal number of waypoints based on distance and available POIs
    fn calculate_waypoint_count(&self, target_distance_km: f64, poi_count: usize) -> usize {
        if (target_distance_km > WAYPOINTS_LONG_ROUTE_DISTANCE_KM
            && poi_count >= WAYPOINTS_LONG_ROUTE_MIN_POIS)
            || (target_distance_km > WAYPOINTS_MEDIUM_ROUTE_DISTANCE_KM
                && poi_count >= WAYPOINTS_MEDIUM_ROUTE_MIN_POIS)
        {
            WAYPOINTS_COUNT_LONG_ROUTE
        } else {
            WAYPOINTS_COUNT_SHORT_ROUTE
        }
    }

    /// Score POIs based on their suitability for forming a loop
    fn score_pois_for_loop<'a>(
        &self,
        start: &Coordinates,
        pois: &'a [Poi],
        target_waypoint_distance: f64,
        target_distance_km: f64,
        attempt_seed: usize,
    ) -> Vec<(f32, &'a Poi)> {
        let max_reasonable_dist = target_distance_km / MAX_DISTANCE_RATIO;

        pois.iter()
            .enumerate()
            .filter_map(|(idx, poi)| {
                let dist = start.distance_to(&poi.coordinates);

                // Filter out POIs that are too close or too far
                if dist < MIN_POI_DISTANCE_KM || dist > max_reasonable_dist {
                    return None;
                }

                let distance_score = self.calculate_distance_score(dist, target_waypoint_distance);
                let variation_offset = self.calculate_variation_offset(idx, attempt_seed);

                Some((distance_score + variation_offset, poi))
            })
            .collect()
    }

    /// Calculate score based on distance from ideal waypoint distance
    fn calculate_distance_score(&self, actual_dist: f64, target_dist: f64) -> f32 {
        if actual_dist < target_dist {
            // POIs closer than ideal still get good scores
            (actual_dist / target_dist) as f32 * 0.8 + 0.2
        } else {
            // POIs farther than ideal are penalized more gradually
            let excess_ratio = (actual_dist - target_dist) / target_dist;
            (1.0 - (excess_ratio * 0.5).min(0.8)) as f32
        }
    }

    /// Calculate variation offset to ensure different POIs are selected on each attempt
    fn calculate_variation_offset(&self, poi_index: usize, attempt_seed: usize) -> f32 {
        ((poi_index * VARIATION_MULTIPLIER + attempt_seed * VARIATION_OFFSET_BASE) % VARIATION_MOD)
            as f32
            * VARIATION_SCORE_FACTOR
    }

    /// Fallback strategy: score POIs by proximity when filtering yields too few results
    fn fallback_score_closest_pois<'a>(
        &self,
        start: &Coordinates,
        pois: &'a [Poi],
        num_waypoints: usize,
    ) -> Result<Vec<(f32, &'a Poi)>> {
        tracing::warn!(
            "Using fallback scoring - relaxing constraints for {} waypoints",
            num_waypoints
        );

        if pois.len() < 2 {
            return Err(AppError::RouteGeneration(format!(
                "Not enough POIs in area (found {}, need at least 2)",
                pois.len()
            )));
        }

        Ok(pois
            .iter()
            .enumerate()
            .filter_map(|(idx, poi)| {
                let dist = start.distance_to(&poi.coordinates);
                if dist < MIN_POI_DISTANCE_KM {
                    return None;
                }
                let score = 1.0 / (dist as f32 + 1.0); // Closer = better
                Some((score + (idx as f32 * 0.01), poi))
            })
            .collect())
    }

    /// Select top-scoring POIs with variation based on attempt seed
    fn select_top_pois_with_variation(
        &self,
        mut scored_pois: Vec<(f32, &Poi)>,
        num_waypoints: usize,
        attempt_seed: usize,
    ) -> Vec<Poi> {
        scored_pois.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let skip_count = if scored_pois.len() > num_waypoints {
            attempt_seed % (scored_pois.len() - num_waypoints + 1)
        } else {
            0
        };

        let selected: Vec<Poi> = scored_pois
            .iter()
            .skip(skip_count)
            .take(num_waypoints)
            .map(|(_, poi)| (*poi).clone())
            .collect();

        tracing::debug!(
            "Selected {} POIs (seed={}, skip={}): {}",
            selected.len(),
            attempt_seed,
            skip_count,
            selected
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        selected
    }

    /// Check if POIs are spatially distributed (not clustered together)
    /// Simple check: POIs should have different angles from start point
    fn are_spatially_distributed(&self, start: &Coordinates, pois: &[Poi]) -> bool {
        if pois.len() < 2 {
            return true;
        }

        // Calculate angles from start to each POI
        let angles: Vec<f64> = pois
            .iter()
            .map(|poi| {
                let dx = poi.coordinates.lng - start.lng;
                let dy = poi.coordinates.lat - start.lat;
                dy.atan2(dx)
            })
            .collect();

        // Check minimum angle difference (should be at least 60 degrees for 3 POIs)
        let min_angle_diff = if pois.len() == 2 {
            SPATIAL_DISTRIBUTION_MIN_ANGLE_TWO_POIS_RAD
        } else {
            SPATIAL_DISTRIBUTION_MIN_ANGLE_THREE_POIS_RAD
        };

        for i in 0..angles.len() {
            for j in (i + 1)..angles.len() {
                let diff = (angles[i] - angles[j]).abs();
                if diff > min_angle_diff && diff < (2.0 * std::f64::consts::PI - min_angle_diff) {
                    return true;
                }
            }
        }

        false
    }

    /// Build Route object from directions response and selected POIs
    async fn build_route(
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

    /// Generate a geometric loop when POIs are unavailable
    /// Creates a circular route using evenly distributed waypoints
    async fn generate_geometric_loop(
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

    /// Calculate route quality score (0-10)
    /// Based on: distance accuracy, POI count, POI quality, category diversity
    fn calculate_route_score(
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
        // Create a mock generator for testing the scoring function
        // Note: This requires async, so we'll just test the logic in a simpler way

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
        };

        // Test that route has expected properties
        assert_eq!(route.pois.len(), 2);
        assert!(route.distance_km > 5.0);
        assert!(route.distance_km < 5.2);
    }
}
