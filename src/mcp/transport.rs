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
///
/// # Read timeout
///
/// `recv()` honors `set_timeout(Some(Duration))`. None or `Duration::ZERO`
/// disables the gate. The reader runs on a dedicated thread (mpsc channel) so
/// the main loop can wake on `recv_timeout`. The thread is spawned lazily on
/// the first `recv()` so transports that only `send()` pay no overhead.
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::Value;

use super::error::McpError;

pub struct Transport {
    child: Child,
    stdin: ChildStdin,
    /// `Some` until the reader thread is started; then taken and moved into the thread.
    stdout: Option<BufReader<ChildStdout>>,
    /// Lazily started on first recv().
    reader: Option<ReaderHandle>,
    timeout: Option<Duration>,
}

struct ReaderHandle {
    rx: Receiver<Result<String, std::io::Error>>,
    _handle: JoinHandle<()>,
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
            stdout: Some(BufReader::new(stdout)),
            reader: None,
            timeout: None,
        })
    }

    /// Configure the per-`recv()` timeout. `None` or `Some(Duration::ZERO)`
    /// disables the gate (waits indefinitely). Setting takes effect on the
    /// next `recv()` call.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = match timeout {
            Some(d) if d.is_zero() => None,
            other => other,
        };
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

    /// Lazily spawn the reader thread (idempotent).
    fn ensure_reader(&mut self) {
        if self.reader.is_some() {
            return;
        }
        // Take the BufReader out of self so the thread can own it.
        let mut stdout = self
            .stdout
            .take()
            .expect("Transport::stdout must be Some until the reader is started exactly once");
        let (tx, rx) = channel::<Result<String, std::io::Error>>();
        let handle = thread::spawn(move || {
            loop {
                let mut line = String::new();
                match stdout.read_line(&mut line) {
                    Ok(0) => break, // EOF — drop tx, main thread will see RecvTimeoutError::Disconnected
                    Ok(_) => {
                        if tx.send(Ok(line)).is_err() {
                            break; // Receiver dropped (Transport was dropped)
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }
            }
        });
        self.reader = Some(ReaderHandle {
            rx,
            _handle: handle,
        });
    }

    /// Read one newline-delimited JSON-RPC response.
    /// Honors `set_timeout`: returns a `kind=transport` error on expiry.
    pub fn recv(&mut self) -> Result<Value, McpError> {
        self.ensure_reader();
        let reader = self
            .reader
            .as_ref()
            .expect("ensure_reader must populate self.reader");

        let raw = match self.timeout {
            None => reader.rx.recv().map_err(|_| {
                McpError::Transport(
                    "server closed stdout (EOF) without sending a response".to_string(),
                )
            })?,
            Some(d) => match reader.rx.recv_timeout(d) {
                Ok(v) => v,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(McpError::Transport(format!(
                        "no response from server within {}s (timeout)",
                        d.as_secs()
                    )));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(McpError::Transport(
                        "server closed stdout (EOF) without sending a response".to_string(),
                    ));
                }
            },
        };

        let line = raw.map_err(|e| McpError::Transport(format!("read error: {}", e)))?;
        if line.is_empty() {
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
