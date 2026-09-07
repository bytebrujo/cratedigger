use rmcp::{
    ErrorData,
    model::{CallToolResult, ErrorCode},
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;

/// The wire object is unchanged; the schema covers both success and failure.
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ToolPayload<T> {
    Success(T),
    Failure(Failure),
}

#[derive(Serialize, JsonSchema)]
pub struct Failure {
    pub error: ErrorDetails,
    /// No successful upstream snapshot exists on failure.
    pub fetched_at: (),
}

#[derive(Serialize, JsonSchema)]
pub struct ErrorDetails {
    pub code: String,
    pub field: Option<String>,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("name: unknown tool '{0}'")]
    UnknownTool(String),
    #[error("{field}: {message}")]
    InvalidInput { field: String, message: String },
    #[error("{field}: {message}")]
    Configuration { field: String, message: String },
    #[error("crates.io returned HTTP {status} after {attempts} attempt(s)")]
    UpstreamStatus { status: u16, attempts: u8 },
    #[error("crates.io request failed: {0}")]
    Network(String),
    #[error("crates.io response is invalid: {0}")]
    InvalidResponse(String),
    #[error("crates.io response exceeds the {0}-byte limit")]
    ResponseTooLarge(usize),
    #[error("crates.io lookup exceeded its 45-second deadline (including queue and retries)")]
    Timeout,
}

impl Error {
    pub fn input(field: &str, message: &str) -> Self {
        Self::InvalidInput {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn configuration(field: &str, message: &str) -> Self {
        Self::Configuration {
            field: field.into(),
            message: message.into(),
        }
    }

    /// All operational errors share this client-visible representation.
    /// fetched_at=null means no successful upstream snapshot exists.
    pub fn tool_result(&self) -> CallToolResult {
        let (code, field, retryable) = match self {
            Self::InvalidInput { field, .. } => ("invalid_input", Some(field.as_str()), false),
            Self::Configuration { field, .. } => ("configuration", Some(field.as_str()), false),
            Self::UpstreamStatus { status: 404, .. } => ("not_found", Some("name"), false),
            Self::UpstreamStatus { status, .. } => {
                ("upstream_http", None, *status == 429 || *status >= 500)
            }
            Self::Network(_) => ("network", None, true),
            Self::InvalidResponse(_) => ("invalid_response", None, false),
            Self::ResponseTooLarge(_) => ("response_too_large", None, false),
            Self::Timeout => ("timeout", None, true),
            Self::UnknownTool(_) => ("unknown_tool", Some("name"), false),
        };
        CallToolResult::structured_error(json!(ToolPayload::<serde_json::Value>::Failure(
            Failure {
                error: ErrorDetails {
                    code: code.into(),
                    field: field.map(str::to_owned),
                    message: self.to_string(),
                    retryable,
                },
                fetched_at: (),
            }
        )))
    }
}

/// Only routing errors become JSON-RPC errors; tool failures use tool_result.
impl From<Error> for ErrorData {
    fn from(error: Error) -> Self {
        Self::new(
            ErrorCode::METHOD_NOT_FOUND,
            error.to_string(),
            Some(json!({"field": "name"})),
        )
    }
}
