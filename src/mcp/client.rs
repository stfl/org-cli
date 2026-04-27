/// MCP client — initialize handshake, tools/list, tools/call.
///
/// Uses the sync Transport underneath. Each `Client` owns a single child
/// process for the lifetime of one CLI invocation.
use serde_json::{Value, json};

use super::error::McpError;
use super::transport::Transport;

pub struct Client {
    transport: Transport,
    next_id: u64,
}

impl Client {
    /// Spawn the server, perform the initialize handshake, and send
    /// `notifications/initialized`. The client is ready to issue requests
    /// after this returns.
    #[allow(dead_code)] // used by integration tests; main bin uses connect_with_timeout
    pub fn connect(argv: &[String]) -> Result<Self, McpError> {
        Self::connect_with_timeout(argv, None)
    }

    /// Like `connect`, but configures a per-`recv()` timeout on the underlying
    /// transport. `None` or `Some(Duration::ZERO)` disables the gate.
    pub fn connect_with_timeout(
        argv: &[String],
        timeout: Option<std::time::Duration>,
    ) -> Result<Self, McpError> {
        let mut transport = Transport::spawn(argv)?;
        transport.set_timeout(timeout);
        let mut client = Client {
            transport,
            next_id: 1,
        };
        client.handshake()?;
        Ok(client)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send a JSON-RPC request and return the result value.
    /// Maps JSON-RPC errors to `McpError::ToolError`.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        self.transport.send(&msg)?;

        let resp = self.transport.recv()?;

        // Validate response id matches
        if let Some(resp_id) = resp.get("id")
            && resp_id != &json!(id)
        {
            return Err(McpError::Protocol(format!(
                "response id {:?} does not match request id {}",
                resp_id, id
            )));
        }

        // Check for JSON-RPC error
        if let Some(err) = resp.get("error") {
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(-1);
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
                .to_string();
            let data = err.get("data").cloned().unwrap_or(Value::Null);
            return Err(McpError::ToolError {
                code,
                message,
                data,
            });
        }

        // Extract result
        resp.get("result")
            .cloned()
            .ok_or_else(|| McpError::Protocol("response missing 'result' field".to_string()))
    }

    /// Send a notification (no id, no response expected).
    fn notify(&mut self, method: &str, params: Value) -> Result<(), McpError> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.transport.send(&msg)
    }

    /// Perform the MCP initialize handshake.
    fn handshake(&mut self) -> Result<(), McpError> {
        let result = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": {
                    "name": "org-cli",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {}
            }),
        )?;

        // Verify server supports tools capability
        let caps = result.get("capabilities");
        if caps.map(|c| c.get("tools").is_none()).unwrap_or(true) {
            return Err(McpError::Protocol(
                "server does not advertise tools capability".to_string(),
            ));
        }

        // Send initialized notification (no response)
        self.notify("notifications/initialized", json!({}))?;

        Ok(())
    }

    /// Check whether the server advertises a specific tool by name.
    ///
    /// Calls tools/list and scans the names. No caching — every call issues
    /// a fresh tools/list request (Phase 9 can add caching if needed).
    pub fn server_has_tool(&mut self, tool_name: &str) -> Result<bool, McpError> {
        let tools = self.tools_list()?;
        Ok(tools.iter().any(|t| {
            t.get("name")
                .and_then(Value::as_str)
                .map(|n| n == tool_name)
                .unwrap_or(false)
        }))
    }

    /// Call tools/list and return the array of tool objects.
    pub fn tools_list(&mut self) -> Result<Vec<Value>, McpError> {
        let result = self.request("tools/list", json!({}))?;
        result
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                McpError::Protocol("tools/list response missing 'tools' array".to_string())
            })
    }

    /// Call tools/call and return the raw content array from the response.
    pub fn tools_call(&mut self, tool_name: &str, arguments: Value) -> Result<Value, McpError> {
        let result = self.request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments
            }),
        )?;

        // Return the content array (or the full result if content is absent)
        Ok(result.get("content").cloned().unwrap_or(result))
    }
}
