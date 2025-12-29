#!/bin/bash
# Import OSM data into PostgreSQL using osm2pgsql
# Usage: ./import_osm.sh <path-to-pbf-file> [--append]
# Example: ./import_osm.sh ./osm/data/monaco-latest.osm.pbf

set -euo pipefail

# Configuration
PBF_FILE="${1:-}"
MODE="${2:-create}"  # 'create' or 'append'
SCRIPT_DIR="$(dirname "$0")"
STYLE_FILE="$SCRIPT_DIR/osm_poi_style.lua"

# Database connection (matches docker-compose.yml)
DB_HOST="${POSTGRES_HOST:-localhost}"
DB_PORT="${POSTGRES_PORT:-5432}"
DB_NAME="${POSTGRES_DB:-easyroute}"
DB_USER="${POSTGRES_USER:-easyroute_user}"
DB_PASSWORD="${POSTGRES_PASSWORD:-easyroute_pass}"

# osm2pgsql settings
OSM2PGSQL_CACHE="${OSM2PGSQL_CACHE:-2000}"  # MB of RAM to use (increase for large imports)
OSM2PGSQL_PROCESSES="${OSM2PGSQL_PROCESSES:-4}"  # Number of parallel processes

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Help message
if [ -z "$PBF_FILE" ]; then
    echo -e "${RED}Error: No PBF file specified${NC}"
    echo ""
    echo "Usage: $0 <path-to-pbf-file> [--append]"
    echo ""
    echo "Examples:"
    echo "  $0 ./osm/data/monaco-latest.osm.pbf"
    echo "  $0 ./osm/data/france-latest.osm.pbf"
    echo "  $0 ./osm/data/update.osc.gz --append   # For incremental updates"
    echo ""
    echo "Options:"
    echo "  --append    Append to existing data (for updates)"
    echo "  --create    Create/replace data (default)"
    echo ""
    echo "Environment variables:"
    echo "  POSTGRES_HOST          Database host (default: localhost)"
    echo "  POSTGRES_PORT          Database port (default: 5432)"
    echo "  POSTGRES_DB            Database name (default: easyroute)"
    echo "  POSTGRES_USER          Database user (default: easyroute_user)"
    echo "  POSTGRES_PASSWORD      Database password (default: easyroute_pass)"
    echo "  OSM2PGSQL_CACHE        RAM cache in MB (default: 2000)"
    echo "  OSM2PGSQL_PROCESSES    Parallel processes (default: 4)"
    exit 1
fi

# Check if PBF file exists
if [ ! -f "$PBF_FILE" ]; then
    echo -e "${RED}Error: File not found: $PBF_FILE${NC}"
    exit 1
fi

# Check if style file exists
if [ ! -f "$STYLE_FILE" ]; then
    echo -e "${RED}Error: Style file not found: $STYLE_FILE${NC}"
    exit 1
fi

# Parse mode flag
if [ "$MODE" = "--append" ] || [ "$MODE" = "append" ]; then
    MODE="append"
    OSM2PGSQL_MODE_FLAG="--append"
else
    MODE="create"
    OSM2PGSQL_MODE_FLAG="--create"
fi

# Display configuration
echo -e "${BLUE}===================================${NC}"
echo -e "${BLUE}OSM Data Import (osm2pgsql)${NC}"
echo -e "${BLUE}===================================${NC}"
echo ""
echo -e "Input file:    ${GREEN}$PBF_FILE${NC}"
echo -e "Mode:          ${GREEN}$MODE${NC}"
echo -e "Style:         ${GREEN}$STYLE_FILE${NC}"
echo -e "Database:      ${GREEN}$DB_USER@$DB_HOST:$DB_PORT/$DB_NAME${NC}"
echo -e "Cache:         ${GREEN}${OSM2PGSQL_CACHE}MB${NC}"
echo -e "Processes:     ${GREEN}$OSM2PGSQL_PROCESSES${NC}"
echo ""

# Check if PostgreSQL is accessible
echo -e "${BLUE}Checking database connection...${NC}"
export PGPASSWORD="$DB_PASSWORD"
if ! psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${RED}Error: Cannot connect to PostgreSQL${NC}"
    echo -e "${YELLOW}Make sure PostgreSQL is running: docker-compose up -d postgres${NC}"
    exit 1
fi
echo -e "${GREEN}Database connection successful${NC}"
echo ""

# Check if PostGIS is enabled
echo -e "${BLUE}Checking PostGIS extension...${NC}"
if ! psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT PostGIS_version();" > /dev/null 2>&1; then
    echo -e "${RED}Error: PostGIS extension not found${NC}"
    echo -e "${YELLOW}Run migrations first: sqlx migrate run${NC}"
    exit 1
