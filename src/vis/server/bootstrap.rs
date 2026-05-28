use anyhow::Result;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing::info;

pub async fn run_server(port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(super::html::dashboard_handler))
        .route("/api/teams", get(super::handlers::teams_handler))
        .route("/api/autopilots", get(super::handlers::autopilots_handler))
        .route("/api/ralphs", get(super::handlers::ralphs_handler))
        .route("/api/metrics", get(super::handlers::metrics_handler))
        .route("/metrics", get(super::handlers::prometheus_metrics_handler))
        .route("/api/health", get(super::handlers::health_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("Starting web dashboard on http://{}", addr);
    info!("omk vis running on http://{} — Press Ctrl+C to stop", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(super::signal::shutdown_signal())
        .await?;
    info!("Web dashboard shut down gracefully");
    Ok(())
}
