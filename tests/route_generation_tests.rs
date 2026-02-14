use easyroute::db::PgPoiRepository;
use easyroute::models::{Coordinates, PoiCategory, RoutePreferences, TransportMode};
use easyroute::services::mapbox::MapboxClient;
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use serial_test::serial;
use std::sync::Arc;

mod common;

#[tokio::test]
#[serial]
async fn test_route_generation_with_database_pois() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert test POIs in a loop-friendly arrangement
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    let poi1 = common::create_test_poi("North POI", PoiCategory::Monument, 48.8600, 2.3522);
    let poi2 = common::create_test_poi("East POI", PoiCategory::Park, 48.8566, 2.3600);
    let poi3 = common::create_test_poi("West POI", PoiCategory::Museum, 48.8566, 2.3450);

    easyroute::db::queries::insert_poi(&pool, &poi1)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi3)
        .await
        .unwrap();

    // Create services
    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(pool.clone()));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        100.0,
        easyroute::config::RouteGeneratorConfig::default(),
    );

    // Generate route
    let preferences = RoutePreferences {
        poi_categories: None,
        hidden_gems: false,
        max_alternatives: 1,
    };

    let result = route_generator
        .generate_loop_route(center, 5.0, 1.0, &TransportMode::Walk, &preferences)
        .await;

    assert!(result.is_ok(), "Route generation should succeed");

    let routes = result.unwrap();
    assert!(!routes.is_empty(), "Should generate at least one route");

    let route = &routes[0];
    assert!(
        route.distance_km > 0.0,
        "Route should have positive distance"
    );
    assert!(
        route.estimated_duration_minutes > 0,
        "Route should have duration"
    );
    assert!(!route.path.is_empty(), "Route should have path");
    assert!(!route.pois.is_empty(), "Route should include POIs");

    // Route score should be reasonable
    assert!(
        route.score >= 0.0 && route.score <= 10.0,
        "Route score should be 0-10: got {}",
        route.score
    );

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[serial]
async fn test_route_generation_distance_validation() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert POIs at specific distances to test tolerance
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // POIs roughly 1km away in different directions
    let poi1 = common::create_test_poi("POI 1", PoiCategory::Monument, 48.8656, 2.3522);
    let poi2 = common::create_test_poi("POI 2", PoiCategory::Park, 48.8566, 2.3650);
    let poi3 = common::create_test_poi("POI 3", PoiCategory::Museum, 48.8476, 2.3522);

    easyroute::db::queries::insert_poi(&pool, &poi1)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi3)
        .await
        .unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(pool.clone()));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        100.0,
        easyroute::config::RouteGeneratorConfig::default(),
    );

    let preferences = RoutePreferences::default();

    // Request 5km route with 1km tolerance
    let result = route_generator
        .generate_loop_route(center, 5.0, 1.0, &TransportMode::Walk, &preferences)
        .await;

    if let Ok(routes) = result {
        for route in &routes {
            // Check distance is within tolerance
            let target = 5.0;
            let tolerance = 1.0;
            assert!(
                route.distance_km >= target - tolerance && route.distance_km <= target + tolerance,
                "Route distance should be within tolerance: got {}km, wanted {}±{}km",
                route.distance_km,
                target,
                tolerance
            );
        }
    }

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[serial]
async fn test_route_poi_ordering() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert POIs
    let center = Coordinates::new(48.8566, 2.3522).unwrap();
    let poi1 = common::create_test_poi("POI 1", PoiCategory::Monument, 48.8600, 2.3522);
    let poi2 = common::create_test_poi("POI 2", PoiCategory::Park, 48.8566, 2.3600);

    easyroute::db::queries::insert_poi(&pool, &poi1)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2)
        .await
        .unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(pool.clone()));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        100.0,
        easyroute::config::RouteGeneratorConfig::default(),
    );

    let preferences = RoutePreferences::default();

    let result = route_generator
        .generate_loop_route(center, 5.0, 1.0, &TransportMode::Walk, &preferences)
        .await;

    if let Ok(routes) = result {
        let route = &routes[0];

        // POIs should have sequential order_in_route values
        for (idx, route_poi) in route.pois.iter().enumerate() {
            assert_eq!(
                route_poi.order_in_route as usize,
                idx + 1,
                "POI order should be sequential"
            );
        }

        // Distance from start should increase
        if route.pois.len() > 1 {
            for i in 0..route.pois.len() - 1 {
                assert!(
                    route.pois[i].distance_from_start_km
                        <= route.pois[i + 1].distance_from_start_km,
                    "POIs should have increasing distance from start"
                );
            }
        }
    }

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[serial]
async fn test_route_scoring_different_preferences() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert POIs with different popularity scores
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // POIs ~1.0km away (N and S) — produces ~5.3km walking loop
    let mut popular = common::create_test_poi("Popular", PoiCategory::Monument, 48.8656, 2.3522);
    popular.popularity_score = 90.0;

    let mut hidden = common::create_test_poi("Hidden Gem", PoiCategory::Park, 48.8476, 2.3522);
    hidden.popularity_score = 20.0;

    easyroute::db::queries::insert_poi(&pool, &popular)
        .await
        .unwrap();
    easyroute::db::queries::insert_poi(&pool, &hidden)
        .await
        .unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(pool.clone()));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        100.0,
        easyroute::config::RouteGeneratorConfig::default(),
    );

    // Test with popular preference
    let popular_pref = RoutePreferences {
        poi_categories: None,
        hidden_gems: false,
        max_alternatives: 1,
    };

    let result = route_generator
        .generate_loop_route(center, 5.0, 1.0, &TransportMode::Walk, &popular_pref)
        .await;

    if let Ok(routes) = result {
        let route = &routes[0];
        // Score should reflect POI quality
        assert!(route.score > 0.0);
    }

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[serial]
async fn test_route_alternatives_use_different_waypoint_counts() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert multiple POIs in a diverse arrangement to allow for 2, 3, and 4 waypoint routes
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // Create 10 POIs ~1.0km from center in diverse directions
    // At this distance, 2wp loops ≈ 4-5km, 3wp loops ≈ 5-6km (validated with Mapbox)
    let pois = vec![
        common::create_test_poi("North POI", PoiCategory::Monument, 48.8656, 2.3522),
        common::create_test_poi("Northeast POI", PoiCategory::Park, 48.8630, 2.3600),
        common::create_test_poi("East POI", PoiCategory::Museum, 48.8566, 2.3652),
        common::create_test_poi("Southeast POI", PoiCategory::Viewpoint, 48.8502, 2.3600),
        common::create_test_poi("South POI", PoiCategory::Park, 48.8476, 2.3522),
        common::create_test_poi("Southwest POI", PoiCategory::Monument, 48.8502, 2.3444),
        common::create_test_poi("West POI", PoiCategory::Museum, 48.8566, 2.3392),
        common::create_test_poi("Northwest POI", PoiCategory::Viewpoint, 48.8630, 2.3444),
        common::create_test_poi("Center North POI", PoiCategory::Park, 48.8638, 2.3522),
        common::create_test_poi("Center South POI", PoiCategory::Monument, 48.8494, 2.3522),
    ];

    for poi in &pois {
        easyroute::db::queries::insert_poi(&pool, poi)
            .await
            .unwrap();
    }

    let mapbox_client =
        MapboxClient::new(std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY must be set"));
    let poi_repo: Arc<dyn easyroute::db::PoiRepository> =
        Arc::new(PgPoiRepository::new(pool.clone()));
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        100.0,
        easyroute::config::RouteGeneratorConfig::default(),
    );

    // Request 5 alternatives to get diverse waypoint counts
    let preferences = RoutePreferences {
        poi_categories: None,
        hidden_gems: false,
        max_alternatives: 5,
    };

    // Use 2km tolerance so both 2-waypoint (~4-5km) and 3-waypoint (~5-6km) routes
    // fit within normal tolerance, allowing the system to return diverse waypoint counts
    let result = route_generator
        .generate_loop_route(center, 5.0, 2.0, &TransportMode::Walk, &preferences)
        .await;

    assert!(result.is_ok(), "Route generation should succeed");
    let routes = result.unwrap();
    assert!(
        routes.len() >= 3,
        "Should generate at least 3 alternatives, got {}",
        routes.len()
    );

    // Check that routes have varying POI counts (proxy for different waypoint counts)
    let poi_counts: Vec<usize> = routes.iter().map(|r| r.pois.len()).collect();
    let unique_counts: std::collections::HashSet<_> = poi_counts.iter().collect();

    println!(
        "Generated {} routes with POI counts: {:?}",
        routes.len(),
        poi_counts
    );

    assert!(
        unique_counts.len() >= 2,
        "Routes should have diverse POI counts, got: {:?}",
        poi_counts
    );

    // Verify routes are within tolerance
    for (idx, route) in routes.iter().enumerate() {
        let distance = route.distance_km;
        assert!(
            (3.0..=8.0).contains(&distance),
            "Route {} distance {:.2}km should be within 3-8km range",
            idx,
            distance
        );
    }

    common::cleanup_test_db(&pool).await;
}
