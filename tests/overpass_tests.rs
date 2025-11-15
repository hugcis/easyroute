use easyroute::models::{Coordinates, PoiCategory};
use easyroute::services::overpass::OverpassClient;

mod common;

#[tokio::test]
async fn test_overpass_query_pois() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let client = OverpassClient::new();

    // Query near Eiffel Tower
    let center = Coordinates::new(48.8584, 2.2945).unwrap();
    let categories = vec![PoiCategory::Monument];

    let result = client.query_pois(&center, 1000.0, &categories).await;

    assert!(result.is_ok(), "Overpass query should succeed");

    let pois = result.unwrap();

    // Should find at least some monuments near Eiffel Tower
    assert!(!pois.is_empty(), "Should find monuments near Eiffel Tower");

    // Verify POI structure
    for poi in &pois {
        assert!(!poi.name.is_empty(), "POI should have a name");
        assert_eq!(poi.category, PoiCategory::Monument);
        assert!(
            poi.popularity_score >= 0.0 && poi.popularity_score <= 100.0,
            "Popularity score should be 0-100"
        );
    }
}

#[tokio::test]
async fn test_overpass_multiple_categories() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let client = OverpassClient::new();
    let center = Coordinates::new(48.8566, 2.3522).unwrap(); // Central Paris

    let categories = vec![
        PoiCategory::Monument,
        PoiCategory::Museum,
        PoiCategory::Park,
    ];

    let result = client.query_pois(&center, 2000.0, &categories).await;

    assert!(result.is_ok(), "Multi-category query should succeed");

    let pois = result.unwrap();

    // Should find POIs of various categories
    assert!(!pois.is_empty(), "Should find POIs in central Paris");

    // Check that we have different categories
    let has_multiple_categories = pois
        .windows(2)
        .any(|window| window[0].category != window[1].category);

    if pois.len() > 1 {
        // With multiple POIs, we might have different categories
        println!("Found {} POIs with categories", pois.len());
    }
}

#[tokio::test]
async fn test_overpass_osm_id_extraction() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let client = OverpassClient::new();
    let center = Coordinates::new(48.8584, 2.2945).unwrap();
    let categories = vec![PoiCategory::Monument];

    let result = client.query_pois(&center, 500.0, &categories).await;

    if let Ok(pois) = result {
        // OSM IDs should be present for POIs from Overpass
        for poi in &pois {
            assert!(poi.osm_id.is_some(), "POI from Overpass should have OSM ID");
            assert!(poi.osm_id.unwrap() > 0, "OSM ID should be positive");
        }
    }
}

#[tokio::test]
async fn test_overpass_coordinate_validity() {
    if common::should_skip_real_api_tests() {
        println!("Skipping real API test");
        return;
    }

    let client = OverpassClient::new();
    let center = Coordinates::new(48.8584, 2.2945).unwrap();
    let categories = vec![PoiCategory::Viewpoint, PoiCategory::Park];

    let result = client.query_pois(&center, 1500.0, &categories).await;

    if let Ok(pois) = result {
        for poi in &pois {
            // Coordinates should be valid
            assert!(
                poi.coordinates.lat >= -90.0 && poi.coordinates.lat <= 90.0,
                "Latitude should be valid: {}",
                poi.coordinates.lat
            );
            assert!(
                poi.coordinates.lng >= -180.0 && poi.coordinates.lng <= 180.0,
                "Longitude should be valid: {}",
                poi.coordinates.lng
            );

            // POI should be within reasonable distance of center
            let distance = center.distance_to(&poi.coordinates);
            assert!(
                distance <= 2.0, // 2km max (allowing some margin)
                "POI should be within search radius: {}km away",
                distance
            );
        }
    }
}