fi
echo -e "${GREEN}PostGIS extension found${NC}"
echo ""

# Check if osm2pgsql slim tables exist (indicates previous osm2pgsql import)
echo -e "${BLUE}Checking for previous osm2pgsql imports...${NC}"
if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1 FROM planet_osm_nodes LIMIT 1" > /dev/null 2>&1; then
    echo -e "${GREEN}Found osm2pgsql slim tables from previous import${NC}"

    # Check if pois table schema matches new style (has 'id' column)
    if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT id FROM pois LIMIT 1" > /dev/null 2>&1; then
        echo -e "${GREEN}pois table schema is compatible - can use append mode${NC}"
        # Schema matches, use append mode
        if [ "$MODE" = "create" ]; then
            echo -e "${YELLOW}Switching to append mode for update${NC}"
            MODE="append"
            OSM2PGSQL_MODE_FLAG="--append"
        fi
        POI_COUNT_BEFORE=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM pois")
        echo -e "Current POI count: ${YELLOW}${POI_COUNT_BEFORE// /}${NC}"
    else
        echo -e "${YELLOW}pois table schema has changed - forcing fresh import${NC}"
        # Schema changed, need to recreate everything
        MODE="create"
        OSM2PGSQL_MODE_FLAG="--create"
        POI_COUNT_BEFORE=0

        echo -e "${YELLOW}Dropping osm2pgsql tables to start fresh...${NC}"
        psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" > /dev/null 2>&1 <<DROPSQL
DROP TABLE IF EXISTS planet_osm_nodes CASCADE;
DROP TABLE IF EXISTS planet_osm_ways CASCADE;
DROP TABLE IF EXISTS planet_osm_rels CASCADE;
DROP TABLE IF EXISTS pois CASCADE;
DROP TABLE IF EXISTS osm2pgsql_properties CASCADE;
DROPSQL
        echo -e "${GREEN}Old tables dropped, ready for fresh import${NC}"
    fi
else
    echo -e "${YELLOW}No previous osm2pgsql import found - first import${NC}"
    # First import: must use create mode and drop existing pois table
    MODE="create"
    OSM2PGSQL_MODE_FLAG="--create"
    POI_COUNT_BEFORE=0

    # Drop the pois table created by migrations so osm2pgsql can create it
    echo -e "${YELLOW}Dropping pois table (if exists) so osm2pgsql can create it...${NC}"
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "DROP TABLE IF EXISTS pois CASCADE" > /dev/null 2>&1
    echo -e "${GREEN}Ready for first import${NC}"
fi
echo ""

    # Run osm2pgsql
    echo -e "${BLUE}Running osm2pgsql...${NC}"
    echo -e "${YELLOW}This may take a while depending on file size...${NC}"
    echo ""

    # Build osm2pgsql command
    # Note: We use slim mode for updates, and output=flex for Lua style
    # We use --append mode to preserve the existing table structure from migrations
    OSM2PGSQL_CMD="osm2pgsql \
        $OSM2PGSQL_MODE_FLAG \
        --output=flex \
        --style=$STYLE_FILE \
        --database=$DB_NAME \
        --username=$DB_USER \
        --host=$DB_HOST \
        --port=$DB_PORT \
        --cache=$OSM2PGSQL_CACHE \
        --number-processes=$OSM2PGSQL_PROCESSES \
        --slim \
        --verbose \
        --log-progress=true \
        --log-level=info \
        $PBF_FILE"

    # Check if osm2pgsql is available
    if command -v osm2pgsql &> /dev/null; then
        # Run locally
        echo -e "${GREEN}Using local osm2pgsql installation${NC}"
        echo ""
        eval "$OSM2PGSQL_CMD"
    else
        # Run via Docker
        echo -e "${GREEN}Using Docker osm2pgsql (local installation not found)${NC}"
        echo ""

        # Get absolute paths for Docker volume mounts
        ABS_PBF_FILE=$(realpath "$PBF_FILE")
        ABS_STYLE_FILE=$(realpath "$STYLE_FILE")

        docker run --rm \
            --network host \
            -v "$ABS_PBF_FILE:/data/input.osm.pbf:ro" \
            -v "$ABS_STYLE_FILE:/style.lua:ro" \
            -e PGPASSWORD="$DB_PASSWORD" \
            iboates/osm2pgsql:latest \
            $OSM2PGSQL_MODE_FLAG \
            --output=flex \
            --style=/style.lua \
            --database="$DB_NAME" \
            --username="$DB_USER" \
            --host="$DB_HOST" \
            --port="$DB_PORT" \
            --cache="$OSM2PGSQL_CACHE" \
            --number-processes="$OSM2PGSQL_PROCESSES" \
            --slim \
            --verbose \
            --log-progress=true \
            --log-level=info \
            /data/input.osm.pbf
    fi

