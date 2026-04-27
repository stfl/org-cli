/// Integration tests for `org read`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_read_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "foo"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0, got {:?}\nstderr: {}\nstdout: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true, "envelope ok must be true");
    assert!(v["data"]["title"].is_string(), "data.title must be present");
    assert!(v["data"]["todo"].is_string(), "data.todo must be present");
    assert!(v["data"]["uri"].is_string(), "data.uri must be present");
    assert!(
        v["data"]["children"].is_array(),
        "data.children must be array"
    );
}

#[test]
fn test_read_accepts_org_prefix() {
    // org://abc should be accepted and stripped to "abc" before sending
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "org://abc"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0 when reading with org:// prefix"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], true);
    // The mock echoes back org://<bare> in the uri field.
    // Since we sent "abc", the mock returns "org://abc".
    assert_eq!(v["data"]["uri"], "org://abc");
}

/// With MOCK_RECORD_REQUESTS=1, verify the CLI strips org:// before sending.
#[test]
fn test_read_strips_org_prefix_in_request() {
    let log_path = std::env::temp_dir().join(format!(
        "org_cli_read_req_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));

    let output = org_bin()
        .args(["--server", mock_bin(), "read", "org://abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path); // cleanup

    // Find the tools/call request for org-read
    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-read")
        })
        .expect("must find a tools/call request for org-read in the log");

    let sent_uri = call_req["params"]["arguments"]["uri"]
        .as_str()
        .expect("uri must be a string in the logged request");
    assert_eq!(
        sent_uri, "abc",
        "CLI must strip org:// before sending to server; got: {sent_uri}"
    );
}

/// MOCK_TOOL_ERROR=org-read → ok:false, exit 1.
#[test]
fn test_read_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "foo"])
        .env("MOCK_TOOL_ERROR", "org-read")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 for tool error"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
    assert_eq!(v["exit_code"], 1);
}
