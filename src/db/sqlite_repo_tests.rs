use super::*;
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_test_repo() -> SqlitePoiRepository {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");
    SqlitePoiRepository::create_schema(&pool)
        .await
        .expect("Failed to create schema");
    SqlitePoiRepository::new(pool)
}

fn make_poi(name: &str, cat: PoiCategory, lat: f64, lng: f64) -> Poi {
    Poi {
        id: Uuid::new_v4(),
        name: name.to_string(),
        category: cat,
        coordinates: Coordinates::new(lat, lng).unwrap(),
        popularity_score: 50.0,
        description: Some(format!("Test POI: {name}")),
        estimated_visit_duration_minutes: Some(15),
        osm_id: None,
    }
}

#[tokio::test]
async fn create_schema_idempotent() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    SqlitePoiRepository::create_schema(&pool).await.unwrap();
    SqlitePoiRepository::create_schema(&pool).await.unwrap();
}

#[tokio::test]
async fn insert_and_count() {
    let repo = setup_test_repo().await;
    assert_eq!(repo.count().await.unwrap(), 0);

    repo.insert(&make_poi("A", PoiCategory::Monument, 48.85, 2.35))
        .await
        .unwrap();
    repo.insert(&make_poi("B", PoiCategory::Park, 48.86, 2.36))
        .await
        .unwrap();
    repo.insert(&make_poi("C", PoiCategory::Museum, 48.87, 2.37))
        .await
        .unwrap();

    assert_eq!(repo.count().await.unwrap(), 3);
}

#[tokio::test]
async fn find_within_radius_basic() {
    let repo = setup_test_repo().await;
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // ~500m away
    repo.insert(&make_poi("Near", PoiCategory::Monument, 48.860, 2.352))
        .await
        .unwrap();
    // ~50km away
    repo.insert(&make_poi("Far", PoiCategory::Monument, 49.3, 2.35))
        .await
        .unwrap();

    let results = repo
        .find_within_radius(&center, 2000.0, None, 100)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Near");
}

#[tokio::test]
async fn find_within_radius_category_filter() {
    let repo = setup_test_repo().await;
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    repo.insert(&make_poi("Monument1", PoiCategory::Monument, 48.857, 2.353))
        .await
        .unwrap();
    repo.insert(&make_poi("Park1", PoiCategory::Park, 48.858, 2.354))
        .await
        .unwrap();
    repo.insert(&make_poi("Museum1", PoiCategory::Museum, 48.856, 2.351))
        .await
        .unwrap();

    let cats = [PoiCategory::Monument, PoiCategory::Museum];
    let results = repo
        .find_within_radius(&center, 2000.0, Some(&cats), 100)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    for poi in &results {
        assert!(poi.category == PoiCategory::Monument || poi.category == PoiCategory::Museum);
    }
}

#[tokio::test]
async fn find_within_radius_distance_ordering() {
    let repo = setup_test_repo().await;
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    // Insert in non-distance order
    repo.insert(&make_poi("Medium", PoiCategory::Monument, 48.860, 2.355))
        .await
        .unwrap();
    repo.insert(&make_poi("Closest", PoiCategory::Monument, 48.857, 2.353))
        .await
        .unwrap();
    repo.insert(&make_poi("Farthest", PoiCategory::Monument, 48.865, 2.360))
        .await
        .unwrap();

    let results = repo
        .find_within_radius(&center, 5000.0, None, 100)
        .await
        .unwrap();
    assert_eq!(results.len(), 3);

    // Verify ascending distance order
    let d0 = center.distance_to(&results[0].coordinates);
    let d1 = center.distance_to(&results[1].coordinates);
    let d2 = center.distance_to(&results[2].coordinates);
    assert!(d0 <= d1, "d0={d0} should <= d1={d1}");
    assert!(d1 <= d2, "d1={d1} should <= d2={d2}");
}