# Ensure table schema matches application expectations
echo ""
echo -e "${BLUE}Ensuring table schema is correct...${NC}"
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" <<EOF
-- Add missing columns that osm2pgsql doesn't create
DO \$\$
BEGIN
    -- Add id column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'pois' AND column_name = 'id'
    ) THEN
        RAISE NOTICE 'Adding id column with UUID default...';
        ALTER TABLE pois ADD COLUMN id uuid DEFAULT gen_random_uuid() NOT NULL;
    END IF;

    -- Add created_at column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'pois' AND column_name = 'created_at'
    ) THEN
        RAISE NOTICE 'Adding created_at column...';
        ALTER TABLE pois ADD COLUMN created_at timestamp with time zone DEFAULT NOW();
    END IF;

    -- Add updated_at column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'pois' AND column_name = 'updated_at'
    ) THEN
        RAISE NOTICE 'Adding updated_at column...';
        ALTER TABLE pois ADD COLUMN updated_at timestamp with time zone DEFAULT NOW();
    END IF;
END
\$\$;

-- Convert location from geometry to geography if needed
DO \$\$
BEGIN
    -- Check if location is geometry type
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'pois'
        AND column_name = 'location'
        AND udt_name = 'geometry'
    ) THEN
        RAISE NOTICE 'Converting location from geometry to geography...';
        -- Drop existing spatial index
        DROP INDEX IF EXISTS idx_pois_location;
        -- Convert column type
        ALTER TABLE pois ALTER COLUMN location TYPE geography(POINT, 4326) USING location::geography;
    END IF;
END
\$\$;

-- Ensure id column has DEFAULT if it doesn't (for create mode)
ALTER TABLE pois ALTER COLUMN id SET DEFAULT gen_random_uuid();

-- Populate any NULL ids with UUIDs (shouldn't happen with new Lua style, but safe fallback)
UPDATE pois SET id = gen_random_uuid() WHERE id IS NULL;

-- Ensure NOT NULL constraint exists
ALTER TABLE pois ALTER COLUMN id SET NOT NULL;

-- Make id the primary key if it isn't already
DO \$\$
BEGIN
    -- Drop existing primary key on osm_id if it exists
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pois_pkey' AND conrelid = 'pois'::regclass) THEN
        ALTER TABLE pois DROP CONSTRAINT IF EXISTS pois_pkey;
    END IF;

    -- Add primary key on id
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'pois_pkey' AND conrelid = 'pois'::regclass) THEN
        ALTER TABLE pois ADD PRIMARY KEY (id);
    END IF;
END
\$\$;

-- Spatial index for fast radius queries (most important!)
CREATE INDEX IF NOT EXISTS idx_pois_location ON pois USING GIST(location);

-- Regular indexes for filtering
CREATE INDEX IF NOT EXISTS idx_pois_category ON pois(category);
CREATE INDEX IF NOT EXISTS idx_pois_popularity ON pois(popularity_score);
CREATE INDEX IF NOT EXISTS idx_pois_osm_id ON pois(osm_id) WHERE osm_id IS NOT NULL;
EOF
echo -e "${GREEN}Table schema corrected${NC}"

# Check import results
echo ""
echo -e "${BLUE}Checking import results...${NC}"
POI_COUNT_AFTER=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM pois")
POIS_IMPORTED=$((POI_COUNT_AFTER - POI_COUNT_BEFORE))

echo -e "${GREEN}===================================${NC}"
echo -e "${GREEN}Import complete!${NC}"
echo -e "${GREEN}===================================${NC}"
echo ""
echo -e "POIs before:   ${YELLOW}${POI_COUNT_BEFORE// /}${NC}"
echo -e "POIs after:    ${YELLOW}${POI_COUNT_AFTER// /}${NC}"
echo -e "POIs imported: ${GREEN}${POIS_IMPORTED}${NC}"
echo ""

# Show category breakdown
echo -e "${BLUE}Category breakdown:${NC}"
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "
    SELECT category, COUNT(*) as count
    FROM pois
    GROUP BY category
    ORDER BY count DESC
    LIMIT 15;
"

echo ""
echo -e "${GREEN}Done! Your POI database is ready.${NC}"
echo -e "Test with: ${YELLOW}cargo run${NC}"
