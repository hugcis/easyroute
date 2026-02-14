use crate::evaluation::EvalScenario;
use crate::models::{Coordinates, TransportMode};
use crate::services::route_generator::route_metrics::PoiDensityContext;

/// Default evaluation scenarios covering diverse environments
pub fn default_scenarios() -> Vec<EvalScenario> {
    vec![
        // --- Monaco (existing) ---
        EvalScenario {
            name: "monaco_3km_walk".to_string(),
            start: Coordinates::new(43.7384, 7.4246).unwrap(),
            distance_km: 3.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        EvalScenario {
            name: "monaco_5km_walk".to_string(),
            start: Coordinates::new(43.7384, 7.4246).unwrap(),
            distance_km: 5.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        EvalScenario {
            name: "monaco_3km_bike".to_string(),
            start: Coordinates::new(43.7384, 7.4246).unwrap(),
            distance_km: 3.0,
            mode: TransportMode::Bike,
            expected_density: PoiDensityContext::Dense,
        },
        // --- Paris (dense urban, various distances) ---
        EvalScenario {
            name: "paris_1_5km_walk".to_string(),
            start: Coordinates::new(48.8566, 2.3522).unwrap(),
            distance_km: 1.5,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        EvalScenario {
            name: "paris_5km_walk".to_string(),
            start: Coordinates::new(48.8566, 2.3522).unwrap(),
            distance_km: 5.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        EvalScenario {
            name: "paris_12km_bike".to_string(),
            start: Coordinates::new(48.8566, 2.3522).unwrap(),
            distance_km: 12.0,
            mode: TransportMode::Bike,
            expected_density: PoiDensityContext::Dense,
        },
        // --- Paris long-walk (regression case: dense POI area, long route) ---
        EvalScenario {
            name: "paris_14km_walk".to_string(),
            start: Coordinates::new(48.854, 2.3723).unwrap(),
            distance_km: 14.5,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        // --- Prague (cross-region validation) ---
        EvalScenario {
            name: "prague_5km_walk".to_string(),
            start: Coordinates::new(50.0755, 14.4378).unwrap(),
            distance_km: 5.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Dense,
        },
        // --- Rennes (moderate city) ---
        EvalScenario {
            name: "rennes_3km_walk".to_string(),
            start: Coordinates::new(48.1173, -1.6778).unwrap(),
            distance_km: 3.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Moderate,
        },
        // --- Angers (clustered POIs, regression case) ---
        EvalScenario {
            name: "angers_5km_walk".to_string(),
            start: Coordinates::new(47.4784, -0.5632).unwrap(),
            distance_km: 5.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Moderate,
        },
        // --- Rural Brittany (sparse POIs) ---
        EvalScenario {
            name: "bretagne_rural_5km_walk".to_string(),
            start: Coordinates::new(48.28, -3.57).unwrap(),
            distance_km: 5.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Sparse,
        },
        // --- Open ocean (zero POIs, forces geometric fallback) ---
        EvalScenario {
            name: "ocean_3km_walk_geometric".to_string(),
            start: Coordinates::new(47.5, -8.0).unwrap(),
            distance_km: 3.0,
            mode: TransportMode::Walk,
            expected_density: PoiDensityContext::Geometric,
        },
    ]
}
