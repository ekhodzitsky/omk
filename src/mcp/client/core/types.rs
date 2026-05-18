use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::client::transport_trait::McpTransport;

pub struct McpClient<T: McpTransport> {
    pub(crate) transport: T,
    pub(crate) request_id: u64,
    pub(crate) server_name: String,
}

impl<T: McpTransport> std::fmt::Debug for McpClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("transport", &"<dyn McpTransport>")
            .field("request_id", &self.request_id)
            .field("server_name", &self.server_name)
            .finish()
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcRequest<P> {
    pub(crate) jsonrpc: String,
    pub(crate) id: u64,
    pub(crate) method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<P>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcResponse<R> {
    pub(crate) jsonrpc: String,
    pub(crate) id: u64,
    #[serde(flatten)]
    pub(crate) payload: JsonRpcPayload<R>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JsonRpcPayload<R> {
    Result(R),
    Error(JsonRpcError),
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) data: Option<Value>,
}
