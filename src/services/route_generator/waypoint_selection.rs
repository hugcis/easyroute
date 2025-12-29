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

        let num_waypoints = self.calculate_waypoint_count(target_distance_km, pois.len());
        let target_waypoint_distance =
            target_distance_km * self.config.waypoint_distance_multiplier;

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

        for _iteration in 0..num_waypoints {
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
                tracing::warn!("No POIs scored in iteration, using fallback");
                scored_pois = self.fallback_score_closest_pois(start, &remaining_pois, 1)?;
            }

            // If still no POIs after fallback, break the loop
            if scored_pois.is_empty() {
                tracing::warn!(
                    "No POIs available after fallback (selected: {}, remaining: {})",
                    selected.len(),
                    remaining_pois.len()
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

    /// Calculate the optimal number of waypoints based on distance and available POIs
    fn calculate_waypoint_count(&self, target_distance_km: f64, poi_count: usize) -> usize {
        if target_distance_km > self.config.long_route_threshold_km
            && poi_count >= self.config.poi_count_threshold_long
        {
            self.config.waypoints_count_long
        } else {
            self.config.waypoints_count_short
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
