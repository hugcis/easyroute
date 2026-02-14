use crate::models::Coordinates;

/// Axis-aligned bounding box in geographic coordinates.
pub struct BoundingBox {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
}

impl BoundingBox {
    /// Compute a bounding box around a center point with a radius in meters.
    pub fn from_center_radius(center: &Coordinates, radius_m: f64) -> Self {
        let lat_delta = radius_m / 111_000.0;
        let lng_delta = if center.lat.abs() > 85.0 {
            lat_delta
        } else {
            radius_m / (111_000.0 * center.lat.to_radians().cos())
        };

        BoundingBox {
            min_lat: center.lat - lat_delta,
            max_lat: center.lat + lat_delta,
            min_lng: center.lng - lng_delta,
            max_lng: center.lng + lng_delta,
        }
    }

    /// Compute a bounding box that encloses a path with a buffer in meters.
    pub fn from_path_with_buffer(path: &[Coordinates], buffer_m: f64) -> Self {
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lng = f64::INFINITY;
        let mut max_lng = f64::NEG_INFINITY;

        for coord in path {
            min_lat = min_lat.min(coord.lat);
            max_lat = max_lat.max(coord.lat);
            min_lng = min_lng.min(coord.lng);
            max_lng = max_lng.max(coord.lng);
        }

        let lat_buffer = buffer_m / 111_000.0;
        let mid_lat = (min_lat + max_lat) / 2.0;

        let lng_buffer = if mid_lat.abs() > 85.0 {
            lat_buffer
        } else {
            buffer_m / (111_000.0 * mid_lat.to_radians().cos())
        };

        BoundingBox {
            min_lat: min_lat - lat_buffer,
            max_lat: max_lat + lat_buffer,
            min_lng: min_lng - lng_buffer,
            max_lng: max_lng + lng_buffer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: f64, lng: f64) -> Coordinates {
        Coordinates::new(lat, lng).unwrap()
    }

    #[test]
    fn path_bbox_single_segment() {
        let path = vec![c(48.85, 2.35), c(48.86, 2.36)];
        let bbox = BoundingBox::from_path_with_buffer(&path, 0.0);
        assert!((bbox.min_lat - 48.85).abs() < 1e-10);
        assert!((bbox.max_lat - 48.86).abs() < 1e-10);
        assert!((bbox.min_lng - 2.35).abs() < 1e-10);
        assert!((bbox.max_lng - 2.36).abs() < 1e-10);
    }

    #[test]
    fn path_bbox_buffer_expansion() {
        let path = vec![c(48.85, 2.35), c(48.86, 2.36)];
        let buffer_m = 1000.0;
        let bbox = BoundingBox::from_path_with_buffer(&path, buffer_m);
        let lat_buffer = buffer_m / 111_000.0;
        assert!((bbox.min_lat - (48.85 - lat_buffer)).abs() < 1e-10);
        assert!((bbox.max_lat - (48.86 + lat_buffer)).abs() < 1e-10);
    }

    #[test]
    fn path_bbox_longitude_buffer_widens_at_higher_latitude() {
        let buffer_m = 1000.0;
        let lat_buffer = buffer_m / 111_000.0;

        let path_eq = vec![c(1.0, 10.0), c(1.0, 10.0)];
        let bbox_eq = BoundingBox::from_path_with_buffer(&path_eq, buffer_m);
        let lng_buf_eq = (bbox_eq.max_lng - 10.0) - 0.0;

        let path_60 = vec![c(60.0, 10.0), c(60.0, 10.0)];
        let bbox_60 = BoundingBox::from_path_with_buffer(&path_60, buffer_m);
        let lng_buf_60 = bbox_60.max_lng - 10.0;

        assert!(
            lng_buf_60 > lng_buf_eq,
            "lng_buf_60={lng_buf_60}, lng_buf_eq={lng_buf_eq}"
        );
        assert!((bbox_eq.max_lat - (1.0 + lat_buffer)).abs() < 1e-10);
        assert!((bbox_60.max_lat - (60.0 + lat_buffer)).abs() < 1e-10);
    }

    #[test]
    fn path_bbox_near_poles_fallback() {
        let path = vec![c(86.0, 10.0), c(86.0, 10.0)];
        let bbox = BoundingBox::from_path_with_buffer(&path, 1000.0);
        let lat_buffer = 1000.0 / 111_000.0;
        let lng_buffer = bbox.max_lng - 10.0;
        assert!(
            (lng_buffer - lat_buffer).abs() < 1e-10,
            "lng_buffer={lng_buffer}, lat_buffer={lat_buffer}"
        );
    }

    #[test]
    fn path_bbox_multi_point_envelope() {
        let path = vec![c(48.85, 2.35), c(48.87, 2.33), c(48.86, 2.38)];
        let bbox = BoundingBox::from_path_with_buffer(&path, 0.0);
        assert!((bbox.min_lat - 48.85).abs() < 1e-10);
        assert!((bbox.max_lat - 48.87).abs() < 1e-10);
        assert!((bbox.min_lng - 2.33).abs() < 1e-10);
        assert!((bbox.max_lng - 2.38).abs() < 1e-10);
    }

    #[test]
    fn center_radius_basic() {
        let center = c(48.8566, 2.3522);
        let bbox = BoundingBox::from_center_radius(&center, 1000.0);
        let lat_delta = 1000.0 / 111_000.0;
        assert!((bbox.min_lat - (48.8566 - lat_delta)).abs() < 1e-10);
        assert!((bbox.max_lat - (48.8566 + lat_delta)).abs() < 1e-10);
        assert!(bbox.min_lng < 2.3522);
        assert!(bbox.max_lng > 2.3522);
    }

    #[test]
    fn center_radius_near_poles() {
        let center = c(86.0, 10.0);
        let bbox = BoundingBox::from_center_radius(&center, 1000.0);
        let lat_delta = 1000.0 / 111_000.0;
        let lng_delta = bbox.max_lng - 10.0;
        assert!(
            (lng_delta - lat_delta).abs() < 1e-10,
            "near-pole: lng_delta={lng_delta}, lat_delta={lat_delta}"
        );
    }
}
