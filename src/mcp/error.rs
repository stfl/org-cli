/// MCP client error types.
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    /// JSON-RPC error returned by the server (code, message, optional data).
    #[error("tool error {code}: {message}")]
    ToolError {
        code: i64,
        message: String,
        /// The `error.data` field from the JSON-RPC error response, if present.
        data: Value,
    },

    /// Transport-level failure (I/O, framing, JSON parse).
    #[error("transport error: {0}")]
    Transport(String),

    /// Server process could not be spawned.
    #[error("spawn error: {0}")]
    Spawn(String),

    /// Protocol violation (unexpected response shape, missing field).
    #[error("protocol error: {0}")]
    Protocol(String),
}

impl McpError {
    /// Map to the CLI exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            McpError::ToolError { .. } => 1,
            McpError::Transport(_) => 3,
            McpError::Spawn(_) => 4,
            McpError::Protocol(_) => 3,
        }
    }

    /// Map to the CLI error kind string.
    pub fn kind_str(&self) -> &'static str {
        match self {
            McpError::ToolError { .. } => "tool",
            McpError::Transport(_) | McpError::Spawn(_) | McpError::Protocol(_) => "transport",
        }
    }

    /// JSON-RPC error code if applicable.
    pub fn rpc_code(&self) -> i64 {
        match self {
            McpError::ToolError { code, .. } => *code,
            _ => -1,
        }
    }

    /// The structured `error.data` payload from the JSON-RPC response, if any.
    pub fn rpc_data(&self) -> Value {
        match self {
            McpError::ToolError { data, .. } => data.clone(),
            _ => Value::Null,
        }
    }
}
