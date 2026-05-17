use super::transport_trait::McpTransport;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, warn};

const RECV_TIMEOUT_SECS: u64 = 60;
const CONNECT_TIMEOUT_SECS: u64 = 30;
const MAX_RECONNECT_DELAY_SECS: u64 = 30;

/// HTTP+SSE transport for MCP servers.
///
/// Sends client→server messages via POST and receives server→client messages
/// over an SSE stream. Handles reconnection with exponential backoff.
#[derive(Debug)]
pub struct HttpMcpTransport {
    client: reqwest::Client,
    post_url: String,
    sse_url: String,
    headers: HashMap<String, String>,
    queue: Arc<tokio::sync::Mutex<VecDeque<String>>>,
    sse_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown: Arc<tokio::sync::Notify>,
}

impl HttpMcpTransport {
    pub fn new(base_url: impl Into<String>, headers: HashMap<String, String>) -> Result<Self> {
        let base_url = base_url.into();
        let trimmed = base_url.trim_end_matches('/');
        let (post_url, sse_url) = if trimmed.ends_with("/sse") {
            let without_sse = trimmed.trim_end_matches("/sse");
            (format!("{}/message", without_sse), trimmed.to_string())
        } else {
            (base_url.clone(), format!("{}/sse", trimmed))
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .context("failed to build reqwest client for MCP HTTP transport")?;

        Ok(Self {
            client,
            post_url,
            sse_url,
            headers,
            queue: Arc::new(tokio::sync::Mutex::new(VecDeque::new())),
            sse_handle: None,
            shutdown: Arc::new(tokio::sync::Notify::new()),
        })
    }

    fn ensure_sse_connected(&mut self) -> Result<()> {
        if self.sse_handle.is_some() {
            return Ok(());
        }
        let client = self.client.clone();
        let sse_url = self.sse_url.clone();
        let headers = self.headers.clone();
        let queue = self.queue.clone();
        let shutdown = self.shutdown.clone();

        let handle = tokio::spawn(async move {
            let mut delay_secs = 1u64;
            loop {
                tokio::select! {
                    _ = shutdown.notified() => {
                        debug!("SSE reconnect loop shutting down");
                        break;
                    }
                    result = connect_sse(&client, &sse_url, &headers, &queue) => {
                        match result {
                            Ok(()) => {
                                debug!("SSE stream ended gracefully");
                                delay_secs = 1;
                            }
                            Err(e) => {
                                warn!(error = %e, delay = delay_secs, "SSE connection error, reconnecting");
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                        delay_secs = (delay_secs * 2).min(MAX_RECONNECT_DELAY_SECS);
                    }
                }
            }
        });

        self.sse_handle = Some(handle);
        Ok(())
    }
}

async fn connect_sse(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
    queue: &Arc<tokio::sync::Mutex<VecDeque<String>>>,
) -> Result<()> {
    let mut request = client.get(url);
    for (k, v) in headers {
        request = request.header(k, v);
    }
    request = request.header("Accept", "text/event-stream");

    let mut response = request
        .send()
        .await
        .with_context(|| format!("failed to connect to SSE endpoint {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("SSE endpoint {url} returned status {}", response.status());
    }

    let mut buf = Vec::new();
    let mut current_data = String::new();

    loop {
        match tokio::time::timeout(
            std::time::Duration::from_secs(RECV_TIMEOUT_SECS),
            response.chunk(),
        )
        .await
        {
            Ok(Ok(Some(chunk))) => {
                buf.extend_from_slice(&chunk);
                // Process complete lines from the buffer.
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let line = line.trim_end_matches('\n').trim_end_matches('\r');
                    if line.is_empty() {
                        if !current_data.is_empty() {
                            let msg = current_data.trim().to_string();
                            if !msg.is_empty() {
                                debug!(len = msg.len(), "SSE message received");
                                queue.lock().await.push_back(msg);
                            }
                            current_data.clear();
                        }
                    } else if let Some(data) = line.strip_prefix("data:") {
                        if !current_data.is_empty() {
                            current_data.push('\n');
                        }
                        current_data.push_str(data.trim_start());
                    }
                    // Ignore other SSE fields (event:, id:, retry:)
                }
            }
            Ok(Ok(None)) => {
                debug!("SSE stream closed by server");
                break;
            }
            Ok(Err(e)) => {
                return Err(anyhow::Error::new(e).context("error reading SSE stream"));
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "SSE read timeout after {RECV_TIMEOUT_SECS}s"
                ));
            }
        }
    }

    Ok(())
}

impl Drop for HttpMcpTransport {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
        if let Some(handle) = self.sse_handle.take() {
            handle.abort();
        }
    }
}

