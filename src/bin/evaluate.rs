use easyroute::config::Config;
use easyroute::db::PgPoiRepository;
use easyroute::evaluation::{
    compare, default_scenarios, format_comparison_report, format_report, load_baseline,
    save_baseline, Baseline, EvalScenario, MetricsAggregate, ScenarioResult,
};
use easyroute::models::{Route, RoutePreferences};
use easyroute::services::mapbox::{AuthMode, MapboxClient};
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_BASELINE_PATH: &str = "evaluation/baseline.json";
const DEFAULT_REGRESSION_THRESHOLD: f32 = 0.15;

fn print_help() {
    eprintln!(
        "\
Usage: evaluate [OPTIONS]

Options:
  --scenario=FILTER     Only run scenarios whose name contains FILTER
  --runs=N              Number of runs per scenario (default: 3)
  --json                Output results as JSON
  --save-baseline       Save results as baseline to evaluation/baseline.json
  --check               Compare results against saved baseline (exit 1 on regression)
  --regression-threshold=F
                        Regression threshold as fraction (default: 0.15 = 15%)
  --help                Show this help message"
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (less verbose for eval)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "easyroute=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse CLI args
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help") {
        print_help();
        return Ok(());
    }

    let scenario_filter = args.iter().find_map(|a| a.strip_prefix("--scenario="));
    let runs: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--runs="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let json_output = args.iter().any(|a| a == "--json");
    let save_baseline_flag = args.iter().any(|a| a == "--save-baseline");
    let check_flag = args.iter().any(|a| a == "--check");
    let regression_threshold: f32 = args
        .iter()
        .find_map(|a| a.strip_prefix("--regression-threshold="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);

    let config = Config::from_env().map_err(|e| format!("Config error: {}", e))?;

    // Connect to database
    let db_pool = easyroute::db::create_pool(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&db_pool).await?;

    // Initialize services
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(db_pool.clone()));
    let mapbox_client = if let Some(ref base_url) = config.mapbox_base_url {
        MapboxClient::with_config(
            config.mapbox_api_key.clone(),
            base_url.clone(),
            AuthMode::BearerHeader,
        )
    } else {
        MapboxClient::new(config.mapbox_api_key.clone())
    };
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        config.snap_radius_m,
        config.route_generator.clone(),
    );

    // Select scenarios
    let all_scenarios = default_scenarios();
    let scenarios: Vec<&EvalScenario> = if let Some(filter) = scenario_filter {
        all_scenarios
            .iter()
            .filter(|s| s.name.contains(filter))
            .collect()
    } else {
        all_scenarios.iter().collect()
    };

    if scenarios.is_empty() {
        eprintln!("No scenarios matched filter. Available:");
        for s in &all_scenarios {
            eprintln!("  {}", s.name);
        }
        std::process::exit(1);
    }

    eprintln!(
        "Running {} scenarios x {} runs each...",
        scenarios.len(),
        runs
    );

    // Run evaluation
    let mut results = Vec::new();
    let preferences = RoutePreferences::default();

    for scenario in &scenarios {
        let mut all_routes: Vec<Route> = Vec::new();
        let mut successes = 0;

        for run in 0..runs {
            eprintln!("  {} (run {}/{})", scenario.name, run + 1, runs);

            match route_generator
                .generate_loop_route(
                    scenario.start,
                    scenario.distance_km,
                    scenario.distance_km * 0.2, // 20% tolerance
                    &scenario.mode,
                    &preferences,
                )
                .await
            {
                Ok(routes) => {
                    successes += 1;
                    all_routes.extend(routes);
                }
                Err(e) => {
                    eprintln!("    Failed: {}", e);
                }
            }
        }

        let route_refs: Vec<&Route> = all_routes.iter().collect();
        let metrics_agg = MetricsAggregate::from_routes(&route_refs, scenario.distance_km);

        results.push(ScenarioResult {
            scenario: (*scenario).clone(),
            runs,
            total_routes: all_routes.len(),
            success_rate: successes as f32 / runs as f32,
            metrics_agg,
        });
    }

    // Handle --save-baseline
    if save_baseline_flag {
        let baseline = Baseline::from_results(&results, runs);
        let path = PathBuf::from(DEFAULT_BASELINE_PATH);
        save_baseline(&baseline, &path)?;
        eprintln!("Baseline saved to {}", path.display());
    }

    // Handle --check
    if check_flag {
        let path = PathBuf::from(DEFAULT_BASELINE_PATH);
        let baseline = load_baseline(&path)?;
        let report = compare(&baseline, &results, regression_threshold);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", format_comparison_report(&report));
        }

        if report.total_regressions > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Standard output
    if json_output {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                let mut obj = serde_json::json!({
                    "scenario": r.scenario.name,
                    "runs": r.runs,
                    "total_routes": r.total_routes,
                    "success_rate": r.success_rate,
                });
                if let Some(ref agg) = r.metrics_agg {
                    obj["metrics"] = serde_json::json!({
                        "circularity_mean": agg.circularity.mean,
                        "circularity_std": agg.circularity.std_dev,
                        "convexity_mean": agg.convexity.mean,
                        "convexity_std": agg.convexity.std_dev,
                        "path_overlap_pct_mean": agg.path_overlap_pct.mean,
                        "path_overlap_pct_std": agg.path_overlap_pct.std_dev,
                        "poi_density_mean": agg.poi_density_per_km.mean,
                        "poi_density_std": agg.poi_density_per_km.std_dev,
                        "category_entropy_mean": agg.category_entropy.mean,
                        "category_entropy_std": agg.category_entropy.std_dev,
                        "landmark_coverage_mean": agg.landmark_coverage.mean,
                        "landmark_coverage_std": agg.landmark_coverage.std_dev,
                        "distance_accuracy_mean": agg.distance_accuracy.mean,
                        "distance_accuracy_std": agg.distance_accuracy.std_dev,
                        "route_score_mean": agg.route_score.mean,
                        "route_score_std": agg.route_score.std_dev,
                    });
                }
                obj
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        println!("{}", format_report(&results));
    }

    Ok(())
}
