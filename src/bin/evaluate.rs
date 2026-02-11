use easyroute::config::Config;
use easyroute::evaluation::{
    default_scenarios, format_report, EvalScenario, MetricsAggregate, ScenarioResult,
};
use easyroute::models::{Route, RoutePreferences};
use easyroute::services::mapbox::MapboxClient;
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    let config = Config::from_env().map_err(|e| format!("Config error: {}", e))?;

    // Parse CLI args
    let args: Vec<String> = env::args().collect();
    let scenario_filter = args.iter().find_map(|a| a.strip_prefix("--scenario="));
    let runs: usize = args
        .iter()
        .find_map(|a| a.strip_prefix("--runs="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let json_output = args.iter().any(|a| a == "--json");

    // Connect to database
    let db_pool = easyroute::db::create_pool(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&db_pool).await?;

    // Initialize services
    let mapbox_client = MapboxClient::new(config.mapbox_api_key.clone());
    let poi_service = PoiService::new(db_pool.clone());
    let snapping_service = SnappingService::new(db_pool.clone());
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
        let metrics_agg = MetricsAggregate::from_routes(&route_refs);

        results.push(ScenarioResult {
            scenario: (*scenario).clone(),
            runs,
            total_routes: all_routes.len(),
            success_rate: successes as f32 / runs as f32,
            metrics_agg,
        });
    }

    if json_output {
        // Simple JSON output for programmatic consumption
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
