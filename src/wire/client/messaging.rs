use anyhow::Result;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::wire::client::WireClient;
use crate::wire::protocol::{
    CancelParams, CancelResult, JsonRpcRequest, PromptParams, PromptResult, ReplayParams,
    ReplayResult, SetPlanModeParams, SetPlanModeResult, SteerParams, SteerResult, UserInput,
};

impl WireClient {
    /// Send a prompt and start a turn.
    pub async fn prompt(&mut self, user_input: &str) -> Result<PromptResult> {
        let id = self.start_prompt(user_input).await?;
        self.read_response::<PromptResult>(&id).await
    }

    /// Send a prompt without waiting for the final prompt response.
    pub async fn start_prompt(&mut self, user_input: &str) -> Result<String> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "prompt".to_string(),
            id: id.clone(),
            params: PromptParams {
                user_input: UserInput::Text(user_input.to_string()),
            },
        };
        self.send_request(&req).await?;
        Ok(id)
    }

    /// Replay events and requests from the current session.
    pub async fn replay(&mut self) -> Result<ReplayResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "replay".to_string(),
            id: id.clone(),
            params: ReplayParams::default(),
        };
        self.send_request(&req).await?;
        self.read_response::<ReplayResult>(&id).await
    }

    /// Steer the current turn with additional user input.
    pub async fn steer(&mut self, user_input: &str) -> Result<SteerResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "steer".to_string(),
            id: id.clone(),
            params: SteerParams {
                user_input: UserInput::Text(user_input.to_string()),
            },
        };
        self.send_request(&req).await?;
        self.read_response::<SteerResult>(&id).await
    }

    /// Enable or disable plan mode for the current wire session.
    pub async fn set_plan_mode(&mut self, enabled: bool) -> Result<SetPlanModeResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "set_plan_mode".to_string(),
            id: id.clone(),
            params: SetPlanModeParams { enabled },
        };
        self.send_request(&req).await?;
        self.read_response::<SetPlanModeResult>(&id).await
    }

    /// Cancel current turn.
    pub async fn cancel(&mut self) -> Result<()> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "cancel".to_string(),
            id: id.clone(),
            params: CancelParams::default(),
        };
        self.send_request(&req).await?;
        let _: CancelResult = self.read_response(&id).await?;
        Ok(())
    }

    /// Send a response to an agent request.
    pub async fn send_response<ResultType: Serialize>(
        &mut self,
        id: &str,
        result: ResultType,
    ) -> Result<()> {
        let resp = crate::wire::protocol::JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result,
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, id: &str, code: i32, message: &str) -> Result<()> {
        let resp = crate::wire::protocol::JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            error: crate::wire::protocol::JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            },
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    pub(super) fn next_id(&mut self) -> String {
        self.request_id_counter += 1;
        format!("req-{}", self.request_id_counter)
    }

    pub(super) async fn send_request<Params: Serialize>(
        &mut self,
        req: &JsonRpcRequest<Params>,
    ) -> Result<()> {
        let line = format!("{}\n", serde_json::to_string(req)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }
}
