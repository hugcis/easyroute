use easyroute::db::queries;
use easyroute::models::{Coordinates, PoiCategory};
use serial_test::serial;

mod common;

#[tokio::test]
#[ignore]
#[serial]
async fn test_insert_and_find_pois() {
    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Create test POIs
    let poi1 = common::create_test_poi("Eiffel Tower", PoiCategory::Monument, 48.8584, 2.2945);
    let poi2 = common::create_test_poi("Louvre Museum", PoiCategory::Museum, 48.8606, 2.3376);
    let poi3 = common::create_test_poi("Luxembourg Gardens", PoiCategory::Park, 48.8462, 2.3372);

    // Insert POIs
    queries::insert_poi(&pool, &poi1).await.unwrap();
    queries::insert_poi(&pool, &poi2).await.unwrap();
    queries::insert_poi(&pool, &poi3).await.unwrap();

    // Search for POIs - use a larger radius to find all test POIs
    // Center point between all three POIs
    let center = Coordinates::new(48.8537, 2.3147).unwrap();
    let pois = queries::find_pois_within_radius(
        &pool, &center, 10000.0, // 10km radius to ensure we find all test POIs
        None, 10,
    )
    .await
    .unwrap();

    // Should find all three POIs
    assert!(
        pois.len() >= 3,
        "Expected to find 3 POIs but got {}",
        pois.len()
    );

    let poi_names: Vec<&str> = pois.iter().map(|p| p.name.as_str()).collect();
    assert!(
        pois.iter().any(|p| p.name == "Eiffel Tower"),
        "Expected to find 'Eiffel Tower' but found: {:?}",
        poi_names
    );
    assert!(
        pois.iter().any(|p| p.name == "Louvre Museum"),
        "Expected to find 'Louvre Museum'"
    );
    assert!(
        pois.iter().any(|p| p.name == "Luxembourg Gardens"),
        "Expected to find 'Luxembourg Gardens'"
    );

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[ignore]
#[serial]
async fn test_find_pois_by_category() {
    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    // Create POIs of different categories
    let monument =
        common::create_test_poi("Arc de Triomphe", PoiCategory::Monument, 48.8738, 2.2950);
    let museum = common::create_test_poi("Musée d'Orsay", PoiCategory::Museum, 48.8600, 2.3266);

    queries::insert_poi(&pool, &monument).await.unwrap();
    queries::insert_poi(&pool, &museum).await.unwrap();

    // Search for monuments only (use a center point between both POIs)
    let center = Coordinates::new(48.8669, 2.3108).unwrap(); // Midpoint
    let categories = vec![PoiCategory::Monument];
    let pois = queries::find_pois_within_radius(
        &pool,
        &center,
        10000.0, // 10km radius to find both POIs
        Some(&categories),
        10,
    )
    .await
    .unwrap();

    // Should only find monuments
    assert!(
        !pois.is_empty(),
        "Expected to find at least the monument POI"
    );
    assert!(
        pois.iter().all(|p| p.category == PoiCategory::Monument),
        "Expected all POIs to be monuments"
    );

    // Verify we found our test POI
    assert!(
        pois.iter().any(|p| p.name == "Arc de Triomphe"),
        "Expected to find 'Arc de Triomphe'"
    );

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[ignore]
#[serial]
async fn test_spatial_distance_ordering() {
    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // Create POIs at different distances
    let close = common::create_test_poi("Close POI", PoiCategory::Monument, 48.8570, 2.3525);
    let far = common::create_test_poi("Far POI", PoiCategory::Monument, 48.8800, 2.4000);

    queries::insert_poi(&pool, &close).await.unwrap();
    queries::insert_poi(&pool, &far).await.unwrap();

    // Search with large radius
    let pois = queries::find_pois_within_radius(&pool, &center, 10000.0, None, 10)
        .await
        .unwrap();

    // Should return closest POIs first
    assert!(!pois.is_empty());
    if pois.len() >= 2 {
        let first_dist = center.distance_to(&pois[0].coordinates);
        let second_dist = center.distance_to(&pois[1].coordinates);
        assert!(
            first_dist <= second_dist,
            "POIs should be ordered by distance"
        );
    }

    common::cleanup_test_db(&pool).await;
}

#[tokio::test]
#[ignore]
#[serial]
async fn test_duplicate_osm_id_handling() {
    let pool = common::setup_test_db().await;
    common::cleanup_test_db(&pool).await;

    let mut poi1 = common::create_test_poi("Test POI", PoiCategory::Monument, 48.8566, 2.3522);
    poi1.osm_id = Some(12345);

    let mut poi2 = common::create_test_poi("Duplicate POI", PoiCategory::Park, 48.8570, 2.3525);
    poi2.osm_id = Some(12345); // Same OSM ID

    // First insert should succeed
    queries::insert_poi(&pool, &poi1).await.unwrap();

    // Second insert should fail due to unique constraint
    let result = queries::insert_poi(&pool, &poi2).await;
    assert!(result.is_err(), "Duplicate OSM ID should be rejected");

    common::cleanup_test_db(&pool).await;
}