impl McpTransport for HttpMcpTransport {
    fn send(&mut self, message: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.ensure_sse_connected()?;

            let mut request = self
                .client
                .post(&self.post_url)
                .header("Content-Type", "application/json")
                .body(message.clone());

            for (k, v) in &self.headers {
                request = request.header(k, v);
            }

            let response = tokio::time::timeout(
                std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
                request.send(),
            )
            .await
            .with_context(|| format!("POST to {} timed out", self.post_url))?
            .with_context(|| format!("POST to {} failed", self.post_url))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = match response.text().await {
                    Ok(b) => b,
                    Err(_) => "(unreadable)".to_string(),
                };
                anyhow::bail!("MCP HTTP transport POST returned {status}: {body}");
            }

            // Some MCP servers return the JSON-RPC response inline in the POST body.
            // If so, push it to the queue so recv() can return it.
            if let Ok(body) = response.text().await {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    debug!(len = trimmed.len(), "MCP HTTP inline response received");
                    self.queue.lock().await.push_back(trimmed.to_string());
                }
            }

            debug!(url = %self.post_url, len = message.len(), "MCP HTTP transport send");
            Ok(())
        })
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
        Box::pin(async move {
            self.ensure_sse_connected()?;

            let result =
                tokio::time::timeout(std::time::Duration::from_secs(RECV_TIMEOUT_SECS), async {
                    loop {
                        if let Some(msg) = self.queue.lock().await.pop_front() {
                            return Some(msg);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                })
                .await;

            match result {
                Ok(msg) => {
                    if let Some(ref m) = msg {
                        debug!(len = m.len(), "MCP HTTP transport recv");
                    }
                    Ok(msg)
                }
                Err(_) => {
                    warn!("MCP HTTP transport recv timeout");
                    Err(anyhow::anyhow!(
                        "MCP HTTP transport recv timeout after {RECV_TIMEOUT_SECS}s"
                    ))
                }
            }
        })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.shutdown.notify_waiters();
            if let Some(handle) = self.sse_handle.take() {
                handle.abort();
                let _ = handle.await;
            }
            info!("MCP HTTP transport closed");
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_derivation_base() {
        let t = HttpMcpTransport::new("https://example.com/mcp", HashMap::new()).unwrap();
        assert_eq!(t.post_url, "https://example.com/mcp");
        assert_eq!(t.sse_url, "https://example.com/mcp/sse");
    }

    #[test]
    fn test_url_derivation_trailing_slash() {
        let t = HttpMcpTransport::new("https://example.com/mcp/", HashMap::new()).unwrap();
        assert_eq!(t.post_url, "https://example.com/mcp/");
        assert_eq!(t.sse_url, "https://example.com/mcp/sse");
    }

    #[test]
    fn test_url_derivation_sse_with_trailing_slash() {
        let t = HttpMcpTransport::new("https://example.com/mcp/sse/", HashMap::new()).unwrap();
        assert_eq!(t.post_url, "https://example.com/mcp/message");
        assert_eq!(t.sse_url, "https://example.com/mcp/sse");
    }

    #[test]
    fn test_url_derivation_explicit_sse() {
        let t = HttpMcpTransport::new("https://example.com/mcp/sse", HashMap::new()).unwrap();
        assert_eq!(t.post_url, "https://example.com/mcp/message");
        assert_eq!(t.sse_url, "https://example.com/mcp/sse");
    }

    #[test]
    fn test_custom_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        let t = HttpMcpTransport::new("https://example.com/mcp", headers).unwrap();
        assert_eq!(
            t.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    }

    #[tokio::test]
    async fn test_close_is_graceful() {
        let mut t = HttpMcpTransport::new("https://example.com/mcp", HashMap::new()).unwrap();
        t.close().await.unwrap();
        assert!(t.sse_handle.is_none());
    }
}
