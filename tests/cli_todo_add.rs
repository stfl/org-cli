/// Integration tests for `org todo add`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_todo_add_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// Happy path: org todo add --parent org://parent --title "Hello" --state TODO
/// → ok envelope, data.success, data.uri matches ^org://new-id-.
#[test]
fn test_todo_add_ok_envelope() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "org://parent",
            "--title",
            "Hello",
            "--state",
            "TODO",
        ])
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
    assert_eq!(v["data"]["success"], true, "data.success must be true");
    let uri = v["data"]["uri"]
        .as_str()
        .expect("data.uri must be a string");
    assert!(
        uri.starts_with("org://new-id-"),
        "data.uri must start with org://new-id-; got: {uri}"
    );
}

/// Prefix stripped on parent: request log shows arguments.parent_uri == "parent".
#[test]
fn test_todo_add_strips_parent_prefix() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "org://parent",
            "--title",
            "T",
            "--state",
            "TODO",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let sent = call_req["params"]["arguments"]["parent_uri"]
        .as_str()
        .expect("parent_uri must be a string");
    assert_eq!(
        sent, "parent",
        "CLI must strip org:// from parent; got: {sent}"
    );
}

/// Tags forwarded as array: --tag a --tag b --tag c → arguments.tags == ["a","b","c"].
#[test]
fn test_todo_add_tags_forwarded_as_array() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
            "--tag",
            "a",
            "--tag",
            "b",
            "--tag",
            "c",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let tags = &call_req["params"]["arguments"]["tags"];
    assert!(tags.is_array(), "tags must be an array");
    let arr: Vec<&str> = tags
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(arr, vec!["a", "b", "c"], "tags array must be [a, b, c]");
}

/// Empty tags: no --tag → arguments.tags == [].
#[test]
fn test_todo_add_empty_tags_sent_as_empty_array() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let tags = &call_req["params"]["arguments"]["tags"];
    assert!(tags.is_array(), "tags must be an array even when empty");
    assert!(
        tags.as_array().unwrap().is_empty(),
        "tags array must be empty when no --tag given"
    );
}

/// Body: omitted when absent, included when present.
#[test]
fn test_todo_add_body_omitted_when_absent() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let args = &call_req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("body"),
        "body key must be absent when --body not given; args: {args}"
    );
}

#[test]
fn test_todo_add_body_included_when_present() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
            "--body",
            "some body text",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let body = call_req["params"]["arguments"]["body"]
        .as_str()
        .expect("body must be a string when --body given");
    assert_eq!(body, "some body text");
}

/// after_uri: --after org://target → arguments.after_uri == "target"; omitted when no --after.
#[test]
fn test_todo_add_after_uri_stripped_and_forwarded() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
            "--after",
            "org://target",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let after = call_req["params"]["arguments"]["after_uri"]
        .as_str()
        .expect("after_uri must be a string when --after given");
    assert_eq!(
        after, "target",
        "CLI must strip org:// from after; got: {after}"
    );
}

#[test]
fn test_todo_add_after_uri_omitted_when_absent() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-add-todo")
        })
        .expect("must find a tools/call for org-add-todo");

    let args = &call_req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("after_uri"),
        "after_uri must be absent when --after not given; args: {args}"
    );
}

/// Tool error: MOCK_TOOL_ERROR=org-add-todo → ok:false, exit 1.
#[test]
fn test_todo_add_tool_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "p",
            "--title",
            "T",
            "--state",
            "TODO",
        ])
        .env("MOCK_TOOL_ERROR", "org-add-todo")
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

/// Missing required flag --state → clap usage error, exit 2.
#[test]
fn test_todo_add_missing_state_is_usage_error() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "add",
            "--parent",
            "x",
            "--title",
            "y",
        ])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for missing required --state"
    );
}
