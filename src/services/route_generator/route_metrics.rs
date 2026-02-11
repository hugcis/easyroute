use crate::models::{Coordinates, PoiCategory, Route};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Overlap detection distance threshold (meters)
const DEFAULT_OVERLAP_THRESHOLD_M: f64 = 25.0;

/// Number of adjacent segments to skip when checking for overlap
const OVERLAP_SKIP_NEIGHBORS: usize = 3;

/// Grid cell size for spatial bucketing (degrees, ~50m at mid-latitudes)
const GRID_CELL_SIZE: f64 = 0.00045;

/// Context for POI density in the search area
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PoiDensityContext {
    Dense,
    Moderate,
    Sparse,
    Geometric,
}

impl PoiDensityContext {
    pub fn from_poi_count(count: usize) -> Self {
        match count {
            0..=1 => PoiDensityContext::Geometric,
            2..=7 => PoiDensityContext::Sparse,
            8..=19 => PoiDensityContext::Moderate,
            _ => PoiDensityContext::Dense,
        }
    }
}

impl std::fmt::Display for PoiDensityContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoiDensityContext::Dense => write!(f, "dense"),
            PoiDensityContext::Moderate => write!(f, "moderate"),
            PoiDensityContext::Sparse => write!(f, "sparse"),
            PoiDensityContext::Geometric => write!(f, "geometric"),
        }
    }
}

/// Route quality metrics computed from path geometry and POI data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetrics {
    /// Isoperimetric ratio: 4*pi*area / perimeter^2 (1.0 = perfect circle)
    pub circularity: f32,
    /// Path polygon area / convex hull area (1.0 = no indentations)
    pub convexity: f32,
    /// Percentage of route length that reuses streets (0.0 = no overlap)
    pub path_overlap_pct: f32,
    /// (waypoint + snapped POIs) / distance_km
    pub poi_density_per_km: f32,
    /// Normalized Shannon entropy of POI categories (0-1)
    pub category_entropy: f32,
    /// Average popularity_score of waypoint POIs / 100
    pub landmark_coverage: f32,
    /// POI density context for the route area
    pub poi_density_context: PoiDensityContext,
}

impl RouteMetrics {
    /// Compute all metrics from a Route object
    pub fn compute(route: &Route, area_poi_count: usize) -> Self {
        let path = &route.path;
        let total_poi_count = route.pois.len() + route.snapped_pois.len();

        RouteMetrics {
            circularity: compute_circularity(path),
            convexity: compute_convexity(path),
            path_overlap_pct: compute_path_overlap(path, DEFAULT_OVERLAP_THRESHOLD_M),
            poi_density_per_km: compute_poi_density(total_poi_count, route.distance_km),
            category_entropy: compute_category_entropy(route),
            landmark_coverage: compute_landmark_coverage(route),
            poi_density_context: PoiDensityContext::from_poi_count(area_poi_count),
        }
    }

    /// Compute metrics with a custom overlap threshold
    pub fn compute_with_threshold(
        route: &Route,
        area_poi_count: usize,
        overlap_threshold_m: f64,
    ) -> Self {
        let path = &route.path;
        let total_poi_count = route.pois.len() + route.snapped_pois.len();

        RouteMetrics {
            circularity: compute_circularity(path),
            convexity: compute_convexity(path),
            path_overlap_pct: compute_path_overlap(path, overlap_threshold_m),
            poi_density_per_km: compute_poi_density(total_poi_count, route.distance_km),
            category_entropy: compute_category_entropy(route),
            landmark_coverage: compute_landmark_coverage(route),
            poi_density_context: PoiDensityContext::from_poi_count(area_poi_count),
        }
    }
}

/// Compute circularity using the isoperimetric ratio: 4*pi*area / perimeter^2
/// Returns 0.0 for degenerate paths, approaches 1.0 for circular loops
fn compute_circularity(path: &[Coordinates]) -> f32 {
    if path.len() < 3 {
        return 0.0;
    }

    let area = shoelace_area(path).abs();
    let perimeter = path_length(path);

    if perimeter < 1e-10 {
        return 0.0;
    }

    let ratio = (4.0 * std::f64::consts::PI * area) / (perimeter * perimeter);
    (ratio as f32).clamp(0.0, 1.0)
}

/// Compute convexity: path polygon area / convex hull area
/// Returns 1.0 for convex shapes, lower for indented/figure-8 shapes
fn compute_convexity(path: &[Coordinates]) -> f32 {
    if path.len() < 3 {
        return 0.0;
    }

    let path_area = shoelace_area(path).abs();
    if path_area < 1e-10 {
        return 0.0;
    }

    let hull = convex_hull(path);
    let hull_area = shoelace_area(&hull).abs();

    if hull_area < 1e-10 {
        return 0.0;
    }

    (path_area / hull_area).clamp(0.0, 1.0) as f32
}

