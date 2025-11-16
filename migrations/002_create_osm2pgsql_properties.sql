-- Create osm2pgsql properties table
-- This table is required by osm2pgsql when running in --append mode (slim mode)
-- It stores metadata about the OSM import state for incremental updates

CREATE TABLE IF NOT EXISTS osm2pgsql_properties (
    property TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

-- No need for indexes - this table is tiny and only used by osm2pgsql
