CREATE TABLE evaluated_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Request context
    start_lat DOUBLE PRECISION NOT NULL,
    start_lng DOUBLE PRECISION NOT NULL,
    target_distance_km DOUBLE PRECISION NOT NULL,
    transport_mode VARCHAR(20) NOT NULL,
    -- Route data
    actual_distance_km DOUBLE PRECISION NOT NULL,
    duration_minutes INTEGER NOT NULL,
    path JSONB NOT NULL,
    poi_names JSONB NOT NULL,
    poi_count INTEGER NOT NULL,
    snapped_poi_count INTEGER NOT NULL,
    -- Computed metrics
    circularity REAL,
    convexity REAL,
    path_overlap_pct REAL,
    poi_density_per_km REAL,
    category_entropy REAL,
    landmark_coverage REAL,
    system_score REAL NOT NULL,
    poi_density_context VARCHAR(20),
    -- Meta
    scoring_strategy VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE route_ratings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    route_id UUID NOT NULL REFERENCES evaluated_routes(id) ON DELETE CASCADE,
    overall_rating SMALLINT NOT NULL CHECK (overall_rating BETWEEN 1 AND 5),
    shape_rating SMALLINT CHECK (shape_rating BETWEEN 1 AND 5),
    scenicness_rating SMALLINT CHECK (scenicness_rating BETWEEN 1 AND 5),
    variety_rating SMALLINT CHECK (variety_rating BETWEEN 1 AND 5),
    comment TEXT,
    rater_id VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_evaluated_routes_created_at ON evaluated_routes(created_at DESC);
CREATE INDEX idx_route_ratings_route_id ON route_ratings(route_id);
