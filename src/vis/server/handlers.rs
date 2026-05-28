use axum::{http::StatusCode, response::Response, Json};
use serde_json::Value;

pub(super) async fn teams_handler() -> Json<Value> {
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

pub(super) async fn autopilots_handler() -> Json<Value> {
    let state_dir = crate::runtime::config::state_dir().join("autopilot");
    let mut autopilots = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&state_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let ap_state = entry.path().join("autopilot-state.json");
            if let Ok(content) = tokio::fs::read_to_string(&ap_state).await {
                if let Ok(value) = serde_json::from_str::<Value>(&content) {
                    autopilots.push(value);
                }
            }
        }
    }

    Json(serde_json::json!({ "autopilots": autopilots }))
}

pub(super) async fn ralphs_handler() -> Json<Value> {
    let state_dir = crate::runtime::config::state_dir().join("ralph");
    let mut ralphs = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&state_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let ralph_state = entry.path().join("ralph-state.json");
            if let Ok(content) = tokio::fs::read_to_string(&ralph_state).await {
                if let Ok(value) = serde_json::from_str::<Value>(&content) {
                    ralphs.push(value);
                }
            }
        }
    }

    Json(serde_json::json!({ "ralphs": ralphs }))
}

pub(super) async fn metrics_handler() -> Json<Value> {
    let metrics_path = crate::runtime::config::state_dir().join("metrics.json");
    let metrics = if let Ok(content) = tokio::fs::read_to_string(&metrics_path).await {
        serde_json::from_str(&content)
            .map(normalize_metrics_value)
            .unwrap_or_else(|_| serde_json::json!(null))
    } else {
        serde_json::json!(null)
    };

    Json(serde_json::json!({ "metrics": metrics }))
}

pub(super) async fn health_handler() -> Json<Value> {
    let mut checks = serde_json::json!({});
    let mut healthy = true;

    // Check kimi
    let kimi_ok = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::process::Command::new("kimi")
            .arg("--version")
            .output(),
    )
    .await
    {
        Ok(Ok(o)) => o.status.success(),
        _ => false,
    };
    checks["kimi"] = serde_json::json!({"status": if kimi_ok { "ok" } else { "error" } });
    if !kimi_ok {
        healthy = false;
    }

    // Check disk space
    let state_dir = crate::runtime::config::state_dir();
    let disk_ok = check_disk_space(&state_dir).await;
    checks["disk"] = serde_json::json!({
        "status": if disk_ok { "ok" } else { "warning" },
        "path": state_dir.to_string_lossy().to_string(),
    });

    Json(serde_json::json!({
        "status": if healthy { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "checks": checks,
    }))
}

pub(super) async fn check_disk_space(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        if tokio::fs::metadata(path).await.is_ok() {
            // This is a simplified check; in production you'd use statvfs
            return true;
        }
    }
    true
}

pub(super) async fn prometheus_metrics_handler() -> std::result::Result<Response<String>, StatusCode>
{
    let metrics_path = crate::runtime::config::state_dir().join("metrics.json");
    let mut output = String::new();

    output.push_str("# HELP omk_info OMK version info\n");
    output.push_str("# TYPE omk_info gauge\n");
    output.push_str(&format!(
        "omk_info{{version=\"{}\"}} 1\n",
        env!("CARGO_PKG_VERSION")
    ));

    if let Ok(content) = tokio::fs::read_to_string(&metrics_path).await {
        if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(team_runs) = metric_u64(&metrics, "total_team_runs", Some("total_spawns")) {
                output.push_str("\n# HELP omk_team_runs_total Total team run starts\n");
                output.push_str("# TYPE omk_team_runs_total counter\n");
                output.push_str(&format!("omk_team_runs_total {}\n", team_runs));
                output.push_str(
                    "\n# HELP omk_team_spawns_total Legacy alias for omk_team_runs_total\n",
                );
                output.push_str("# TYPE omk_team_spawns_total counter\n");
                output.push_str(&format!("omk_team_spawns_total {}\n", team_runs));
            }
            if let Some(shutdowns) = metrics["total_shutdowns"].as_u64() {
                output.push_str("\n# HELP omk_team_shutdowns_total Total team shutdowns\n");
                output.push_str("# TYPE omk_team_shutdowns_total counter\n");
                output.push_str(&format!("omk_team_shutdowns_total {}\n", shutdowns));
            }
            if let Some(tasks) = metrics["total_tasks_created"].as_u64() {
                output.push_str("\n# HELP omk_tasks_created_total Total tasks created\n");
                output.push_str("# TYPE omk_tasks_created_total counter\n");
                output.push_str(&format!("omk_tasks_created_total {}\n", tasks));
            }
            if let Some(ask) = metrics["total_ask_calls"].as_u64() {
                output.push_str("\n# HELP omk_ask_calls_total Total ask calls\n");
                output.push_str("# TYPE omk_ask_calls_total counter\n");
                output.push_str(&format!("omk_ask_calls_total {}\n", ask));
            }
            if let Some(tasks_completed) = metrics["total_tasks_completed"].as_u64() {
                output.push_str("\n# HELP omk_tasks_completed_total Total tasks completed\n");
                output.push_str("# TYPE omk_tasks_completed_total counter\n");
                output.push_str(&format!("omk_tasks_completed_total {}\n", tasks_completed));
            }
            if let Some(tasks_failed) = metrics["total_tasks_failed"].as_u64() {
                output.push_str("\n# HELP omk_tasks_failed_total Total tasks failed\n");
                output.push_str("# TYPE omk_tasks_failed_total counter\n");
                output.push_str(&format!("omk_tasks_failed_total {}\n", tasks_failed));
            }
            if let Some(ask_errors) = metrics["total_ask_errors"].as_u64() {
                output.push_str("\n# HELP omk_ask_errors_total Total ask errors\n");
                output.push_str("# TYPE omk_ask_errors_total counter\n");
                output.push_str(&format!("omk_ask_errors_total {}\n", ask_errors));
            }
            if let Some(autopilot_runs) = metrics["total_autopilot_runs"].as_u64() {
                output.push_str("\n# HELP omk_autopilot_runs_total Total autopilot runs\n");
                output.push_str("# TYPE omk_autopilot_runs_total counter\n");
                output.push_str(&format!("omk_autopilot_runs_total {}\n", autopilot_runs));
            }
            if let Some(ralph_runs) = metrics["total_ralph_runs"].as_u64() {
                output.push_str("\n# HELP omk_ralph_runs_total Total ralph runs\n");
                output.push_str("# TYPE omk_ralph_runs_total counter\n");
                output.push_str(&format!("omk_ralph_runs_total {}\n", ralph_runs));
            }
        }
    }

    axum::response::Response::builder()
        .header("Content-Type", "text/plain; version=0.0.4")
        .body(output)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

fn metric_u64(metrics: &Value, primary: &str, legacy: Option<&str>) -> Option<u64> {
    metrics
        .get(primary)
        .and_then(Value::as_u64)
        .or_else(|| legacy.and_then(|key| metrics.get(key).and_then(Value::as_u64)))
}

fn normalize_metrics_value(mut metrics: Value) -> Value {
    let Some(team_runs) = metric_u64(&metrics, "total_team_runs", Some("total_spawns")) else {
        return metrics;
    };
    if let Some(obj) = metrics.as_object_mut() {
        let value = Value::Number(team_runs.into());
        obj.entry("total_team_runs")
            .or_insert_with(|| value.clone());
        obj.entry("total_spawns").or_insert(value);
    }
    metrics
}
