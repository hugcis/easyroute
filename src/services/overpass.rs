use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, PoiCategory};
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Primary Overpass API endpoints with automatic fallback
const OVERPASS_ENDPOINTS: &[&str] = &[
    "https://overpass-api.de/api/interpreter", // Official main endpoint
    "https://overpass.private.coffee/api/interpreter", // Community mirror
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter", // Mail.ru mirror
];

#[derive(Clone)]
pub struct OverpassClient {
    client: Client,
    endpoints: Vec<String>,
    current_endpoint_idx: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl OverpassClient {
    pub fn new() -> Self {
        let endpoints: Vec<String> = OVERPASS_ENDPOINTS.iter().map(|s| s.to_string()).collect();

        OverpassClient {
            client: Client::new(),
            endpoints,
            current_endpoint_idx: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Get the next endpoint to try (round-robin)
    fn get_next_endpoint(&self) -> String {
        let idx = self
            .current_endpoint_idx
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.endpoints[idx % self.endpoints.len()].clone()
    }

    /// Query POIs within a radius from a center point
    /// Returns POIs from OpenStreetMap via Overpass API
    /// Uses exponential backoff retry for rate limiting and timeouts
    pub async fn query_pois(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: &[PoiCategory],
    ) -> Result<Vec<Poi>> {
        self.query_pois_internal(center, radius_meters, categories, false)
            .await
    }

    /// Query POIs using batched parallel requests for better resilience
    /// Splits categories into batches and executes in parallel
    /// Returns merged results from all successful batches
    pub async fn query_pois_batched(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: &[PoiCategory],
    ) -> Result<Vec<Poi>> {
        self.query_pois_internal(center, radius_meters, categories, true)
            .await
    }

    /// Internal query method with optional batching
    async fn query_pois_internal(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: &[PoiCategory],
        use_batching: bool,
    ) -> Result<Vec<Poi>> {
        if use_batching && categories.len() > 3 {
            tracing::info!(
                "Using batched parallel queries for {} categories",
                categories.len()
            );
            return self
                .query_pois_batched_parallel(center, radius_meters, categories)
                .await;
        }

        // Single union query (standard approach)
        let query = self.build_query(center, radius_meters, categories);

        tracing::debug!("Overpass single query: {}", query);

        // Use standard retry logic with more retries for single query
        // (batched queries have their own retry logic per batch)
        self.execute_query_with_retry_extended(query).await
    }

    /// Extended retry for single queries (3 attempts vs 2 for batches)
    async fn execute_query_with_retry_extended(&self, query: String) -> Result<Vec<Poi>> {
        let max_retries = 2; // Total of 3 attempts for single queries
        let mut retry_count = 0;

        loop {
            let endpoint = self.get_next_endpoint();

            let response_result = self
                .client
                .post(&endpoint)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(format!("data={}", urlencoding::encode(&query)))
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await;

            let response = match response_result {
                Ok(resp) => resp,
                Err(e) => {
                    let is_timeout = e.is_timeout();
                    let error_msg = if is_timeout {
                        "Request timed out".to_string()
                    } else {
                        format!("Request failed: {}", e)
                    };

                    if retry_count < max_retries {
                        retry_count += 1;
                        let backoff_ms = 1000 * (2_u64.pow(retry_count as u32));

                        tracing::warn!(
                            "Overpass API {} ({}), retrying in {}ms (attempt {}/{})",
                            error_msg,
                            endpoint,
                            backoff_ms,
                            retry_count + 1,
                            max_retries + 1
                        );

                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        continue;
                    } else {
                        return Err(AppError::OverpassApi(format!(
                            "{} after {} retries",
                            error_msg,
                            max_retries + 1
                        )));
                    }
                }
            };

            let status = response.status();

            if status.is_success() {
                let api_response: OverpassResponse = response.json().await.map_err(|e| {
                    AppError::OverpassApi(format!("Failed to parse response: {}", e))
                })?;

                return Ok(self.convert_elements_to_pois(api_response.elements));
            }

            let is_retryable = status == 429 || status == 504;

            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if is_retryable && retry_count < max_retries {
                retry_count += 1;
                let backoff_ms = 1000 * (2_u64.pow(retry_count as u32));

                tracing::warn!(
                    "Overpass API returned HTTP {}, retrying in {}ms (attempt {}/{})",
                    status,
                    backoff_ms,
                    retry_count + 1,
                    max_retries + 1
                );

                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                continue;
            }

            return Err(AppError::OverpassApi(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }
    }

    /// Execute batched parallel queries
    async fn query_pois_batched_parallel(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: &[PoiCategory],
    ) -> Result<Vec<Poi>> {
        // Split categories into balanced batches
        let batches = self.create_category_batches(categories);

        tracing::info!("Executing {} parallel batches", batches.len());

        // Execute all batches in parallel
        let batch_futures: Vec<_> = batches
            .into_iter()
            .enumerate()
            .map(|(idx, batch_categories)| {
                let center = *center;
                async move {
                    let query = self.build_query(&center, radius_meters, &batch_categories);

                    tracing::debug!("Batch {} query: {}", idx + 1, query);

                    // Each batch has its own retry logic
                    let result = self.execute_query_with_retry(query).await;

                    match &result {
                        Ok(pois) => {
                            tracing::info!("Batch {} returned {} POIs", idx + 1, pois.len())
                        }
                        Err(e) => tracing::warn!("Batch {} failed: {}", idx + 1, e),
                    }

                    (idx, result)
                }
            })
            .collect();

        // Wait for all batches to complete
        let results = futures::future::join_all(batch_futures).await;

        // Collect successful results and merge
        let mut all_pois = Vec::new();
        let mut successful_batches = 0;
        let mut failed_batches = 0;

        for (idx, result) in results {
            match result {
                Ok(mut pois) => {
                    successful_batches += 1;
                    all_pois.append(&mut pois);
                }
                Err(e) => {
                    failed_batches += 1;
                    tracing::warn!("Batch {} error: {}", idx + 1, e);
                }
            }
        }

        // Deduplicate POIs by OSM ID
        let mut seen_ids = HashSet::new();
        let unique_pois: Vec<Poi> = all_pois
            .into_iter()
            .filter(|poi| {
                if let Some(osm_id) = poi.osm_id {
                    seen_ids.insert(osm_id)
                } else {
                    true // Keep POIs without OSM IDs
                }
            })
            .collect();

        tracing::info!(
            "Batched query complete: {}/{} batches successful, {} unique POIs",
            successful_batches,
            successful_batches + failed_batches,
            unique_pois.len()
        );

        if successful_batches == 0 {
            return Err(AppError::OverpassApi("All batches failed".to_string()));
        }

        Ok(unique_pois)
    }

    /// Create balanced category batches for parallel execution
    fn create_category_batches(&self, categories: &[PoiCategory]) -> Vec<Vec<PoiCategory>> {
        // Strategy: Group by semantic similarity to balance query complexity

        let mut high_value = Vec::new(); // Tourist attractions, monuments
        let mut nature_outdoor = Vec::new(); // Parks, waterfronts, nature
        let mut urban_cultural = Vec::new(); // Museums, plazas, urban POIs

        for category in categories {
            match category {
                // High-value tourist attractions
                PoiCategory::Monument
                | PoiCategory::Viewpoint
                | PoiCategory::Castle
                | PoiCategory::Historic => {
                    high_value.push(category.clone());
                }

                // Nature and outdoor
                PoiCategory::Park
                | PoiCategory::Waterfront
                | PoiCategory::Waterfall
                | PoiCategory::NatureReserve => {
                    nature_outdoor.push(category.clone());
                }

                // Urban and cultural (everything else)
                _ => {
                    urban_cultural.push(category.clone());
                }
            }
        }

        // Build batches, only including non-empty ones
        let mut batches = Vec::new();

        if !high_value.is_empty() {
            batches.push(high_value);
        }
        if !nature_outdoor.is_empty() {
            batches.push(nature_outdoor);
        }
        if !urban_cultural.is_empty() {
            batches.push(urban_cultural);
        }

        // If we ended up with just one batch, split it
        if batches.len() == 1 && batches[0].len() > 4 {
            if let Some(single) = batches.pop() {
                let mid = single.len() / 2;
                batches.push(single[..mid].to_vec());
                batches.push(single[mid..].to_vec());
            }
        }

        tracing::debug!(
            "Created {} batches: {}",
            batches.len(),
            batches
                .iter()
                .map(|b| format!("{} categories", b.len()))
                .collect::<Vec<_>>()
                .join(", ")
        );

        batches
    }

    /// Execute a query with retry logic (extracted for reuse in batches)
    async fn execute_query_with_retry(&self, query: String) -> Result<Vec<Poi>> {
        // Retry configuration: 2 attempts for batched queries (faster failover)
        let max_retries = 1; // Total of 2 attempts for individual batches
        let mut retry_count = 0;

        loop {
            let endpoint = self.get_next_endpoint();

            // Send request and handle both timeout and HTTP errors
            let response_result = self
                .client
                .post(&endpoint)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(format!("data={}", urlencoding::encode(&query)))
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await;

            // Handle request-level errors (timeouts, connection issues)
            let response = match response_result {
                Ok(resp) => resp,
                Err(e) => {
                    let is_timeout = e.is_timeout();
                    let error_msg = if is_timeout {
                        "Request timed out".to_string()
                    } else {
                        format!("Request failed: {}", e)
                    };

                    // Retry on timeout or connection errors
                    if retry_count < max_retries {
                        retry_count += 1;
                        let backoff_ms = 1000 * (2_u64.pow(retry_count as u32)); // 2s, 4s

                        tracing::warn!(
                            "Batch query {} ({}), retrying in {}ms (attempt {}/{})",
                            error_msg,
                            endpoint,
                            backoff_ms,
                            retry_count + 1,
                            max_retries + 1
                        );

                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        continue;
                    } else {
                        return Err(AppError::OverpassApi(format!(
                            "{} after {} retries",
                            error_msg,
                            max_retries + 1
                        )));
                    }
                }
            };

            let status = response.status();

            // Handle success
            if status.is_success() {
                let api_response: OverpassResponse = response.json().await.map_err(|e| {
                    AppError::OverpassApi(format!("Failed to parse response: {}", e))
                })?;

                return Ok(self.convert_elements_to_pois(api_response.elements));
            }

            // Handle retryable HTTP errors (429 Too Many Requests, 504 Gateway Timeout)
            let is_retryable = status == 429 || status == 504;

            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if is_retryable && retry_count < max_retries {
                retry_count += 1;
                let backoff_ms = 1000 * (2_u64.pow(retry_count as u32));

                tracing::warn!(
                    "Batch query returned HTTP {}, retrying in {}ms (attempt {}/{})",
                    status,
                    backoff_ms,
                    retry_count + 1,
                    max_retries + 1
                );

                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                continue;
            }

            // Non-retryable error or max retries exceeded
            return Err(AppError::OverpassApi(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }
    }

    fn build_query(
        &self,
        center: &Coordinates,
        radius_meters: f64,
        categories: &[PoiCategory],
    ) -> String {
        let mut query_parts = vec!["[out:json];(".to_string()];

        for category in categories {
            let osm_tags = category_to_osm_tags(category);
            for (key, value) in osm_tags {
                let tag_query = if value == "*" {
                    format!(
                        r#"nwr["{}"]["name"](around:{},{},{});"#,
                        key, radius_meters, center.lat, center.lng
                    )
                } else {
                    format!(
                        r#"nwr["{}"="{}"]["name"](around:{},{},{});"#,
                        key, value, radius_meters, center.lat, center.lng
                    )
                };
                query_parts.push(tag_query);
            }
        }

        query_parts.push(");out geom;".to_string());
        query_parts.join("\n")
    }

    /// Calculate centroid from geometry points (for ways/relations)
    fn calculate_centroid(&self, geometry: &[GeometryPoint]) -> Option<(f64, f64)> {
        if geometry.is_empty() {
            return None;
        }

        let mut lat_sum = 0.0;
        let mut lon_sum = 0.0;
        let count = geometry.len() as f64;

        for point in geometry {
            lat_sum += point.lat;
            lon_sum += point.lon;
        }

        Some((lat_sum / count, lon_sum / count))
    }

    fn convert_elements_to_pois(&self, elements: Vec<OverpassElement>) -> Vec<Poi> {
        elements
            .into_iter()
            .filter_map(|elem| {
                // Extract coordinates:
                // 1. For nodes: use direct lat/lon
                // 2. For ways/relations: compute centroid from geometry
                let (lat, lon) = if let (Some(lat), Some(lon)) = (elem.lat, elem.lon) {
                    // Node with direct coordinates
                    (lat, lon)
                } else if let Some(ref geometry) = elem.geometry {
                    // Way/relation with geometry - compute centroid
                    self.calculate_centroid(geometry)?
                } else if let Some(center) = elem.center {
                    // Fallback to center if provided (shouldn't happen with out geom)
                    (center.lat, center.lon)
                } else {
                    return None; // Skip elements without any coordinates
                };

                let coords = Coordinates::new(lat, lon).ok()?;
                let name = elem.tags.get("name")?.clone();
                let category = self.infer_category(&elem.tags)?;

                let popularity_score = self.calculate_popularity(&elem.tags);

                let description = elem
                    .tags
                    .get("description")
                    .or_else(|| elem.tags.get("wikipedia"))
                    .cloned();

                Some(Poi {
                    id: uuid::Uuid::new_v4(),
                    name,
                    category,
                    coordinates: coords,
                    popularity_score,
                    description,
                    estimated_visit_duration_minutes: None,
                    osm_id: Some(elem.id),
                })
            })
            .collect()
    }

    fn infer_category(&self, tags: &HashMap<String, String>) -> Option<PoiCategory> {
        // Check tourism tags
        if let Some(tourism_type) = tags.get("tourism") {
            return match tourism_type.as_str() {
                "monument" | "memorial" => Some(PoiCategory::Monument),
                "viewpoint" => Some(PoiCategory::Viewpoint),
                "museum" | "gallery" => Some(PoiCategory::Museum),
                "attraction" => Some(PoiCategory::Cultural),
                "artwork" => Some(PoiCategory::Artwork),
                "wine_cellar" => Some(PoiCategory::Winery),
                _ => None,
            };
        }

        // Check historic tags - prioritize specific types
        if let Some(historic_type) = tags.get("historic") {
            return match historic_type.as_str() {
                "castle" | "fort" | "fortress" => Some(PoiCategory::Castle),
                _ => Some(PoiCategory::Historic), // Catch-all for other historic sites
            };
        }

        // Check amenity tags
        if let Some(amenity_type) = tags.get("amenity") {
            return match amenity_type.as_str() {
                "restaurant" => Some(PoiCategory::Restaurant),
                "cafe" => Some(PoiCategory::Cafe),
                "place_of_worship" => Some(PoiCategory::Church),
                "fountain" => Some(PoiCategory::Fountain),
                "marketplace" => Some(PoiCategory::Market),
                "arts_centre" => Some(PoiCategory::Cultural),
                "theatre" | "cinema" => Some(PoiCategory::Theatre),
                "library" => Some(PoiCategory::Library),
                _ => None,
            };
        }

        // Check leisure tags
        if let Some(leisure_type) = tags.get("leisure") {
            return match leisure_type.as_str() {
                "park" | "garden" => Some(PoiCategory::Park),
                "nature_reserve" => Some(PoiCategory::NatureReserve),
                "beach_resort" => Some(PoiCategory::Waterfront),
                "plaza" => Some(PoiCategory::Plaza),
                _ => None,
            };
        }

        // Check natural tags
        if let Some(natural_type) = tags.get("natural") {
            return match natural_type.as_str() {
                "beach" | "coastline" => Some(PoiCategory::Waterfront),
                _ => None,
            };
        }

        // Check waterway tags
        if let Some(waterway_type) = tags.get("waterway") {
            return match waterway_type.as_str() {
                "waterfall" => Some(PoiCategory::Waterfall),
                _ => None,
            };
        }

        // Check man_made tags
        if let Some(man_made_type) = tags.get("man_made") {
            return match man_made_type.as_str() {
                "tower" => Some(PoiCategory::Tower),
                "lighthouse" => Some(PoiCategory::Lighthouse),
                "bridge" => Some(PoiCategory::Bridge),
                _ => None,
            };
        }

        // Check craft tags
        if let Some(craft_type) = tags.get("craft") {
            return match craft_type.as_str() {
                "winery" => Some(PoiCategory::Winery),
                "brewery" => Some(PoiCategory::Brewery),
                _ => None,
            };
        }

        // Check building tags (for churches/cathedrals)
        if let Some(building_type) = tags.get("building") {
            return match building_type.as_str() {
                "church" | "cathedral" => Some(PoiCategory::Church),
                _ => None,
            };
        }

        // Check place tags (for plazas/squares)
        if let Some(place_type) = tags.get("place") {
            return match place_type.as_str() {
                "square" => Some(PoiCategory::Plaza),
                _ => None,
            };
        }

        // Check shop tags
        if let Some(shop_type) = tags.get("shop") {
            return match shop_type.as_str() {
                "wine" => Some(PoiCategory::Winery),
                "marketplace" => Some(PoiCategory::Market),
                _ => None,
            };
        }

        // Check boundary tags
        if let Some(boundary_type) = tags.get("boundary") {
            return match boundary_type.as_str() {
                "protected_area" => Some(PoiCategory::NatureReserve),
                _ => None,
            };
        }

        None
    }

    fn calculate_popularity(&self, tags: &HashMap<String, String>) -> f32 {
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
}

impl Default for OverpassClient {
    fn default() -> Self {
        Self::new()
    }
}

// Overpass API response types

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    id: i64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    center: Option<OverpassCenter>,
    #[serde(default)]
    geometry: Option<Vec<GeometryPoint>>,
    tags: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct OverpassCenter {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct GeometryPoint {
    lat: f64,
    lon: f64,
}

// Map POI categories to OpenStreetMap tags
fn category_to_osm_tags(category: &PoiCategory) -> Vec<(&str, &str)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_to_osm_tags() {
        // Test original categories
        let tags = category_to_osm_tags(&PoiCategory::Monument);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&("tourism", "monument")));

        // Test new architectural categories
        let tags = category_to_osm_tags(&PoiCategory::Castle);
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&("historic", "castle")));

        let tags = category_to_osm_tags(&PoiCategory::Church);
        assert!(tags.contains(&("amenity", "place_of_worship")));

        // Test natural/scenic categories
        let tags = category_to_osm_tags(&PoiCategory::Waterfall);
        assert!(tags.contains(&("waterway", "waterfall")));

        let tags = category_to_osm_tags(&PoiCategory::Waterfront);
        assert!(tags.contains(&("natural", "beach")));

        // Test activity categories
        let tags = category_to_osm_tags(&PoiCategory::Winery);
        assert!(tags.contains(&("craft", "winery")));

        let tags = category_to_osm_tags(&PoiCategory::Brewery);
        assert!(tags.contains(&("craft", "brewery")));
    }

    #[test]
    fn test_build_query() {
        let client = OverpassClient::new();
        let center = Coordinates::new(48.8566, 2.3522).unwrap();
        let query = client.build_query(&center, 1000.0, &[PoiCategory::Monument]);

        assert!(query.contains("[out:json]"));
        assert!(query.contains("around:1000"));
        assert!(query.contains("48.8566"));
        assert!(query.contains("2.3522"));
        // Verify correct Overpass syntax for tag values
        assert!(
            query.contains(r#"["tourism"="monument"]"#)
                || query.contains(r#"["tourism"="memorial"]"#)
        );
    }
}
