use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Coordinates {
    pub lat: f64,
    pub lng: f64,
}

impl Coordinates {
    pub fn new(lat: f64, lng: f64) -> Result<Self, String> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(format!(
                "Invalid latitude: {} (must be between -90 and 90)",
                lat
            ));
        }
        if !(-180.0..=180.0).contains(&lng) {
            return Err(format!(
                "Invalid longitude: {} (must be between -180 and 180)",
                lng
            ));
        }
        Ok(Coordinates { lat, lng })
    }

    /// Calculate distance between two coordinates using Haversine formula
    /// Returns distance in kilometers
    pub fn distance_to(&self, other: &Coordinates) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;

        let lat1_rad = self.lat.to_radians();
        let lat2_rad = other.lat.to_radians();
        let delta_lat = (other.lat - self.lat).to_radians();
        let delta_lng = (other.lng - self.lng).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lng / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_KM * c
    }

    /// Round coordinates to specified decimal places for caching
    pub fn round(&self, decimal_places: u32) -> Self {
        let multiplier = 10_f64.powi(decimal_places as i32);
        Coordinates {
            lat: (self.lat * multiplier).round() / multiplier,
            lng: (self.lng * multiplier).round() / multiplier,
        }
    }

    /// Calculate perpendicular distance from this point to a line segment
    /// Returns (distance_km, t) where t is the position along the segment [0,1]
    fn distance_to_segment(&self, p1: &Coordinates, p2: &Coordinates) -> (f64, f64) {
        // Calculate squared length of the segment
        let segment_length_sq = p1.distance_to(p2).powi(2);

        if segment_length_sq < 1e-10 {
            // Segment is essentially a point
            return (self.distance_to(p1), 0.0);
        }

        // Calculate projection parameter t (0 to 1 represents point on segment)
        // Using dot product in lat/lng space (approximation, but good enough for short segments)
        let dx = p2.lng - p1.lng;
        let dy = p2.lat - p1.lat;
        let t = ((self.lng - p1.lng) * dx + (self.lat - p1.lat) * dy) / (dx * dx + dy * dy);

        // Clamp t to [0, 1] to stay on the segment
        let t_clamped = t.clamp(0.0, 1.0);

        // Calculate the closest point on the segment
        let closest = Coordinates {
            lat: p1.lat + t_clamped * dy,
            lng: p1.lng + t_clamped * dx,
        };

        (self.distance_to(&closest), t_clamped)
    }

    /// Find minimum distance from this point to a linestring (path)
    /// Returns (distance_km, closest_segment_index, distance_along_path_km)
    pub fn distance_to_linestring(&self, path: &[Coordinates]) -> Option<(f64, usize, f64)> {
        if path.len() < 2 {
            return None;
        }

        let mut min_distance = f64::INFINITY;
        let mut closest_segment = 0;
        let mut distance_to_closest_point = 0.0;

        let mut cumulative_distance = 0.0;

        for (i, window) in path.windows(2).enumerate() {
            let segment_start = &window[0];
            let segment_end = &window[1];

            let (dist, t) = self.distance_to_segment(segment_start, segment_end);

            if dist < min_distance {
                min_distance = dist;
                closest_segment = i;

                // Distance along path = cumulative distance to segment start + t * segment length
                let segment_length = segment_start.distance_to(segment_end);
                distance_to_closest_point = cumulative_distance + (t * segment_length);
            }

            cumulative_distance += segment_start.distance_to(segment_end);
        }

        Some((min_distance, closest_segment, distance_to_closest_point))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinates_validation() {
        assert!(Coordinates::new(48.8566, 2.3522).is_ok());
        assert!(Coordinates::new(91.0, 0.0).is_err()); // Invalid lat
        assert!(Coordinates::new(0.0, 181.0).is_err()); // Invalid lng
    }

    #[test]
    fn test_distance_calculation() {
        let paris = Coordinates::new(48.8566, 2.3522).unwrap();
        let london = Coordinates::new(51.5074, -0.1278).unwrap();

        let distance = paris.distance_to(&london);
        // Paris to London is approximately 344 km
        assert!((distance - 344.0).abs() < 10.0);
    }

    #[test]
    fn test_rounding() {
        let coords = Coordinates::new(48.856614, 2.352222).unwrap();
        let rounded = coords.round(3);
        assert_eq!(rounded.lat, 48.857);
        assert_eq!(rounded.lng, 2.352);
    }

    #[test]
    fn test_distance_to_segment() {
        let p1 = Coordinates::new(48.8566, 2.3522).unwrap();
        let p2 = Coordinates::new(48.8600, 2.3600).unwrap();

        // Point on the segment (midpoint)
        let midpoint = Coordinates::new(48.8583, 2.3561).unwrap();
        let (dist, t) = midpoint.distance_to_segment(&p1, &p2);
        assert!(dist < 0.1, "Midpoint should be close to segment");
        assert!((t - 0.5).abs() < 0.1, "Midpoint t should be around 0.5");

        // Point perpendicular to segment
        let perpendicular = Coordinates::new(48.8550, 2.3561).unwrap();
        let (dist, _t) = perpendicular.distance_to_segment(&p1, &p2);
        assert!(
            dist > 0.0,
            "Perpendicular point should have non-zero distance"
        );
    }

    #[test]
    fn test_distance_to_linestring() {
        // Create a simple path: 3 points in a line
        let path = vec![
            Coordinates::new(48.8566, 2.3522).unwrap(),
            Coordinates::new(48.8600, 2.3600).unwrap(),
            Coordinates::new(48.8650, 2.3700).unwrap(),
        ];

        // Point near the middle of the path
        let point = Coordinates::new(48.8585, 2.3565).unwrap();
        let result = point.distance_to_linestring(&path);

        assert!(result.is_some());
        let (dist, segment_idx, dist_along) = result.unwrap();

        assert!(dist < 0.5, "Point should be close to path");
        assert!(segment_idx < 2, "Should find a valid segment");
        assert!(dist_along > 0.0, "Distance along path should be positive");

        // Empty or single-point path
        let empty_path: Vec<Coordinates> = vec![];
        assert!(point.distance_to_linestring(&empty_path).is_none());

        let single_point = vec![Coordinates::new(48.8566, 2.3522).unwrap()];
        assert!(point.distance_to_linestring(&single_point).is_none());
    }
}
