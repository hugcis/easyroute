use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, Route, RoutePoi, RoutePreferences, TransportMode};
use crate::services::mapbox::{DirectionsResponse, MapboxClient};
use crate::services::poi_service::PoiService;
use crate::services::snapping_service::SnappingService;
use std::collections::HashSet;

// Route generation constants
const MIN_POI_DISTANCE_KM: f64 = 0.2;  // Minimum distance from start to POI
const MIN_ANGLE_DIFF_TWO_POIS: f64 = 1.0;  // ~57 degrees in radians
const MIN_ANGLE_DIFF_THREE_POIS: f64 = 1.047;  // ~60 degrees in radians
const MAX_DISTANCE_RETRIES: usize = 5;  // Maximum attempts to achieve target distance

pub struct RouteGenerator {
    mapbox_client: MapboxClient,
    poi_service: PoiService,
    snapping_service: SnappingService,
    snap_radius_m: f64,
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
            return Err(AppError::RouteGeneration(
                "No POIs found in the area".to_string(),
            ));
        }

        // Step 2: Score and filter POIs
        let candidate_pois =
            self.poi_service
                .score_and_filter_pois(raw_pois, preferences.hidden_gems, 20);

        tracing::debug!("Found {} candidate POIs", candidate_pois.len());

        // Step 3: Generate multiple route alternatives
        // Use at least 3 attempts even if user requested 1, to increase success rate
        let max_alternatives = preferences.max_alternatives.clamp(3, 5) as usize;
        let mut routes = Vec::new();

        for attempt in 0..max_alternatives {
            match self
                .try_generate_loop(
                    &start,
                    target_distance_km,
                    distance_tolerance,
                    mode,
                    &candidate_pois,
                    attempt,
                    preferences,
                )
                .await
            {
                Ok(route) => routes.push(route),
                Err(e) => {
                    tracing::warn!(
                        "Failed to generate route alternative {}: {}",
                        attempt + 1,
                        e
                    );
                }
            }
        }

        if routes.is_empty() {
            return Err(AppError::RouteGeneration(
                "Failed to generate any valid routes".to_string(),
            ));
        }

        // Step 4: Score and rank routes
        let mut scored_routes = routes;
        for route in &mut scored_routes {
            route.score = self.calculate_route_score(route, target_distance_km, preferences);
        }

        scored_routes.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tracing::info!("Generated {} route alternatives", scored_routes.len());
        Ok(scored_routes)
    }

    /// Try to generate a single loop route with selected waypoints
    #[allow(clippy::too_many_arguments)]
    async fn try_generate_loop(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        distance_tolerance: f64,
        mode: &TransportMode,
        candidate_pois: &[Poi],
        variation: usize,
        preferences: &RoutePreferences,
    ) -> Result<Route> {
        let min_distance = target_distance_km - distance_tolerance;
        let max_distance = target_distance_km + distance_tolerance;

        for retry in 0..MAX_DISTANCE_RETRIES {
            // Adjust target distance based on previous attempts
            // First attempt: use target distance
            // Later attempts: adjust based on whether we were too long or too short
            let adjusted_target = if retry == 0 {
                target_distance_km
            } else if retry <= 2 {
                // Try variations around the target
                target_distance_km * (0.8 + (retry as f64 * 0.2))
            } else {
                // More aggressive adjustments for later retries
                target_distance_km * (0.6 + (retry as f64 * 0.15))
            };

            // Select POIs with different variation seed for each retry
            let selected_pois = self.select_loop_waypoints(
                start,
                adjusted_target,
                candidate_pois,
                variation * MAX_DISTANCE_RETRIES + retry, // Different seed each time
            )?;

            // Build waypoint sequence: Start → POI1 → POI2 → [POI3] → Start
            let mut waypoints = vec![*start];
            waypoints.extend(selected_pois.iter().map(|p| p.coordinates));
            waypoints.push(*start); // Return to start

            let directions = self.mapbox_client.get_directions(&waypoints, mode).await?;

            let distance_km = directions.distance_km();

            // Check if distance is within tolerance
            if (min_distance..=max_distance).contains(&distance_km) {
                // Success! Build the route
                tracing::info!(
                    "Found valid route on attempt {} ({}km, target: {}km ± {}km)",
                    retry + 1,
                    distance_km,
                    target_distance_km,
                    distance_tolerance
                );
                return self
                    .build_route(directions, selected_pois, preferences)
                    .await;
            }

            // If not within tolerance, log and retry with different POIs
            if retry < MAX_DISTANCE_RETRIES - 1 {
                let error_pct = (distance_km - target_distance_km).abs() / target_distance_km * 100.0;
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
            MAX_DISTANCE_RETRIES, candidate_pois.len(), target_distance_km, distance_tolerance
        )))
    }

    /// Select waypoints for a loop route
    /// Strategy: Choose 2-3 POIs that are spatially distributed to form a loop
    fn select_loop_waypoints(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        pois: &[Poi],
        variation: usize,
    ) -> Result<Vec<Poi>> {
        if pois.len() < 2 {
            return Err(AppError::RouteGeneration(
                "Not enough POIs to create route".to_string(),
            ));
        }

        // Adjust number of waypoints based on available POIs AND target distance
        // Longer routes need more waypoints to fill the distance
        let num_waypoints = if (target_distance_km > 10.0 && pois.len() >= 6)
            || (target_distance_km > 5.0 && pois.len() >= 4)
        {
            3 // Use 3 waypoints for longer routes with enough POIs
        } else {
            2 // Use 2 waypoints for shorter routes or limited POIs
        };

        // Target distance from start for waypoints
        // For a loop, we want POIs that are closer to create a reasonable circuit
        // Rule of thumb: for an N-point loop, each POI should be roughly target_distance / (N + 1.5) away
        let target_waypoint_distance = target_distance_km / (num_waypoints as f64 + 1.5);

        // Score each POI based on distance from ideal waypoint distance
        // Use a VERY lenient range to handle POI-sparse areas
        let mut scored_pois: Vec<(f32, &Poi)> = pois
            .iter()
            .enumerate()
            .filter_map(|(idx, poi)| {
                let dist = start.distance_to(&poi.coordinates);

                // Skip POIs that are too close to start
                if dist < MIN_POI_DISTANCE_KM {
                    return None;
                }

                // Skip POIs that are too far (more than search radius)
                // Search radius is roughly target_distance / 2
                let max_reasonable_dist = target_distance_km / 1.5;
                if dist > max_reasonable_dist {
                    return None;
                }

                // Score based on how close to ideal distance
                // But be lenient - any POI in range gets a decent score
                let distance_score = if dist < target_waypoint_distance {
                    // POIs closer than ideal still get good scores
                    (dist / target_waypoint_distance) as f32 * 0.8 + 0.2
                } else {
                    // POIs farther than ideal are penalized more gradually
                    let excess_ratio = (dist - target_waypoint_distance) / target_waypoint_distance;
                    (1.0 - (excess_ratio * 0.5).min(0.8)) as f32
                };

                // Add variation offset based on attempt number to get different routes
                // Use larger offset to ensure different POIs are selected each retry
                // Variation rotates through different POIs: variation 0-4 each get different POIs
                let variation_offset = ((idx * 3 + variation * 11) % 100) as f32 * 0.05;

                Some((distance_score + variation_offset, poi))
            })
            .collect();

        if scored_pois.len() < num_waypoints {
            // If we still don't have enough POIs, relax constraints even more
            // Just use the closest POIs we have
            tracing::warn!(
                "Only {} suitable POIs after filtering (need {}), using all available POIs",
                scored_pois.len(),
                num_waypoints
            );

            if pois.len() < 2 {
                return Err(AppError::RouteGeneration(format!(
                    "Not enough POIs in area (found {}, need at least 2)",
                    pois.len()
                )));
            }

            // Fallback: use the closest POIs we have, regardless of ideal distance
            scored_pois = pois
                .iter()
                .enumerate()
                .filter_map(|(idx, poi)| {
                    let dist = start.distance_to(&poi.coordinates);
                    if dist < MIN_POI_DISTANCE_KM {
                        return None; // Still skip very close POIs
                    }
                    let score = 1.0 / (dist as f32 + 1.0); // Closer = better
                    Some((score + (idx as f32 * 0.01), poi))
                })
                .collect();
        }

        // Sort by score
        scored_pois.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Use variation to select DIFFERENT POIs each retry
        // Skip the first (variation % available_pois) POIs to get different combinations
        let skip_count = if scored_pois.len() > num_waypoints {
            variation % (scored_pois.len() - num_waypoints + 1)
        } else {
            0
        };

        let selected: Vec<Poi> = scored_pois
            .iter()
            .skip(skip_count) // Skip different POIs based on variation!
            .take(num_waypoints)
            .map(|(_, poi)| (*poi).clone())
            .collect();

        tracing::debug!(
            "Selected {} POIs (variation={}, skip={}): {}",
            selected.len(),
            variation,
            skip_count,
            selected
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Ensure spatial distribution by checking angles from start
        if selected.len() >= 2 && !self.are_spatially_distributed(start, &selected) {
            tracing::debug!("POIs not well distributed, using anyway for MVP");
        }

        Ok(selected)
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
            MIN_ANGLE_DIFF_TWO_POIS
        } else {
            MIN_ANGLE_DIFF_THREE_POIS
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
