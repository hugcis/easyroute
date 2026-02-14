//! CLI tool that reads an OSM PBF file and writes a SQLite region database.
//!
//! ```text
//! cargo run --features sqlite --bin build_region -- \
//!     --input osm/data/monaco-latest.osm.pbf \
//!     --output regions/monaco.db
//! ```

use easyroute::db::SqlitePoiRepository;
use easyroute::models::{Coordinates, Poi, PoiCategory};
use easyroute::osm;
use osmpbf::{Element, ElementReader};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use std::{env, fs};
use uuid::Uuid;

const BATCH_SIZE: usize = 1000;
const SCAN_PROGRESS_INTERVAL: usize = 500_000;

/// Try to build a POI from OSM tags and coordinates. Returns `None` if the
/// tags don't have a name or a recognized category.
fn try_build_poi(tags: &HashMap<&str, &str>, id: i64, lat: f64, lon: f64) -> Option<Poi> {
    if !tags.contains_key("name") {
        return None;
    }
    let category = osm::determine_category(tags)?;
    let popularity = osm::calculate_popularity(tags);
    let duration = osm::estimate_duration(tags, &category);
    let description = osm::build_description(tags);
    let name = tags["name"].to_string();
    let coords = Coordinates::new(lat, lon).ok()?;

    Some(Poi {
        id: Uuid::new_v4(),
        name,
        category,
        coordinates: coords,
        popularity_score: popularity,
        description,
        estimated_visit_duration_minutes: Some(duration),
        osm_id: Some(id),
    })
}

/// Format a number with thousands separators (e.g. 1_234_567 -> "1,234,567").
fn fmt_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

fn print_help() {
    eprintln!(
        "\
Usage: build_region [OPTIONS]

Build a SQLite region database from an OSM PBF file.

Options:
  --input=PATH     Path to the .osm.pbf input file (required)
  --output=PATH    Path to the .db output file (required)
  --help           Show this help message"
    );
}

