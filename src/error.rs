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
