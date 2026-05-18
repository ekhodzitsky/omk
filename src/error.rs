use std::path::PathBuf;
use thiserror::Error;

/// Structured errors for the omk API surface.
///
/// These errors are returned from public functions and can be serialized
/// to JSON for MCP/web responses.
#[derive(Error, Debug, Clone)]
#[allow(dead_code)]
pub enum OmkError {
    #[error("team '{name}' not found")]
    TeamNotFound { name: String },

    #[error("team '{name}' already exists")]
    TeamAlreadyExists { name: String },

    #[error("invalid configuration: {field} = {value}")]
    InvalidConfig { field: String, value: String },

    #[error("registry '{url}' is unreachable: {reason}")]
    RegistryUnreachable { url: String, reason: String },

    #[error("registry '{url}' returned invalid JSON: {reason}")]
    RegistryInvalid { url: String, reason: String },

    #[error("skill '{name}' not found")]
    SkillNotFound { name: String },

    #[error("skill '{name}' already exists at {path}")]
    SkillAlreadyExists { name: String, path: PathBuf },

    #[error("shell command failed: {command}")]
    ShellFailed { command: String },

    #[error("input validation failed: {reason}")]
    InvalidInput { reason: String },

    #[error("IO error: {path}: {reason}")]
    Io { path: PathBuf, reason: String },

    #[error("state serialization failed: {reason}")]
    StateSerialization { reason: String },

    #[error("provider '{name}' is not installed")]
    ProviderNotInstalled { name: String },

    #[error("synthesis failed: {reason}")]
    SynthesisFailed { reason: String },

    #[error("operation timed out after {secs}s")]
    Timeout { secs: u64 },

    #[error("MCP transport error for server '{server}': {reason}")]
    McpTransport { server: String, reason: String },

    #[error("MCP tool call failed on server '{server}' tool '{tool}': {reason}")]
    McpToolCall {
        server: String,
        tool: String,
        reason: String,
    },

    #[error("MCP config error at {path}: {reason}")]
    McpConfig { path: PathBuf, reason: String },
}

impl OmkError {
    /// HTTP-like status code for categorization.
    pub fn status_code(&self) -> u16 {
        match self {
            OmkError::TeamNotFound { .. } => 404,
            OmkError::SkillNotFound { .. } => 404,
            OmkError::TeamAlreadyExists { .. } => 409,
            OmkError::SkillAlreadyExists { .. } => 409,
            OmkError::InvalidConfig { .. } => 400,
            OmkError::InvalidInput { .. } => 400,
            OmkError::RegistryUnreachable { .. } => 503,
            OmkError::RegistryInvalid { .. } => 502,
            OmkError::ShellFailed { .. } => 500,
            OmkError::Io { .. } => 500,
            OmkError::StateSerialization { .. } => 500,
            OmkError::ProviderNotInstalled { .. } => 503,
            OmkError::SynthesisFailed { .. } => 500,
            OmkError::Timeout { .. } => 504,
            OmkError::McpTransport { .. } => 502,
            OmkError::McpToolCall { .. } => 500,
            OmkError::McpConfig { .. } => 400,
        }
    }

    /// Error category for metrics/logging.
    pub fn category(&self) -> &'static str {
        match self {
            OmkError::TeamNotFound { .. } | OmkError::TeamAlreadyExists { .. } => "team",
            OmkError::InvalidConfig { .. } | OmkError::InvalidInput { .. } => "validation",
            OmkError::RegistryUnreachable { .. } | OmkError::RegistryInvalid { .. } => "registry",
            OmkError::SkillNotFound { .. } | OmkError::SkillAlreadyExists { .. } => "skill",
            OmkError::ShellFailed { .. } => "shell",
            OmkError::Io { .. } | OmkError::StateSerialization { .. } => "io",
            OmkError::ProviderNotInstalled { .. }
            | OmkError::SynthesisFailed { .. }
            | OmkError::Timeout { .. } => "runtime",
            OmkError::McpTransport { .. } | OmkError::McpToolCall { .. } => "mcp",
            OmkError::McpConfig { .. } => "validation",
        }
    }
}

/// Convert from common error types.
impl From<std::io::Error> for OmkError {
    fn from(e: std::io::Error) -> Self {
        OmkError::Io {
            path: PathBuf::from("<unknown>"),
            reason: e.to_string(),
        }
    }
}

