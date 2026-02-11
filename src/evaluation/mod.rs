pub mod baseline;
pub mod scenarios;

use serde::{Deserialize, Serialize};

use crate::models::{Coordinates, Route, TransportMode};
use crate::services::route_generator::route_metrics::{PoiDensityContext, RouteMetrics};

pub use baseline::{
    compare, format_comparison_report, load_baseline, save_baseline, Baseline, ComparisonReport,
};
pub use scenarios::default_scenarios;

/// A test scenario for route evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScenario {
    pub name: String,
    pub start: Coordinates,
    pub distance_km: f64,
    pub mode: TransportMode,
    pub expected_density: PoiDensityContext,
}

/// Aggregated results for a single scenario across N runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario: EvalScenario,
    pub runs: usize,
    pub total_routes: usize,
    pub success_rate: f32,
    pub metrics_agg: Option<MetricsAggregate>,
}

/// Statistical aggregates for each metric dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsAggregate {
    pub circularity: StatSummary,
    pub convexity: StatSummary,
    pub path_overlap_pct: StatSummary,
    pub poi_density_per_km: StatSummary,
    pub category_entropy: StatSummary,
    pub landmark_coverage: StatSummary,
}

/// Mean and standard deviation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatSummary {
    pub mean: f32,
    pub std_dev: f32,
}

impl MetricsAggregate {
    /// Aggregate metrics from a collection of routes
    pub fn from_routes(routes: &[&Route]) -> Option<Self> {
        let metrics: Vec<&RouteMetrics> =
            routes.iter().filter_map(|r| r.metrics.as_ref()).collect();

        if metrics.is_empty() {
            return None;
        }

        Some(MetricsAggregate {
            circularity: stat_summary(&metrics.iter().map(|m| m.circularity).collect::<Vec<_>>()),
            convexity: stat_summary(&metrics.iter().map(|m| m.convexity).collect::<Vec<_>>()),
            path_overlap_pct: stat_summary(
                &metrics
                    .iter()
                    .map(|m| m.path_overlap_pct)
                    .collect::<Vec<_>>(),
            ),
            poi_density_per_km: stat_summary(
                &metrics
                    .iter()
                    .map(|m| m.poi_density_per_km)
                    .collect::<Vec<_>>(),
            ),
            category_entropy: stat_summary(
                &metrics
                    .iter()
                    .map(|m| m.category_entropy)
                    .collect::<Vec<_>>(),
            ),
            landmark_coverage: stat_summary(
                &metrics
                    .iter()
                    .map(|m| m.landmark_coverage)
                    .collect::<Vec<_>>(),
            ),
        })
    }
}

fn stat_summary(values: &[f32]) -> StatSummary {
    if values.is_empty() {
        return StatSummary {
            mean: 0.0,
            std_dev: 0.0,
        };
    }

    let n = values.len() as f32;
    let mean = values.iter().sum::<f32>() / n;
    let variance = if values.len() > 1 {
        values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (n - 1.0)
    } else {
        0.0
    };

    StatSummary {
        mean,
        std_dev: variance.sqrt(),
    }
}

/// Format a single scenario result for display
pub fn format_scenario_result(result: &ScenarioResult) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{} ({} runs, {} routes total)\n",
        result.scenario.name, result.runs, result.total_routes,
    ));

    if let Some(ref agg) = result.metrics_agg {
        out.push_str(&format!(
            "  circularity:      {:.2} +/- {:.2}\n",
            agg.circularity.mean, agg.circularity.std_dev,
        ));
        out.push_str(&format!(
            "  convexity:        {:.2} +/- {:.2}\n",
            agg.convexity.mean, agg.convexity.std_dev,
        ));
        out.push_str(&format!(
            "  path_overlap:     {:.0}% +/- {:.0}%\n",
            agg.path_overlap_pct.mean * 100.0,
            agg.path_overlap_pct.std_dev * 100.0,
        ));
        out.push_str(&format!(
            "  poi_density:      {:.1}/km +/- {:.1}\n",
            agg.poi_density_per_km.mean, agg.poi_density_per_km.std_dev,
        ));
        out.push_str(&format!(
            "  category_entropy: {:.2} +/- {:.2}\n",
            agg.category_entropy.mean, agg.category_entropy.std_dev,
        ));
        out.push_str(&format!(
            "  landmark_coverage:{:.2} +/- {:.2}\n",
            agg.landmark_coverage.mean, agg.landmark_coverage.std_dev,
        ));
    } else {
        out.push_str("  (no routes with metrics)\n");
    }

    out.push_str(&format!(
        "  success_rate:     {:.0}%\n",
        result.success_rate * 100.0,
    ));

    out
}

/// Format the full evaluation report
pub fn format_report(results: &[ScenarioResult]) -> String {
    let mut report = String::from("=== Route Quality Evaluation Report ===\n");

    for result in results {
        report.push_str(&format_scenario_result(result));
    }

    report
}
