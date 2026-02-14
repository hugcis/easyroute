use serde::{Deserialize, Serialize};
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::evaluation::ScenarioResult;

const BASELINE_VERSION: u32 = 1;

// ── Types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u32,
    pub timestamp: String,
    pub runs_per_scenario: usize,
    pub scenarios: Vec<BaselineScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineScenario {
    pub name: String,
    pub success_rate: f32,
    pub metrics: Option<BaselineMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMetrics {
    pub circularity: f32,
    pub convexity: f32,
    pub path_overlap_pct: f32,
    pub poi_density_per_km: f32,
    pub category_entropy: f32,
    pub landmark_coverage: f32,
}

// ── Comparison types ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub baseline_timestamp: String,
    pub threshold: f32,
    pub scenario_comparisons: Vec<ScenarioComparison>,
    pub total_regressions: usize,
    pub new_scenarios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioComparison {
    pub name: String,
    pub runs: usize,
    pub metric_comparisons: Vec<MetricComparison>,
    pub regressions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricComparison {
    pub name: String,
    pub current: f32,
    pub baseline: f32,
    pub change_pct: f32,
    pub regressed: bool,
}

// ── Build baseline from results ─────────────────────────────

impl Baseline {
    pub fn from_results(results: &[ScenarioResult], runs: usize) -> Self {
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string());

        let scenarios = results
            .iter()
            .map(|r| {
                let metrics = r.metrics_agg.as_ref().map(|agg| BaselineMetrics {
                    circularity: agg.circularity.mean,
                    convexity: agg.convexity.mean,
                    path_overlap_pct: agg.path_overlap_pct.mean,
                    poi_density_per_km: agg.poi_density_per_km.mean,
                    category_entropy: agg.category_entropy.mean,
                    landmark_coverage: agg.landmark_coverage.mean,
                });
                BaselineScenario {
                    name: r.scenario.name.clone(),
                    success_rate: r.success_rate,
                    metrics,
                }
            })
            .collect();

        Baseline {
            version: BASELINE_VERSION,
            timestamp,
            runs_per_scenario: runs,
            scenarios,
        }
    }
}

// ── Save / Load ─────────────────────────────────────────────

pub fn save_baseline(baseline: &Baseline, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(baseline).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

pub fn load_baseline(path: &Path) -> Result<Baseline, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read baseline at {}: {e}", path.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse baseline: {e}"))
}

// ── Comparison logic ────────────────────────────────────────

/// Compare current evaluation results against a saved baseline.
/// `threshold` is a fraction (e.g. 0.15 = 15%).
pub fn compare(
    baseline: &Baseline,
    results: &[ScenarioResult],
    threshold: f32,
) -> ComparisonReport {
    let baseline_map: std::collections::HashMap<&str, &BaselineScenario> = baseline
        .scenarios
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    let mut scenario_comparisons = Vec::new();
    let mut total_regressions = 0;
    let mut new_scenarios = Vec::new();

    for result in results {
        let name = &result.scenario.name;
        let Some(base_scenario) = baseline_map.get(name.as_str()) else {
            new_scenarios.push(name.clone());
            continue;
        };

        let mut metric_comparisons = Vec::new();

        // success_rate: higher is better
        metric_comparisons.push(compare_metric(
            "success_rate",
            result.success_rate,
            base_scenario.success_rate,
            threshold,
            true,
        ));

        // Compare individual metrics if both sides have them
        if let (Some(cur_agg), Some(base_m)) = (&result.metrics_agg, &base_scenario.metrics) {
            // Higher-is-better metrics
            metric_comparisons.push(compare_metric(
                "circularity",
                cur_agg.circularity.mean,
                base_m.circularity,
                threshold,
                true,
            ));
            metric_comparisons.push(compare_metric(
                "convexity",
                cur_agg.convexity.mean,
                base_m.convexity,
                threshold,
                true,
            ));
            metric_comparisons.push(compare_metric(
                "poi_density_per_km",
                cur_agg.poi_density_per_km.mean,
                base_m.poi_density_per_km,
                threshold,
                true,
            ));
            metric_comparisons.push(compare_metric(
                "category_entropy",
                cur_agg.category_entropy.mean,
                base_m.category_entropy,
                threshold,
                true,
            ));
            metric_comparisons.push(compare_metric(
                "landmark_coverage",
                cur_agg.landmark_coverage.mean,
                base_m.landmark_coverage,
                threshold,
                true,
            ));
            // Lower-is-better metric
            metric_comparisons.push(compare_metric(
                "path_overlap_pct",
                cur_agg.path_overlap_pct.mean,
                base_m.path_overlap_pct,
                threshold,
                false,
            ));
        }

        let regressions = metric_comparisons.iter().filter(|m| m.regressed).count();
        total_regressions += regressions;

        scenario_comparisons.push(ScenarioComparison {
            name: name.clone(),
            runs: result.runs,
            metric_comparisons,
            regressions,
        });
    }

    ComparisonReport {
        baseline_timestamp: baseline.timestamp.clone(),
        threshold,
        scenario_comparisons,
        total_regressions,
        new_scenarios,
    }
}