/// Compute path overlap percentage using spatial bucketing
/// For each segment, checks if any non-adjacent segment is within threshold distance
fn compute_path_overlap(path: &[Coordinates], threshold_m: f64) -> f32 {
    if path.len() < 4 {
        return 0.0;
    }

    // Convert threshold from meters to approximate degrees
    let threshold_deg = threshold_m / 111_000.0;

    // Build spatial index: grid cells -> list of segment indices
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let segments: Vec<(Coordinates, Coordinates)> = path.windows(2).map(|w| (w[0], w[1])).collect();

    for (idx, (p1, p2)) in segments.iter().enumerate() {
        // Add segment to all grid cells it touches
        let min_lat = p1.lat.min(p2.lat) - threshold_deg;
        let max_lat = p1.lat.max(p2.lat) + threshold_deg;
        let min_lng = p1.lng.min(p2.lng) - threshold_deg;
        let max_lng = p1.lng.max(p2.lng) + threshold_deg;

        let cell_min_y = (min_lat / GRID_CELL_SIZE).floor() as i64;
        let cell_max_y = (max_lat / GRID_CELL_SIZE).ceil() as i64;
        let cell_min_x = (min_lng / GRID_CELL_SIZE).floor() as i64;
        let cell_max_x = (max_lng / GRID_CELL_SIZE).ceil() as i64;

        for cy in cell_min_y..=cell_max_y {
            for cx in cell_min_x..=cell_max_x {
                grid.entry((cx, cy)).or_default().push(idx);
            }
        }
    }

    let mut overlapping_length = 0.0;
    let mut total_length = 0.0;

    for (idx, (p1, p2)) in segments.iter().enumerate() {
        let seg_len = segment_length_m(p1, p2);
        total_length += seg_len;

        // Find grid cell for midpoint of this segment
        let mid_lat = (p1.lat + p2.lat) / 2.0;
        let mid_lng = (p1.lng + p2.lng) / 2.0;
        let cell_x = (mid_lng / GRID_CELL_SIZE).floor() as i64;
        let cell_y = (mid_lat / GRID_CELL_SIZE).floor() as i64;

        // Check neighboring cells
        let mut is_overlapping = false;
        'outer: for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(nearby_indices) = grid.get(&(cell_x + dx, cell_y + dy)) {
                    for &other_idx in nearby_indices {
                        // Skip self and adjacent segments
                        if idx.abs_diff(other_idx) <= OVERLAP_SKIP_NEIGHBORS {
                            continue;
                        }

                        let (q1, q2) = &segments[other_idx];
                        let dist = min_segment_distance(p1, p2, q1, q2);

                        // Convert distance to meters (approximate)
                        let dist_m = dist * 111_000.0;
                        if dist_m < threshold_m {
                            is_overlapping = true;
                            break 'outer;
                        }
                    }
                }
            }
        }

        if is_overlapping {
            overlapping_length += seg_len;
        }
    }

    if total_length < 1e-10 {
        return 0.0;
    }

    (overlapping_length / total_length).clamp(0.0, 1.0) as f32
}

/// Compute POI density (POIs per km)
fn compute_poi_density(poi_count: usize, distance_km: f64) -> f32 {
    if distance_km < 0.01 {
        return 0.0;
    }
    (poi_count as f64 / distance_km) as f32
}

/// Compute normalized Shannon entropy of POI categories
fn compute_category_entropy(route: &Route) -> f32 {
    let mut category_counts: HashMap<&PoiCategory, usize> = HashMap::new();

    for rp in &route.pois {
        *category_counts.entry(&rp.poi.category).or_insert(0) += 1;
    }
    for sp in &route.snapped_pois {
        *category_counts.entry(&sp.poi.category).or_insert(0) += 1;
    }

    let total = category_counts.values().sum::<usize>();
    if total <= 1 {
        return 0.0;
    }

    let n_categories = category_counts.len();
    if n_categories <= 1 {
        return 0.0;
    }

    let entropy: f64 = category_counts
        .values()
        .map(|&count| {
            let p = count as f64 / total as f64;
            if p > 0.0 {
                -p * p.ln()
            } else {
                0.0
            }
        })
        .sum();

    let max_entropy = (n_categories as f64).ln();
    if max_entropy < 1e-10 {
        return 0.0;
    }

    (entropy / max_entropy).clamp(0.0, 1.0) as f32
}

