use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedRoute {
    pub id: Uuid,
    pub start_lat: f64,
    pub start_lng: f64,
    pub target_distance_km: f64,
    pub transport_mode: String,
    pub actual_distance_km: f64,
    pub duration_minutes: i32,
    pub poi_names: Vec<String>,
    pub poi_count: i32,
    pub snapped_poi_count: i32,
    pub circularity: Option<f32>,
    pub convexity: Option<f32>,
    pub path_overlap_pct: Option<f32>,
    pub poi_density_per_km: Option<f32>,
    pub category_entropy: Option<f32>,
    pub landmark_coverage: Option<f32>,
    pub system_score: f32,
    pub poi_density_context: Option<String>,
    pub scoring_strategy: String,
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratings: Option<Vec<RouteRating>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRating {
    pub id: Uuid,
    pub route_id: Uuid,
    pub overall_rating: i16,
    pub shape_rating: Option<i16>,
    pub scenicness_rating: Option<i16>,
    pub variety_rating: Option<i16>,
    pub comment: Option<String>,
    pub rater_id: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RatingRequest {
    pub overall_rating: i16,
    #[serde(default)]
    pub shape_rating: Option<i16>,
    #[serde(default)]
    pub scenicness_rating: Option<i16>,
    #[serde(default)]
    pub variety_rating: Option<i16>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub rater_id: Option<String>,
}

impl RatingRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=5).contains(&self.overall_rating) {
            return Err("overall_rating must be between 1 and 5".to_string());
        }
        if let Some(r) = self.shape_rating {
            if !(1..=5).contains(&r) {
                return Err("shape_rating must be between 1 and 5".to_string());
            }
        }
        if let Some(r) = self.scenicness_rating {
            if !(1..=5).contains(&r) {
                return Err("scenicness_rating must be between 1 and 5".to_string());
            }
        }
        if let Some(r) = self.variety_rating {
            if !(1..=5).contains(&r) {
                return Err("variety_rating must be between 1 and 5".to_string());
            }
        }
        Ok(())
    }
}

/// Correlation data between a metric and human ratings
#[derive(Debug, Serialize)]
pub struct MetricCorrelation {
    pub metric_name: String,
    pub pearson_r: f64,
    pub sample_count: usize,
}

/// Overall evaluation statistics
#[derive(Debug, Serialize)]
pub struct EvaluationStats {
    pub total_routes: i64,
    pub total_ratings: i64,
    pub correlations: Vec<MetricCorrelation>,
}
