//! Wire Protocol integration for OMK.
//!
//! Thin re-export layer over the `kimi-wire` crate.  All generic wire logic
//! (parsing, dispatch, extension traits, redaction) lives in `kimi-wire`;
//! this module only preserves a few OMK-specific compatibility aliases.

// Re-export core client / error types.
pub use kimi_wire::{InMemoryWireClient, WireClient, WireError};

// Re-export protocol primitives.
pub use kimi_wire::protocol::{
    content::{
        ContentPart, DisplayBlock, DisplayBlockType, MediaUrl, TextPart, ThinkPart,
        TodoDisplayItem, TodoStatus, ToolOutput, ToolReturnValue, UserInput,
    },
    event::{
        ApprovalResponseKind, Event, HookAction, StatusUpdate, SubagentEventPayload, TokenUsage,
        ToolCallFunction,
    },
    jsonrpc::{
        JsonRpcError, JsonRpcErrorResponse, JsonRpcNotification, JsonRpcRequest,
        JsonRpcSuccessResponse, JsonRpcVersion, RawWireMessage, METHOD_NOT_FOUND,
    },
    method::{
        CancelParams, CancelResult, ClientCapabilities, ClientInfo, ExternalTool, InitializeParams,
        InitializeResult, PromptParams, PromptResult, PromptStatus, ReplayParams, ReplayResult,
        ReplayStatus, SetPlanModeParams, SetPlanModeResult, SetPlanModeStatus, SteerParams,
        SteerResult, SteerStatus, WireHookSubscription,
    },
    request::{
        ApprovalRequest, HookRequest, QuestionItem, QuestionOption, Request, ToolCallRequest,
    },
};

// Re-export transport types.
pub use kimi_wire::transport::{
    ChildProcessTransport, Transport, TransportWireClient, MAX_WIRE_LINE_LENGTH,
};

// Re-export high-level dispatch, parsing, and extension traits from kimi-wire.
pub use kimi_wire::dispatch::{process_messages, WireResponse};
pub use kimi_wire::message::{parse_wire_message, WireMessage};
pub use kimi_wire::{EventExt, RequestExt, WireClientExt};

// Re-export secret redaction.
pub use kimi_wire::redact_secrets;

// ---------------------------------------------------------------------------
// OMK-specific compatibility aliases.
// ---------------------------------------------------------------------------

/// OMK used to call this `ProcessWireClient`.
pub type ProcessWireClient =
    kimi_wire::transport::TransportWireClient<kimi_wire::transport::ChildProcessTransport>;

/// OMK used to call this `ApprovalResponseType`.
pub use kimi_wire::protocol::event::ApprovalResponseKind as ApprovalResponseType;

/// Raw secret-scrubbing helper operating on `&str`.
pub use kimi_wire::protocol::redact::scrub_secret_patterns;

/// OMK used to call this `redact_wire_secrets`.
///
/// Thin wrapper around [`kimi_wire::redact_secrets`] so existing call-sites
/// in OMK keep compiling.
pub fn redact_wire_secrets(value: &serde_json::Value) -> serde_json::Value {
    kimi_wire::redact_secrets(value)
}

/// OMK used to call this `KIMI_WIRE_PROTOCOL_VERSION`.
pub use kimi_wire::WIRE_PROTOCOL_VERSION;
pub use kimi_wire::WIRE_PROTOCOL_VERSION as KIMI_WIRE_PROTOCOL_VERSION;