/// Compute landmark coverage: average popularity of waypoint POIs / 100
fn compute_landmark_coverage(route: &Route) -> f32 {
    if route.pois.is_empty() {
        return 0.0;
    }

    let total_popularity: f32 = route.pois.iter().map(|rp| rp.poi.popularity_score).sum();

    (total_popularity / route.pois.len() as f32 / 100.0).clamp(0.0, 1.0)
}

// --- Geometry helpers ---

/// Compute signed area of a polygon using the Shoelace formula
/// Uses lat/lng as approximate planar coordinates (fine for small areas)
fn shoelace_area(points: &[Coordinates]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    let n = points.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].lng * points[j].lat;
        area -= points[j].lng * points[i].lat;
    }

    area / 2.0
}

/// Compute total path length in degrees (approximate for circularity ratio)
fn path_length(path: &[Coordinates]) -> f64 {
    path.windows(2)
        .map(|w| {
            let dlat = w[1].lat - w[0].lat;
            let dlng = w[1].lng - w[0].lng;
            (dlat * dlat + dlng * dlng).sqrt()
        })
        .sum()
}

/// Compute segment length in meters (approximate)
fn segment_length_m(p1: &Coordinates, p2: &Coordinates) -> f64 {
    p1.distance_to(p2) * 1000.0
}

/// Compute minimum distance between two line segments (in degrees, approximate)
fn min_segment_distance(
    p1: &Coordinates,
    p2: &Coordinates,
    q1: &Coordinates,
    q2: &Coordinates,
) -> f64 {
    // Check distance from each endpoint to the other segment
    let d1 = point_to_segment_distance_deg(p1, q1, q2);
    let d2 = point_to_segment_distance_deg(p2, q1, q2);
    let d3 = point_to_segment_distance_deg(q1, p1, p2);
    let d4 = point_to_segment_distance_deg(q2, p1, p2);

    d1.min(d2).min(d3).min(d4)
}

