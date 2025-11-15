# OSM Import Quick Start Guide

This guide will help you get started with importing OSM data to eliminate Overpass API dependency.

## Prerequisites

1. PostgreSQL + PostGIS running (via docker-compose)
2. Database migrations applied
3. ~500MB free disk space (for France region)

## Quick Start (5 minutes with Monaco)

Monaco is perfect for testing - it's tiny (~2MB) but has all the POI types you need to verify the import works.

```bash
# 1. Start PostgreSQL
docker-compose up -d postgres

# 2. Run migrations (if not done already)
sqlx migrate run

# 3. Download Monaco OSM data (~2MB)
./osm/download_osm.sh monaco

# 4. Import into database (~30 seconds)
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf

# 5. Verify import
psql -U easyroute_user -h localhost -d easyroute -c "
    SELECT category, COUNT(*) as count
    FROM pois
    GROUP BY category
    ORDER BY count DESC;
"

# Expected output: ~500-1000 POIs across various categories
```

## Production Setup (France)

For production use in France (or your target region):

```bash
# 1. Download France extract (~3.8GB, takes 10-30 min depending on connection)
./osm/download_osm.sh europe/france

# 2. Import into database (~5-15 minutes depending on your hardware)
#    This will import ~800,000 POIs
./osm/import_osm.sh ./osm/data/france-latest.osm.pbf

# 3. Verify
psql -U easyroute_user -h localhost -d easyroute -c "SELECT COUNT(*) FROM pois;"
# Expected: 700,000 - 900,000 POIs
```

## Regional Imports (Faster)

If you only care about specific regions (like Paris), download just that area:

```bash
# Paris region only (~300MB, ~150,000 POIs)
./osm/download_osm.sh europe/france/ile-de-france
./osm/import_osm.sh ./osm/data/ile-de-france-latest.osm.pbf

# Berlin (~80MB, ~50,000 POIs)
./osm/download_osm.sh europe/germany/berlin
./osm/import_osm.sh ./osm/data/berlin-latest.osm.pbf
```

## Weekly Updates

Set up automatic weekly updates to keep your POI data fresh:

```bash
# Manual update
./osm/update_osm.sh france

# Automated with cron (every Sunday at 2 AM)
crontab -e
# Add this line:
0 2 * * 0 cd /path/to/easyroute && ./osm/update_osm.sh france >> /var/log/osm_update.log 2>&1
```

## Verify It Works

After importing, test that the API now uses local data instead of Overpass:

```bash
# Start the API
cargo run

# Test a route in Paris (previously timed out with Overpass)
curl -X POST http://localhost:3000/api/routes/loop \
  -H "Content-Type: application/json" \
  -d '{
    "start": {"lat": 48.8566, "lng": 2.3522},
    "distance_km": 5.0,
    "mode": "walking"
  }'
```

Watch the logs - you should see:
```
Found 150+ POIs in database within 2.5km (min threshold: 12)
```

Instead of:
```
Fetching POIs from Overpass API...
```

## Troubleshooting

### Import fails with "osm2pgsql: command not found"

The script automatically uses Docker if osm2pgsql isn't installed locally. Make sure Docker is running:

```bash
docker ps  # Should show postgres container
```

### "Cannot connect to PostgreSQL"

```bash
# Start PostgreSQL
docker-compose up -d postgres

# Check it's running
docker-compose ps postgres
```

### "PostGIS extension not found"

```bash
# Run migrations
sqlx migrate run
```

### Import succeeds but 0 POIs imported

Check the Lua style file is working:

```bash
# Look for errors in the import output
./osm/import_osm.sh ./osm/data/monaco-latest.osm.pbf 2>&1 | grep -i error
```

### POIs imported but API still uses Overpass

Check the `find_pois` threshold in `src/services/poi_service.rs`. The minimum POI count might be too high for your test area.

## Performance Tuning

For large imports (France, Europe), you can tune osm2pgsql:

```bash
# Use more RAM cache (default: 2GB)
OSM2PGSQL_CACHE=8000 ./osm/import_osm.sh ./osm/data/france-latest.osm.pbf

# Use more CPU cores (default: 4)
OSM2PGSQL_PROCESSES=8 ./osm/import_osm.sh ./osm/data/france-latest.osm.pbf
```

## Disk Space

| Region | Download Size | Database Size | Total |
|--------|---------------|---------------|-------|
| Monaco | 2 MB | 0.1 MB | ~2 MB |
| Ile-de-France (Paris) | 300 MB | 15 MB | ~315 MB |
| France | 3.8 GB | 80 MB | ~3.9 GB |
| Europe | 27 GB | 800 MB | ~28 GB |

## Available Regions

Full list: https://download.geofabrik.de/

Common ones:
- `monaco` - Tiny test dataset
- `europe/france/ile-de-france` - Paris region
- `europe/france` - All of France
- `europe/germany/berlin` - Berlin
- `europe/spain` - Spain
- `north-america/us/california` - California
- `north-america/us/new-york` - New York state

## Next Steps

After importing:

1. **Remove Overpass fallback** (optional): Edit `poi_service.rs` to remove the Overpass API calls entirely
2. **Set up weekly updates**: Add a cron job to keep data fresh
3. **Monitor database size**: Set up alerts if `pois` table grows unexpectedly
4. **Optimize queries**: Add additional indexes if you filter by specific categories frequently

## Going Further

### Custom Category Mapping

Edit `osm/osm_poi_style.lua` to:
- Add new POI categories
- Adjust popularity score calculations
- Filter out unwanted POI types

### True Incremental Updates

The current `update_osm.sh` does a full re-import. For true incremental updates with OSM diff files:

1. Use `osmosis` or `pyosmium` for diff processing
2. Track sequence numbers in `state.txt`
3. Apply daily diffs instead of weekly full imports

See: https://wiki.openstreetmap.org/wiki/Osmosis/Detailed_Usage

### Multiple Regions

You can import multiple regions into the same database:

```bash
./osm/download_osm.sh europe/france
./osm/download_osm.sh europe/germany
./osm/import_osm.sh ./osm/data/france-latest.osm.pbf
./osm/import_osm.sh ./osm/data/germany-latest.osm.pbf
```

The `osm_id` ensures no duplicates at borders.

## Support

- OSM Wiki: https://wiki.openstreetmap.org/
- osm2pgsql docs: https://osm2pgsql.org/doc/
- Geofabrik: https://download.geofabrik.de/
- PostGIS docs: https://postgis.net/documentation/
