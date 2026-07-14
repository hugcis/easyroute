use crate::config::{RouteGeneratorConfig, ScoringStrategy};
use crate::constants::*;
use crate::error::{AppError, Result};
use crate::models::{Coordinates, Poi, RoutePreferences};
use rand::{rngs::StdRng, seq::IndexedRandom, SeedableRng};

use super::geometry::angle_from_start;
use super::scoring_strategy::{
    AdvancedStrategy, PoiScoringStrategy, ScoringContext, SimpleStrategy,
};
