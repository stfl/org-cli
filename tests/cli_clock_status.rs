/// Integration tests for `org clock status` and `org clock dangling`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_clock_status_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

fn find_tools_call(log: &str, tool: &str) -> serde_json::Value {
    log.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call") && v["params"]["name"].as_str() == Some(tool)
        })
        .unwrap_or_else(|| panic!("must find a tools/call for {tool}"))
}

// ---------------------------------------------------------------------------
// org clock status
// ---------------------------------------------------------------------------

/// Happy path: clock status → ok envelope, data.clocked_in == true, data.current.uri correct.
#[test]
fn test_clock_status_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "status"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["clocked_in"], true);
    assert_eq!(v["data"]["current"]["uri"], "org://current-task");
}

/// clock status sends empty arguments object.
#[test]
fn test_clock_status_empty_arguments() {
    let log_path = temp_log("status_args");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "status"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-get-active");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().map(|o| o.is_empty()).unwrap_or(false),
        "clock status must send empty arguments object, got: {args}"
    );
}

/// Tool error on clock status → ok:false, exit 1.
#[test]
fn test_clock_status_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "status"])
        .env("MOCK_TOOL_ERROR", "org-clock-get-active")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}

// ---------------------------------------------------------------------------
// org clock dangling
// ---------------------------------------------------------------------------

/// Happy path: clock dangling → ok envelope, data.dangling array, data.count == 1.
#[test]
fn test_clock_dangling_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "dangling"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert!(
        v["data"]["dangling"].is_array(),
        "data.dangling must be an array"
    );
    assert_eq!(
        v["data"]["dangling"].as_array().unwrap().len(),
        1,
        "data.dangling must have 1 entry"
    );
    assert_eq!(v["data"]["count"], 1);
}

/// clock dangling sends empty arguments object.
#[test]
fn test_clock_dangling_empty_arguments() {
    let log_path = temp_log("dangling_args");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "dangling"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-find-dangling");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().map(|o| o.is_empty()).unwrap_or(false),
        "clock dangling must send empty arguments object, got: {args}"
    );
}

/// Tool error on clock dangling → ok:false, exit 1.
#[test]
fn test_clock_dangling_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "dangling"])
        .env("MOCK_TOOL_ERROR", "org-clock-find-dangling")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
