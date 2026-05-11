use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::wire::client::WireClient;
use crate::wire::protocol::{
    EventParams, JsonRpcErrorResponse, JsonRpcNotification, JsonRpcRequest, JsonRpcSuccessResponse,
    RequestParams,
};

/// A union type for all incoming wire messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireMessage {
    Request(JsonRpcRequest<RequestParams>),
    Event(JsonRpcNotification<EventParams>),
    SuccessResponse(JsonRpcSuccessResponse<Value>),
    ErrorResponse(JsonRpcErrorResponse),
}

/// A response to be sent back to the agent.
pub struct WireResponse {
    pub id: String,
    pub result: serde_json::Value,
}

/// Process wire messages in a loop, handling events and requests.
pub async fn process_messages<F, Fut>(client: &mut WireClient, mut handler: F) -> Result<()>
where
    F: FnMut(WireMessage) -> Fut,
    Fut: std::future::Future<Output = Result<Option<WireResponse>>>,
{
    loop {
        match client.read_message().await {
            Ok(msg) => {
                match &msg {
                    WireMessage::Request(req) if req.method != "request" => {
                        warn!(method = %req.method, "Unknown wire request method, skipping");
                        continue;
                    }
                    WireMessage::Request(req) if req.params.to_request().is_err() => {
                        warn!(
                            request_id = %req.id,
                            request_type = %req.params.request_type,
                            "Unknown wire request type, replying with error"
                        );
                        client
                            .send_error(&req.id, -32601, "Unknown request type")
                            .await?;
                        continue;
                    }
                    WireMessage::Event(ev) if ev.params.to_event().is_err() => {
                        warn!(event_type = %ev.params.event_type, "Unknown wire event kind");
                        continue;
                    }
                    _ => {}
                }
                if let Some(response) = handler(msg).await? {
                    client.send_response(&response.id, response.result).await?;
                }
            }
            Err(e) => {
                warn!(error = %e, "Wire message error, exiting loop");
                break;
            }
        }
    }
    Ok(())
}
