# OSM Import Infrastructure

This directory contains tooling for importing and maintaining POI data from OpenStreetMap.

## Overview

Instead of relying on the Overpass API (which has timeout issues in dense areas like Paris), we import OSM data directly into our PostgreSQL/PostGIS database. This provides:

- **No API timeouts**: All data is local
- **Instant queries**: PostGIS spatial indexes
- **Offline capability**: No external dependency
- **Cost control**: No API rate limits

## Architecture

```
OSM Data Flow:
Geofabrik → OSM PBF Extract → osm2pgsql → PostgreSQL (pois table)
                                    ↓
                            OSM Diff Files (weekly updates)
```

## Components

### 1. OSM Tag Mapping (`osm_poi_mapping.lua`)
Lua script for osm2pgsql that:
- Filters OSM nodes/ways by tags (tourism, historic, amenity, etc.)
- Maps OSM tags to our `PoiCategory` enum
- Calculates popularity scores based on OSM metadata
- Extracts relevant attributes (name, description, etc.)

### 2. Download Script (`download_osm.sh`)
Downloads OSM extracts from Geofabrik:
- Supports country/region selection
- Downloads .osm.pbf files (compressed)
- Validates checksums
- Stores in `./data/` directory

### 3. Import Script (`import_osm.sh`)
Runs osm2pgsql to load data:
- Uses the Lua style file
- Connects to PostgreSQL
- Imports POIs into `pois` table
- Creates/updates spatial indexes

### 4. Update Script (`update_osm.sh`)
Incremental updates using OSM diff files:
- Downloads daily/weekly diffs
- Applies changes to existing data
- Keeps data fresh without full re-import

## OSM Tag → Category Mapping

| OSM Tags | Our Category | Popularity Boost |
|----------|--------------|------------------|
| `tourism=monument` | Monument | +20 if `wikipedia` tag |
| `tourism=viewpoint` | Viewpoint | +10 if `ele` (elevation) |
| `leisure=park` | Park | +10 if area > 10 hectares |
| `tourism=museum` | Museum | +20 if `wikipedia` tag |
| `historic=*` | Historic | +15 if `heritage` tag |
| `tourism=attraction` | Cultural | +10 if multiple lang names |
| `waterway=waterfall` | Waterfall | +5 |
| `natural=water` + `water=reservoir` | Waterfront | Base |
| `amenity=place_of_worship` + `building=church` | Church | +5 if `denomination` |
| `historic=castle` | Castle | +20 if `unesco` tag |
| `man_made=tower` | Tower | +5 if `tower:type=observation` |
| `place=square` | Plaza | +10 if in city center |
| `amenity=fountain` | Fountain | Base |
| `amenity=marketplace` | Market | +5 |
| `tourism=artwork` | Artwork | +5 if `artist` tag |
| `man_made=lighthouse` | Lighthouse | +10 |
| `tourism=wine_cellar`, `craft=winery` | Winery | +5 |
| `craft=brewery` | Brewery | +5 |
| `amenity=theatre` | Theatre | +10 |
| `amenity=library` | Library | +5 |

## Popularity Score Calculation

Base score: 50

Boosts:
- `+20`: Wikipedia tag present
- `+15`: UNESCO World Heritage
- `+10`: Multiple language names (>3)
- `+10`: Tourist importance tag
- `+5`: Has opening hours
- `+5`: Has website
- `-20`: Hidden gems (rare tags, no Wikipedia)

Final score: Clamped to 0-100

## Usage

### Initial Import (France example)

```bash
# 1. Download OSM extract
./osm/download_osm.sh france

# 2. Import into database (requires postgres running)
./osm/import_osm.sh ./osm/data/france-latest.osm.pbf

# 3. Verify import
psql -U easyroute_user -d easyroute -c "SELECT category, COUNT(*) FROM pois GROUP BY category ORDER BY count DESC;"
```

### Weekly Updates

```bash
# Download and apply incremental updates
./osm/update_osm.sh france
```

### Start Small (Testing)

```bash
# Monaco is tiny (~2MB), perfect for testing
./osm/download_osm.sh monaco
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf
```

## Storage Requirements

| Region | Compressed (.pbf) | Imported POIs (est.) | Disk Usage |
|--------|-------------------|----------------------|------------|
| Monaco | ~2 MB | ~500 | ~50 KB |
| Ile-de-France (Paris) | ~300 MB | ~150,000 | ~15 MB |
| France | ~3.8 GB | ~800,000 | ~80 MB |
| Europe | ~27 GB | ~8,000,000 | ~800 MB |

*Note: We only store POIs, not roads/buildings, so database size is much smaller than full OSM import*

## Updating POI Service

Once imported, the `poi_service.rs` will automatically use the database instead of falling back to Overpass API. The fallback hierarchy becomes:

1. **Check database** (now has full OSM data) → Success ✓
2. ~~Overpass API~~ → Rarely needed
3. Error only if database is truly empty

## Dependencies

- **osm2pgsql**: OSM data processor (installed via Docker)
- **PostgreSQL + PostGIS**: Database with spatial extensions
- **wget/curl**: For downloading OSM extracts

## Automation

Set up a weekly cron job:

```cron
# Every Sunday at 2 AM, update OSM data
0 2 * * 0 /path/to/easyroute/osm/update_osm.sh france >> /var/log/osm_update.log 2>&1
```

## Geofabrik Regions

Available extracts: https://download.geofabrik.de/

Common regions:
- `europe/france` - Full country
- `europe/france/ile-de-france` - Paris region only
- `europe/monaco` - Tiny test dataset
- `europe` - Entire continent
- `north-america/us/california` - State-level

## Troubleshooting

**Import fails with "relation already exists"**
- osm2pgsql creates its own tables. Use `--drop` flag to reset, or use our Lua style to write to existing `pois` table.

**Popularity scores all 50**
- Check Lua script is calculating boosts correctly
- Verify OSM tags are present in source data

**Missing POIs in area**
- Check category mapping includes your desired tags
- OSM data might be incomplete in that region
- Try Overpass API as fallback for real-time data

**Update script fails**
- Ensure base import matches update source
- Geofabrik provides .state.txt files for tracking
