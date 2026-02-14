//! OSM tag-to-POI mapping logic.
//!
//! Ported from `osm/osm_poi_style.lua`. Converts raw OSM tags into
//! [`PoiCategory`], popularity scores, visit duration estimates, and
//! human-readable descriptions.

use std::collections::HashMap;

use crate::models::PoiCategory;

/// Collect an osmpbf tag iterator into a borrowed `HashMap`.
pub fn collect_tags<'a>(
    iter: impl Iterator<Item = (&'a str, &'a str)>,
) -> HashMap<&'a str, &'a str> {
    iter.collect()
}

// ---------------------------------------------------------------------------
// Category mapping
// ---------------------------------------------------------------------------

/// Determine the [`PoiCategory`] for a set of OSM tags.
///
/// Tag priority: tourism > historic > amenity > leisure > natural >
/// man_made > craft > building > place.
pub fn determine_category(tags: &HashMap<&str, &str>) -> Option<PoiCategory> {
    // Tourism (highest priority)
    if let Some(v) = tags.get("tourism") {
        if let Some(cat) = map_tourism(v) {
            return Some(cat);
        }
    }

    // Historic
    if let Some(v) = tags.get("historic") {
        if let Some(cat) = map_historic(v) {
            return Some(cat);
        }
    }

    // Amenity
    if let Some(v) = tags.get("amenity") {
        if let Some(cat) = map_amenity(v, tags) {
            return Some(cat);
        }
    }

    // Leisure
    if let Some(v) = tags.get("leisure") {
        match *v {
            "park" | "garden" => return Some(PoiCategory::Park),
            "nature_reserve" => return Some(PoiCategory::NatureReserve),
            _ => {}
        }
    }

    // Natural
    if let Some(v) = tags.get("natural") {
        match *v {
            "waterfall" => return Some(PoiCategory::Waterfall),
            "beach" | "water" => return Some(PoiCategory::Waterfront),
            _ => {}
        }
    }

    // Man-made
    if let Some(v) = tags.get("man_made") {
        match *v {
            "lighthouse" => return Some(PoiCategory::Lighthouse),
            "tower" => {
                if tags.get("tower:type") == Some(&"observation") {
                    return Some(PoiCategory::Viewpoint);
                }
                return Some(PoiCategory::Tower);
            }
            _ => {}
        }
    }

    // Craft
    if let Some(v) = tags.get("craft") {
        match *v {
            "winery" => return Some(PoiCategory::Winery),
            "brewery" => return Some(PoiCategory::Brewery),
            _ => {}
        }
    }

    // Building
    if let Some(v) = tags.get("building") {
        match *v {
            "church" | "cathedral" | "chapel" => return Some(PoiCategory::Church),
            "castle" => return Some(PoiCategory::Castle),
            _ => {}
        }
    }

    // Place
    if tags.get("place") == Some(&"square") {
        return Some(PoiCategory::Plaza);
    }

    None
}

fn map_tourism(value: &str) -> Option<PoiCategory> {
    match value {
        "monument" => Some(PoiCategory::Monument),
        "viewpoint" => Some(PoiCategory::Viewpoint),
        "museum" => Some(PoiCategory::Museum),
        "attraction" => Some(PoiCategory::Cultural),
        "artwork" => Some(PoiCategory::Artwork),
        _ => None,
    }
}

fn map_historic(value: &str) -> Option<PoiCategory> {
    match value {
        "castle" | "manor" | "fort" => Some(PoiCategory::Castle),
        "ruins" | "archaeological_site" => Some(PoiCategory::Historic),
        "memorial" => Some(PoiCategory::Monument),
        "monument" => Some(PoiCategory::Monument),
        _ => None,
    }
}

fn map_amenity(value: &str, tags: &HashMap<&str, &str>) -> Option<PoiCategory> {
    match value {
        "place_of_worship" => {
            if tags.get("religion") == Some(&"christian") {
                Some(PoiCategory::Church)
            } else {
                Some(PoiCategory::Cultural)
            }
        }
        "theatre" | "cinema" => Some(PoiCategory::Theatre),
        "library" => Some(PoiCategory::Library),
        "fountain" => Some(PoiCategory::Fountain),
        "marketplace" => Some(PoiCategory::Market),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Popularity
// ---------------------------------------------------------------------------

/// Calculate a popularity score (0–100) from OSM tags.
pub fn calculate_popularity(tags: &HashMap<&str, &str>) -> f32 {
    let mut score: f32 = 50.0;

    // Wikipedia/Wikidata presence
    if tags.contains_key("wikipedia") || tags.contains_key("wikidata") {
        score += 20.0;
    }

    // UNESCO World Heritage
    if tags.contains_key("heritage:unesco") || tags.get("heritage") == Some(&"yes") {
        score += 15.0;
    }

    // Tourist importance
    if tags.get("tourist") == Some(&"yes") || tags.get("tourism") == Some(&"attraction") {
        score += 10.0;
    }

    // Multiple language names
    let lang_count = tags.keys().filter(|k| k.starts_with("name:")).count();
    if lang_count > 3 {
        score += 10.0;
    } else if lang_count > 1 {
        score += 5.0;
    }

    // Website
    if tags.contains_key("website") || tags.contains_key("contact:website") {
        score += 5.0;
    }

    // Opening hours
    if tags.contains_key("opening_hours") {
        score += 5.0;
    }

    // Star rating
    if let Some(stars_str) = tags.get("stars") {
        if let Ok(stars) = stars_str.parse::<f32>() {
            score += stars * 2.0;
        }
    }

    // Historic importance
    if let Some(h) = tags.get("historic") {
        if *h == "monument" || *h == "castle" {
            score += 10.0;
        }
    }

    // Sparse-tag penalty
    let tag_count = tags.len();
    if tag_count < 5 && !tags.contains_key("wikipedia") {
        score -= 10.0;
    }

    score.clamp(0.0, 100.0)
}

// ---------------------------------------------------------------------------
// Duration
// ---------------------------------------------------------------------------

/// Estimate visit duration in minutes from OSM tags and category.
pub fn estimate_duration(tags: &HashMap<&str, &str>, category: &PoiCategory) -> u32 {
    // Explicit duration tag takes precedence
    if let Some(d) = tags.get("duration") {
        if let Ok(mins) = d.parse::<u32>() {
            return mins;
        }
    }

    match category {
        PoiCategory::Museum => 90,
        PoiCategory::Castle => 120,
        PoiCategory::Monument => 30,
        PoiCategory::Viewpoint => 20,
        PoiCategory::Park => 60,
        PoiCategory::Church => 30,
        PoiCategory::Theatre => 180,
        PoiCategory::Library => 45,
        PoiCategory::Cultural => 60,
        PoiCategory::Waterfall => 30,
        PoiCategory::Market => 45,
        _ => 30,
    }
}

// ---------------------------------------------------------------------------
// Description
// ---------------------------------------------------------------------------

/// Build a human-readable description from OSM tags.
pub fn build_description(tags: &HashMap<&str, &str>) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(d) = tags.get("description") {
        parts.push((*d).to_string());
    }
    if tags.contains_key("heritage:unesco") {
        parts.push("UNESCO World Heritage Site".to_string());
    }
    if let Some(a) = tags.get("architect") {
        parts.push(format!("Architect: {a}"));
    }
    if let Some(a) = tags.get("artist") {
        parts.push(format!("Artist: {a}"));
    }
    if let Some(e) = tags.get("ele") {
        parts.push(format!("Elevation: {e}m"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(". "))
    }
}

#[cfg(test)]
mod tests;
