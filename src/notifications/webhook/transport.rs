use anyhow::{Context, Result};
use std::future::Future;
use std::pin::Pin;

/// Trait boundary for webhook HTTP delivery.
///
/// The only place `reqwest` is allowed under `src/notifications/`.
pub trait WebhookTransport: Send + Sync {
    /// POST `body` JSON to `url` with a 30-second timeout.
    fn post_json<'a>(
        &'a self,
        url: &'a str,
        body: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

/// Production transport backed by `reqwest`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestWebhookTransport;

impl WebhookTransport for ReqwestWebhookTransport {
    fn post_json<'a>(
        &'a self,
        url: &'a str,
        body: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            reqwest::Client::new()
                .post(url)
                .timeout(std::time::Duration::from_secs(30))
                .json(&body)
                .send()
                .await
                .context("Webhook POST failed")?
                .error_for_status()
                .context("Webhook returned error status")?;
            Ok(())
        })
    }
}

/// In-memory transport for unit tests.
#[derive(Default, Debug)]
pub struct MockWebhookTransport {
    pub calls: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

impl WebhookTransport for MockWebhookTransport {
    fn post_json<'a>(
        &'a self,
        url: &'a str,
        body: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let url = url.to_string();
        Box::pin(async move {
            self.calls
                .lock()
                .expect("mock transport lock")
                .push((url, body));
            Ok(())
        })
    }
}
