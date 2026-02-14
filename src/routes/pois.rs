use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, PoiCategory};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Query parameters for POI search
#[derive(Debug, Deserialize)]
pub struct PoiQueryParams {
    /// Latitude of the center point
    pub lat: f64,
    /// Longitude of the center point
    pub lng: f64,
    /// Search radius in kilometers (default: 1.0, max: 25.0)
    #[serde(default = "default_radius")]
    pub radius_km: f64,
    /// Comma-separated list of categories to filter by
    #[serde(default)]
    pub categories: Option<String>,
    /// Maximum number of results (default: 50, max: 200)
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_radius() -> f64 {
    1.0
}

fn default_limit() -> u32 {
    50
}

impl PoiQueryParams {
    pub fn validate(&self) -> Result<()> {
        // Validate coordinates
        if !(-90.0..=90.0).contains(&self.lat) {
            return Err(AppError::InvalidRequest(
                "lat must be between -90 and 90".to_string(),
            ));
        }
        if !(-180.0..=180.0).contains(&self.lng) {
            return Err(AppError::InvalidRequest(
                "lng must be between -180 and 180".to_string(),
            ));
        }

        // Validate radius
        if !(0.1..=25.0).contains(&self.radius_km) {
            return Err(AppError::InvalidRequest(
                "radius_km must be between 0.1 and 25".to_string(),
            ));
        }

        // Validate limit
        if self.limit == 0 || self.limit > 200 {
            return Err(AppError::InvalidRequest(
                "limit must be between 1 and 200".to_string(),
            ));
        }

        Ok(())
    }

    /// Parse categories from comma-separated string
    pub fn parse_categories(&self) -> Result<Option<Vec<PoiCategory>>> {
        match &self.categories {
            None => Ok(None),
            Some(cats_str) if cats_str.is_empty() => Ok(None),
            Some(cats_str) => {
                let categories: std::result::Result<Vec<PoiCategory>, _> = cats_str
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse::<PoiCategory>())
                    .collect();

                match categories {
                    Ok(cats) if cats.is_empty() => Ok(None),
                    Ok(cats) => Ok(Some(cats)),
                    Err(e) => Err(AppError::InvalidRequest(e)),
                }
            }
        }
    }
}

/// Response for POI queries
#[derive(Debug, Serialize)]
pub struct PoiResponse {
    /// List of POIs found
    pub pois: Vec<Poi>,
    /// Total count of results
    pub count: usize,
    /// Search parameters used
    pub query: PoiQueryInfo,
}

#[derive(Debug, Serialize)]
pub struct PoiQueryInfo {
    pub center: Coordinates,
    pub radius_km: f64,
    pub categories: Option<Vec<String>>,
}

/// GET /pois - Query POIs within a radius
pub async fn query_pois(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PoiQueryParams>,
) -> Result<Json<PoiResponse>> {
    // Validate parameters
    params.validate()?;

    // Parse categories
    let categories = params.parse_categories()?;

    // Create coordinates
    let center = Coordinates::new(params.lat, params.lng)
        .map_err(|e| AppError::InvalidRequest(e.to_string()))?;

    tracing::info!(
        "POI query: center={:?}, radius={}km, categories={:?}, limit={}",
        center,
        params.radius_km,
        categories,
        params.limit
    );

    // Query database
    let radius_meters = params.radius_km * 1000.0;
    let pois = state
        .poi_repo
        .find_within_radius(
            &center,
            radius_meters,
            categories.as_deref(),
            params.limit as i64,
        )
        .await?;

    let count = pois.len();

    tracing::info!("POI query returned {} results", count);

    Ok(Json(PoiResponse {
        pois,
        count,
        query: PoiQueryInfo {
            center,
            radius_km: params.radius_km,
            categories: categories.map(|cats| cats.iter().map(|c| c.to_string()).collect()),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poi_query_params_validation() {
        // Valid params
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: None,
            limit: 50,
        };
        assert!(params.validate().is_ok());

        // Invalid lat
        let params = PoiQueryParams {
            lat: 100.0,
            lng: 2.3522,
            radius_km: 5.0,
            categories: None,
            limit: 50,
        };
        assert!(params.validate().is_err());

        // Invalid lng
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 200.0,
            radius_km: 5.0,
            categories: None,
            limit: 50,
        };
        assert!(params.validate().is_err());

        // Invalid radius
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 50.0,
            categories: None,
            limit: 50,
        };
        assert!(params.validate().is_err());

        // Invalid limit
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: None,
            limit: 500,
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_parse_categories() {
        // No categories
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: None,
            limit: 50,
        };
        assert!(params.parse_categories().unwrap().is_none());

        // Empty string
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: Some("".to_string()),
            limit: 50,
        };
        assert!(params.parse_categories().unwrap().is_none());

        // Single category
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: Some("monument".to_string()),
            limit: 50,
        };
        let cats = params.parse_categories().unwrap().unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0], PoiCategory::Monument);

        // Multiple categories
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: Some("monument, park, museum".to_string()),
            limit: 50,
        };
        let cats = params.parse_categories().unwrap().unwrap();
        assert_eq!(cats.len(), 3);

        // Invalid category
        let params = PoiQueryParams {
            lat: 48.8566,
            lng: 2.3522,
            radius_km: 5.0,
            categories: Some("invalid_category".to_string()),
            limit: 50,
        };
        assert!(params.parse_categories().is_err());
    }
}
