use crate::models::{Coordinates, Poi, PoiCategory};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    #[default]
    Walk,
    Bike,
}

impl TransportMode {
    /// Returns the Mapbox profile name for this transport mode
    pub fn mapbox_profile(&self) -> &str {
        match self {
            TransportMode::Walk => "walking",
            TransportMode::Bike => "cycling",
        }
    }
}

impl fmt::Display for TransportMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportMode::Walk => write!(f, "walk"),
            TransportMode::Bike => write!(f, "bike"),
        }
    }
}

impl FromStr for TransportMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "walk" | "walking" => Ok(TransportMode::Walk),
            "bike" | "cycling" | "bicycle" => Ok(TransportMode::Bike),
            _ => Err(format!("Invalid transport mode: '{}'", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poi_categories: Option<Vec<PoiCategory>>,
    #[serde(default)]
    pub hidden_gems: bool,
    #[serde(default = "default_max_alternatives")]
    pub max_alternatives: u32,
}

fn default_max_alternatives() -> u32 {
    3
}

impl Default for RoutePreferences {
    fn default() -> Self {
        RoutePreferences {
            poi_categories: None,
            hidden_gems: false,
            max_alternatives: default_max_alternatives(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: Uuid,
    pub distance_km: f64,
    pub estimated_duration_minutes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_gain_m: Option<f32>,
    /// GeoJSON LineString path
    pub path: Vec<Coordinates>,
    /// POIs used as waypoints in the route
    pub pois: Vec<RoutePoi>,
    /// POIs near the route path but not used as waypoints
    pub snapped_pois: Vec<SnappedPoi>,
    /// Route quality score (0-10)
    pub score: f32,
}

impl Route {
    pub fn new(
        distance_km: f64,
        estimated_duration_minutes: u32,
        path: Vec<Coordinates>,
        pois: Vec<RoutePoi>,
    ) -> Self {
        Route {
            id: Uuid::new_v4(),
            distance_km,
            estimated_duration_minutes,
            elevation_gain_m: None,
            path,
            pois,
            snapped_pois: Vec::new(),
            score: 0.0, // Will be calculated later
        }
    }

    pub fn with_snapped_pois(mut self, snapped_pois: Vec<SnappedPoi>) -> Self {
        self.snapped_pois = snapped_pois;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePoi {
    #[serde(flatten)]
    pub poi: Poi,
    pub order_in_route: u32,
    pub distance_from_start_km: f64,
}

impl RoutePoi {
    pub fn new(poi: Poi, order_in_route: u32, distance_from_start_km: f64) -> Self {
        RoutePoi {
            poi,
            order_in_route,
            distance_from_start_km,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnappedPoi {
    #[serde(flatten)]
    pub poi: Poi,
    /// Distance along route from start (km)
    pub distance_from_start_km: f64,
    /// Perpendicular distance from route path (meters)
    pub distance_from_path_m: f32,
}

impl SnappedPoi {
    pub fn new(poi: Poi, distance_from_start_km: f64, distance_from_path_m: f32) -> Self {
        SnappedPoi {
            poi,
            distance_from_start_km,
            distance_from_path_m,
        }
    }
}

// Request/Response types for API endpoints

#[derive(Debug, Deserialize)]
pub struct LoopRouteRequest {
    pub start_point: Coordinates,
    pub distance_km: f64,
    #[serde(default = "default_distance_tolerance")]
    pub distance_tolerance: f64,
    pub mode: TransportMode,
    #[serde(default)]
    pub preferences: RoutePreferences,
}

fn default_distance_tolerance() -> f64 {
    0.5 // ±0.5 km
}

impl LoopRouteRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.5..=50.0).contains(&self.distance_km) {
            return Err("distance_km must be between 0.5 and 50".to_string());
        }
        if self.distance_tolerance < 0.0 || self.distance_tolerance > self.distance_km {
            return Err(
                "distance_tolerance must be positive and less than distance_km".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct PointToPointRouteRequest {
    pub start_point: Coordinates,
    pub end_point: Coordinates,
    pub mode: TransportMode,
    #[serde(default)]
    pub preferences: RoutePreferences,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub routes: Vec<Route>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_route_request_validation() {
        let mut req = LoopRouteRequest {
            start_point: Coordinates::new(48.8566, 2.3522).unwrap(),
            distance_km: 5.0,
            distance_tolerance: 0.5,
            mode: TransportMode::Walk,
            preferences: RoutePreferences::default(),
        };

        assert!(req.validate().is_ok());

        req.distance_km = 0.1; // Too short
        assert!(req.validate().is_err());

        req.distance_km = 100.0; // Too long
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_transport_mode_mapbox_profile() {
        assert_eq!(TransportMode::Walk.mapbox_profile(), "walking");
        assert_eq!(TransportMode::Bike.mapbox_profile(), "cycling");
    }

    #[test]
    fn test_transport_mode_display() {
        assert_eq!(TransportMode::Walk.to_string(), "walk");
        assert_eq!(TransportMode::Bike.to_string(), "bike");
    }

    #[test]
    fn test_transport_mode_from_str() {
        assert_eq!(
            "walk".parse::<TransportMode>().unwrap(),
            TransportMode::Walk
        );
        assert_eq!(
            "WALK".parse::<TransportMode>().unwrap(),
            TransportMode::Walk
        );
        assert_eq!(
            "walking".parse::<TransportMode>().unwrap(),
            TransportMode::Walk
        );
        assert_eq!(
            "bike".parse::<TransportMode>().unwrap(),
            TransportMode::Bike
        );
        assert_eq!(
            "cycling".parse::<TransportMode>().unwrap(),
            TransportMode::Bike
        );
        assert_eq!(
            "bicycle".parse::<TransportMode>().unwrap(),
            TransportMode::Bike
        );
        assert!("invalid".parse::<TransportMode>().is_err());
    }

    #[test]
    fn test_transport_mode_default() {
        assert_eq!(TransportMode::default(), TransportMode::Walk);
    }
}
