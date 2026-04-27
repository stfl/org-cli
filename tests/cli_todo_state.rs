/// Integration tests for `org todo state`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_todo_state_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// Happy path: org todo state org://abc DONE → ok envelope with success/new_state/uri.
#[test]
fn test_todo_state_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "todo", "state", "org://abc", "DONE"])
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
    assert_eq!(
        v["data"]["new_state"], "DONE",
        "data.new_state must be DONE"
    );
    assert_eq!(v["data"]["uri"], "org://abc", "data.uri must be org://abc");
}

/// URI prefix stripped: request log shows arguments.uri == "abc" (no org://).
#[test]
fn test_todo_state_strips_org_prefix() {
    let log_path = temp_log();

    let output = org_bin()
        .args(["--server", mock_bin(), "todo", "state", "org://abc", "DONE"])
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
                && v["params"]["name"].as_str() == Some("org-update-todo-state")
        })
        .expect("must find a tools/call for org-update-todo-state");

    let sent_uri = call_req["params"]["arguments"]["uri"]
        .as_str()
        .expect("uri must be a string");
    assert_eq!(
        sent_uri, "abc",
        "CLI must strip org:// before sending; got: {sent_uri}"
    );
}

/// All flags: --from and --note are forwarded to the request.
#[test]
fn test_todo_state_all_flags_forwarded() {
    let log_path = temp_log();

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "state",
            "foo",
            "NEXT",
            "--from",
            "TODO",
            "--note",
            "x",
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
                && v["params"]["name"].as_str() == Some("org-update-todo-state")
        })
        .expect("must find a tools/call for org-update-todo-state");

    let args = &call_req["params"]["arguments"];
    assert_eq!(args["from"].as_str(), Some("TODO"), "from must be TODO");
    assert_eq!(args["note"].as_str(), Some("x"), "note must be x");
}

/// Omit None: no --from or --note → those keys must NOT appear in the request.
#[test]
fn test_todo_state_omits_optional_keys_when_absent() {
    let log_path = temp_log();

    let output = org_bin()
        .args(["--server", mock_bin(), "todo", "state", "foo", "DONE"])
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
                && v["params"]["name"].as_str() == Some("org-update-todo-state")
        })
        .expect("must find a tools/call for org-update-todo-state");

    let args = &call_req["params"]["arguments"];
    assert!(
        !args.as_object().unwrap().contains_key("from"),
        "from key must be absent when --from not given; args: {args}"
    );
    assert!(
        !args.as_object().unwrap().contains_key("note"),
        "note key must be absent when --note not given; args: {args}"
    );
}

/// Tool error: MOCK_TOOL_ERROR=org-update-todo-state → ok:false, exit 1.
#[test]
fn test_todo_state_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "todo", "state", "foo", "DONE"])
        .env("MOCK_TOOL_ERROR", "org-update-todo-state")
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
