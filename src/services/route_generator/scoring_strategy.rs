use crate::config::RouteGeneratorConfig;
use crate::constants::*;
use crate::models::{Coordinates, Poi, RoutePreferences};

/// Context passed to scoring strategies
pub struct ScoringContext<'a> {
    pub start: &'a Coordinates,
    pub target_waypoint_distance: f64,
    pub target_distance_km: f64,
    pub attempt_seed: usize,
    pub preferences: &'a RoutePreferences,
    pub already_selected: &'a [Poi],
}

/// Trait for POI scoring strategies
pub trait PoiScoringStrategy: Send + Sync {
    /// Score POIs for loop route selection
    /// Returns a vector of (score, POI) tuples
    fn score_pois<'a>(&self, pois: &'a [Poi], context: &ScoringContext) -> Vec<(f32, &'a Poi)>;
}

/// Simple distance-based scoring (original algorithm)
pub struct SimpleStrategy {
    config: RouteGeneratorConfig,
}

impl SimpleStrategy {
    pub fn new(config: RouteGeneratorConfig) -> Self {
        Self { config }
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
    fn calculate_variation_offset(poi_index: usize, attempt_seed: usize) -> f32 {
        ((poi_index * VARIATION_MULTIPLIER + attempt_seed * VARIATION_OFFSET_BASE) % VARIATION_MOD)
            as f32
            * VARIATION_SCORE_FACTOR
    }
}

impl PoiScoringStrategy for SimpleStrategy {
    fn score_pois<'a>(&self, pois: &'a [Poi], context: &ScoringContext) -> Vec<(f32, &'a Poi)> {
        // Adaptive distance filtering - stricter for accuracy
        let max_reasonable_dist = if context.target_distance_km > 8.0 {
            context.target_distance_km * 0.7
        } else {
            context.target_distance_km * self.config.max_poi_distance_multiplier
        };

        pois.iter()
            .enumerate()
            .filter_map(|(idx, poi)| {
                let dist = context.start.distance_to(&poi.coordinates);

                // Filter out POIs that are too close or too far
                if dist < self.config.min_poi_distance_km || dist > max_reasonable_dist {
                    return None;
                }

                let distance_score =
                    self.calculate_distance_score(dist, context.target_waypoint_distance);
                let variation_offset = Self::calculate_variation_offset(idx, context.attempt_seed);

                Some((distance_score + variation_offset, poi))
            })
            .collect()
    }
}

/// Advanced context-aware scoring with quality, clustering, and angular diversity
pub struct AdvancedStrategy {
    config: RouteGeneratorConfig,
}

impl AdvancedStrategy {
    pub fn new(config: RouteGeneratorConfig) -> Self {
        Self { config }
    }

    /// Calculate score based on distance from ideal waypoint distance
    fn calculate_distance_score(&self, actual_dist: f64, target_dist: f64) -> f32 {
        if actual_dist < target_dist {
            (actual_dist / target_dist) as f32 * 0.8 + 0.2
        } else {
            let excess_ratio = (actual_dist - target_dist) / target_dist;
            (1.0 - (excess_ratio * 0.5).min(0.8)) as f32
        }
    }

    /// Calculate variation offset
    fn calculate_variation_offset(poi_index: usize, attempt_seed: usize) -> f32 {
        ((poi_index * VARIATION_MULTIPLIER + attempt_seed * VARIATION_OFFSET_BASE) % VARIATION_MOD)
            as f32
            * VARIATION_SCORE_FACTOR
    }

    /// Calculate angle from start to POI (in radians)
    fn calculate_angle(start: &Coordinates, poi: &Poi) -> f64 {
        let dx = poi.coordinates.lng - start.lng;
        let dy = poi.coordinates.lat - start.lat;
        dy.atan2(dx) // Returns angle in radians (-π to π)
    }

