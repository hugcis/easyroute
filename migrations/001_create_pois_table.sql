-- Enable PostGIS extension
CREATE EXTENSION IF NOT EXISTS postgis;

-- POIs table
CREATE TABLE pois (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    category VARCHAR(50) NOT NULL,
    location GEOGRAPHY(POINT, 4326) NOT NULL,  -- PostGIS geography type (WGS84)
    popularity_score REAL DEFAULT 50.0 CHECK (popularity_score >= 0 AND popularity_score <= 100),  -- REAL = FLOAT4 = f32
    description TEXT,
    estimated_visit_duration_minutes INTEGER,
    osm_id BIGINT UNIQUE,  -- OpenStreetMap ID for deduplication
    metadata JSONB,  -- Flexible additional data
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Spatial index for fast radius queries (most important!)
CREATE INDEX idx_pois_location ON pois USING GIST(location);

-- Regular indexes for filtering
CREATE INDEX idx_pois_category ON pois(category);
CREATE INDEX idx_pois_popularity ON pois(popularity_score);
CREATE INDEX idx_pois_osm_id ON pois(osm_id) WHERE osm_id IS NOT NULL;

-- Routes cache table (optional - can use Redis only, but helpful for analytics)
CREATE TABLE cached_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cache_key VARCHAR(255) UNIQUE NOT NULL,
    route_data JSONB NOT NULL,  -- Serialized route object
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    access_count INTEGER DEFAULT 0,
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_cached_routes_key ON cached_routes(cache_key);
CREATE INDEX idx_cached_routes_last_accessed ON cached_routes(last_accessed_at);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update updated_at on pois
CREATE TRIGGER update_pois_updated_at
    BEFORE UPDATE ON pois
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
