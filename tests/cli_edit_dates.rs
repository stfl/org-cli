/// Integration tests for `org edit scheduled` and `org edit deadline`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_edit_dates_{}_{}.jsonl",
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
// org edit scheduled
// ---------------------------------------------------------------------------

/// --date "2026-04-30" → request log shows that string.
#[test]
fn test_scheduled_with_date() {
    let log_path = temp_log("sched_date");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "scheduled",
            "org://abc",
            "--date",
            "2026-04-30",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(
        output.status.success(),
        "stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-scheduled");
    let args = &req["params"]["arguments"];
    assert_eq!(args["date"].as_str(), Some("2026-04-30"));
    assert_eq!(args["uri"].as_str(), Some("abc"), "uri must be stripped");
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("date"));
}

/// No --date → arguments.date == null (clear semantics).
#[test]
fn test_scheduled_no_date_sends_null() {
    let log_path = temp_log("sched_null");
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "scheduled", "abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-scheduled");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().unwrap().contains_key("date"),
        "date key must be present (explicit null for clear semantics)"
    );
    assert_eq!(
        args["date"],
        serde_json::Value::Null,
        "date must be null when absent"
    );
}

/// Tool error on scheduled → ok:false, exit 1.
#[test]
fn test_scheduled_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "scheduled", "abc"])
        .env("MOCK_TOOL_ERROR", "org-edit-scheduled")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}

// ---------------------------------------------------------------------------
// org edit deadline
// ---------------------------------------------------------------------------

/// --date "2026-05-15 14:00" → request log shows that string.
#[test]
fn test_deadline_with_date() {
    let log_path = temp_log("deadline_date");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "deadline",
            "org://abc",
            "--date",
            "2026-05-15 14:00",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(
        output.status.success(),
        "stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-deadline");
    let args = &req["params"]["arguments"];
    assert_eq!(args["date"].as_str(), Some("2026-05-15 14:00"));
    assert_eq!(args["uri"].as_str(), Some("abc"));
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("date"));
}

/// No --date → arguments.date == null (clear semantics).
#[test]
fn test_deadline_no_date_sends_null() {
    let log_path = temp_log("deadline_null");
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "deadline", "abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-deadline");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().unwrap().contains_key("date"),
        "date key must be present (explicit null for clear semantics)"
    );
    assert_eq!(
        args["date"],
        serde_json::Value::Null,
        "date must be null when absent"
    );
}

/// Tool error on deadline → ok:false, exit 1.
#[test]
fn test_deadline_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "deadline", "abc"])
        .env("MOCK_TOOL_ERROR", "org-edit-deadline")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
