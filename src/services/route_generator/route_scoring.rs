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
        let distance_km = directions.distance_km();

        let poi_count = pois.len();
        let route_pois: Vec<RoutePoi> = pois
            .into_iter()
            .enumerate()
            .map(|(idx, poi)| {
                let distance_fraction = (idx + 1) as f64 / (poi_count + 1) as f64;
                RoutePoi::new(poi, idx as u32 + 1, distance_km * distance_fraction)
            })
            .collect();

        let mut route = Route::new(distance_km, directions.duration_minutes(), path, route_pois);

        match self
            .snapping_service
            .find_snapped_pois(
                &route.path,
                &route.pois,
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
                    path_points = route.path.len(),
                    snap_radius_m = self.snap_radius_m,
                    "Failed to snap POIs ({} path points, {}m radius): {}",
                    route.path.len(), self.snap_radius_m, e
                );
            }
        }

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

    /// Distance accuracy score: 1.0 for perfect match, 0.0 for 100%+ error
    fn distance_accuracy(route: &Route, target_distance_km: f64) -> f32 {
        let error_ratio = (route.distance_km - target_distance_km).abs() / target_distance_km;
        (1.0 - error_ratio.min(1.0)) as f32
    }

    /// Average POI quality (0.0-1.0), or 0.0 if no POIs
    fn avg_poi_quality(route: &Route, hidden_gems: bool) -> f32 {
        if route.pois.is_empty() {
            return 0.0;
        }
        route
            .pois
            .iter()
            .map(|rp| rp.poi.quality_score(hidden_gems) / 100.0)
            .sum::<f32>()
            / route.pois.len() as f32
    }

    /// Category diversity score (0.0-1.0): unique categories / 3
    fn category_diversity(route: &Route) -> f32 {
        let unique: HashSet<_> = route.pois.iter().map(|rp| &rp.poi.category).collect();
        (unique.len() as f32 / 3.0).min(1.0)
    }

    /// V1 scoring: original algorithm (0-10)
    fn calculate_route_score_v1(
        &self,
        route: &Route,
        target_distance_km: f64,
        preferences: &RoutePreferences,
    ) -> f32 {
        let score = 3.0 * Self::distance_accuracy(route, target_distance_km)
            + (route.pois.len() as f32).min(3.0)
            + 2.0 * Self::avg_poi_quality(route, preferences.hidden_gems)
            + 2.0 * Self::category_diversity(route);

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
        let poi_count_normalized = (route.pois.len() as f32 / 3.0).min(1.0);

        let mut score = 2.5 * Self::distance_accuracy(route, target_distance_km)
            + 2.0 * poi_count_normalized
            + 1.5 * Self::avg_poi_quality(route, preferences.hidden_gems)
            + Self::category_diversity(route);

        if let Some(ref metrics) = route.metrics {
            let shape_score = (metrics.circularity + metrics.convexity) / 2.0;
            score += 2.0 * shape_score;
            score += 1.0 - metrics.path_overlap_pct;
        }

        score.clamp(0.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Coordinates, Poi, PoiCategory};
    use crate::services::snapping_service::SnappingService;

    fn c(lat: f64, lng: f64) -> Coordinates {
        Coordinates::new(lat, lng).unwrap()
    }

    fn make_poi(name: &str, category: PoiCategory, popularity: f32) -> Poi {
        Poi::new(name.to_string(), category, c(48.856, 2.352), popularity)
    }

    fn make_route_poi(name: &str, category: PoiCategory, popularity: f32, order: u32) -> RoutePoi {
        RoutePoi::new(make_poi(name, category, popularity), order, 1.0)
    }

    fn make_route(distance_km: f64, pois: Vec<RoutePoi>) -> Route {
        Route::new(distance_km, 60, vec![c(48.856, 2.352)], pois)
    }

    fn scorer_v1() -> RouteScorer {
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/fake").unwrap();
        let repo = std::sync::Arc::new(crate::db::PgPoiRepository::new(pool));
        let snap = SnappingService::new(repo);
        RouteScorer::new(snap, 100.0, RouteGeneratorConfig::default())
    }

    fn scorer_v2() -> RouteScorer {
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/fake").unwrap();
        let repo = std::sync::Arc::new(crate::db::PgPoiRepository::new(pool));
        let snap = SnappingService::new(repo);
        let config = RouteGeneratorConfig {
            scoring_version: 2,
            ..RouteGeneratorConfig::default()
        };
        RouteScorer::new(snap, 100.0, config)
    }

    fn default_prefs() -> RoutePreferences {
        RoutePreferences::default()
    }

    // --- V1 Scoring ---

    #[tokio::test]
    async fn v1_perfect_distance_no_pois() {
        let scorer = scorer_v1();
        let route = make_route(5.0, vec![]);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // Perfect distance = 3.0 pts, 0 POIs = 0, 0 quality, 0 diversity
        assert!((score - 3.0).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v1_zero_distance_match() {
        let scorer = scorer_v1();
        // Route is 10km, target is 5km → 100% error → 0 distance pts
        let route = make_route(10.0, vec![]);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        assert!(score < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v1_three_pois_max_count() {
        let scorer = scorer_v1();
        let pois = vec![
            make_route_poi("A", PoiCategory::Monument, 50.0, 1),
            make_route_poi("B", PoiCategory::Museum, 50.0, 2),
            make_route_poi("C", PoiCategory::Park, 50.0, 3),
        ];
        let route = make_route(5.0, pois);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // 3.0 (dist) + 3.0 (3 POIs) + 2.0*(0.5) (quality) + 2.0*1.0 (3 categories) = 9.0
        assert!((score - 9.0).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v1_single_category() {
        let scorer = scorer_v1();
        let pois = vec![
            make_route_poi("A", PoiCategory::Monument, 50.0, 1),
            make_route_poi("B", PoiCategory::Monument, 50.0, 2),
            make_route_poi("C", PoiCategory::Monument, 50.0, 3),
        ];
        let route = make_route(5.0, pois);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // 3.0 (dist) + 3.0 (3 POIs) + 1.0 (quality) + 2.0*(1/3) (1 cat) ≈ 7.67
        let diversity = 2.0 * (1.0_f32 / 3.0);
        let expected = 3.0 + 3.0 + 1.0 + diversity;
        assert!((score - expected).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v1_high_quality_pois() {
        let scorer = scorer_v1();
        let pois = vec![make_route_poi("A", PoiCategory::Monument, 100.0, 1)];
        let route = make_route(5.0, pois);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // 3.0 + 1.0 (1 POI) + 2.0*1.0 (quality) + 2.0*(1/3) ≈ 6.67
        let expected = 3.0 + 1.0 + 2.0 + 2.0 * (1.0_f32 / 3.0);
        assert!((score - expected).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v1_score_capped_at_10() {
        let scorer = scorer_v1();
        // 5 POIs (capped at 3) + perfect distance + max quality + max diversity
        let pois = vec![
            make_route_poi("A", PoiCategory::Monument, 100.0, 1),
            make_route_poi("B", PoiCategory::Museum, 100.0, 2),
            make_route_poi("C", PoiCategory::Park, 100.0, 3),
            make_route_poi("D", PoiCategory::Church, 100.0, 4),
            make_route_poi("E", PoiCategory::Castle, 100.0, 5),
        ];
        let route = make_route(5.0, pois);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        assert!(score <= 10.0, "score={score}");
    }

    // --- V2 Scoring ---

    #[tokio::test]
    async fn v2_dispatches_correctly() {
        let scorer = scorer_v2();
        let route = make_route(5.0, vec![]);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // V2 perfect distance = 2.5 pts (not 3.0 like V1)
        assert!((score - 2.5).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v2_poi_count_normalized_differently() {
        let scorer = scorer_v2();
        // 1 POI: V2 = 2.0*(1/3), V1 = 1.0 (min)
        let pois = vec![make_route_poi("A", PoiCategory::Monument, 50.0, 1)];
        let route = make_route(5.0, pois);
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        let v2_poi_pts = 2.0 * (1.0_f32 / 3.0);
        // 2.5 (dist) + 0.667 (1 POI) + 1.5*0.5 (quality) + 1.0*(1/3) (diversity)
        let expected = 2.5 + v2_poi_pts + 0.75 + (1.0 / 3.0);
        assert!((score - expected).abs() < 0.02, "score={score}");
    }

    #[tokio::test]
    async fn v2_with_metrics_shape_score() {
        let scorer = scorer_v2();
        let mut route = make_route(5.0, vec![]);
        route.metrics = Some(RouteMetrics {
            circularity: 0.8,
            convexity: 0.9,
            path_overlap_pct: 0.1,
            poi_density_per_km: 0.0,
            category_entropy: 0.0,
            landmark_coverage: 0.0,
            poi_density_context: super::super::route_metrics::PoiDensityContext::Sparse,
        });
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // 2.5 (dist) + 0 (pois) + 0 (quality) + 0 (diversity)
        // + 2.0 * (0.8+0.9)/2 (shape=1.7) + 1.0 * (1.0-0.1) (path=0.9) = 5.1
        let expected = 2.5 + 2.0 * ((0.8 + 0.9) / 2.0) + 1.0 * (1.0 - 0.1);
        assert!((score - expected).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v2_without_metrics_skips_shape() {
        let scorer = scorer_v2();
        let route = make_route(5.0, vec![]);
        assert!(route.metrics.is_none());
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // No metrics → no shape or path diversity component
        assert!((score - 2.5).abs() < 0.01, "score={score}");
    }

    #[tokio::test]
    async fn v2_path_overlap_penalty() {
        let scorer = scorer_v2();
        let mut route = make_route(5.0, vec![]);
        route.metrics = Some(RouteMetrics {
            circularity: 1.0,
            convexity: 1.0,
            path_overlap_pct: 0.5, // 50% overlap
            poi_density_per_km: 0.0,
            category_entropy: 0.0,
            landmark_coverage: 0.0,
            poi_density_context: super::super::route_metrics::PoiDensityContext::Sparse,
        });
        let score = scorer.calculate_route_score(&route, 5.0, &default_prefs());
        // 2.5 + 2.0*1.0 (shape) + 1.0*0.5 (path diversity) = 5.0
        assert!((score - 5.0).abs() < 0.01, "score={score}");
    }
}
