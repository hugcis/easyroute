use super::*;

fn tags<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
    pairs.iter().copied().collect()
}

// -- determine_category --

#[test]
fn tourism_monument() {
    let t = tags(&[("tourism", "monument"), ("name", "Obelisk")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Monument));
}

#[test]
fn tourism_museum() {
    let t = tags(&[("tourism", "museum"), ("name", "Louvre")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Museum));
}

#[test]
fn historic_castle() {
    let t = tags(&[("historic", "castle"), ("name", "Château")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Castle));
}

#[test]
fn historic_fort_maps_to_castle() {
    let t = tags(&[("historic", "fort"), ("name", "Fort X")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Castle));
}

#[test]
fn amenity_place_of_worship_christian() {
    let t = tags(&[
        ("amenity", "place_of_worship"),
        ("religion", "christian"),
        ("name", "St. Paul"),
    ]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Church));
}

#[test]
fn amenity_place_of_worship_non_christian() {
    let t = tags(&[
        ("amenity", "place_of_worship"),
        ("religion", "muslim"),
        ("name", "Mosque"),
    ]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Cultural));
}

#[test]
fn amenity_place_of_worship_no_religion() {
    let t = tags(&[("amenity", "place_of_worship"), ("name", "Temple")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Cultural));
}

#[test]
fn leisure_park() {
    let t = tags(&[("leisure", "park"), ("name", "Central Park")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Park));
}

#[test]
fn leisure_garden() {
    let t = tags(&[("leisure", "garden"), ("name", "Botanical Garden")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Park));
}

#[test]
fn natural_waterfall() {
    let t = tags(&[("natural", "waterfall"), ("name", "Falls")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Waterfall));
}

#[test]
fn man_made_observation_tower() {
    let t = tags(&[
        ("man_made", "tower"),
        ("tower:type", "observation"),
        ("name", "Sky Tower"),
    ]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Viewpoint));
}

#[test]
fn man_made_tower_non_observation() {
    let t = tags(&[("man_made", "tower"), ("name", "Clock Tower")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Tower));
}

#[test]
fn craft_winery() {
    let t = tags(&[("craft", "winery"), ("name", "Domaine")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Winery));
}

#[test]
fn building_church() {
    let t = tags(&[("building", "cathedral"), ("name", "Notre-Dame")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Church));
}

#[test]
fn place_square() {
    let t = tags(&[("place", "square"), ("name", "Place Vendôme")]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Plaza));
}

#[test]
fn no_match_returns_none() {
    let t = tags(&[("shop", "bakery"), ("name", "Bread Shop")]);
    assert_eq!(determine_category(&t), None);
}

#[test]
fn tourism_priority_over_historic() {
    // If both tourism and historic are present, tourism wins
    let t = tags(&[
        ("tourism", "viewpoint"),
        ("historic", "castle"),
        ("name", "Castle View"),
    ]);
    assert_eq!(determine_category(&t), Some(PoiCategory::Viewpoint));
}

// -- calculate_popularity --

#[test]
fn base_score_with_sparse_tags() {
    // < 5 tags, no wikipedia -> base 50 - 10 = 40
    let t = tags(&[("name", "X"), ("tourism", "monument")]);
    assert!((calculate_popularity(&t) - 40.0).abs() < f32::EPSILON);
}

#[test]
fn wikipedia_boost() {
    let t = tags(&[
        ("name", "X"),
        ("tourism", "monument"),
        ("wikipedia", "en:X"),
        ("tag1", "a"),
        ("tag2", "b"),
    ]);
    // 50 + 20 = 70
    assert!((calculate_popularity(&t) - 70.0).abs() < f32::EPSILON);
}

#[test]
fn unesco_boost() {
    let t = tags(&[
        ("name", "X"),
        ("heritage:unesco", "1"),
        ("wikipedia", "en:X"),
        ("a", "1"),
        ("b", "2"),
    ]);
    // 50 + 20 (wiki) + 15 (unesco) = 85
    assert!((calculate_popularity(&t) - 85.0).abs() < f32::EPSILON);
}

#[test]
fn language_names_boost() {
    let t = tags(&[
        ("name", "X"),
        ("name:en", "X"),
        ("name:fr", "Y"),
        ("name:de", "Z"),
        ("name:es", "W"),
    ]);
    // 50 + 10 (>3 langs) = 60
    assert!((calculate_popularity(&t) - 60.0).abs() < f32::EPSILON);
}

#[test]
fn star_rating_boost() {
    let t = tags(&[
        ("name", "Hotel"),
        ("stars", "4"),
        ("a", "1"),
        ("b", "2"),
        ("c", "3"),
    ]);
    // 50 + 8 (4*2) = 58
    assert!((calculate_popularity(&t) - 58.0).abs() < f32::EPSILON);
}

#[test]
fn score_clamped_to_100() {
    let t = tags(&[
        ("name", "X"),
        ("wikipedia", "en:X"),
        ("wikidata", "Q1"),
        ("heritage:unesco", "1"),
        ("heritage", "yes"),
        ("tourism", "attraction"),
        ("historic", "monument"),
        ("website", "http://x"),
        ("opening_hours", "24/7"),
        ("stars", "5"),
        ("name:en", "A"),
        ("name:fr", "B"),
        ("name:de", "C"),
        ("name:es", "D"),
    ]);
    assert!((calculate_popularity(&t) - 100.0).abs() < f32::EPSILON);
}

// -- estimate_duration --

#[test]
fn explicit_duration_tag() {
    let t = tags(&[("duration", "45")]);
    assert_eq!(estimate_duration(&t, &PoiCategory::Museum), 45);
}

#[test]
fn default_museum_duration() {
    let t = tags(&[]);
    assert_eq!(estimate_duration(&t, &PoiCategory::Museum), 90);
}

#[test]
fn default_fallback_duration() {
    let t = tags(&[]);
    assert_eq!(estimate_duration(&t, &PoiCategory::Brewery), 30);
}

// -- build_description --

#[test]
fn description_from_tags() {
    let t = tags(&[
        ("description", "A grand palace"),
        ("architect", "Le Vau"),
        ("ele", "100"),
    ]);
    let desc = build_description(&t).unwrap();
    assert!(desc.contains("A grand palace"));
    assert!(desc.contains("Architect: Le Vau"));
    assert!(desc.contains("Elevation: 100m"));
}

#[test]
fn description_with_unesco() {
    let t = tags(&[("heritage:unesco", "1")]);
    let desc = build_description(&t).unwrap();
    assert_eq!(desc, "UNESCO World Heritage Site");
}

#[test]
fn description_none_when_empty() {
    let t = tags(&[("name", "X")]);
    assert!(build_description(&t).is_none());
}
