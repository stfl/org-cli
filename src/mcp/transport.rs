/// MCP stdio transport — newline-delimited JSON-RPC.
///
/// # Framing
///
/// The current org-mcp / mcp-server-lib uses **newline-delimited JSON** (ND-JSON):
/// one JSON-RPC object per line, terminated by `\n`, in both directions.
///
/// This is distinct from the Content-Length framing used by some other MCP
/// implementations (e.g. LSP-style). If org-mcp ever switches to Content-Length
/// framing, replace `send` / `recv` below while keeping the public `Transport`
/// interface unchanged.
///
/// # Usage
///
/// ```ignore
/// let mut t = Transport::spawn(&["emacs-mcp-stdio.sh"])?;
/// t.send(&json_value)?;
/// let response = t.recv()?;
/// ```
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::Value;

use super::error::McpError;

pub struct Transport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Transport {
    /// Spawn a child process and wire up stdin/stdout for JSON-RPC.
    ///
    /// `argv` must be non-empty; `argv[0]` is the executable path and
    /// `argv[1..]` are its arguments.
    pub fn spawn(argv: &[String]) -> Result<Self, McpError> {
        if argv.is_empty() {
            return Err(McpError::Spawn(
                "server command must not be empty".to_string(),
            ));
        }

        let mut cmd = Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // let server stderr flow to CLI stderr

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Spawn(format!("cannot spawn '{}': {}", argv[0], e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Spawn("child stdin unavailable".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Spawn("child stdout unavailable".to_string()))?;

        Ok(Transport {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    /// Send one JSON-RPC message (newline-terminated).
    pub fn send(&mut self, msg: &Value) -> Result<(), McpError> {
        let mut line = serde_json::to_string(msg)
            .map_err(|e| McpError::Transport(format!("serialize error: {}", e)))?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| McpError::Transport(format!("write error: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| McpError::Transport(format!("flush error: {}", e)))?;
        Ok(())
    }

    /// Read one newline-delimited JSON-RPC response.
    pub fn recv(&mut self) -> Result<Value, McpError> {
        let mut line = String::new();
        let n = self
            .stdout
            .read_line(&mut line)
            .map_err(|e| McpError::Transport(format!("read error: {}", e)))?;
        if n == 0 {
            return Err(McpError::Transport(
                "server closed stdout (EOF) without sending a response".to_string(),
            ));
        }
        serde_json::from_str(line.trim()).map_err(|e| {
            McpError::Transport(format!("JSON parse error: {} (line: {:?})", e, line.trim()))
        })
    }

    /// Kill the child process. Errors are ignored — best-effort cleanup.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.kill();
    }
}
