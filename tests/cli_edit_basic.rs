/// Integration tests for `org edit rename`, `org edit tags`, `org edit priority`, `org edit log-note`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_edit_basic_{}_{}.jsonl",
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
// org edit rename
// ---------------------------------------------------------------------------

/// Happy path: rename → ok envelope, success true.
#[test]
fn test_rename_ok_envelope() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "rename",
            "org://abc",
            "--from",
            "Old Title",
            "--to",
            "New Title",
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

/// URI prefix stripped in rename request.
#[test]
fn test_rename_strips_uri_prefix() {
    let log_path = temp_log("rename_strip");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "rename",
            "org://abc",
            "--from",
            "Old",
            "--to",
            "New",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-rename");
    let args = &req["params"]["arguments"];
    assert_eq!(args["uri"].as_str(), Some("abc"), "uri must be stripped");
    assert_eq!(args["from"].as_str(), Some("Old"));
    assert_eq!(args["to"].as_str(), Some("New"));
    // Contract assertion: all three keys present
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("from"));
    assert!(args.as_object().unwrap().contains_key("to"));
}

/// Tool error on rename → ok:false, exit 1.
#[test]
fn test_rename_tool_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "rename",
            "foo",
            "--from",
            "A",
            "--to",
            "B",
        ])
        .env("MOCK_TOOL_ERROR", "org-edit-rename")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}

// ---------------------------------------------------------------------------
// org edit tags
// ---------------------------------------------------------------------------

/// Tags forwarded as array: --tag a --tag b → arguments.tags == ["a","b"].
#[test]
fn test_tags_forwarded_as_array() {
    let log_path = temp_log("tags_array");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "tags",
            "org://abc",
            "--tag",
            "a",
            "--tag",
            "b",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-tags");
    let args = &req["params"]["arguments"];
    let tags = &args["tags"];
    assert!(tags.is_array(), "tags must be array");
    let arr: Vec<&str> = tags
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(arr, vec!["a", "b"]);
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("tags"));
}

/// No tags → arguments.tags == [] (clear semantics).
#[test]
fn test_tags_empty_sends_empty_array() {
    let log_path = temp_log("tags_empty");
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "tags", "abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-tags");
    let tags = &req["params"]["arguments"]["tags"];
    assert!(tags.is_array(), "tags must be array even when empty");
    assert!(
        tags.as_array().unwrap().is_empty(),
        "tags must be [] when no --tag given"
    );
}

// ---------------------------------------------------------------------------
// org edit priority
// ---------------------------------------------------------------------------

/// --priority A → arguments.priority == "A".
#[test]
fn test_priority_with_value() {
    let log_path = temp_log("priority_a");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "priority",
            "org://abc",
            "--priority",
            "A",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-priority");
    let args = &req["params"]["arguments"];
    assert_eq!(args["priority"], serde_json::json!("A"));
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("priority"));
}

/// No --priority → arguments.priority == null (clear semantics — explicit null, not absent).
#[test]
fn test_priority_none_sends_null() {
    let log_path = temp_log("priority_none");
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "priority", "abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-priority");
    let args = &req["params"]["arguments"];
    // Key must be present (explicit null distinguishes "clear" from "missing")
    assert!(
        args.as_object().unwrap().contains_key("priority"),
        "priority key must be present even when --priority not given (explicit null)"
    );
    assert_eq!(
        args["priority"],
        serde_json::Value::Null,
        "priority must be null when absent"
    );
}

// ---------------------------------------------------------------------------
// org edit log-note
// ---------------------------------------------------------------------------

/// --note "hello" → request log shows note field.
#[test]
fn test_log_note_forwarded() {
    let log_path = temp_log("log_note");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "log-note",
            "org://abc",
            "--note",
            "hello",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-log-note");
    let args = &req["params"]["arguments"];
    assert_eq!(args["note"].as_str(), Some("hello"));
    assert_eq!(args["uri"].as_str(), Some("abc"), "uri must be stripped");
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("note"));
}

/// Tool error on log-note → ok:false, exit 1.
#[test]
fn test_log_note_tool_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "log-note",
            "abc",
            "--note",
            "x",
        ])
        .env("MOCK_TOOL_ERROR", "org-edit-log-note")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
