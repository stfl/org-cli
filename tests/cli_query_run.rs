/// Integration tests for `org query run`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_query_run_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// `org query run "(todo \"TODO\")"` returns ok envelope with data.matches of length 2.
#[test]
fn test_query_run_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", r#"(todo "TODO")"#])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true, "envelope ok must be true");
    assert!(
        v["data"]["matches"].is_array(),
        "data.matches must be array"
    );
    assert_eq!(
        v["data"]["matches"].as_array().unwrap().len(),
        2,
        "data.matches must have 2 items"
    );
    assert_eq!(v["data"]["count"], 2, "data.count must be 2");
}

/// Query with "empty" in expression returns empty matches.
#[test]
fn test_query_run_empty_result() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", "(empty-marker)"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert_eq!(
        v["data"]["matches"].as_array().unwrap().len(),
        0,
        "empty query must return 0 matches"
    );
    assert_eq!(v["data"]["count"], 0);
}

/// `--files /a /b` is forwarded — verify via request log.
#[test]
fn test_query_run_files_forwarded() {
    let log_path = temp_log_path("files");

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "query",
            "run",
            "(todo)",
            "--files",
            "/a",
            "--files",
            "/b",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("org-ql-query")
        })
        .expect("must find a tools/call for org-ql-query");

    let files = call_req["params"]["arguments"]["files"]
        .as_array()
        .expect("arguments.files must be array");
    assert_eq!(files.len(), 2, "files must have 2 entries");
    assert_eq!(files[0], "/a");
    assert_eq!(files[1], "/b");
}

/// MOCK_TOOL_ERROR=org-ql-query → ok:false, exit 1.
#[test]
fn test_query_run_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", "(todo)"])
        .env("MOCK_TOOL_ERROR", "org-ql-query")
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
