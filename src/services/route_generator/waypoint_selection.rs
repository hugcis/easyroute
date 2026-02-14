use crate::config::{RouteGeneratorConfig, ScoringStrategy};
use crate::constants::*;
use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, RoutePreferences};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

use super::scoring_strategy::{
    AdvancedStrategy, PoiScoringStrategy, ScoringContext, SimpleStrategy,
};

/// Handles waypoint selection and spatial ordering for loop routes
pub struct WaypointSelector {
    config: RouteGeneratorConfig,
    scoring_strategy: Box<dyn PoiScoringStrategy>,
}

impl WaypointSelector {
    pub fn new(config: RouteGeneratorConfig) -> Self {
        // Select scoring strategy based on configuration
        let scoring_strategy: Box<dyn PoiScoringStrategy> = match config.poi_scoring_strategy {
            ScoringStrategy::Simple => {
                tracing::info!("Using Simple POI scoring strategy (distance-only)");
                Box::new(SimpleStrategy::new(config.clone()))
            }
            ScoringStrategy::Advanced => {
                tracing::info!("Using Advanced POI scoring strategy (quality + clustering + angular diversity)");
                Box::new(AdvancedStrategy::new(config.clone()))
            }
        };

        Self {
            config,
            scoring_strategy,
        }
    }

