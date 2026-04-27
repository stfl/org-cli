/// Integration tests for `org clock add` and `org clock delete`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_clock_modify_{}_{}.jsonl",
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
// org clock add
// ---------------------------------------------------------------------------

/// Happy path: clock add → ok envelope, timestamps and URI forwarded.
#[test]
fn test_clock_add_ok() {
    let start = "2026-04-27T08:00:00Z";
    let end = "2026-04-27T09:00:00Z";
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "add",
            "org://abc",
            "--start",
            start,
            "--end",
            end,
        ])
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
    assert_eq!(v["data"]["success"], true);
}

/// clock add request log: uri stripped, start and end forwarded.
#[test]
fn test_clock_add_params_in_request() {
    let log_path = temp_log("add_params");
    let start = "2026-04-27T08:00:00Z";
    let end = "2026-04-27T09:00:00Z";
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "add",
            "org://abc",
            "--start",
            start,
            "--end",
            end,
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-add");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["uri"].as_str(),
        Some("abc"),
        "org:// prefix must be stripped"
    );
    assert_eq!(
        args["start"].as_str(),
        Some(start),
        "start must be forwarded"
    );
    assert_eq!(args["end"].as_str(), Some(end), "end must be forwarded");
}

/// Missing --start → clap usage error, exit 2.
#[test]
fn test_clock_add_missing_start_exit2() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "add",
            "org://abc",
            "--end",
            "2026-04-27T09:00:00Z",
        ])
        .output()
        .expect("failed to run org");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing --start must produce exit 2"
    );
}

/// Missing --end → clap usage error, exit 2.
#[test]
fn test_clock_add_missing_end_exit2() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "add",
            "org://abc",
            "--start",
            "2026-04-27T08:00:00Z",
        ])
        .output()
        .expect("failed to run org");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing --end must produce exit 2"
    );
}

/// Tool error on clock add → ok:false, exit 1.
#[test]
fn test_clock_add_tool_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "add",
            "abc",
            "--start",
            "2026-04-27T08:00:00Z",
            "--end",
            "2026-04-27T09:00:00Z",
        ])
        .env("MOCK_TOOL_ERROR", "org-clock-add")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}

// ---------------------------------------------------------------------------
// org clock delete
// ---------------------------------------------------------------------------

/// Happy path: clock delete → ok envelope.
#[test]
fn test_clock_delete_ok() {
    let at = "2026-04-27T08:00:00Z";
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "delete",
            "org://abc",
            "--at",
            at,
        ])
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
    assert_eq!(v["data"]["success"], true);
}

/// clock delete request log: uri stripped, at forwarded.
#[test]
fn test_clock_delete_params_in_request() {
    let log_path = temp_log("delete_params");
    let at = "2026-04-27T08:30:00Z";
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "delete",
            "org://abc",
            "--at",
            at,
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-clock-delete");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["uri"].as_str(),
        Some("abc"),
        "org:// prefix must be stripped"
    );
    assert_eq!(args["at"].as_str(), Some(at), "at must be forwarded");
}

/// Missing --at → clap usage error, exit 2.
#[test]
fn test_clock_delete_missing_at_exit2() {
    let output = org_bin()
        .args(["--server", mock_bin(), "clock", "delete", "org://abc"])
        .output()
        .expect("failed to run org");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing --at must produce exit 2"
    );
}

/// Tool error on clock delete → ok:false, exit 1.
#[test]
fn test_clock_delete_tool_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "delete",
            "abc",
            "--at",
            "2026-04-27T08:00:00Z",
        ])
        .env("MOCK_TOOL_ERROR", "org-clock-delete")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
