use easyroute::models::{Coordinates, TransportMode};
use easyroute::services::mapbox::MapboxClient;

mod common;

#[tokio::test]
async fn test_mapbox_walking_directions() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let api_key =
        std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY must be set for integration tests");
    let client = MapboxClient::new(api_key);

    // Create a simple route: Eiffel Tower to Louvre
    let eiffel = Coordinates::new(48.8584, 2.2945).unwrap();
    let louvre = Coordinates::new(48.8606, 2.3376).unwrap();

    let waypoints = vec![eiffel, louvre];

    let result = client
        .get_directions(&waypoints, &TransportMode::Walk)
        .await;

    assert!(result.is_ok(), "Mapbox API call should succeed");

    let directions = result.unwrap();
    assert!(
        directions.distance_meters > 0.0,
        "Distance should be positive"
    );
    assert!(
        directions.duration_seconds > 0.0,
        "Duration should be positive"
    );
    assert!(
        !directions.geometry.is_empty(),
        "Geometry should not be empty"
    );

    // Rough sanity check: walking from Eiffel to Louvre should be ~3-5km
    let distance_km = directions.distance_km();
    assert!(
        distance_km > 2.0 && distance_km < 7.0,
        "Distance should be reasonable: got {}km",
        distance_km
    );
}

#[tokio::test]
async fn test_mapbox_loop_route() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let api_key =
        std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY must be set for integration tests");
    let client = MapboxClient::new(api_key);

    // Create a loop: Start -> POI1 -> POI2 -> Start
    let start = Coordinates::new(48.8566, 2.3522).unwrap();
    let poi1 = Coordinates::new(48.8584, 2.2945).unwrap(); // Eiffel Tower
    let poi2 = Coordinates::new(48.8606, 2.3376).unwrap(); // Louvre

    let waypoints = vec![start, poi1, poi2, start];

    let result = client
        .get_directions(&waypoints, &TransportMode::Walk)
        .await;

    assert!(result.is_ok(), "Loop route should succeed");

    let directions = result.unwrap();

    // Path should start and end at same location
    let coords = directions.to_coordinates();
    assert!(!coords.is_empty());

    let first = coords.first().unwrap();
    let last = coords.last().unwrap();

    let distance_diff = first.distance_to(last);
    assert!(
        distance_diff < 0.1,
        "Loop should return to start (distance: {}km)",
        distance_diff
    );
}

#[tokio::test]
async fn test_mapbox_bike_mode() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let api_key =
        std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY must be set for integration tests");
    let client = MapboxClient::new(api_key);

    let start = Coordinates::new(48.8566, 2.3522).unwrap();
    let end = Coordinates::new(48.8584, 2.2945).unwrap();

    let waypoints = vec![start, end];

    let result = client
        .get_directions(&waypoints, &TransportMode::Bike)
        .await;

    assert!(result.is_ok(), "Bike directions should work");

    let directions = result.unwrap();
    assert!(directions.distance_meters > 0.0);
    assert!(directions.duration_seconds > 0.0);
}

#[tokio::test]
async fn test_mapbox_invalid_coordinates() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let api_key = std::env::var("MAPBOX_API_KEY").expect("MAPBOX_API_KEY must be set");
    let client = MapboxClient::new(api_key);

    // Try with only one waypoint (should fail)
    let waypoints = vec![Coordinates::new(48.8566, 2.3522).unwrap()];

    let result = client
        .get_directions(&waypoints, &TransportMode::Walk)
        .await;

    assert!(result.is_err(), "Should fail with less than 2 waypoints");
}

#[test]
fn test_mapbox_response_conversions() {
    use easyroute::services::mapbox::DirectionsResponse;

    let response = DirectionsResponse {
        distance_meters: 5240.0,
        duration_seconds: 3720.0,
        geometry: vec![[2.3522, 48.8566], [2.2945, 48.8584]],
    };

    assert_eq!(response.distance_km(), 5.24);
    assert_eq!(response.duration_minutes(), 62);

    let coords = response.to_coordinates();
    assert_eq!(coords.len(), 2);
    assert_eq!(coords[0].lat, 48.8566);
    assert_eq!(coords[0].lng, 2.3522);
}