    /// Calculate angular diversity score
    /// Rewards POIs in compass directions not yet covered
    fn angular_diversity_score(candidate_angle: f64, selected_angles: &[f64]) -> f32 {
        if selected_angles.is_empty() {
            return 1.0;
        }

        // Find minimum angular distance to any selected POI
        let min_angle_diff = selected_angles
            .iter()
            .map(|&angle| {
                let diff = (candidate_angle - angle).abs();
                diff.min(2.0 * std::f64::consts::PI - diff)
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Normalize to 0-1 (more separation = higher score)
        (min_angle_diff / std::f64::consts::PI).min(1.0) as f32
    }

    /// Calculate penalty for POIs that cluster together
    fn cluster_penalty(candidate: &Poi, selected: &[Poi], min_separation_km: f64) -> f32 {
        if selected.is_empty() {
            return 0.0;
        }

        let mut max_penalty: f32 = 0.0;

        for poi in selected {
            let dist = candidate.coordinates.distance_to(&poi.coordinates);
            if dist < min_separation_km {
                let penalty = (1.0 - dist / min_separation_km) * 100.0;
                max_penalty = max_penalty.max(penalty as f32);
            }
        }

        max_penalty
    }
}

impl PoiScoringStrategy for AdvancedStrategy {
    fn score_pois<'a>(&self, pois: &'a [Poi], context: &ScoringContext) -> Vec<(f32, &'a Poi)> {
        // Adaptive distance filtering - stricter for route accuracy
        // For 5km route: max_dist = 5 * 0.6 = 3km
        let max_reasonable_dist = if context.target_distance_km > 8.0 {
            context.target_distance_km * 0.7
        } else {
            context.target_distance_km * self.config.max_poi_distance_multiplier
        };

        // Calculate angles for already selected POIs
        let selected_angles: Vec<f64> = context
            .already_selected
            .iter()
            .map(|poi| Self::calculate_angle(context.start, poi))
            .collect();

        pois.iter()
            .enumerate()
            .filter_map(|(idx, poi)| {
                let dist = context.start.distance_to(&poi.coordinates);

                // Filter out POIs that are too close or too far
                if dist < self.config.min_poi_distance_km || dist > max_reasonable_dist {
                    return None;
                }

                let mut score = 0.0;

                // 1. Distance score (weighted by config)
                let distance_score =
                    self.calculate_distance_score(dist, context.target_waypoint_distance);
                score += distance_score * self.config.poi_score_weight_distance;

                // 2. POI quality score (weighted by config)
                let quality_score = poi.quality_score(context.preferences.hidden_gems) / 100.0;
                score += quality_score * self.config.poi_score_weight_quality;

                // 3. Angular diversity score (weighted by config)
                let angle = Self::calculate_angle(context.start, poi);
                let angular_score = Self::angular_diversity_score(angle, &selected_angles);
                score += angular_score * self.config.poi_score_weight_angular;

                // 4. Cluster penalty (weighted by config)
                let cluster_pen = Self::cluster_penalty(
                    poi,
                    context.already_selected,
                    self.config.poi_min_separation_km,
                );
                score -= cluster_pen * self.config.poi_score_weight_clustering;

                // 5. Variation offset (weighted by config)
                let variation_offset = Self::calculate_variation_offset(idx, context.attempt_seed);
                score += variation_offset * self.config.poi_score_weight_variation;

                Some((score, poi))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PoiCategory;

    #[test]
    fn test_simple_strategy_distance_scoring() {
        let config = RouteGeneratorConfig::default();
        let strategy = SimpleStrategy::new(config.clone());

        // Test distance score calculation
        let target_dist = 2.0;

        // POI at ideal distance should score high
        let score_ideal = strategy.calculate_distance_score(2.0, target_dist);
        assert!(score_ideal > 0.9);

        // POI closer than ideal still gets good score
        let score_close = strategy.calculate_distance_score(1.5, target_dist);
        assert!(score_close > 0.7);

        // POI far from ideal gets penalized (4.0 is 2x target, so excess_ratio=1.0, score ~0.5)
        let score_far = strategy.calculate_distance_score(4.0, target_dist);
        assert!(score_far < 0.6); // Adjusted: formula gives 1.0 - (1.0 * 0.5) = 0.5
    }

    #[test]
    fn test_advanced_strategy_angular_diversity() {
        let selected_angles = vec![0.0]; // POI at 0 radians (east)

        // POI at opposite direction (π radians, west) should score high
        let score_opposite =
            AdvancedStrategy::angular_diversity_score(std::f64::consts::PI, &selected_angles);
        assert!(score_opposite > 0.9);

        // POI at same angle should score low
        let score_same = AdvancedStrategy::angular_diversity_score(0.0, &selected_angles);
        assert!(score_same < 0.1);

        // POI at 90 degrees should score medium-high
        let score_perpendicular =
            AdvancedStrategy::angular_diversity_score(std::f64::consts::PI / 2.0, &selected_angles);
        assert!(score_perpendicular > 0.4 && score_perpendicular < 0.6);
    }

    #[test]
    fn test_advanced_strategy_cluster_penalty() {
        let poi1 = Poi::new(
            "Test 1".to_string(),
            PoiCategory::Monument,
            Coordinates::new(48.8566, 2.3522).unwrap(),
            80.0,
        );

        let poi2 = Poi::new(
            "Test 2".to_string(),
            PoiCategory::Park,
            Coordinates::new(48.8576, 2.3532).unwrap(), // ~150m away
            70.0,
        );

        let selected = vec![poi1];
        let min_separation = 0.3; // 300m

        // POI closer than min separation should get penalty
        let penalty = AdvancedStrategy::cluster_penalty(&poi2, &selected, min_separation);
        assert!(penalty > 0.0);
    }
}