    /// Select waypoints for a loop route
    /// Strategy: Choose 2-4 POIs that are spatially distributed to form a loop
    pub fn select_loop_waypoints(
        &self,
        start: &Coordinates,
        target_distance_km: f64,
        pois: &[Poi],
        attempt_seed: usize,
        preferences: &RoutePreferences,
    ) -> Result<Vec<Poi>> {
        if pois.len() < 2 {
            return Err(AppError::RouteGeneration(
                "Not enough POIs to create route".to_string(),
            ));
        }

        let num_waypoints =
            self.calculate_waypoint_count(target_distance_km, pois.len(), attempt_seed);
        let multiplier = self.get_waypoint_distance_multiplier(num_waypoints);
        let target_waypoint_distance = target_distance_km * multiplier;

        tracing::debug!(
            "Attempt {}: Using {} waypoints with multiplier {:.2} (target waypoint distance: {:.2}km)",
            attempt_seed,
            num_waypoints,
            multiplier,
            target_waypoint_distance
        );

        // Use iterative selection with strategy pattern
        let selected = self.select_pois_iteratively(
            start,
            pois,
            target_waypoint_distance,
            target_distance_km,
            num_waypoints,
            attempt_seed,
            preferences,
        )?;

        // Spatial distribution check (informational only for MVP)
        if selected.len() >= 2 && !Self::are_spatially_distributed(start, &selected) {
            tracing::debug!("POIs not well distributed, using anyway for MVP");
        }

        tracing::debug!(
            "Selected {} POIs (seed={}): {}",
            selected.len(),
            attempt_seed,
            selected
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(selected)
    }

    /// Iteratively select POIs using the scoring strategy
    /// This allows the strategy to consider already-selected POIs for clustering/angular diversity
    #[allow(clippy::too_many_arguments)]
    fn select_pois_iteratively(
        &self,
        start: &Coordinates,
        pois: &[Poi],
        target_waypoint_distance: f64,
        target_distance_km: f64,
        num_waypoints: usize,
        attempt_seed: usize,
        preferences: &RoutePreferences,
    ) -> Result<Vec<Poi>> {
        let mut selected: Vec<Poi> = Vec::new();
        let mut remaining_pois: Vec<Poi> = pois.to_vec();

        for iteration in 0..num_waypoints {
            if remaining_pois.is_empty() {
                break;
            }

            // Create scoring context
            let context = ScoringContext {
                start,
                target_waypoint_distance,
                target_distance_km,
                attempt_seed,
                preferences,
                already_selected: &selected,
            };

            // Score remaining POIs using strategy
            let mut scored_pois = self.scoring_strategy.score_pois(&remaining_pois, &context);

            // Fallback if no POIs scored
            if scored_pois.is_empty() {
                tracing::warn!(
                    iteration = iteration,
                    remaining_pois = remaining_pois.len(),
                    min_distance_km = %format!("{:.2}", self.config.min_poi_distance_km),
                    max_distance_mult = self.config.max_poi_distance_multiplier,
                    "No POIs scored in iteration {} ({} remaining, filter: min {:.2}km, max mult {:.1}x), using fallback",
                    iteration, remaining_pois.len(), self.config.min_poi_distance_km, self.config.max_poi_distance_multiplier
                );
                scored_pois = self.fallback_score_closest_pois(start, &remaining_pois, 1)?;
            }

            // If still no POIs after fallback, break the loop
            if scored_pois.is_empty() {
                tracing::warn!(
                    iteration = iteration,
                    selected = selected.len(),
                    remaining = remaining_pois.len(),
                    "No POIs available after fallback in iteration {} (selected: {}, remaining: {})",
                    iteration, selected.len(), remaining_pois.len()
                );
                break;
            }

            // Sort by score descending
            scored_pois.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            // Select top POI with some randomization
            let pool_size = (scored_pois.len() / 3).max(1).min(scored_pois.len());
            let mut rng = StdRng::seed_from_u64((attempt_seed + selected.len()) as u64);
            let selected_poi = scored_pois[..pool_size]
                .choose(&mut rng)
                .map(|(_, poi)| (*poi).clone())
                .unwrap_or_else(|| scored_pois[0].1.clone());

            // Remove selected POI from remaining
            remaining_pois.retain(|poi| poi.id != selected_poi.id);

            selected.push(selected_poi);
        }

        if selected.len() < 2 {
            return Err(AppError::RouteGeneration(format!(
                "Could only select {} POI(s), need at least 2",
                selected.len()
            )));
        }

        Ok(selected)
    }

    /// Order POIs in clockwise direction around start point for efficient loop routing
    pub fn order_pois_clockwise(start: &Coordinates, pois: &[Poi]) -> Vec<Poi> {
        if pois.len() <= 1 {
            return pois.to_vec();
        }

        let mut pois_with_angles: Vec<(f64, Poi)> = pois
            .iter()
            .map(|poi| {
                // Calculate angle from start point
                let dx = poi.coordinates.lng - start.lng;
                let dy = poi.coordinates.lat - start.lat;
                let angle = dy.atan2(dx); // Returns angle in radians (-π to π)
                (angle, poi.clone())
            })
            .collect();

        // Sort by angle (creates clockwise traversal from east)
        pois_with_angles.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        pois_with_angles.into_iter().map(|(_, poi)| poi).collect()
    }

    /// Calculate the optimal number of waypoints based on distance, available POIs, and attempt seed
    /// Uses alternating pattern: even seeds → fewer waypoints, odd seeds → more waypoints
    fn calculate_waypoint_count(
        &self,
        target_distance_km: f64,
        poi_count: usize,
        attempt_seed: usize,
    ) -> usize {
        // Ensure minimum POI availability
        if poi_count < self.config.poi_count_threshold_long {
            return self.config.waypoints_count_short;
        }

        // Determine range based on distance
        let (min_wp, max_wp) = if target_distance_km > self.config.long_route_threshold_km {
            // Long routes: use 3 or 4 waypoints
            (
                self.config.waypoints_count_medium,
                self.config.waypoints_count_long,
            )
        } else {
            // Short routes: use 2 or 3 waypoints
            (
                self.config.waypoints_count_short,
                self.config.waypoints_count_medium,
            )
        };

        // Short routes: prefer max (3wp), fall back to min (2wp) every 3rd attempt
        // Long routes: alternate evenly between min (3wp) and max (4wp)
        if target_distance_km <= self.config.long_route_threshold_km {
            if attempt_seed % 3 == 2 {
                min_wp
            } else {
                max_wp
            }
        } else if attempt_seed % 2 == 0 {
            min_wp
        } else {
            max_wp
        }
    }

    /// Get the appropriate distance multiplier for the given waypoint count
    /// More waypoints require tighter loops (smaller multiplier) to prevent overshooting
    fn get_waypoint_distance_multiplier(&self, waypoint_count: usize) -> f64 {
        match waypoint_count {
            2 => self.config.waypoint_distance_multiplier_2wp,
            3 => self.config.waypoint_distance_multiplier_3wp,
            4 => self.config.waypoint_distance_multiplier_4wp,
            _ => {
                tracing::warn!(
                    "Unexpected waypoint count {}, using 3-waypoint multiplier",
                    waypoint_count
                );
                self.config.waypoint_distance_multiplier_3wp
            }
        }
    }

    /// Fallback strategy: score POIs by proximity when filtering yields too few results
    fn fallback_score_closest_pois<'a, P>(
        &self,
        start: &Coordinates,
        pois: &'a [P],
        num_waypoints: usize,
    ) -> Result<Vec<(f32, &'a Poi)>>
    where
        P: std::borrow::Borrow<Poi>,
    {
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
            .filter_map(|(idx, poi_ref)| {
                let poi = poi_ref.borrow();
                let dist = start.distance_to(&poi.coordinates);
                if dist < self.config.min_poi_distance_km {
                    return None;
                }
                let score = 1.0 / (dist as f32 + 1.0); // Closer = better
                Some((score + (idx as f32 * 0.01), poi))
            })
            .collect())
    }

