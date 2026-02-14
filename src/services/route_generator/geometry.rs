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
