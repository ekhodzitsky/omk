use anyhow::Result;
use axum::{
    routing::get,
    Json, Router,
};
use serde_json::Value;
use std::net::SocketAddr;
use tracing::info;

pub async fn run_server(port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/teams", get(teams_handler))
        .route("/api/metrics", get(metrics_handler))
        .route("/api/health", get(health_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("Starting web dashboard on http://{}", addr);
    println!("🌐 omk vis running on http://{}", addr);
    println!("Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler() -> &'static str {
    "omk vis - Web dashboard for oh-my-kimi\n\nEndpoints:\n  GET /api/teams    - List active teams\n  GET /api/metrics  - Metrics data\n  GET /api/health   - Health check\n"
}

async fn teams_handler() -> Json<Value> {
    let state_dir = crate::runtime::config::state_dir().join("team");
    let mut teams = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&state_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let team_state = entry.path().join("team-state.json");
            if let Ok(content) = tokio::fs::read_to_string(&team_state).await {
                if let Ok(value) = serde_json::from_str::<Value>(&content) {
                    teams.push(value);
                }
            }
        }
    }

    Json(serde_json::json!({ "teams": teams }))
}

async fn metrics_handler() -> Json<Value> {
    let metrics_path = crate::runtime::config::state_dir().join("metrics.json");
    let metrics = if let Ok(content) = tokio::fs::read_to_string(&metrics_path).await {
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!(null))
    } else {
        serde_json::json!(null)
    };

    Json(serde_json::json!({ "metrics": metrics }))
}

async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
