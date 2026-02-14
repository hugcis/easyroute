use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use easyroute::db::PgPoiRepository;
use easyroute::models::route::LoopRouteRequest;
use easyroute::models::{Coordinates, RoutePreferences, TransportMode};
use easyroute::services::mapbox::MapboxClient;
use easyroute::services::poi_service::PoiService;
use easyroute::services::route_generator::RouteGenerator;
use easyroute::services::snapping_service::SnappingService;
use easyroute::AppState;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

mod common;

async fn setup_test_app() -> axum::Router {
    let pool = common::setup_test_db().await;
    let config = common::get_test_config();

    let poi_repo: Arc<dyn easyroute::db::PoiRepository> = Arc::new(PgPoiRepository::new(pool));
    let mapbox_client = MapboxClient::new(config.mapbox_api_key.clone());
    let poi_service = PoiService::new(poi_repo.clone());
    let snapping_service = SnappingService::new(poi_repo.clone());
    let route_generator = RouteGenerator::new(
        mapbox_client,
        poi_service,
        snapping_service,
        config.snap_radius_m,
        config.route_generator.clone(),
    );

    let state = Arc::new(AppState {
        poi_repo,
        route_generator,
        cache: None, // No Redis cache in tests
    });

    easyroute::routes::create_router(state)
}

#[tokio::test]
async fn test_health_check_endpoint() {
    let app = setup_test_app().await;

    let request = Request::builder()
        .uri("/debug/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert!(json["checks"]["database"].is_string() || json["checks"]["database"].is_object());
}

#[tokio::test]
async fn test_loop_route_endpoint_validation() {
    let app = setup_test_app().await;

    // Test with invalid distance (too small)
    let invalid_request = json!({
        "start_point": {"lat": 48.8566, "lng": 2.3522},
        "distance_km": 0.1,  // Too small
        "mode": "walk",
        "preferences": {}
    });

    let request = Request::builder()
        .method("POST")
        .uri("/routes/loop")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&invalid_request).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Should reject invalid distance"
    );
}

#[tokio::test]
async fn test_loop_route_request_deserialization() {
    let json_data = json!({
        "start_point": {"lat": 48.8566, "lng": 2.3522},
        "distance_km": 5.0,
        "distance_tolerance": 0.5,
        "mode": "walk",
        "preferences": {
            "poi_categories": ["monument", "park"],
            "hidden_gems": false,
            "max_alternatives": 2
        }
    });

    let request: LoopRouteRequest = serde_json::from_value(json_data).unwrap();

    assert_eq!(request.start_point.lat, 48.8566);
    assert_eq!(request.distance_km, 5.0);
    assert_eq!(request.mode, TransportMode::Walk);
    assert_eq!(request.preferences.max_alternatives, 2);
}

#[tokio::test]
async fn test_loop_route_default_values() {
    let json_data = json!({
        "start_point": {"lat": 48.8566, "lng": 2.3522},
        "distance_km": 5.0,
        "mode": "walk"
    });

    let request: LoopRouteRequest = serde_json::from_value(json_data).unwrap();

    // Should have default values
    assert_eq!(request.distance_tolerance, 0.5);
    assert_eq!(request.preferences.max_alternatives, 3);
    assert!(!request.preferences.hidden_gems);
}

#[tokio::test]
async fn test_coordinates_validation() {
    // Valid coordinates
    assert!(Coordinates::new(48.8566, 2.3522).is_ok());
    assert!(Coordinates::new(0.0, 0.0).is_ok());
    assert!(Coordinates::new(-90.0, -180.0).is_ok());
    assert!(Coordinates::new(90.0, 180.0).is_ok());

    // Invalid coordinates
    assert!(Coordinates::new(91.0, 0.0).is_err()); // Invalid lat
    assert!(Coordinates::new(-91.0, 0.0).is_err()); // Invalid lat
    assert!(Coordinates::new(0.0, 181.0).is_err()); // Invalid lng
    assert!(Coordinates::new(0.0, -181.0).is_err()); // Invalid lng
}

#[tokio::test]
async fn test_route_preferences_serialization() {
    let prefs = RoutePreferences {
        poi_categories: Some(vec![
            easyroute::models::PoiCategory::Monument,
            easyroute::models::PoiCategory::Park,
        ]),
        hidden_gems: true,
        max_alternatives: 5,
    };

    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["hidden_gems"], true);
    assert_eq!(json["max_alternatives"], 5);

    let deserialized: RoutePreferences = serde_json::from_value(json).unwrap();
    assert!(deserialized.hidden_gems);
    assert_eq!(deserialized.max_alternatives, 5);
}