/// A way that needs its node coordinates resolved after the node pass.
struct PendingWay {
    name: String,
    category: PoiCategory,
    popularity: f32,
    description: Option<String>,
    duration: u32,
    osm_id: i64,
    node_refs: Vec<i64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help") {
        print_help();
        return Ok(());
    }

    let input = args
        .iter()
        .find_map(|a| a.strip_prefix("--input="))
        .map(PathBuf::from)
        .ok_or("Missing --input=PATH argument")?;

    let output = args
        .iter()
        .find_map(|a| a.strip_prefix("--output="))
        .map(PathBuf::from)
        .ok_or("Missing --output=PATH argument")?;

    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input.display()).into());
    }

    let source_file_size = fs::metadata(&input)?.len();

    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // Remove existing output file so we start fresh
    if output.exists() {
        fs::remove_file(&output)?;
    }

    let file_size_mb = source_file_size as f64 / (1024.0 * 1024.0);
    eprintln!("Reading PBF: {} ({:.1} MB)", input.display(), file_size_mb);
    eprintln!("Output DB:   {}", output.display());
    eprintln!();

    let t_total = Instant::now();

    // ── Phase 1: Read PBF ───────────────────────────────────
    eprintln!("[1/4] Scanning PBF elements...");
    let t_scan = Instant::now();
    let reader = ElementReader::from_path(&input)?;

    // Node coordinates (id -> (lat, lon))
    let mut node_coords: HashMap<i64, (f64, f64)> = HashMap::new();
    let mut pois: Vec<Poi> = Vec::new();
    let mut pending_ways: Vec<PendingWay> = Vec::new();
    let mut elements_scanned: usize = 0;

    reader.for_each(|element| {
        elements_scanned += 1;
        if elements_scanned % SCAN_PROGRESS_INTERVAL == 0 {
            eprint!(
                "\r      {} elements scanned, {} POIs found so far...",
                fmt_count(elements_scanned),
                fmt_count(pois.len()),
            );
        }
        match element {
            Element::Node(node) => {
                let (id, lat, lon) = (node.id(), node.lat(), node.lon());
                node_coords.insert(id, (lat, lon));
                let tags = osm::collect_tags(node.tags());
                if let Some(poi) = try_build_poi(&tags, id, lat, lon) {
                    pois.push(poi);
                }
            }
            Element::DenseNode(node) => {
                let (id, lat, lon) = (node.id(), node.lat(), node.lon());
                node_coords.insert(id, (lat, lon));
                let tags = osm::collect_tags(node.tags());
                if let Some(poi) = try_build_poi(&tags, id, lat, lon) {
                    pois.push(poi);
                }
            }
            Element::Way(way) => {
                let tags = osm::collect_tags(way.tags());
                if !tags.contains_key("name") {
                    return;
                }
                let refs: Vec<i64> = way.refs().collect();
                // Only process closed ways (areas)
                if refs.len() < 3 || refs.first() != refs.last() {
                    return;
                }
                if let Some(category) = osm::determine_category(&tags) {
                    let popularity = osm::calculate_popularity(&tags);
                    let duration = osm::estimate_duration(&tags, &category);
                    let description = osm::build_description(&tags);
                    let name = tags["name"].to_string();

                    pending_ways.push(PendingWay {
                        name,
                        category,
                        popularity,
                        description,
                        duration,
                        osm_id: way.id(),
                        node_refs: refs,
                    });
                }
            }
            Element::Relation(_) => {} // skip relations
        }
    })?;

    eprintln!(
        "\r      {} elements scanned in {:.1}s — {} node POIs, {} pending ways",
        fmt_count(elements_scanned),
        t_scan.elapsed().as_secs_f64(),
        fmt_count(pois.len()),
        fmt_count(pending_ways.len()),
    );

    // ── Phase 2: Resolve pending ways ───────────────────────
    eprintln!(
        "[2/4] Resolving {} way centroids...",
        fmt_count(pending_ways.len())
    );
    let t_resolve = Instant::now();
    let ways_count = pending_ways.len();
    for way in pending_ways {
        let mut sum_lat = 0.0;
        let mut sum_lon = 0.0;
        let mut resolved = 0usize;

        for nref in &way.node_refs {
            if let Some(&(lat, lon)) = node_coords.get(nref) {
                sum_lat += lat;
                sum_lon += lon;
                resolved += 1;
            }
        }

        if resolved == 0 {
            continue;
        }

        let centroid_lat = sum_lat / resolved as f64;
        let centroid_lon = sum_lon / resolved as f64;

        if let Ok(coords) = Coordinates::new(centroid_lat, centroid_lon) {
            pois.push(Poi {
                id: Uuid::new_v4(),
                name: way.name,
                category: way.category,
                coordinates: coords,
                popularity_score: way.popularity,
                description: way.description,
                estimated_visit_duration_minutes: Some(way.duration),
                osm_id: Some(way.osm_id),
            });
        }
    }

    // Free memory — node_coords no longer needed
    drop(node_coords);

    let total_pois = pois.len();
    eprintln!(
        "      {} ways resolved in {:.1}s — {} total POIs",
        fmt_count(ways_count),
        t_resolve.elapsed().as_secs_f64(),
        fmt_count(total_pois),
    );

    // ── Phase 3: Write SQLite ───────────────────────────────
    eprintln!("[3/4] Writing {} POIs to SQLite...", fmt_count(total_pois));
    let db_url = format!("sqlite:{}?mode=rwc", output.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;

    // WAL mode + performance pragmas for bulk loading
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA cache_size = -65536") // 64MB
        .execute(&pool)
        .await?;

    SqlitePoiRepository::create_schema(&pool).await?;

    let repo = SqlitePoiRepository::new(pool);
    let t_write = Instant::now();

    let mut total_inserted = 0usize;
    for chunk in pois.chunks(BATCH_SIZE) {
        let n = repo.insert_batch(chunk).await?;
        total_inserted += n;
        let pct = (total_inserted * 100) / total_pois.max(1);
        eprint!(
            "\r      {}/{} POIs inserted ({}%)...",
            fmt_count(total_inserted),
            fmt_count(total_pois),
            pct,
        );
    }

    let dupes = total_pois - total_inserted;
    eprintln!(
        "\r      {} POIs written in {:.1}s{}",
        fmt_count(total_inserted),
        t_write.elapsed().as_secs_f64(),
        if dupes > 0 {
            format!(" ({} duplicates skipped)", fmt_count(dupes))
        } else {
            String::new()
        },
    );

    // ── Phase 4: Write metadata ─────────────────────────────
    eprintln!("[4/4] Writing region metadata...");
    let region_name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .trim_end_matches("-latest");

    let build_date = time::OffsetDateTime::now_utc();
    let build_date_str = build_date
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());

    repo.set_meta("region_name", region_name).await?;
    repo.set_meta("build_date", &build_date_str).await?;
    repo.set_meta("poi_count", &total_inserted.to_string())
        .await?;
    repo.set_meta("source_file", &input.display().to_string())
        .await?;
    repo.set_meta("builder_version", env!("CARGO_PKG_VERSION"))
        .await?;
    repo.set_meta("source_file_size_bytes", &source_file_size.to_string())
        .await?;

    let db_size = fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    eprintln!();
    eprintln!(
        "Done in {:.1}s! {} POIs written to {} ({:.1} MB)",
        t_total.elapsed().as_secs_f64(),
        fmt_count(total_inserted),
        output.display(),
        db_size as f64 / (1024.0 * 1024.0),
    );

    Ok(())
}
