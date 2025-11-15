use easyroute::models::{Coordinates, PoiCategory, RoutePreferences, TransportMode};
use easyroute::services::mapbox::MapboxClient;
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;

mod common;

#[tokio::test]
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

    easyroute::db::queries::insert_poi(&pool, &poi1).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi3).await.unwrap();

    // Create services
    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_service = PoiService::new(pool.clone());
    let snapping_service = SnappingService::new(pool.clone());
    let route_generator = RouteGenerator::new(mapbox_client, poi_service, snapping_service, 100.0);

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
    assert!(route.distance_km > 0.0, "Route should have positive distance");
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

    easyroute::db::queries::insert_poi(&pool, &poi1).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi3).await.unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_service = PoiService::new(pool.clone());
    let snapping_service = SnappingService::new(pool.clone());
    let route_generator = RouteGenerator::new(mapbox_client, poi_service, snapping_service, 100.0);

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
                route.distance_km >= target - tolerance
                    && route.distance_km <= target + tolerance,
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

    easyroute::db::queries::insert_poi(&pool, &poi1).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &poi2).await.unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_service = PoiService::new(pool.clone());
    let snapping_service = SnappingService::new(pool.clone());
    let route_generator = RouteGenerator::new(mapbox_client, poi_service, snapping_service, 100.0);

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
async fn test_route_scoring_different_preferences() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Insert POIs with different popularity scores
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    let mut popular = common::create_test_poi("Popular", PoiCategory::Monument, 48.8600, 2.3522);
    popular.popularity_score = 90.0;

    let mut hidden = common::create_test_poi("Hidden Gem", PoiCategory::Park, 48.8566, 2.3600);
    hidden.popularity_score = 20.0;

    easyroute::db::queries::insert_poi(&pool, &popular).await.unwrap();
    easyroute::db::queries::insert_poi(&pool, &hidden).await.unwrap();

    let mapbox_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY required");
    let mapbox_client = MapboxClient::new(mapbox_key);
    let poi_service = PoiService::new(pool.clone());
    let route_generator = RouteGenerator::new(mapbox_client, poi_service);

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
