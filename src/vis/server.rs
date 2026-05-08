use anyhow::Result;
use axum::{
    response::Html,
    routing::get,
    Json, Router,
};
use serde_json::Value;
use std::net::SocketAddr;
use tracing::info;

pub async fn run_server(port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(dashboard_handler))
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

async fn dashboard_handler() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>omk dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f23;
            color: #e0e0e0;
            line-height: 1.6;
        }
        .container { max-width: 1200px; margin: 0 auto; padding: 2rem; }
        header {
            display: flex;
            align-items: center;
            gap: 1rem;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid #333;
        }
        header h1 { font-size: 1.8rem; color: #fff; }
        header .version {
            background: #2563eb;
            color: white;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.875rem;
            font-weight: 500;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 1.5rem;
            margin-bottom: 2rem;
        }
        .card {
            background: #1a1a2e;
            border: 1px solid #2a2a4a;
            border-radius: 12px;
            padding: 1.5rem;
            transition: border-color 0.2s;
        }
        .card:hover { border-color: #2563eb; }
        .card h2 {
            font-size: 1.1rem;
            color: #fff;
            margin-bottom: 1rem;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            padding: 0.5rem 0;
            border-bottom: 1px solid #2a2a4a;
        }
        .metric:last-child { border-bottom: none; }
        .metric-value { font-weight: 600; color: #60a5fa; }
        .status-ok { color: #22c55e; }
        .status-warn { color: #f59e0b; }
        .status-err { color: #ef4444; }
        .team-list { list-style: none; }
        .team-item {
            padding: 0.75rem;
            margin-bottom: 0.5rem;
            background: #0f0f23;
            border-radius: 8px;
            border-left: 3px solid #2563eb;
        }
        .team-name { font-weight: 600; color: #fff; }
        .team-meta { font-size: 0.875rem; color: #888; margin-top: 0.25rem; }
        .phase-badge {
            display: inline-block;
            padding: 0.15rem 0.5rem;
            border-radius: 4px;
            font-size: 0.75rem;
            font-weight: 500;
            text-transform: uppercase;
        }
        .phase-planning { background: #f59e0b20; color: #f59e0b; }
        .phase-executing { background: #2563eb20; color: #60a5fa; }
        .phase-complete { background: #22c55e20; color: #22c55e; }
        .phase-failed { background: #ef444420; color: #ef4444; }
        .refresh-btn {
            position: fixed;
            bottom: 2rem;
            right: 2rem;
            background: #2563eb;
            color: white;
            border: none;
            padding: 0.75rem 1.5rem;
            border-radius: 9999px;
            cursor: pointer;
            font-size: 1rem;
            box-shadow: 0 4px 12px rgba(37, 99, 235, 0.3);
            transition: transform 0.2s, box-shadow 0.2s;
        }
        .refresh-btn:hover { transform: translateY(-2px); box-shadow: 0 6px 20px rgba(37, 99, 235, 0.4); }
        footer {
            text-align: center;
            padding: 2rem;
            color: #666;
            font-size: 0.875rem;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .live-indicator {
            display: inline-block;
            width: 8px;
            height: 8px;
            background: #22c55e;
            border-radius: 50%;
            animation: pulse 2s infinite;
            margin-right: 0.5rem;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🌙 omk dashboard</h1>
            <span class="version">v0.2.0</span>
            <span style="margin-left:auto;color:#666;font-size:0.875rem;">
                <span class="live-indicator"></span>Live
            </span>
        </header>

        <div class="grid">
            <div class="card">
                <h2>📊 Metrics</h2>
                <div id="metrics">
                    <div class="metric"><span>Total Spawns</span><span class="metric-value" id="m-spawns">—</span></div>
                    <div class="metric"><span>Total Shutdowns</span><span class="metric-value" id="m-shutdowns">—</span></div>
                    <div class="metric"><span>Tasks Created</span><span class="metric-value" id="m-tasks">—</span></div>
                    <div class="metric"><span>Ask Calls</span><span class="metric-value" id="m-ask">—</span></div>
                </div>
            </div>

            <div class="card">
                <h2>🤖 Active Teams</h2>
                <ul class="team-list" id="teams">
                    <li class="team-item">
                        <div class="team-name">No active teams</div>
                        <div class="team-meta">Run <code>omk team spawn</code> to start</div>
                    </li>
                </ul>
            </div>

            <div class="card">
                <h2>🩺 Health</h2>
                <div id="health">
                    <div class="metric"><span>Status</span><span class="metric-value status-ok" id="h-status">Loading...</span></div>
                    <div class="metric"><span>Version</span><span class="metric-value" id="h-version">—</span></div>
                </div>
            </div>
        </div>

        <footer>
            <p>oh-my-kimi — Multi-agent orchestration for Kimi CLI</p>
            <p><a href="https://github.com/ekhodzitsky/oh-my-kimi" style="color:#2563eb;">GitHub</a></p>
        </footer>
    </div>

    <button class="refresh-btn" onclick="loadData()">↻ Refresh</button>

    <script>
        async function loadData() {
            try {
                const [teamsRes, metricsRes, healthRes] = await Promise.all([
                    fetch('/api/teams'),
                    fetch('/api/metrics'),
                    fetch('/api/health')
                ]);

                const teams = await teamsRes.json();
                const metrics = await metricsRes.json();
                const health = await healthRes.json();

                // Teams
                const teamsList = document.getElementById('teams');
                if (teams.teams && teams.teams.length > 0) {
                    teamsList.innerHTML = teams.teams.map(t => {
                        const phase = t.phase || 'Unknown';
                        const phaseClass = 'phase-' + phase.toLowerCase();
                        return `<li class="team-item">
                            <div class="team-name">${t.name || 'Unnamed'}</div>
                            <div class="team-meta">
                                <span class="phase-badge ${phaseClass}">${phase}</span>
                                ${t.task ? '• ' + t.task.substring(0, 60) + (t.task.length > 60 ? '...' : '') : ''}
                            </div>
                        </li>`;
                    }).join('');
                }

                // Metrics
                if (metrics.metrics) {
                    const m = metrics.metrics;
                    document.getElementById('m-spawns').textContent = m.total_spawns || 0;
                    document.getElementById('m-shutdowns').textContent = m.total_shutdowns || 0;
                    document.getElementById('m-tasks').textContent = m.total_tasks_created || 0;
                    document.getElementById('m-ask').textContent = m.total_ask_calls || 0;
                }

                // Health
                document.getElementById('h-status').textContent = health.status || 'unknown';
                document.getElementById('h-status').className = 'metric-value ' + (health.status === 'ok' ? 'status-ok' : 'status-err');
                document.getElementById('h-version').textContent = health.version || '—';
            } catch (e) {
                console.error('Failed to load data:', e);
            }
        }

        loadData();
        setInterval(loadData, 5000);
    </script>
</body>
</html>"#;

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