    /// Verify that clockwise-ordered waypoints form a good loop shape.
    /// Checks that consecutive waypoints have minimum angular separation
    /// to prevent out-and-back routes.
    /// The `retry` parameter progressively relaxes the threshold:
    /// - Retry 0-1: full threshold (strict)
    /// - Retry 2: 80% threshold
    /// - Retry 3: 60% threshold
    /// - Retry 4+: 40% threshold
    ///
    /// Returns true if the shape is acceptable.
    pub fn verify_loop_shape(start: &Coordinates, ordered_pois: &[Poi], retry: usize) -> bool {
        if ordered_pois.len() < 2 {
            return true;
        }

        let num_waypoints = ordered_pois.len();
        let base_min_gap = std::f64::consts::PI / (num_waypoints as f64 + 1.0);
        let relaxation = (retry.saturating_sub(1) as f64 * 0.2).min(0.6);
        let min_gap = base_min_gap * (1.0 - relaxation);

        // Calculate angles for each POI
        let angles: Vec<f64> = ordered_pois
            .iter()
            .map(|poi| {
                let dx = poi.coordinates.lng - start.lng;
                let dy = poi.coordinates.lat - start.lat;
                dy.atan2(dx)
            })
            .collect();

        // Check consecutive angular gaps (including wrap-around)
        for i in 0..angles.len() {
            let j = (i + 1) % angles.len();
            let mut diff = angles[j] - angles[i];

            // Normalize to [0, 2*PI)
            if diff < 0.0 {
                diff += 2.0 * std::f64::consts::PI;
            }

            if diff < min_gap {
                tracing::debug!(
                    "Loop shape rejected: angular gap {:.2} rad between waypoints {} and {} is below minimum {:.2} rad",
                    diff,
                    i,
                    j,
                    min_gap
                );
                return false;
            }
        }

        true
    }

