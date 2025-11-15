#!/bin/bash
# Download and apply OSM updates incrementally
# This script downloads daily/weekly diff files from Geofabrik and applies them
# Usage: ./update_osm.sh <region>
# Example: ./update_osm.sh france

set -euo pipefail

# Configuration
REGION="${1:-}"
SCRIPT_DIR="$(dirname "$0")"
DATA_DIR="$SCRIPT_DIR/data"
GEOFABRIK_BASE="https://download.geofabrik.de"

# Database connection
DB_HOST="${POSTGRES_HOST:-localhost}"
DB_PORT="${POSTGRES_PORT:-5432}"
DB_NAME="${POSTGRES_DB:-easyroute}"
DB_USER="${POSTGRES_USER:-easyroute_user}"
DB_PASSWORD="${POSTGRES_PASSWORD:-easyroute_pass}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Help message
if [ -z "$REGION" ]; then
    echo -e "${RED}Error: No region specified${NC}"
    echo ""
    echo "Usage: $0 <region>"
    echo ""
    echo "Examples:"
    echo "  $0 monaco"
    echo "  $0 europe/france"
    echo "  $0 europe/france/ile-de-france"
    echo ""
    echo "Note: The region must match the one used for initial import"
    exit 1
fi

# Normalize region path
REGION_PATH="$REGION"
if [[ ! "$REGION" =~ europe|north-america|south-america|africa|asia|australia-oceania ]]; then
    REGION_PATH="europe/$REGION"
fi

# State tracking
STATE_FILE="$DATA_DIR/$(basename "$REGION_PATH").state.txt"
UPDATE_DIR="$DATA_DIR/$(basename "$REGION_PATH")-updates"
mkdir -p "$UPDATE_DIR"

echo -e "${BLUE}===================================${NC}"
echo -e "${BLUE}OSM Incremental Update${NC}"
echo -e "${BLUE}===================================${NC}"
echo ""
echo -e "Region:        ${GREEN}$REGION_PATH${NC}"
echo -e "State file:    ${GREEN}$STATE_FILE${NC}"
echo -e "Update dir:    ${GREEN}$UPDATE_DIR${NC}"
echo ""

# Check if we have a state file
if [ ! -f "$STATE_FILE" ]; then
    echo -e "${RED}Error: State file not found${NC}"
    echo -e "${YELLOW}You need to run the initial import first:${NC}"
    echo -e "${YELLOW}  ./osm/download_osm.sh $REGION${NC}"
    echo -e "${YELLOW}  ./osm/import_osm.sh ./osm/data/$(basename "$REGION_PATH")-latest.osm.pbf${NC}"
    exit 1
fi

# Read current state
if [ -f "$STATE_FILE" ]; then
    CURRENT_TIMESTAMP=$(grep 'timestamp' "$STATE_FILE" | cut -d= -f2 | tr -d '\\' | sed 's/T/ /' | sed 's/Z//')
    echo -e "Current data timestamp: ${YELLOW}$CURRENT_TIMESTAMP${NC}"
else
    echo -e "${YELLOW}Warning: Could not read state timestamp${NC}"
fi

# Download latest state
LATEST_STATE_URL="${GEOFABRIK_BASE}/${REGION_PATH}-updates/state.txt"
LATEST_STATE_FILE="$UPDATE_DIR/latest.state.txt"

echo ""
echo -e "${BLUE}Checking for updates...${NC}"
if ! wget -q -O "$LATEST_STATE_FILE" "$LATEST_STATE_URL"; then
    echo -e "${RED}Error: Could not download latest state${NC}"
    echo -e "${YELLOW}This region may not support incremental updates${NC}"
    echo -e "${YELLOW}Try a full re-import instead${NC}"
    exit 1
fi

LATEST_TIMESTAMP=$(grep 'timestamp' "$LATEST_STATE_FILE" | cut -d= -f2 | tr -d '\\' | sed 's/T/ /' | sed 's/Z//')
echo -e "Latest data timestamp:  ${GREEN}$LATEST_TIMESTAMP${NC}"

# Check if update is needed
if [ "$CURRENT_TIMESTAMP" = "$LATEST_TIMESTAMP" ]; then
    echo ""
    echo -e "${GREEN}Database is already up to date!${NC}"
    exit 0
fi

echo ""
echo -e "${YELLOW}Update available!${NC}"
echo ""

# For simplicity, we'll do a full re-download and re-import
# True incremental updates with osmosis/pyosmium are more complex
# and require maintaining sequence numbers

echo -e "${BLUE}Downloading latest extract...${NC}"
PBF_FILENAME="$(basename "$REGION_PATH")-latest.osm.pbf"
PBF_URL="${GEOFABRIK_BASE}/${REGION_PATH}-latest.osm.pbf"

if ! wget -c -O "$DATA_DIR/$PBF_FILENAME.new" "$PBF_URL"; then
    echo -e "${RED}Download failed${NC}"
    exit 1
fi

# Backup current database POIs count
export PGPASSWORD="$DB_PASSWORD"
POI_COUNT_BEFORE=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM pois")

echo ""
echo -e "${BLUE}Current POI count: ${POI_COUNT_BEFORE// /}${NC}"
echo ""
echo -e "${YELLOW}Clearing existing POIs (we'll re-import everything)...${NC}"
echo -e "${YELLOW}Note: For production, consider incremental updates with osmosis${NC}"

# Clear old POIs (alternative: keep them and use ON CONFLICT)
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "TRUNCATE TABLE pois;"

# Import new data
echo ""
echo -e "${BLUE}Importing updated data...${NC}"
"$SCRIPT_DIR/import_osm.sh" "$DATA_DIR/$PBF_FILENAME.new"

# Replace old file with new one
mv "$DATA_DIR/$PBF_FILENAME.new" "$DATA_DIR/$PBF_FILENAME"

# Update state file
cp "$LATEST_STATE_FILE" "$STATE_FILE"

# Show results
POI_COUNT_AFTER=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM pois")

echo ""
echo -e "${GREEN}===================================${NC}"
echo -e "${GREEN}Update complete!${NC}"
echo -e "${GREEN}===================================${NC}"
echo ""
echo -e "POIs before:   ${YELLOW}${POI_COUNT_BEFORE// /}${NC}"
echo -e "POIs after:    ${GREEN}${POI_COUNT_AFTER// /}${NC}"
echo -e "Difference:    ${GREEN}$((POI_COUNT_AFTER - POI_COUNT_BEFORE))${NC}"
echo ""
echo -e "Data timestamp: ${GREEN}$LATEST_TIMESTAMP${NC}"
echo ""

# Suggest cron setup
echo -e "${BLUE}Tip: Set up automatic weekly updates with cron:${NC}"
echo -e "${YELLOW}0 2 * * 0 cd $(pwd) && ./osm/update_osm.sh $REGION >> /var/log/osm_update.log 2>&1${NC}"
echo ""
