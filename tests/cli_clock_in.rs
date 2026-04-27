/// Integration tests for `org clock in`.
///
/// The critical quirk tested here: `--resolve` must be sent to the server as
/// the JSON STRING "true"/"false", NOT the JSON boolean true/false.
/// See contract.rs `ServerValue::BoolAsString` and `org-mcp--tool-clock-in` in ../org-mcp/org-mcp.el.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_clock_in_{}_{}.jsonl",
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
// org clock in — CRITICAL: bool-as-string for --resolve
// ---------------------------------------------------------------------------

/// --resolve flag → server receives resolve as the STRING "true" (not bool true).
/// This is the critical contract test for `ServerValue::BoolAsString` in contract.rs.
#[test]
fn test_clock_in_resolve_is_string_true() {
    let log_path = temp_log("resolve_true");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "in",
            "org://abc",
            "--resolve",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-in");
    let args = &req["params"]["arguments"];

    // CRITICAL: resolve must be the JSON string "true", NOT the boolean true
    assert_eq!(
        args["resolve"],
        serde_json::Value::String("true".to_string()),
        "resolve must be the JSON STRING \"true\", not bool true — see ServerValue::BoolAsString in contract.rs"
    );
}

/// No --resolve flag → server receives resolve as the STRING "false" (still a string, not absent).
#[test]
fn test_clock_in_no_resolve_is_string_false() {
    let log_path = temp_log("resolve_false");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "in", "foo"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-in");
    let args = &req["params"]["arguments"];

    // resolve is always sent — as STRING "false" when flag not given
    assert_eq!(
        args["resolve"],
        serde_json::Value::String("false".to_string()),
        "resolve must be the JSON STRING \"false\" when --resolve not given"
    );
}

/// --at timestamp is forwarded to server.
#[test]
fn test_clock_in_at_forwarded() {
    let log_path = temp_log("at_fwd");
    let ts = "2026-04-27T08:00:00Z";
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "in",
            "org://abc",
            "--at",
            ts,
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-in");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["start_time"].as_str(),
        Some(ts),
        "start_time timestamp must be forwarded"
    );
}

/// No --at → arguments object does NOT contain the "at" key.
#[test]
fn test_clock_in_no_at_key_absent() {
    let log_path = temp_log("at_absent");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "in", "org://abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-in");
    let args = &req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("start_time"),
        "start_time key must be absent when --at not given"
    );
}

/// URI prefix stripped: org://abc → uri = "abc" in arguments.
#[test]
fn test_clock_in_uri_stripped() {
    let log_path = temp_log("uri_strip");
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "in", "org://abc123"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-in");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["uri"].as_str(),
        Some("abc123"),
        "org:// prefix must be stripped"
    );
}

/// ok envelope: data.resolve echoes the string "true" when --resolve given.
#[test]
fn test_clock_in_ok_envelope_with_resolve() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "in",
            "org://abc",
            "--resolve",
        ])
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    // Mock echoes the resolve value back — must be the string "true"
    assert_eq!(
        v["data"]["resolve"],
        serde_json::Value::String("true".to_string()),
        "response data.resolve must be string \"true\""
    );
}

/// Tool error on clock in → ok:false, exit 1.
#[test]
fn test_clock_in_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "in", "abc"])
        .env("MOCK_TOOL_ERROR", "org-clock-in")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