/// JSON representation for API responses.
impl serde::Serialize for OmkError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("OmkError", 3)?;
        state.serialize_field("error", &self.to_string())?;
        state.serialize_field("code", &self.status_code())?;
        state.serialize_field("category", &self.category())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn status_code_mapping() {
        assert_eq!(
            OmkError::TeamNotFound {
                name: "x".to_string()
            }
            .status_code(),
            404
        );
        assert_eq!(
            OmkError::TeamAlreadyExists {
                name: "x".to_string()
            }
            .status_code(),
            409
        );
        assert_eq!(
            OmkError::InvalidConfig {
                field: "f".to_string(),
                value: "v".to_string()
            }
            .status_code(),
            400
        );
        assert_eq!(
            OmkError::RegistryUnreachable {
                url: "u".to_string(),
                reason: "r".to_string()
            }
            .status_code(),
            503
        );
        assert_eq!(
            OmkError::RegistryInvalid {
                url: "u".to_string(),
                reason: "r".to_string()
            }
            .status_code(),
            502
        );
        assert_eq!(
            OmkError::SkillNotFound {
                name: "x".to_string()
            }
            .status_code(),
            404
        );
        assert_eq!(
            OmkError::SkillAlreadyExists {
                name: "x".to_string(),
                path: PathBuf::from("p")
            }
            .status_code(),
            409
        );
        assert_eq!(
            OmkError::ShellFailed {
                command: "c".to_string()
            }
            .status_code(),
            500
        );
        assert_eq!(
            OmkError::InvalidInput {
                reason: "r".to_string()
            }
            .status_code(),
            400
        );
        assert_eq!(
            OmkError::Io {
                path: PathBuf::from("p"),
                reason: "r".to_string()
            }
            .status_code(),
            500
        );
        assert_eq!(
            OmkError::StateSerialization {
                reason: "r".to_string()
            }
            .status_code(),
            500
        );
        assert_eq!(
            OmkError::ProviderNotInstalled {
                name: "p".to_string()
            }
            .status_code(),
            503
        );
        assert_eq!(
            OmkError::SynthesisFailed {
                reason: "r".to_string()
            }
            .status_code(),
            500
        );
        assert_eq!(OmkError::Timeout { secs: 1 }.status_code(), 504);
        assert_eq!(
            OmkError::McpTransport {
                server: "s".to_string(),
                reason: "r".to_string()
            }
            .status_code(),
            502
        );
        assert_eq!(
            OmkError::McpToolCall {
                server: "s".to_string(),
                tool: "t".to_string(),
                reason: "r".to_string()
            }
            .status_code(),
            500
        );
        assert_eq!(
            OmkError::McpConfig {
                path: PathBuf::from("p"),
                reason: "r".to_string()
            }
            .status_code(),
            400
        );
    }

    #[test]
    fn category_mapping() {
        assert_eq!(
            OmkError::TeamNotFound {
                name: "x".to_string()
            }
            .category(),
            "team"
        );
        assert_eq!(
            OmkError::InvalidConfig {
                field: "f".to_string(),
                value: "v".to_string()
            }
            .category(),
            "validation"
        );
        assert_eq!(
            OmkError::RegistryUnreachable {
                url: "u".to_string(),
                reason: "r".to_string()
            }
            .category(),
            "registry"
        );
        assert_eq!(
            OmkError::SkillNotFound {
                name: "x".to_string()
            }
            .category(),
            "skill"
        );
        assert_eq!(
            OmkError::ShellFailed {
                command: "c".to_string()
            }
            .category(),
            "shell"
        );
        assert_eq!(
            OmkError::Io {
                path: PathBuf::from("p"),
                reason: "r".to_string()
            }
            .category(),
            "io"
        );
        assert_eq!(
            OmkError::ProviderNotInstalled {
                name: "p".to_string()
            }
            .category(),
            "runtime"
        );
        assert_eq!(
            OmkError::McpTransport {
                server: "s".to_string(),
                reason: "r".to_string()
            }
            .category(),
            "mcp"
        );
        assert_eq!(
            OmkError::McpConfig {
                path: PathBuf::from("p"),
                reason: "r".to_string()
            }
            .category(),
            "validation"
        );
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: OmkError = io_err.into();
        match err {
            OmkError::Io { path, reason } => {
                assert_eq!(path, PathBuf::from("<unknown>"));
                assert_eq!(reason, "missing");
            }
            other => panic!("expected Io error, got {:?}", other),
        }
    }

    #[test]
    fn serialize_json_shape() {
        let err = OmkError::TeamNotFound {
            name: "alpha".to_string(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["error"], "team 'alpha' not found");
        assert_eq!(json["code"], 404);
        assert_eq!(json["category"], "team");
    }
}
