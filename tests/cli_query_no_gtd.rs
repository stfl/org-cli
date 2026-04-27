/// Integration tests for GTD commands when server has no GTD tools (MOCK_NO_GTD=1).
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// Read a JSON-line request log produced by MOCK_RECORD_REQUESTS=1 and return
/// the methods (in order) of every JSON-RPC request received by the mock.
fn read_request_methods(path: &std::path::Path) -> Vec<String> {
    let raw = std::fs::read_to_string(path).expect("request log must exist");
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value =
                serde_json::from_str(l).expect("each log line must be valid JSON");
            v.get("method")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

/// With MOCK_NO_GTD=1, `org query inbox` returns ok:false, kind=usage, exit 2,
/// and message mentions "not advertised".
#[test]
fn test_no_gtd_inbox_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "inbox"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], false, "envelope ok must be false");
    assert_eq!(v["error"]["kind"], "usage", "error kind must be 'usage'");
    assert_eq!(v["exit_code"], 2, "exit_code in envelope must be 2");

    let msg = v["error"]["message"]
        .as_str()
        .expect("message must be string");
    assert!(
        msg.contains("not advertised"),
        "message must mention 'not advertised'; got: {msg}"
    );
}

/// With MOCK_NO_GTD=1, `org query next` returns ok:false, kind=usage, exit 2.
#[test]
fn test_no_gtd_next_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "next"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);

    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("not advertised"),
        "message must mention 'not advertised'; got: {msg}"
    );
}

/// With MOCK_NO_GTD=1, `org query backlog` returns ok:false, kind=usage, exit 2.
#[test]
fn test_no_gtd_backlog_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "backlog"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

/// `org query inbox` against MOCK_NO_GTD=1 must NOT call tools/list — the
/// request log should contain exactly one tools/call (proving the
/// try-call+method-not-found path replaced the discovery pre-check).
#[test]
fn test_no_gtd_inbox_uses_single_rpc() {
    let log_path = std::env::temp_dir().join(format!(
        "org_cli_no_gtd_req_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));

    let output = org_bin()
        .args(["--server", mock_bin(), "query", "inbox"])
        .env("MOCK_NO_GTD", "1")
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    // Behavior contract still holds.
    assert_eq!(output.status.code(), Some(2));

    let methods = read_request_methods(&log_path);
    let _ = std::fs::remove_file(&log_path);
    let calls: Vec<&str> = methods.iter().map(String::as_str).collect();

    // Exactly one tools/call, zero tools/list. initialize is allowed.
    let tools_list_count = calls.iter().filter(|m| **m == "tools/list").count();
    let tools_call_count = calls.iter().filter(|m| **m == "tools/call").count();
    assert_eq!(
        tools_list_count, 0,
        "expected ZERO tools/list requests on the GTD path; got methods: {calls:?}"
    );
    assert_eq!(
        tools_call_count, 1,
        "expected exactly ONE tools/call request; got methods: {calls:?}"
    );
}

/// With MOCK_NO_GTD=1, `org query run` STILL works (org-ql-query is not GTD).
#[test]
fn test_no_gtd_query_run_still_works() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", "(todo)"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0 — org-ql-query is not GTD\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true, "envelope ok must be true");
}
