/// OSM tag mapping utilities for Overpass API queries
/// This module centralizes all OpenStreetMap tag-to-category mappings
use crate::models::PoiCategory;
use std::collections::HashMap;

/// Map POI categories to OpenStreetMap tags for Overpass queries
pub fn category_to_osm_tags(category: &PoiCategory) -> Vec<(&str, &str)> {
    match category {
        // Original categories
        PoiCategory::Monument => vec![("tourism", "monument"), ("tourism", "memorial")],
        PoiCategory::Viewpoint => vec![("tourism", "viewpoint")],
        PoiCategory::Park => vec![("leisure", "park"), ("leisure", "garden")],
        PoiCategory::Museum => vec![("tourism", "museum"), ("tourism", "gallery")],
        PoiCategory::Restaurant => vec![("amenity", "restaurant")],
        PoiCategory::Cafe => vec![("amenity", "cafe")],
        PoiCategory::Historic => vec![("historic", "*")],
        PoiCategory::Cultural => vec![("tourism", "attraction"), ("amenity", "arts_centre")],

        // Natural/Scenic
        PoiCategory::Waterfront => vec![
            ("natural", "beach"),
            ("natural", "coastline"),
            ("leisure", "beach_resort"),
        ],
        PoiCategory::Waterfall => vec![("waterway", "waterfall")],
        PoiCategory::NatureReserve => vec![
            ("leisure", "nature_reserve"),
            ("boundary", "protected_area"),
        ],

        // Architectural
        PoiCategory::Church => vec![("amenity", "place_of_worship")],
        PoiCategory::Castle => vec![("historic", "castle")],
        PoiCategory::Bridge => vec![("man_made", "bridge")],
        PoiCategory::Tower => vec![("man_made", "tower")],

        // Urban Interest
        PoiCategory::Plaza => vec![("place", "square")],
        PoiCategory::Fountain => vec![("amenity", "fountain")],
        PoiCategory::Market => vec![("amenity", "marketplace")],
        PoiCategory::Artwork => vec![("tourism", "artwork")],
        PoiCategory::Lighthouse => vec![("man_made", "lighthouse")],

        // Activity
        PoiCategory::Winery => vec![("craft", "winery")],
        PoiCategory::Brewery => vec![("craft", "brewery")],
        PoiCategory::Theatre => vec![("amenity", "theatre")],
        PoiCategory::Library => vec![("amenity", "library")],
    }
}

/// Calculate popularity score from OSM tags
pub fn calculate_popularity(tags: &HashMap<String, String>) -> f32 {
    let mut score: f32 = 50.0; // Base score

    // Increase score for Wikipedia entries
    if tags.contains_key("wikipedia") || tags.contains_key("wikidata") {
        score += 20.0;
    }

    // Increase score for official tourism attractions
    if tags.get("tourism") == Some(&"attraction".to_string()) {
        score += 15.0;
    }

    // Monuments and viewpoints tend to be popular
    if let Some(tourism_type) = tags.get("tourism") {
        match tourism_type.as_str() {
            "monument" | "memorial" => score += 10.0,
            "viewpoint" => score += 15.0,
            _ => {}
        }
    }

    score.min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_to_osm_tags() {
        let tags = category_to_osm_tags(&PoiCategory::Monument);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&("tourism", "monument")));

        let tags = category_to_osm_tags(&PoiCategory::Castle);
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&("historic", "castle")));
    }

    #[test]
    fn test_calculate_popularity() {
        let mut tags = HashMap::new();
        assert_eq!(calculate_popularity(&tags), 50.0);

        tags.insert("wikipedia".to_string(), "...".to_string());
        assert!(calculate_popularity(&tags) > 50.0);
    }
}