    /// Check if POIs are spatially distributed (not clustered together)
    /// Simple check: POIs should have different angles from start point
    fn are_spatially_distributed(start: &Coordinates, pois: &[Poi]) -> bool {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RouteGeneratorConfig;

    #[test]
    fn test_waypoint_count_varies_by_attempt_seed() {
        let config = RouteGeneratorConfig::default();
        let selector = WaypointSelector::new(config);

        // Short routes (5km): prefer 3wp, fall back to 2wp every 3rd attempt (seed%3==2)
        assert_eq!(
            selector.calculate_waypoint_count(5.0, 10, 0),
            3,
            "Attempt 0 should use 3 waypoints for short route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(5.0, 10, 1),
            3,
            "Attempt 1 should use 3 waypoints for short route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(5.0, 10, 2),
            2,
            "Attempt 2 should use 2 waypoints for short route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(5.0, 10, 3),
            3,
            "Attempt 3 should use 3 waypoints for short route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(5.0, 10, 4),
            3,
            "Attempt 4 should use 3 waypoints for short route"
        );

        // Long routes (10km): even seeds=3wp, odd seeds=4wp
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 10, 0),
            3,
            "Attempt 0 should use 3 waypoints for long route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 10, 1),
            4,
            "Attempt 1 should use 4 waypoints for long route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 10, 2),
            3,
            "Attempt 2 should use 3 waypoints for long route"
        );
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 10, 3),
            4,
            "Attempt 3 should use 4 waypoints for long route"
        );
    }

    #[test]
    fn test_waypoint_count_respects_poi_threshold() {
        let config = RouteGeneratorConfig::default();
        let selector = WaypointSelector::new(config);

        // With only 2 POIs (below threshold of 3), should always use minimum (2)
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 2, 0),
            2,
            "Should fallback to 2 waypoints when insufficient POIs (even seed)"
        );
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 2, 1),
            2,
            "Should fallback to 2 waypoints when insufficient POIs (odd seed)"
        );

        // At exactly the threshold (3 POIs), should allow variation
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 3, 0),
            3,
            "With 3 POIs, should allow 3 waypoints"
        );
        assert_eq!(
            selector.calculate_waypoint_count(10.0, 3, 1),
            4,
            "With 3 POIs, should allow 4 waypoints"
        );
    }

    #[test]
    fn test_distance_multiplier_per_waypoint_count() {
        let config = RouteGeneratorConfig::default();
        let selector = WaypointSelector::new(config);

        assert_eq!(
            selector.get_waypoint_distance_multiplier(2),
            0.50,
            "2 waypoints should use 0.50 multiplier"
        );
        assert_eq!(
            selector.get_waypoint_distance_multiplier(3),
            0.35,
            "3 waypoints should use 0.35 multiplier"
        );
        assert_eq!(
            selector.get_waypoint_distance_multiplier(4),
            0.28,
            "4 waypoints should use 0.28 multiplier"
        );

        // Unknown count should fall back to 3-waypoint multiplier
        assert_eq!(
            selector.get_waypoint_distance_multiplier(5),
            0.35,
            "Unknown waypoint count should fallback to 3-waypoint multiplier"
        );
        assert_eq!(
            selector.get_waypoint_distance_multiplier(1),
            0.35,
            "Invalid waypoint count should fallback to 3-waypoint multiplier"
        );
    }

    #[test]
    fn test_waypoint_count_at_distance_threshold() {
        let config = RouteGeneratorConfig::default();
        let selector = WaypointSelector::new(config);

        // At exactly 8km threshold (short route: prefer 3wp)
        assert_eq!(
            selector.calculate_waypoint_count(8.0, 10, 0),
            3,
            "At threshold, should use short route settings (3wp preferred)"
        );

        // Just above threshold
        assert_eq!(
            selector.calculate_waypoint_count(8.1, 10, 0),
            3,
            "Above threshold, should use long route settings (3 for even seed)"
        );
        assert_eq!(
            selector.calculate_waypoint_count(8.1, 10, 1),
            4,
            "Above threshold, should use long route settings (4 for odd seed)"
        );
    }

    #[test]
    fn test_verify_loop_shape_well_distributed() {
        use crate::models::PoiCategory;

        let start = Coordinates::new(48.8566, 2.3522).unwrap();

        // 3 POIs spread evenly around start (120 degrees apart)
        let pois = vec![
            Poi::new(
                "North".to_string(),
                PoiCategory::Monument,
                Coordinates::new(48.87, 2.35).unwrap(),
                80.0,
            ),
            Poi::new(
                "SE".to_string(),
                PoiCategory::Park,
                Coordinates::new(48.85, 2.37).unwrap(),
                70.0,
            ),
            Poi::new(
                "SW".to_string(),
                PoiCategory::Museum,
                Coordinates::new(48.85, 2.33).unwrap(),
                60.0,
            ),
        ];

        let ordered = WaypointSelector::order_pois_clockwise(&start, &pois);
        assert!(
            WaypointSelector::verify_loop_shape(&start, &ordered, 0),
            "Well-distributed POIs should pass loop shape verification"
        );
    }

    #[test]
    fn test_verify_loop_shape_clustered() {
        use crate::models::PoiCategory;

        let start = Coordinates::new(48.8566, 2.3522).unwrap();

        // 3 POIs all very close together (same direction from start)
        let pois = vec![
            Poi::new(
                "A".to_string(),
                PoiCategory::Monument,
                Coordinates::new(48.87, 2.352).unwrap(),
                80.0,
            ),
            Poi::new(
                "B".to_string(),
                PoiCategory::Park,
                Coordinates::new(48.871, 2.3521).unwrap(),
                70.0,
            ),
            Poi::new(
                "C".to_string(),
                PoiCategory::Museum,
                Coordinates::new(48.872, 2.3522).unwrap(),
                60.0,
            ),
        ];

        let ordered = WaypointSelector::order_pois_clockwise(&start, &pois);
        assert!(
            !WaypointSelector::verify_loop_shape(&start, &ordered, 0),
            "Clustered POIs should fail loop shape verification"
        );
    }

    #[test]
    fn test_verify_loop_shape_two_pois() {
        use crate::models::PoiCategory;

        let start = Coordinates::new(48.8566, 2.3522).unwrap();

        // 2 POIs on opposite sides - should pass
        let pois = vec![
            Poi::new(
                "N".to_string(),
                PoiCategory::Monument,
                Coordinates::new(48.87, 2.35).unwrap(),
                80.0,
            ),
            Poi::new(
                "S".to_string(),
                PoiCategory::Park,
                Coordinates::new(48.84, 2.35).unwrap(),
                70.0,
            ),
        ];

        let ordered = WaypointSelector::order_pois_clockwise(&start, &pois);
        assert!(
            WaypointSelector::verify_loop_shape(&start, &ordered, 0),
            "Opposite POIs should pass loop shape verification"
        );
    }
}
