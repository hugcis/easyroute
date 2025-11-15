use crate::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

/// GET /debug/health - Check if services are working
pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut status = json!({
        "status": "ok",
        "checks": {}
    });

    // Check database
    match sqlx::query("SELECT 1").fetch_one(&state.db_pool).await {
        Ok(_) => {
            status["checks"]["database"] = json!("ok");
        }
        Err(e) => {
            status["checks"]["database"] = json!({"error": e.to_string()});
            status["status"] = json!("error");
        }
    }

    // Check PostGIS extension
    match sqlx::query("SELECT PostGIS_Version()")
        .fetch_one(&state.db_pool)
        .await
    {
        Ok(_) => {
            status["checks"]["postgis"] = json!("ok");
        }
        Err(e) => {
            status["checks"]["postgis"] = json!({"error": e.to_string()});
            status["status"] = json!("error");
        }
    }

    // Check POI count
    match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pois")
        .fetch_one(&state.db_pool)
        .await
    {
        Ok(count) => {
            status["checks"]["poi_count"] = json!(count);
        }
        Err(e) => {
            status["checks"]["poi_count"] = json!({"error": e.to_string()});
        }
    }

    Json(status)
}