/// Distance from a point to a line segment in degree-space
fn point_to_segment_distance_deg(
    point: &Coordinates,
    seg_start: &Coordinates,
    seg_end: &Coordinates,
) -> f64 {
    let dx = seg_end.lng - seg_start.lng;
    let dy = seg_end.lat - seg_start.lat;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-20 {
        let dlat = point.lat - seg_start.lat;
        let dlng = point.lng - seg_start.lng;
        return (dlat * dlat + dlng * dlng).sqrt();
    }

    let t = ((point.lng - seg_start.lng) * dx + (point.lat - seg_start.lat) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_lng = seg_start.lng + t * dx;
    let proj_lat = seg_start.lat + t * dy;

    let dlat = point.lat - proj_lat;
    let dlng = point.lng - proj_lng;
    (dlat * dlat + dlng * dlng).sqrt()
}

/// Compute convex hull using Andrew's monotone chain algorithm
fn convex_hull(points: &[Coordinates]) -> Vec<Coordinates> {
    if points.len() < 3 {
        return points.to_vec();
    }

    // Sort by lng, then lat
    let mut sorted: Vec<Coordinates> = points.to_vec();
    sorted.sort_by(|a, b| {
        a.lng
            .partial_cmp(&b.lng)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.lat
                    .partial_cmp(&b.lat)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    sorted.dedup_by(|a, b| (a.lat - b.lat).abs() < 1e-12 && (a.lng - b.lng).abs() < 1e-12);

    if sorted.len() < 3 {
        return sorted;
    }

    let n = sorted.len();
    let mut hull: Vec<Coordinates> = Vec::with_capacity(2 * n);

    // Build lower hull
    for point in &sorted {
        while hull.len() >= 2 && cross(&hull[hull.len() - 2], &hull[hull.len() - 1], point) <= 0.0 {
            hull.pop();
        }
        hull.push(*point);
    }

    // Build upper hull
    let lower_len = hull.len() + 1;
    for point in sorted.iter().rev().skip(1) {
        while hull.len() >= lower_len
            && cross(&hull[hull.len() - 2], &hull[hull.len() - 1], point) <= 0.0
        {
            hull.pop();
        }
        hull.push(*point);
    }

    hull.pop(); // Remove last point (duplicate of first)
    hull
}

/// Cross product of vectors OA and OB
fn cross(o: &Coordinates, a: &Coordinates, b: &Coordinates) -> f64 {
    (a.lng - o.lng) * (b.lat - o.lat) - (a.lat - o.lat) * (b.lng - o.lng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Poi, PoiCategory, RoutePoi, SnappedPoi};

    fn make_coord(lat: f64, lng: f64) -> Coordinates {
        Coordinates::new(lat, lng).unwrap()
    }

    /// Create a roughly circular path centered at (lat, lng) with given radius in degrees
    fn make_circle_path(
        center_lat: f64,
        center_lng: f64,
        radius_deg: f64,
        n: usize,
    ) -> Vec<Coordinates> {
        let mut path = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            let lat = center_lat + radius_deg * angle.cos();
            let lng = center_lng + radius_deg * angle.sin();
            path.push(make_coord(lat, lng));
        }
        path
    }

    /// Create an out-and-back path (low circularity)
    fn make_out_and_back_path() -> Vec<Coordinates> {
        let mut path = Vec::new();
        // Go north
        for i in 0..20 {
            let lat = 48.85 + i as f64 * 0.001;
            path.push(make_coord(lat, 2.35));
        }
        // Come back south (same street, slightly offset)
        for i in (0..20).rev() {
            let lat = 48.85 + i as f64 * 0.001;
            path.push(make_coord(lat, 2.35001));
        }
        path
    }

    /// Create a figure-8 path (low convexity)
    fn make_figure8_path() -> Vec<Coordinates> {
        let mut path = Vec::new();
        let n = 40;
        // First loop (top)
        for i in 0..n {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            let lat = 48.86 + 0.005 * angle.cos();
            let lng = 2.35 + 0.005 * angle.sin();
            path.push(make_coord(lat, lng));
        }
        // Second loop (bottom)
        for i in 0..n {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            let lat = 48.85 + 0.005 * angle.cos();
            let lng = 2.35 + 0.005 * angle.sin();
            path.push(make_coord(lat, lng));
        }
        path
    }

    fn make_test_route(
        path: Vec<Coordinates>,
        pois: Vec<RoutePoi>,
        snapped: Vec<SnappedPoi>,
    ) -> Route {
        Route {
            id: uuid::Uuid::new_v4(),
            distance_km: 5.0,
            estimated_duration_minutes: 75,
            elevation_gain_m: None,
            path,
            pois,
            snapped_pois: snapped,
            score: 0.0,
            metrics: None,
        }
    }

    fn make_poi(name: &str, category: PoiCategory, popularity: f32) -> Poi {
        Poi::new(
            name.to_string(),
            category,
            make_coord(48.856, 2.352),
            popularity,
        )
    }

    #[test]
    fn test_circularity_perfect_circle() {
        let path = make_circle_path(48.85, 2.35, 0.01, 100);
        let circ = compute_circularity(&path);
        // Perfect circle should be close to 1.0
        assert!(
            circ > 0.9,
            "Circle circularity should be > 0.9, got {}",
            circ
        );
    }

    #[test]
    fn test_circularity_out_and_back() {
        let path = make_out_and_back_path();
        let circ = compute_circularity(&path);
        // Out-and-back should have very low circularity
        assert!(
            circ < 0.1,
            "Out-and-back circularity should be < 0.1, got {}",
            circ
        );
    }

    #[test]
    fn test_circularity_degenerate() {
        assert_eq!(compute_circularity(&[]), 0.0);
        assert_eq!(compute_circularity(&[make_coord(48.85, 2.35)]), 0.0);
        assert_eq!(
            compute_circularity(&[make_coord(48.85, 2.35), make_coord(48.86, 2.36)]),
            0.0
        );
    }

    #[test]
    fn test_convexity_circle() {
        let path = make_circle_path(48.85, 2.35, 0.01, 100);
        let conv = compute_convexity(&path);
        // Circle is convex, so convexity should be high
        assert!(conv > 0.9, "Circle convexity should be > 0.9, got {}", conv);
    }

    #[test]
    fn test_convexity_figure8() {
        let path = make_figure8_path();
        let conv = compute_convexity(&path);
        // Figure-8 should have lower convexity than a perfect circle
        assert!(
            conv < 0.95,
            "Figure-8 convexity should be < 0.95, got {}",
            conv
        );
    }

    #[test]
    fn test_path_overlap_no_overlap() {
        // A simple circle has no overlap
        let path = make_circle_path(48.85, 2.35, 0.01, 50);
        let overlap = compute_path_overlap(&path, 25.0);
        assert!(
            overlap < 0.05,
            "Circle should have near-zero overlap, got {}",
            overlap
        );
    }

    #[test]
    fn test_path_overlap_out_and_back() {
        let path = make_out_and_back_path();
        let overlap = compute_path_overlap(&path, 25.0);
        // Out-and-back should have high overlap
        assert!(
            overlap > 0.5,
            "Out-and-back should have high overlap, got {}",
            overlap
        );
    }

    #[test]
    fn test_poi_density() {
        assert_eq!(compute_poi_density(10, 5.0), 2.0);
        assert_eq!(compute_poi_density(0, 5.0), 0.0);
        assert_eq!(compute_poi_density(5, 0.0), 0.0);
    }

    #[test]
    fn test_category_entropy_single_category() {
        let route = make_test_route(
            vec![make_coord(48.85, 2.35)],
            vec![
                RoutePoi::new(make_poi("A", PoiCategory::Monument, 80.0), 1, 1.0),
                RoutePoi::new(make_poi("B", PoiCategory::Monument, 70.0), 2, 2.0),
            ],
            vec![],
        );
        let entropy = compute_category_entropy(&route);
        assert_eq!(entropy, 0.0, "Single category should have 0 entropy");
    }

    #[test]
    fn test_category_entropy_diverse() {
        let route = make_test_route(
            vec![make_coord(48.85, 2.35)],
            vec![
                RoutePoi::new(make_poi("A", PoiCategory::Monument, 80.0), 1, 1.0),
                RoutePoi::new(make_poi("B", PoiCategory::Park, 70.0), 2, 2.0),
                RoutePoi::new(make_poi("C", PoiCategory::Museum, 60.0), 3, 3.0),
            ],
            vec![],
        );
        let entropy = compute_category_entropy(&route);
        // 3 categories, equal distribution = maximum entropy = 1.0
        assert!(
            (entropy - 1.0).abs() < 0.01,
            "Equal distribution of 3 categories should have entropy ~1.0, got {}",
            entropy
        );
    }

    #[test]
    fn test_landmark_coverage() {
        let route = make_test_route(
            vec![make_coord(48.85, 2.35)],
            vec![
                RoutePoi::new(make_poi("A", PoiCategory::Monument, 80.0), 1, 1.0),
                RoutePoi::new(make_poi("B", PoiCategory::Park, 60.0), 2, 2.0),
            ],
            vec![],
        );
        let coverage = compute_landmark_coverage(&route);
        // (80 + 60) / 2 / 100 = 0.7
        assert!(
            (coverage - 0.7).abs() < 0.01,
            "Expected 0.7 landmark coverage, got {}",
            coverage
        );
    }

    #[test]
    fn test_landmark_coverage_empty() {
        let route = make_test_route(vec![make_coord(48.85, 2.35)], vec![], vec![]);
        assert_eq!(compute_landmark_coverage(&route), 0.0);
    }

    #[test]
    fn test_poi_density_context() {
        assert_eq!(
            PoiDensityContext::from_poi_count(0),
            PoiDensityContext::Geometric
        );
        assert_eq!(
            PoiDensityContext::from_poi_count(1),
            PoiDensityContext::Geometric
        );
        assert_eq!(
            PoiDensityContext::from_poi_count(5),
            PoiDensityContext::Sparse
        );
        assert_eq!(
            PoiDensityContext::from_poi_count(10),
            PoiDensityContext::Moderate
        );
        assert_eq!(
            PoiDensityContext::from_poi_count(25),
            PoiDensityContext::Dense
        );
    }

    #[test]
    fn test_convex_hull_triangle() {
        let points = vec![
            make_coord(0.0, 0.0),
            make_coord(0.0, 1.0),
            make_coord(1.0, 0.0),
        ];
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 3);
    }

    #[test]
    fn test_convex_hull_with_interior_point() {
        let points = vec![
            make_coord(0.0, 0.0),
            make_coord(0.0, 2.0),
            make_coord(2.0, 0.0),
            make_coord(2.0, 2.0),
            make_coord(1.0, 1.0), // Interior point
        ];
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 4, "Interior point should not be in hull");
    }

    #[test]
    fn test_full_metrics_computation() {
        let path = make_circle_path(48.85, 2.35, 0.01, 50);
        let route = make_test_route(
            path,
            vec![
                RoutePoi::new(make_poi("A", PoiCategory::Monument, 80.0), 1, 1.0),
                RoutePoi::new(make_poi("B", PoiCategory::Park, 60.0), 2, 2.0),
            ],
            vec![SnappedPoi::new(
                make_poi("C", PoiCategory::Cafe, 50.0),
                3.0,
                20.0,
            )],
        );

        let metrics = RouteMetrics::compute(&route, 15);

        assert!(metrics.circularity > 0.5);
        assert!(metrics.convexity > 0.5);
        assert!(metrics.path_overlap_pct < 0.1);
        assert!(metrics.poi_density_per_km > 0.0);
        assert!(metrics.category_entropy > 0.0);
        assert!(metrics.landmark_coverage > 0.0);
        assert_eq!(metrics.poi_density_context, PoiDensityContext::Moderate);
    }
}