fn compare_metric(
    name: &str,
    current: f32,
    baseline: f32,
    threshold: f32,
    higher_is_better: bool,
) -> MetricComparison {
    let change_pct = if baseline.abs() < f32::EPSILON {
        // Baseline is zero — can't compute meaningful % change
        0.0
    } else {
        (current - baseline) / baseline
    };

    let regressed = if baseline.abs() < f32::EPSILON {
        false
    } else if higher_is_better {
        current < baseline * (1.0 - threshold)
    } else {
        current > baseline * (1.0 + threshold)
    };

    MetricComparison {
        name: name.to_string(),
        current,
        baseline,
        change_pct,
        regressed,
    }
}

// ── Display formatting ──────────────────────────────────────

pub fn format_comparison_report(report: &ComparisonReport) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "Checking against baseline (saved {})...\n",
        report.baseline_timestamp
    ));
    out.push_str(&format!(
        "Regression threshold: {:.0}%\n",
        report.threshold * 100.0
    ));

    for sc in &report.scenario_comparisons {
        out.push_str(&format!("\nScenario: {} ({} runs)\n", sc.name, sc.runs));

        for mc in &sc.metric_comparisons {
            let icon = if mc.regressed { "x" } else { "ok" };
            let change_str = if mc.baseline.abs() < f32::EPSILON {
                "  (baseline=0)".to_string()
            } else {
                format!(
                    " (baseline: {:.2}, {:+.1}%)",
                    mc.baseline,
                    mc.change_pct * 100.0
                )
            };
            let regression_marker = if mc.regressed { " <- REGRESSION" } else { "" };
            out.push_str(&format!(
                "  [{icon}] {:<22} {:.2}{change_str}{regression_marker}\n",
                mc.name, mc.current,
            ));
        }
    }

    if !report.new_scenarios.is_empty() {
        out.push_str("\nNew scenarios (no baseline):\n");
        for name in &report.new_scenarios {
            out.push_str(&format!("  - {name}\n"));
        }
    }

    let scenario_count = report.scenario_comparisons.len() + report.new_scenarios.len();
    if report.total_regressions > 0 {
        out.push_str(&format!(
            "\nRESULT: {} regression(s) detected across {} scenarios\n",
            report.total_regressions, scenario_count,
        ));
    } else {
        out.push_str(&format!(
            "\nRESULT: No regressions detected across {} scenarios\n",
            scenario_count,
        ));
    }

    out
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metric_comparison(
        current: f32,
        baseline: f32,
        threshold: f32,
        higher_is_better: bool,
    ) -> MetricComparison {
        compare_metric(
            "test_metric",
            current,
            baseline,
            threshold,
            higher_is_better,
        )
    }

    #[test]
    fn test_higher_is_better_no_regression() {
        let mc = make_metric_comparison(0.70, 0.70, 0.15, true);
        assert!(!mc.regressed);
        assert!((mc.change_pct).abs() < 0.01);
    }

    #[test]
    fn test_higher_is_better_improvement() {
        let mc = make_metric_comparison(0.80, 0.70, 0.15, true);
        assert!(!mc.regressed);
        assert!(mc.change_pct > 0.0);
    }

    #[test]
    fn test_higher_is_better_regression() {
        // 0.50 < 0.70 * 0.85 = 0.595 => regression
        let mc = make_metric_comparison(0.50, 0.70, 0.15, true);
        assert!(mc.regressed);
    }

    #[test]
    fn test_higher_is_better_within_threshold() {
        // 0.62 > 0.70 * 0.85 = 0.595 => no regression
        let mc = make_metric_comparison(0.62, 0.70, 0.15, true);
        assert!(!mc.regressed);
    }

    #[test]
    fn test_lower_is_better_no_regression() {
        let mc = make_metric_comparison(0.15, 0.15, 0.15, false);
        assert!(!mc.regressed);
    }

    #[test]
    fn test_lower_is_better_improvement() {
        // Lower is better, so 0.10 < 0.15 is an improvement
        let mc = make_metric_comparison(0.10, 0.15, 0.15, false);
        assert!(!mc.regressed);
    }

    #[test]
    fn test_lower_is_better_regression() {
        // 0.20 > 0.15 * 1.15 = 0.1725 => regression
        let mc = make_metric_comparison(0.20, 0.15, 0.15, false);
        assert!(mc.regressed);
    }

    #[test]
    fn test_lower_is_better_within_threshold() {
        // 0.17 < 0.15 * 1.15 = 0.1725 => no regression
        let mc = make_metric_comparison(0.17, 0.15, 0.15, false);
        assert!(!mc.regressed);
    }

    #[test]
    fn test_zero_baseline_no_regression() {
        let mc = make_metric_comparison(0.50, 0.0, 0.15, true);
        assert!(!mc.regressed);
        assert!((mc.change_pct).abs() < f32::EPSILON);
    }

    #[test]
    fn test_baseline_roundtrip() {
        let baseline = Baseline {
            version: 1,
            timestamp: "2026-02-11T14:30:00Z".to_string(),
            runs_per_scenario: 3,
            scenarios: vec![
                BaselineScenario {
                    name: "test_scenario".to_string(),
                    success_rate: 1.0,
                    metrics: Some(BaselineMetrics {
                        circularity: 0.72,
                        convexity: 0.85,
                        path_overlap_pct: 0.15,
                        poi_density_per_km: 2.5,
                        category_entropy: 1.8,
                        landmark_coverage: 0.6,
                    }),
                },
                BaselineScenario {
                    name: "no_metrics".to_string(),
                    success_rate: 0.5,
                    metrics: None,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&baseline).unwrap();
        let loaded: Baseline = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.scenarios.len(), 2);
        assert_eq!(loaded.scenarios[0].name, "test_scenario");
        assert!(
            (loaded.scenarios[0].metrics.as_ref().unwrap().circularity - 0.72).abs() < f32::EPSILON
        );
        assert!(loaded.scenarios[1].metrics.is_none());
    }

    #[test]
    fn test_comparison_counts_regressions() {
        let baseline = Baseline {
            version: 1,
            timestamp: "2026-02-11T14:30:00Z".to_string(),
            runs_per_scenario: 3,
            scenarios: vec![BaselineScenario {
                name: "s1".to_string(),
                success_rate: 1.0,
                metrics: Some(BaselineMetrics {
                    circularity: 0.80,
                    convexity: 0.90,
                    path_overlap_pct: 0.10,
                    poi_density_per_km: 3.0,
                    category_entropy: 2.0,
                    landmark_coverage: 0.7,
                }),
            }],
        };

        // Build a ScenarioResult with significantly worse circularity
        let result = ScenarioResult {
            scenario: crate::evaluation::EvalScenario {
                name: "s1".to_string(),
                start: crate::models::Coordinates::new(43.7, 7.4).unwrap(),
                distance_km: 3.0,
                mode: crate::models::TransportMode::Walk,
                expected_density:
                    crate::services::route_generator::route_metrics::PoiDensityContext::Dense,
            },
            runs: 3,
            total_routes: 3,
            success_rate: 1.0,
            metrics_agg: Some(crate::evaluation::MetricsAggregate {
                circularity: crate::evaluation::StatSummary {
                    mean: 0.50,
                    std_dev: 0.05,
                },
                convexity: crate::evaluation::StatSummary {
                    mean: 0.88,
                    std_dev: 0.02,
                },
                path_overlap_pct: crate::evaluation::StatSummary {
                    mean: 0.10,
                    std_dev: 0.01,
                },
                poi_density_per_km: crate::evaluation::StatSummary {
                    mean: 2.8,
                    std_dev: 0.3,
                },
                category_entropy: crate::evaluation::StatSummary {
                    mean: 1.9,
                    std_dev: 0.1,
                },
                landmark_coverage: crate::evaluation::StatSummary {
                    mean: 0.65,
                    std_dev: 0.05,
                },
            }),
        };

        let report = compare(&baseline, &[result], 0.15);
        assert_eq!(report.total_regressions, 1); // circularity: 0.50 < 0.80*0.85=0.68
        assert_eq!(report.scenario_comparisons.len(), 1);
        assert_eq!(report.scenario_comparisons[0].regressions, 1);
    }

    #[test]
    fn test_new_scenario_not_counted_as_regression() {
        let baseline = Baseline {
            version: 1,
            timestamp: "2026-02-11T14:30:00Z".to_string(),
            runs_per_scenario: 3,
            scenarios: vec![],
        };

        let result = ScenarioResult {
            scenario: crate::evaluation::EvalScenario {
                name: "brand_new".to_string(),
                start: crate::models::Coordinates::new(43.7, 7.4).unwrap(),
                distance_km: 3.0,
                mode: crate::models::TransportMode::Walk,
                expected_density:
                    crate::services::route_generator::route_metrics::PoiDensityContext::Dense,
            },
            runs: 1,
            total_routes: 1,
            success_rate: 1.0,
            metrics_agg: None,
        };

        let report = compare(&baseline, &[result], 0.15);
        assert_eq!(report.total_regressions, 0);
        assert_eq!(report.new_scenarios, vec!["brand_new"]);
    }
}