#[tokio::test]
async fn find_within_radius_limit() {
    let repo = setup_test_repo().await;
    let center = Coordinates::new(48.8566, 2.3522).unwrap();

    for i in 0..10 {
        let lat = 48.856 + (i as f64) * 0.001;
        repo.insert(&make_poi(
            &format!("POI{i}"),
            PoiCategory::Monument,
            lat,
            2.353,
        ))
        .await
        .unwrap();
    }

    let results = repo
        .find_within_radius(&center, 5000.0, None, 3)
        .await
        .unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn find_within_radius_haversine_rejects_bbox_corners() {
    let repo = setup_test_repo().await;
    // Center at equator to maximize bbox/circle difference
    let center = Coordinates::new(0.0, 0.0).unwrap();
    let radius_m = 1000.0; // 1km

    // Place POI at the bbox corner (diagonal from center).
    // Bbox extends ~0.009° in each direction. Corner is at (0.009, 0.009),
    // which is ~sqrt(2) * 1km ≈ 1.41km from center — outside the 1km circle.
    let lat_delta = radius_m / 111_000.0;
    let lng_delta = radius_m / (111_000.0 * 0.0_f64.to_radians().cos());
    let corner_lat = lat_delta * 0.99; // just inside bbox
    let corner_lng = lng_delta * 0.99;

    repo.insert(&make_poi(
        "Corner",
        PoiCategory::Monument,
        corner_lat,
        corner_lng,
    ))
    .await
    .unwrap();

    let results = repo
        .find_within_radius(&center, radius_m, None, 100)
        .await
        .unwrap();
    // Corner POI is ~1.40km from center, outside 1km radius
    assert!(
        results.is_empty(),
        "Corner POI at ({corner_lat}, {corner_lng}) should be outside haversine radius"
    );
}

#[tokio::test]
async fn find_in_bbox_basic() {
    let repo = setup_test_repo().await;

    repo.insert(&make_poi("Inside", PoiCategory::Monument, 48.857, 2.353))
        .await
        .unwrap();
    repo.insert(&make_poi("Outside", PoiCategory::Monument, 49.0, 3.0))
        .await
        .unwrap();

    let results = repo
        .find_in_bbox(48.85, 48.86, 2.35, 2.36, None, 100)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Inside");
}

#[tokio::test]
async fn find_in_bbox_category_filter() {
    let repo = setup_test_repo().await;

    repo.insert(&make_poi("Mon", PoiCategory::Monument, 48.857, 2.353))
        .await
        .unwrap();
    repo.insert(&make_poi("Park", PoiCategory::Park, 48.858, 2.354))
        .await
        .unwrap();

    let cats = [PoiCategory::Park];
    let results = repo
        .find_in_bbox(48.85, 48.86, 2.35, 2.36, Some(&cats), 100)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Park");
}

#[tokio::test]
async fn insert_rtree_sync() {
    let repo = setup_test_repo().await;
    let poi = make_poi("Synced", PoiCategory::Monument, 48.857, 2.353);
    repo.insert(&poi).await.unwrap();

    // Verify R-tree entry has correct lat/lng bounds
    let (min_lat, max_lat, min_lng, max_lng): (f64, f64, f64, f64) =
        sqlx::query_as("SELECT min_lat, max_lat, min_lng, max_lng FROM pois_rtree WHERE id = 1")
            .fetch_one(&repo.pool)
            .await
            .unwrap();

    // R-tree stores values as 32-bit floats, so use relaxed tolerance
    assert!((min_lat - 48.857).abs() < 1e-4, "min_lat={min_lat}");
    assert!((max_lat - 48.857).abs() < 1e-4, "max_lat={max_lat}");
    assert!((min_lng - 2.353).abs() < 1e-4, "min_lng={min_lng}");
    assert!((max_lng - 2.353).abs() < 1e-4, "max_lng={max_lng}");
}

#[tokio::test]
async fn insert_batch_basic() {
    let repo = setup_test_repo().await;
    let pois = vec![
        make_poi("A", PoiCategory::Monument, 48.85, 2.35),
        make_poi("B", PoiCategory::Park, 48.86, 2.36),
        make_poi("C", PoiCategory::Museum, 48.87, 2.37),
    ];
    let inserted = repo.insert_batch(&pois).await.unwrap();
    assert_eq!(inserted, 3);
    assert_eq!(repo.count().await.unwrap(), 3);
}

#[tokio::test]
async fn insert_batch_skips_duplicate_osm_id() {
    let repo = setup_test_repo().await;
    let mut poi_a = make_poi("A", PoiCategory::Monument, 48.85, 2.35);
    poi_a.osm_id = Some(12345);
    let mut poi_b = make_poi("B", PoiCategory::Park, 48.86, 2.36);
    poi_b.osm_id = Some(12345); // same osm_id

    let inserted = repo.insert_batch(&[poi_a, poi_b]).await.unwrap();
    assert_eq!(inserted, 1);
    assert_eq!(repo.count().await.unwrap(), 1);
}

#[tokio::test]
async fn set_and_read_meta() {
    let repo = setup_test_repo().await;
    repo.set_meta("region_name", "monaco").await.unwrap();
    repo.set_meta("poi_count", "42").await.unwrap();

    let val: String = sqlx::query_scalar("SELECT value FROM region_meta WHERE key = 'region_name'")
        .fetch_one(&repo.pool)
        .await
        .unwrap();
    assert_eq!(val, "monaco");

    // Overwrite
    repo.set_meta("region_name", "paris").await.unwrap();
    let val: String = sqlx::query_scalar("SELECT value FROM region_meta WHERE key = 'region_name'")
        .fetch_one(&repo.pool)
        .await
        .unwrap();
    assert_eq!(val, "paris");
}

#[tokio::test]
async fn insert_null_optionals() {
    let repo = setup_test_repo().await;
    let poi = Poi {
        id: Uuid::new_v4(),
        name: "Minimal".to_string(),
        category: PoiCategory::Historic,
        coordinates: Coordinates::new(48.85, 2.35).unwrap(),
        popularity_score: 0.0,
        description: None,
        estimated_visit_duration_minutes: None,
        osm_id: None,
    };

    repo.insert(&poi).await.unwrap();

    let results = repo
        .find_within_radius(&Coordinates::new(48.85, 2.35).unwrap(), 1000.0, None, 10)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Minimal");
    assert!(results[0].description.is_none());
    assert!(results[0].estimated_visit_duration_minutes.is_none());
    assert!(results[0].osm_id.is_none());
}
