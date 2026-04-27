/// Integration tests for GTD commands when server has no GTD tools (MOCK_NO_GTD=1).
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// With MOCK_NO_GTD=1, `org query inbox` returns ok:false, kind=usage, exit 2,
/// and message mentions "not advertised".
#[test]
fn test_no_gtd_inbox_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "inbox"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], false, "envelope ok must be false");
    assert_eq!(v["error"]["kind"], "usage", "error kind must be 'usage'");
    assert_eq!(v["exit_code"], 2, "exit_code in envelope must be 2");

    let msg = v["error"]["message"]
        .as_str()
        .expect("message must be string");
    assert!(
        msg.contains("not advertised"),
        "message must mention 'not advertised'; got: {msg}"
    );
}

/// With MOCK_NO_GTD=1, `org query next` returns ok:false, kind=usage, exit 2.
#[test]
fn test_no_gtd_next_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "next"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);

    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("not advertised"),
        "message must mention 'not advertised'; got: {msg}"
    );
}

/// With MOCK_NO_GTD=1, `org query backlog` returns ok:false, kind=usage, exit 2.
#[test]
fn test_no_gtd_backlog_blocked() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "backlog"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for GTD capability missing"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

/// With MOCK_NO_GTD=1, `org query run` STILL works (org-ql-query is not GTD).
#[test]
fn test_no_gtd_query_run_still_works() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", "(todo)"])
        .env("MOCK_NO_GTD", "1")
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0 — org-ql-query is not GTD\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true, "envelope ok must be true");
}
