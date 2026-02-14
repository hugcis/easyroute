use crate::error::{AppError, Result};
use crate::models::{Coordinates, TransportMode};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const MAPBOX_DIRECTIONS_BASE_URL: &str = "https://api.mapbox.com/directions/v5/mapbox";

/// How the client authenticates with the directions API.
#[derive(Clone, Debug)]
pub enum AuthMode {
    /// Current default: send `access_token` query param (direct Mapbox).
    DirectToken,
    /// Proxy mode: send `Authorization: Bearer` header.
    BearerHeader,
}

#[derive(Clone)]
pub struct MapboxClient {
    client: Client,
    api_key: String,
    base_url: String,
    auth_mode: AuthMode,
}

impl MapboxClient {
    pub fn new(api_key: String) -> Self {
        MapboxClient {
            client: Client::new(),
            api_key,
            base_url: MAPBOX_DIRECTIONS_BASE_URL.to_string(),
            auth_mode: AuthMode::DirectToken,
        }
    }

    pub fn with_config(api_key: String, base_url: String, auth_mode: AuthMode) -> Self {
        MapboxClient {
            client: Client::new(),
            api_key,
            base_url,
            auth_mode,
        }
    }

    /// Get directions between waypoints
    /// Returns the route with full geometry, distance, and duration
    pub async fn get_directions(
        &self,
        waypoints: &[Coordinates],
        mode: &TransportMode,
    ) -> Result<DirectionsResponse> {
        if waypoints.len() < 2 {
            return Err(AppError::InvalidRequest(
                "At least 2 waypoints required".to_string(),
            ));
        }

        // Mapbox allows up to 25 waypoints
        if waypoints.len() > 25 {
            return Err(AppError::InvalidRequest(
                "Maximum 25 waypoints allowed".to_string(),
            ));
        }

        // Format coordinates as "lng,lat;lng,lat;..."
        let coordinates_str = waypoints
            .iter()
            .map(|c| format!("{},{}", c.lng, c.lat))
            .collect::<Vec<_>>()
            .join(";");

        let url = format!(
            "{}/{}/{}",
            self.base_url,
            mode.mapbox_profile(),
            coordinates_str
        );

        tracing::debug!(
            waypoints = waypoints.len(),
            mode = %mode.mapbox_profile(),
            "Mapbox API request: {} waypoints, profile {}",
            waypoints.len(), mode.mapbox_profile()
        );

        let mut request = self.client.get(&url).query(&[
            ("geometries", "geojson"),
            ("overview", "full"),
            ("steps", "false"),
        ]);

        match self.auth_mode {
            AuthMode::DirectToken => {
                request = request.query(&[("access_token", &self.api_key)]);
            }
            AuthMode::BearerHeader => {
                request = request.bearer_auth(&self.api_key);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::MapboxApi(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::warn!(
                status = %status,
                waypoints = waypoints.len(),
                "Mapbox API HTTP error {}: {}",
                status, error_text
            );
            return Err(AppError::MapboxApi(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let directions: MapboxDirectionsApiResponse = response
            .json()
            .await
            .map_err(|e| AppError::MapboxApi(format!("Failed to parse response: {}", e)))?;

        if directions.routes.is_empty() {
            tracing::warn!(
                waypoints = waypoints.len(),
                mode = %mode.mapbox_profile(),
                "Mapbox returned 0 routes for {} waypoints ({})",
                waypoints.len(), mode.mapbox_profile()
            );
            return Err(AppError::MapboxApi("No routes found".to_string()));
        }

        // Convert first route to our format
        let route = &directions.routes[0];
        tracing::debug!(
            distance_km = %format!("{:.2}", route.distance / 1000.0),
            duration_min = %format!("{:.0}", route.duration / 60.0),
            path_points = route.geometry.coordinates.len(),
            "Mapbox response: {:.2}km, {:.0}min, {} path points",
            route.distance / 1000.0, route.duration / 60.0, route.geometry.coordinates.len()
        );
        Ok(DirectionsResponse {
            distance_meters: route.distance,
            duration_seconds: route.duration,
            geometry: route.geometry.coordinates.clone(),
        })
    }
}

// Mapbox API response types

#[derive(Debug, Deserialize)]
struct MapboxDirectionsApiResponse {
    routes: Vec<MapboxRoute>,
    #[allow(dead_code)]
    code: String,
}

#[derive(Debug, Deserialize)]
struct MapboxRoute {
    distance: f64, // meters
    duration: f64, // seconds
    geometry: MapboxGeometry,
}

#[derive(Debug, Deserialize)]
struct MapboxGeometry {
    coordinates: Vec<[f64; 2]>, // [lng, lat] pairs
    #[allow(dead_code)]
    #[serde(rename = "type")]
    geometry_type: String,
}

// Our simplified response type

#[derive(Debug, Clone, Serialize)]
pub struct DirectionsResponse {
    pub distance_meters: f64,
    pub duration_seconds: f64,
    /// GeoJSON coordinates as [lng, lat] pairs
    pub geometry: Vec<[f64; 2]>,
}

impl DirectionsResponse {
    pub fn distance_km(&self) -> f64 {
        self.distance_meters / 1000.0
    }

    pub fn duration_minutes(&self) -> u32 {
        (self.duration_seconds / 60.0).round() as u32
    }

    /// Convert GeoJSON coordinates to our Coordinates type
    pub fn to_coordinates(&self) -> Vec<Coordinates> {
        self.geometry
            .iter()
            .filter_map(|coord| Coordinates::new(coord[1], coord[0]).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults_to_direct_token() {
        let client = MapboxClient::new("pk.test123".to_string());
        assert_eq!(client.base_url, MAPBOX_DIRECTIONS_BASE_URL);
        assert!(matches!(client.auth_mode, AuthMode::DirectToken));
    }

    #[test]
    fn test_with_config_bearer_mode() {
        let client = MapboxClient::with_config(
            "my-key".to_string(),
            "http://localhost:4000/v1/directions".to_string(),
            AuthMode::BearerHeader,
        );
        assert_eq!(client.base_url, "http://localhost:4000/v1/directions");
        assert!(matches!(client.auth_mode, AuthMode::BearerHeader));
    }

    #[test]
    fn test_directions_response_conversions() {
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
}
