/// Integration tests for `org query <expr>` (bare positional form, PLAN §6).
///
/// These tests verify that the bare form `org query "<expr>"` works identically
/// to `org query run "<expr>"`, while existing subcommands still work.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_query_bare_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// `org query "(todo \"TODO\")"` (bare, no `run`) → ok envelope, data.matches length 2.
#[test]
fn test_query_bare_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", r#"(todo "TODO")"#])
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

/// `org query "(empty-marker)"` → empty matches (same empty substring trick as query run tests).
#[test]
fn test_query_bare_empty_result() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "(empty-marker)"])
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

/// `org query "(todo)" --files /a --files /b` → request log shows files == ["/a", "/b"].
#[test]
fn test_query_bare_files_forwarded() {
    let log_path = temp_log_path("files");

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "query",
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

/// `org query` (NO args, no subcommand) → ok:false, exit 2, kind=usage.
#[test]
fn test_query_bare_no_args_usage_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for no-args query\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

/// Existing `org query run "<expr>"` still works (no regression).
#[test]
fn test_query_run_still_works() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", r#"(todo "TODO")"#])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "query run must still work\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["matches"].as_array().unwrap().len(), 2);
}

/// Existing `org query inbox` still works (no regression).
#[test]
fn test_query_inbox_still_works() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "inbox"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "query inbox must still work\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
}

/// `--server` flag still propagates correctly when bare `query` is used.
#[test]
fn test_query_bare_server_flag_propagates() {
    // Use the mock as server, bare query form — verifies --server is used for transport.
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "(todo)"])
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
}
