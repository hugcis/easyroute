use crate::models::Coordinates;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PoiCategory {
    // Original categories
    Monument,
    Viewpoint,
    Park,
    Museum,
    Restaurant,
    Cafe,
    Historic,
    Cultural,

    // Natural/Scenic
    Waterfront,
    Waterfall,
    NatureReserve,

    // Architectural
    Church,
    Castle,
    Bridge,
    Tower,

    // Urban Interest
    Plaza,
    Fountain,
    Market,
    Artwork,
    Lighthouse,

    // Activity
    Winery,
    Brewery,
    Theatre,
    Library,
}

impl fmt::Display for PoiCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            // Original
            PoiCategory::Monument => "monument",
            PoiCategory::Viewpoint => "viewpoint",
            PoiCategory::Park => "park",
            PoiCategory::Museum => "museum",
            PoiCategory::Restaurant => "restaurant",
            PoiCategory::Cafe => "cafe",
            PoiCategory::Historic => "historic",
            PoiCategory::Cultural => "cultural",
            // Natural/Scenic
            PoiCategory::Waterfront => "waterfront",
            PoiCategory::Waterfall => "waterfall",
            PoiCategory::NatureReserve => "nature_reserve",
            // Architectural
            PoiCategory::Church => "church",
            PoiCategory::Castle => "castle",
            PoiCategory::Bridge => "bridge",
            PoiCategory::Tower => "tower",
            // Urban Interest
            PoiCategory::Plaza => "plaza",
            PoiCategory::Fountain => "fountain",
            PoiCategory::Market => "market",
            PoiCategory::Artwork => "artwork",
            PoiCategory::Lighthouse => "lighthouse",
            // Activity
            PoiCategory::Winery => "winery",
            PoiCategory::Brewery => "brewery",
            PoiCategory::Theatre => "theatre",
            PoiCategory::Library => "library",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for PoiCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            // Original
            "monument" => Ok(PoiCategory::Monument),
            "viewpoint" => Ok(PoiCategory::Viewpoint),
            "park" => Ok(PoiCategory::Park),
            "museum" => Ok(PoiCategory::Museum),
            "restaurant" => Ok(PoiCategory::Restaurant),
            "cafe" => Ok(PoiCategory::Cafe),
            "historic" => Ok(PoiCategory::Historic),
            "cultural" => Ok(PoiCategory::Cultural),
            // Natural/Scenic
            "waterfront" => Ok(PoiCategory::Waterfront),
            "waterfall" => Ok(PoiCategory::Waterfall),
            "nature_reserve" => Ok(PoiCategory::NatureReserve),
            // Architectural
            "church" => Ok(PoiCategory::Church),
            "castle" => Ok(PoiCategory::Castle),
            "bridge" => Ok(PoiCategory::Bridge),
            "tower" => Ok(PoiCategory::Tower),
            // Urban Interest
            "plaza" => Ok(PoiCategory::Plaza),
            "fountain" => Ok(PoiCategory::Fountain),
            "market" => Ok(PoiCategory::Market),
            "artwork" => Ok(PoiCategory::Artwork),
            "lighthouse" => Ok(PoiCategory::Lighthouse),
            // Activity
            "winery" => Ok(PoiCategory::Winery),
            "brewery" => Ok(PoiCategory::Brewery),
            "theatre" => Ok(PoiCategory::Theatre),
            "library" => Ok(PoiCategory::Library),
            _ => Err(format!("Invalid POI category: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poi {
    pub id: Uuid,
    pub name: String,
    pub category: PoiCategory,
    pub coordinates: Coordinates,
    /// Popularity score from 0-100 (higher = more popular)
    pub popularity_score: f32,
    pub description: Option<String>,
    pub estimated_visit_duration_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub osm_id: Option<i64>,
}

impl Poi {
    pub fn new(
        name: String,
        category: PoiCategory,
        coordinates: Coordinates,
        popularity_score: f32,
    ) -> Self {
        Poi {
            id: Uuid::new_v4(),
            name,
            category,
            coordinates,
            popularity_score: popularity_score.clamp(0.0, 100.0),
            description: None,
            estimated_visit_duration_minutes: None,
            osm_id: None,
        }
    }

    /// Calculate a quality score for route selection
    /// If hidden_gems is true, prefer lower popularity
    pub fn quality_score(&self, hidden_gems: bool) -> f32 {
        if hidden_gems {
            100.0 - self.popularity_score
        } else {
            self.popularity_score
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poi_category_parsing() {
        assert_eq!("monument".parse::<PoiCategory>().unwrap(), PoiCategory::Monument);
        assert_eq!("VIEWPOINT".parse::<PoiCategory>().unwrap(), PoiCategory::Viewpoint);
        assert!("invalid".parse::<PoiCategory>().is_err());
    }

    #[test]
    fn test_quality_score() {
        let poi = Poi::new(
            "Eiffel Tower".to_string(),
            PoiCategory::Monument,
            Coordinates::new(48.8584, 2.2945).unwrap(),
            95.0,
        );

        assert_eq!(poi.quality_score(false), 95.0); // Popular
        assert_eq!(poi.quality_score(true), 5.0);   // Hidden gem (inverted)
    }
}
