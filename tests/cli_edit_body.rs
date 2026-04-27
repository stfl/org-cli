/// Integration tests for `org edit body`.
/// CRITICAL: server param key is `resource_uri` (not `uri`) and `new_body`/`old_body`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_edit_body_{}_{}.jsonl",
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

/// Happy path: edit body → ok envelope, success true.
#[test]
fn test_body_ok_envelope() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "org://abc",
            "--new",
            "Hello world",
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
}

/// CRITICAL: request log must show `resource_uri`, NOT `uri`.
#[test]
fn test_body_uses_resource_uri_key() {
    let log_path = temp_log("resource_uri");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "org://abc",
            "--new",
            "Hello",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];

    // The URI key MUST be `resource_uri`, not `uri`
    assert!(
        args.as_object().unwrap().contains_key("resource_uri"),
        "resource_uri key must be present; args: {args}"
    );
    assert!(
        !args.as_object().unwrap().contains_key("uri"),
        "uri key must NOT be present for edit body; args: {args}"
    );
    assert_eq!(
        args["resource_uri"].as_str(),
        Some("abc"),
        "resource_uri must be stripped of org:// prefix"
    );
}

/// --new maps to new_body in the request.
#[test]
fn test_body_new_maps_to_new_body() {
    let log_path = temp_log("new_body");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "abc",
            "--new",
            "my new content",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];
    assert_eq!(args["new_body"].as_str(), Some("my new content"));
}

/// --append true is sent as bool true.
#[test]
fn test_body_append_true_sent_as_bool() {
    let log_path = temp_log("append_true");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "abc",
            "--new",
            "more",
            "--append",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["append"],
        serde_json::json!(true),
        "append must be bool true"
    );
}

/// Default --append false is sent as bool false (not omitted — it's always sent).
#[test]
fn test_body_append_default_false_sent_as_bool() {
    let log_path = temp_log("append_false");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "abc",
            "--new",
            "content",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().unwrap().contains_key("append"),
        "append key must always be present"
    );
    assert_eq!(
        args["append"],
        serde_json::json!(false),
        "append must be bool false by default"
    );
}

/// --old omitted when not given.
#[test]
fn test_body_old_omitted_when_absent() {
    let log_path = temp_log("old_absent");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "abc",
            "--new",
            "content",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("old_body"),
        "old_body key must be absent when --old not given; args: {args}"
    );
}

/// --old present → old_body in request.
#[test]
fn test_body_old_present_when_given() {
    let log_path = temp_log("old_present");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "abc",
            "--new",
            "new content",
            "--old",
            "old content",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-body");
    let args = &req["params"]["arguments"];
    assert_eq!(args["old_body"].as_str(), Some("old content"));
}

/// Tool error → ok:false, exit 1.
#[test]
fn test_body_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "body", "abc", "--new", "x"])
        .env("MOCK_TOOL_ERROR", "org-edit-body")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
