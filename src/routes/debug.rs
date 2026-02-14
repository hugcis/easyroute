use crate::db::queries;
use crate::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;

/// GET /debug/health - Check if services are working (works with any backend)
pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut status = json!({
        "status": "ok",
        "checks": {}
    });

    // Database check via PoiRepository (works with any backend)
    match state.poi_repo.count().await {
        Ok(count) => {
            status["checks"]["database"] = json!("ok");
            status["checks"]["poi_count"] = json!(count);
        }
        Err(e) => {
            status["checks"]["database"] = json!({"error": e.to_string()});
            status["status"] = json!("error");
        }
    }

    // Check cache
    if let Some(ref cache) = state.cache {
        if cache.health_check().await {
            let stats = cache.get_stats().await;
            status["checks"]["cache"] = json!({
                "status": "ok",
                "backend": cache.backend_name(),
                "hits": stats.hits,
                "misses": stats.misses,
                "hit_rate": format!("{:.1}%", stats.hit_rate)
            });
        } else {
            status["checks"]["cache"] = json!({"status": "error", "backend": cache.backend_name(), "message": "Cache connection failed"});
            status["status"] = json!("degraded");
        }
    } else {
        status["checks"]["cache"] = json!({"status": "not_configured"});
    }

    Json(status)
}

/// GET /debug/coverage - Return convex hull of all POIs as GeoJSON (PostgreSQL only)
pub async fn data_coverage(State(pool): State<PgPool>) -> Json<Value> {
    match queries::get_poi_coverage(&pool).await {
        Ok((geojson_str, poi_count, cluster_count)) => {
            let coverage = geojson_str.and_then(|s| serde_json::from_str::<Value>(&s).ok());

            Json(json!({
                "poi_count": poi_count,
                "cluster_count": cluster_count,
                "coverage": coverage
            }))
        }
        Err(e) => Json(json!({
            "error": e.to_string()
        })),
    }
}
