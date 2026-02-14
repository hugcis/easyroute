use crate::models::Coordinates;

/// Compute signed area of a polygon using the Shoelace formula
/// Uses lat/lng as approximate planar coordinates (fine for small areas)
pub fn shoelace_area(points: &[Coordinates]) -> f64 {
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
pub fn path_length(path: &[Coordinates]) -> f64 {
    path.windows(2)
        .map(|w| {
            let dlat = w[1].lat - w[0].lat;
            let dlng = w[1].lng - w[0].lng;
            (dlat * dlat + dlng * dlng).sqrt()
        })
        .sum()
}

/// Compute segment length in meters (approximate)
pub fn segment_length_m(p1: &Coordinates, p2: &Coordinates) -> f64 {
    p1.distance_to(p2) * 1000.0
}

/// Compute minimum distance between two line segments (in degrees, approximate)
pub fn min_segment_distance(
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
pub fn point_to_segment_distance_deg(
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
pub fn convex_hull(points: &[Coordinates]) -> Vec<Coordinates> {
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

/// Compute the area of a convex hull from a set of points.
/// Combines `convex_hull()` + `shoelace_area()` for convenience.
pub fn convex_hull_area(points: &[Coordinates]) -> f64 {
    let hull = convex_hull(points);
    shoelace_area(&hull).abs()
}

/// Calculate the angle (in radians, -PI to PI) from `start` to `target`
pub fn angle_from_start(start: &Coordinates, target: &Coordinates) -> f64 {
    let dx = target.lng - start.lng;
    let dy = target.lat - start.lat;
    dy.atan2(dx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn c(lat: f64, lng: f64) -> Coordinates {
        Coordinates::new(lat, lng).unwrap()
    }

    // --- shoelace_area ---

    #[test]
    fn shoelace_area_unit_square() {
        // CCW square: (0,0) -> (1,0) -> (1,1) -> (0,1) — but in (lat,lng) coords
        let pts = vec![c(0.0, 0.0), c(0.0, 1.0), c(1.0, 1.0), c(1.0, 0.0)];
        let area = shoelace_area(&pts);
        assert!((area.abs() - 1.0).abs() < 1e-10, "area={area}");
    }

    #[test]
    fn shoelace_area_triangle() {
        let pts = vec![c(0.0, 0.0), c(0.0, 1.0), c(1.0, 0.0)];
        assert!((shoelace_area(&pts).abs() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn shoelace_area_degenerate() {
        assert_eq!(shoelace_area(&[]), 0.0);
        assert_eq!(shoelace_area(&[c(1.0, 2.0)]), 0.0);
        assert_eq!(shoelace_area(&[c(1.0, 2.0), c(3.0, 4.0)]), 0.0);
    }

    #[test]
    fn shoelace_area_collinear() {
        let pts = vec![c(0.0, 0.0), c(1.0, 1.0), c(2.0, 2.0)];
        assert!(shoelace_area(&pts).abs() < 1e-10);
    }

    // --- path_length ---

    #[test]
    fn path_length_known() {
        // Horizontal path: 3 points, total length = 2.0 in degree-space
        let pts = vec![c(0.0, 0.0), c(0.0, 1.0), c(0.0, 2.0)];
        assert!((path_length(&pts) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn path_length_empty_and_single() {
        assert_eq!(path_length(&[]), 0.0);
        assert_eq!(path_length(&[c(5.0, 5.0)]), 0.0);
    }

    #[test]
    fn path_length_diagonal() {
        let pts = vec![c(0.0, 0.0), c(3.0, 4.0)];
        assert!((path_length(&pts) - 5.0).abs() < 1e-10);
    }

    // --- segment_length_m ---

    #[test]
    fn segment_length_m_matches_distance_to() {
        let a = c(48.8566, 2.3522);
        let b = c(48.8600, 2.3600);
        let expected = a.distance_to(&b) * 1000.0;
        assert!((segment_length_m(&a, &b) - expected).abs() < 1e-6);
    }

    // --- point_to_segment_distance_deg ---

    #[test]
    fn point_on_segment_distance_zero() {
        let a = c(0.0, 0.0);
        let b = c(0.0, 2.0);
        let mid = c(0.0, 1.0);
        assert!(point_to_segment_distance_deg(&mid, &a, &b) < 1e-10);
    }

    #[test]
    fn point_perpendicular_projection() {
        let a = c(0.0, 0.0);
        let b = c(0.0, 2.0);
        let p = c(1.0, 1.0); // 1 degree above midpoint
        let dist = point_to_segment_distance_deg(&p, &a, &b);
        assert!((dist - 1.0).abs() < 1e-10, "dist={dist}");
    }

    #[test]
    fn point_clamped_to_endpoint() {
        let a = c(0.0, 0.0);
        let b = c(0.0, 1.0);
        let p = c(0.0, 5.0); // Beyond segment end
        let dist = point_to_segment_distance_deg(&p, &a, &b);
        // Should clamp to endpoint b, distance = 4.0 in degree-space
        assert!((dist - 4.0).abs() < 1e-10, "dist={dist}");
    }

    #[test]
    fn point_to_zero_length_segment() {
        let a = c(1.0, 1.0);
        let p = c(4.0, 5.0);
        let dist = point_to_segment_distance_deg(&p, &a, &a);
        let expected = ((3.0_f64).powi(2) + (4.0_f64).powi(2)).sqrt();
        assert!((dist - expected).abs() < 1e-10);
    }

    // --- min_segment_distance ---

    #[test]
    fn identical_segments_zero_distance() {
        let a = c(0.0, 0.0);
        let b = c(0.0, 1.0);
        assert!(min_segment_distance(&a, &b, &a, &b) < 1e-10);
    }

    #[test]
    fn parallel_segments_correct_distance() {
        let p1 = c(0.0, 0.0);
        let p2 = c(0.0, 1.0);
        let q1 = c(2.0, 0.0);
        let q2 = c(2.0, 1.0);
        let dist = min_segment_distance(&p1, &p2, &q1, &q2);
        assert!((dist - 2.0).abs() < 1e-10, "dist={dist}");
    }

    // --- convex_hull ---

    #[test]
    fn convex_hull_triangle() {
        let pts = vec![c(0.0, 0.0), c(0.0, 2.0), c(2.0, 1.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 3);
    }

    #[test]
    fn convex_hull_square_with_interior_point() {
        let pts = vec![
            c(0.0, 0.0),
            c(0.0, 2.0),
            c(2.0, 2.0),
            c(2.0, 0.0),
            c(1.0, 1.0), // interior
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4, "hull={hull:?}");
    }

    #[test]
    fn convex_hull_collinear() {
        let pts = vec![c(0.0, 0.0), c(1.0, 1.0), c(2.0, 2.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 2, "Collinear points should yield 2 endpoints");
    }

    #[test]
    fn convex_hull_duplicates() {
        let pts = vec![c(0.0, 0.0), c(0.0, 0.0), c(1.0, 1.0), c(1.0, 1.0)];
        let hull = convex_hull(&pts);
        assert!(hull.len() <= 2);
    }

    #[test]
    fn convex_hull_fewer_than_three() {
        assert_eq!(convex_hull(&[]).len(), 0);
        assert_eq!(convex_hull(&[c(1.0, 2.0)]).len(), 1);
        let two = vec![c(0.0, 0.0), c(1.0, 1.0)];
        assert_eq!(convex_hull(&two).len(), 2);
    }

    // --- convex_hull_area ---

    #[test]
    fn convex_hull_area_known_square() {
        let pts = vec![c(0.0, 0.0), c(0.0, 1.0), c(1.0, 1.0), c(1.0, 0.0)];
        assert!((convex_hull_area(&pts) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn convex_hull_area_empty() {
        assert_eq!(convex_hull_area(&[]), 0.0);
    }

    // --- angle_from_start ---

    #[test]
    fn angle_from_start_cardinal_directions() {
        let origin = c(0.0, 0.0);

        // East (positive lng) → 0 radians
        let east = angle_from_start(&origin, &c(0.0, 1.0));
        assert!(east.abs() < 1e-10, "east={east}");

        // North (positive lat) → PI/2
        let north = angle_from_start(&origin, &c(1.0, 0.0));
        assert!((north - PI / 2.0).abs() < 1e-10, "north={north}");

        // West (negative lng) → ±PI
        let west = angle_from_start(&origin, &c(0.0, -1.0));
        assert!((west.abs() - PI).abs() < 1e-10, "west={west}");

        // South (negative lat) → -PI/2
        let south = angle_from_start(&origin, &c(-1.0, 0.0));
        assert!((south + PI / 2.0).abs() < 1e-10, "south={south}");
    }
}
