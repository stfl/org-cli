use std::io::{BufRead, BufReader, Write};
/// Tests for MCP initialize handshake and tools/list via the mock server.
/// These tests spawn the mock_org_mcp binary directly using the transport layer.
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn spawn_mock() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mock_org_mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn mock_org_mcp")
}

fn send_recv(child: &mut std::process::Child, msg: Value) -> Value {
    let stdin = child.stdin.as_mut().unwrap();
    let mut line = serde_json::to_string(&msg).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).unwrap();
    serde_json::from_str(response_line.trim()).expect("invalid JSON from mock")
}

#[test]
fn test_initialize_handshake() {
    let mut child = spawn_mock();

    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "org-cli-test", "version": "0.1.0"},
            "capabilities": {}
        }
    });

    let resp = send_recv(&mut child, init_req);

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(
        resp["error"].is_null(),
        "initialize should not return an error"
    );
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert!(resp["result"]["capabilities"]["tools"].is_object());
    assert_eq!(resp["result"]["serverInfo"]["name"], "mock-org-mcp");

    child.kill().ok();
}

#[test]
fn test_notifications_initialized_no_response() {
    let mut child = spawn_mock();

    // First do initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    let stdin = child.stdin.as_mut().unwrap();
    let mut line = serde_json::to_string(&init_req).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();

    // Send notification (no id)
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let mut notif_line = serde_json::to_string(&notif).unwrap();
    notif_line.push('\n');
    stdin.write_all(notif_line.as_bytes()).unwrap();

    // Now send tools/list — only two responses should come back (for id=1 and id=2)
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let mut list_line = serde_json::to_string(&list_req).unwrap();
    list_line.push('\n');
    stdin.write_all(list_line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);

    // First response: initialize
    let mut r1 = String::new();
    reader.read_line(&mut r1).unwrap();
    let v1: Value = serde_json::from_str(r1.trim()).unwrap();
    assert_eq!(v1["id"], 1);

    // Second response: tools/list (NOT a response to the notification)
    let mut r2 = String::new();
    reader.read_line(&mut r2).unwrap();
    let v2: Value = serde_json::from_str(r2.trim()).unwrap();
    assert_eq!(v2["id"], 2, "notification must not produce a response");
    assert!(v2["result"]["tools"].is_array());

    child.kill().ok();
}

#[test]
fn test_tools_list_returns_expected_tools() {
    let mut child = spawn_mock();

    // Handshake first
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let _init_resp = send_recv(&mut child, init);

    // Send initialized notification (no response expected)
    {
        let stdin = child.stdin.as_mut().unwrap();
        let notif = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        let mut line = serde_json::to_string(&notif).unwrap();
        line.push('\n');
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    let list_req = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let resp = send_recv(&mut child, list_req);

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools must be array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    let required = [
        "org-read",
        "org-read-headline",
        "org-outline",
        "org-update-todo-state",
        "org-add-todo",
        "org-edit-body",
        "org-edit-rename",
        "org-edit-properties",
        "org-edit-tags",
        "org-edit-priority",
        "org-edit-scheduled",
        "org-edit-deadline",
        "org-edit-log-note",
        "org-clock-status",
        "org-clock-in",
        "org-clock-out",
        "org-clock-add",
        "org-clock-delete",
        "org-clock-dangling",
        "org-config-todo",
        "org-config-tags",
        "org-config-tag-candidates",
        "org-config-priority",
        "org-config-files",
        "org-config-clock",
        "org-ql-query",
        // GTD tools (present by default)
        "query-inbox",
        "query-next",
        "query-backlog",
    ];

    for name in &required {
        assert!(names.contains(name), "missing tool: {}", name);
    }

    child.kill().ok();
}

#[test]
fn test_tools_list_no_gtd_when_env_set() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mock_org_mcp"))
        .env("MOCK_NO_GTD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn mock_org_mcp");

    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let _init_resp = send_recv(&mut child, init);

    let list_req = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let resp = send_recv(&mut child, list_req);

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools must be array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(
        !names.contains(&"query-inbox"),
        "query-inbox should be absent with MOCK_NO_GTD=1"
    );
    assert!(
        !names.contains(&"query-next"),
        "query-next should be absent with MOCK_NO_GTD=1"
    );
    assert!(
        !names.contains(&"query-backlog"),
        "query-backlog should be absent with MOCK_NO_GTD=1"
    );
    // Core tools still present
    assert!(names.contains(&"org-read"));

    child.kill().ok();
}
