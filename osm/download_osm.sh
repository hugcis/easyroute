#!/bin/bash
# Download OSM extracts from Geofabrik for importing into easyroute
# Usage: ./download_osm.sh <region>
# Example: ./download_osm.sh france
#          ./download_osm.sh europe/monaco
#          ./download_osm.sh europe/france/ile-de-france

set -euo pipefail

# Configuration
GEOFABRIK_BASE="https://download.geofabrik.de"
DATA_DIR="$(dirname "$0")/data"
REGION="${1:-}"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Help message
if [ -z "$REGION" ]; then
    echo -e "${RED}Error: No region specified${NC}"
    echo ""
    echo "Usage: $0 <region>"
    echo ""
    echo "Examples:"
    echo "  $0 monaco                           # Small test dataset (~2MB)"
    echo "  $0 europe/france                    # Full France (~3.8GB)"
    echo "  $0 europe/france/ile-de-france      # Paris region (~300MB)"
    echo "  $0 europe/germany/berlin            # Berlin (~80MB)"
    echo "  $0 north-america/us/california      # California (~400MB)"
    echo ""
    echo "Available regions: https://download.geofabrik.de/"
    echo "  - europe/monaco        (tiny, for testing)"
    echo "  - europe/france/ile-de-france"
    echo "  - europe/france"
    echo "  - europe/germany/berlin"
    echo "  - north-america/us/california"
    echo "  - europe                (entire continent, ~27GB)"
    exit 1
fi

# Normalize region path
REGION_PATH="$REGION"
if [[ ! "$REGION" =~ europe|north-america|south-america|africa|asia|australia-oceania ]]; then
    # Assume it's a country in Europe (most common case)
    REGION_PATH="europe/$REGION"
fi

# Construct URLs
PBF_URL="${GEOFABRIK_BASE}/${REGION_PATH}-latest.osm.pbf"
MD5_URL="${GEOFABRIK_BASE}/${REGION_PATH}-latest.osm.pbf.md5"
STATE_URL="${GEOFABRIK_BASE}/${REGION_PATH}-updates/state.txt"

# Extract filename
FILENAME="$(basename "$REGION_PATH")-latest.osm.pbf"
MD5_FILE="${FILENAME}.md5"
STATE_FILE="$(basename "$REGION_PATH").state.txt"

# Create data directory
mkdir -p "$DATA_DIR"

echo -e "${BLUE}===================================${NC}"
echo -e "${BLUE}OSM Data Download from Geofabrik${NC}"
echo -e "${BLUE}===================================${NC}"
echo ""
echo -e "Region:        ${GREEN}$REGION_PATH${NC}"
echo -e "Download URL:  ${YELLOW}$PBF_URL${NC}"
echo -e "Output file:   ${GREEN}$DATA_DIR/$FILENAME${NC}"
echo ""

# Check if file already exists
if [ -f "$DATA_DIR/$FILENAME" ]; then
    echo -e "${YELLOW}File already exists: $DATA_DIR/$FILENAME${NC}"
    read -p "Re-download? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${GREEN}Using existing file.${NC}"
        exit 0
    fi
fi

# Download the PBF file
echo -e "${BLUE}Downloading OSM data...${NC}"
if ! wget -c -O "$DATA_DIR/$FILENAME" "$PBF_URL"; then
    echo -e "${RED}Failed to download OSM data${NC}"
    echo -e "${YELLOW}Check if region exists: https://download.geofabrik.de/${NC}"
    exit 1
fi

# Download the MD5 checksum
echo ""
echo -e "${BLUE}Downloading MD5 checksum...${NC}"
if wget -q -O "$DATA_DIR/$MD5_FILE" "$MD5_URL"; then
    # Verify checksum
    echo -e "${BLUE}Verifying checksum...${NC}"
    cd "$DATA_DIR"
    if md5sum -c "$MD5_FILE"; then
        echo -e "${GREEN}Checksum verified successfully!${NC}"
    else
        echo -e "${RED}Checksum verification failed!${NC}"
        echo -e "${YELLOW}The file may be corrupted. Try re-downloading.${NC}"
        exit 1
    fi
    cd - > /dev/null
else
    echo -e "${YELLOW}Warning: Could not download MD5 checksum, skipping verification${NC}"
fi

# Download state file (for updates)
echo ""
echo -e "${BLUE}Downloading state file (for future updates)...${NC}"
if wget -q -O "$DATA_DIR/$STATE_FILE" "$STATE_URL"; then
    echo -e "${GREEN}State file downloaded${NC}"
else
    echo -e "${YELLOW}Warning: Could not download state file (updates may not work)${NC}"
fi

# Display file info
FILE_SIZE=$(du -h "$DATA_DIR/$FILENAME" | cut -f1)
echo ""
echo -e "${GREEN}===================================${NC}"
echo -e "${GREEN}Download complete!${NC}"
echo -e "${GREEN}===================================${NC}"
echo ""
echo -e "File: ${GREEN}$DATA_DIR/$FILENAME${NC}"
echo -e "Size: ${GREEN}$FILE_SIZE${NC}"
echo ""
echo -e "Next steps:"
echo -e "  1. Ensure PostgreSQL is running:  ${YELLOW}docker-compose up -d postgres${NC}"
echo -e "  2. Import the data:                ${YELLOW}./osm/import_osm.sh $DATA_DIR/$FILENAME${NC}"
echo ""
