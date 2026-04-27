/// Integration tests for `org clock out`.
///
/// The tricky part: the URI positional argument is OPTIONAL.
/// `org clock out` with no URI means "clock out current" — we just omit the key.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_clock_out_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

fn find_tools_call<'a>(log: &'a str, tool: &str) -> serde_json::Value {
    log.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call") && v["params"]["name"].as_str() == Some(tool)
        })
        .unwrap_or_else(|| panic!("must find a tools/call for {tool}"))
}

// ---------------------------------------------------------------------------
// org clock out (no URI — current clock)
// ---------------------------------------------------------------------------

/// `org clock out` with no URI → ok envelope, data.uri == "org://current".
#[test]
fn test_clock_out_no_uri_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out"])
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
    assert_eq!(
        v["data"]["uri"], "org://current",
        "mock echoes 'org://current' when no URI given"
    );
}

/// `org clock out` with no URI → request log does NOT contain the "uri" key in arguments.
#[test]
fn test_clock_out_no_uri_key_absent() {
    let log_path = temp_log("no_uri");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-out");
    let args = &req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("uri"),
        "uri key must be absent when no positional URI given, got: {args}"
    );
}

// ---------------------------------------------------------------------------
// org clock out (with URI)
// ---------------------------------------------------------------------------

/// `org clock out org://abc` → request log shows arguments.uri == "abc" (stripped).
#[test]
fn test_clock_out_with_uri_stripped() {
    let log_path = temp_log("with_uri");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out", "org://abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-out");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["uri"].as_str(),
        Some("abc"),
        "org:// prefix must be stripped"
    );
}

// ---------------------------------------------------------------------------
// org clock out — --at flag
// ---------------------------------------------------------------------------

/// --at forwarded when given.
#[test]
fn test_clock_out_at_forwarded() {
    let log_path = temp_log("at_fwd");
    let ts = "2026-04-27T09:00:00Z";
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out", "--at", ts])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-out");
    let args = &req["params"]["arguments"];
    assert_eq!(args["at"].as_str(), Some(ts), "at must be forwarded");
}

/// No --at → arguments object does NOT contain the "at" key.
#[test]
fn test_clock_out_no_at_key_absent() {
    let log_path = temp_log("at_absent");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-out");
    let args = &req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("at"),
        "at key must be absent when --at not given"
    );
}

/// Tool error on clock out → ok:false, exit 1.
#[test]
fn test_clock_out_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "out"])
        .env("MOCK_TOOL_ERROR", "org-clock-out")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
