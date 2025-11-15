use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

/// Distance in kilometers
/// Prevents mixing up units and provides type safety
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DistanceKm(pub f64);

impl DistanceKm {
    pub fn new(km: f64) -> Result<Self, String> {
        if km < 0.0 {
            return Err("Distance cannot be negative".to_string());
        }
        if !km.is_finite() {
            return Err("Distance must be a finite number".to_string());
        }
        Ok(DistanceKm(km))
    }

    /// Convert to meters
    pub fn to_meters(self) -> DistanceMeters {
        DistanceMeters(self.0 * 1000.0)
    }

    /// Get the raw kilometers value
    pub fn as_km(self) -> f64 {
        self.0
    }

    /// Create from raw value without validation (use carefully)
    pub fn from_raw(km: f64) -> Self {
        DistanceKm(km)
    }
}

impl fmt::Display for DistanceKm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}km", self.0)
    }
}

impl From<DistanceMeters> for DistanceKm {
    fn from(meters: DistanceMeters) -> Self {
        DistanceKm(meters.0 / 1000.0)
    }
}

impl Add for DistanceKm {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        DistanceKm(self.0 + other.0)
    }
}

impl Sub for DistanceKm {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        DistanceKm(self.0 - other.0)
    }
}

impl Mul<f64> for DistanceKm {
    type Output = Self;

    fn mul(self, scalar: f64) -> Self {
        DistanceKm(self.0 * scalar)
    }
}

impl Div<f64> for DistanceKm {
    type Output = Self;

    fn div(self, scalar: f64) -> Self {
        DistanceKm(self.0 / scalar)
    }
}

/// Distance in meters
/// Commonly used for smaller distances and radii
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DistanceMeters(pub f64);

impl DistanceMeters {
    pub fn new(meters: f64) -> Result<Self, String> {
        if meters < 0.0 {
            return Err("Distance cannot be negative".to_string());
        }
        if !meters.is_finite() {
            return Err("Distance must be a finite number".to_string());
        }
        Ok(DistanceMeters(meters))
    }

    /// Convert to kilometers
    pub fn to_km(self) -> DistanceKm {
        DistanceKm(self.0 / 1000.0)
    }

    /// Get the raw meters value
    pub fn as_meters(self) -> f64 {
        self.0
    }

    /// Create from raw value without validation (use carefully)
    pub fn from_raw(meters: f64) -> Self {
        DistanceMeters(meters)
    }
}

impl fmt::Display for DistanceMeters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}m", self.0)
    }
}

impl From<DistanceKm> for DistanceMeters {
    fn from(km: DistanceKm) -> Self {
        DistanceMeters(km.0 * 1000.0)
    }
}

impl Add for DistanceMeters {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        DistanceMeters(self.0 + other.0)
    }
}

impl Sub for DistanceMeters {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        DistanceMeters(self.0 - other.0)
    }
}

/// Radius in meters - semantically similar to DistanceMeters but clearer intent
pub type RadiusMeters = DistanceMeters;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_km_creation() {
        assert!(DistanceKm::new(5.0).is_ok());
        assert!(DistanceKm::new(0.0).is_ok());
        assert!(DistanceKm::new(-1.0).is_err());
        assert!(DistanceKm::new(f64::INFINITY).is_err());
        assert!(DistanceKm::new(f64::NAN).is_err());
    }

    #[test]
    fn test_distance_km_conversion() {
        let km = DistanceKm::new(5.0).unwrap();
        let meters = km.to_meters();
        assert_eq!(meters.as_meters(), 5000.0);

        let back_to_km: DistanceKm = meters.into();
        assert_eq!(back_to_km.as_km(), 5.0);
    }

    #[test]
    fn test_distance_km_arithmetic() {
        let d1 = DistanceKm::new(5.0).unwrap();
        let d2 = DistanceKm::new(3.0).unwrap();

        assert_eq!((d1 + d2).as_km(), 8.0);
        assert_eq!((d1 - d2).as_km(), 2.0);
        assert_eq!((d1 * 2.0).as_km(), 10.0);
        assert_eq!((d1 / 2.0).as_km(), 2.5);
    }

    #[test]
    fn test_distance_km_display() {
        let d = DistanceKm::new(5.123).unwrap();
        assert_eq!(format!("{}", d), "5.12km");
    }

    #[test]
    fn test_distance_meters_creation() {
        assert!(DistanceMeters::new(500.0).is_ok());
        assert!(DistanceMeters::new(0.0).is_ok());
        assert!(DistanceMeters::new(-1.0).is_err());
    }

    #[test]
    fn test_distance_meters_display() {
        let d = DistanceMeters::new(150.5).unwrap();
        assert_eq!(format!("{}", d), "150.5m");
    }
}
