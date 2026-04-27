/// Integration tests for `org query inbox/next/backlog` (GTD tools, present).
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_query_gtd_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// `org query inbox` returns ok envelope with data.matches[0].title == "Inbox 1".
#[test]
fn test_query_inbox_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "inbox"])
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
        v["data"]["matches"][0]["title"], "Inbox 1",
        "first match title must be 'Inbox 1'"
    );
}

/// `org query next --tag work` → data.matches[0].tags contains "work".
/// Request log must show arguments.tag == "work".
#[test]
fn test_query_next_with_tag() {
    let log_path = temp_log_path("next_tag");

    let output = org_bin()
        .args(["--server", mock_bin(), "query", "next", "--tag", "work"])
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);

    // Verify tags contain "work"
    let tags = v["data"]["matches"][0]["tags"]
        .as_array()
        .expect("tags must be array");
    assert!(
        tags.iter().any(|t| t.as_str() == Some("work")),
        "tags must contain 'work'"
    );

    // Verify request log shows arguments.tag == "work"
    let log = std::fs::read_to_string(&log_path).expect("request log must exist");
    let _ = std::fs::remove_file(&log_path);

    let call_req: serde_json::Value = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call")
                && v["params"]["name"].as_str() == Some("query-next")
        })
        .expect("must find tools/call for query-next");

    assert_eq!(
        call_req["params"]["arguments"]["tag"], "work",
        "arguments.tag must be 'work'"
    );
}

/// `org query next` (no tag) → request log shows arguments does NOT contain key "tag".
#[test]
fn test_query_next_no_tag_omits_key() {
    let log_path = temp_log_path("next_notag");

    let output = org_bin()
        .args(["--server", mock_bin(), "query", "next"])
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
                && v["params"]["name"].as_str() == Some("query-next")
        })
        .expect("must find tools/call for query-next");

    let args = &call_req["params"]["arguments"];
    assert!(
        args.get("tag").is_none(),
        "arguments must not contain 'tag' key when --tag is omitted; got: {args}"
    );
}

/// `org query backlog` → ok envelope.
#[test]
fn test_query_backlog_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "backlog"])
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
}

/// `org query backlog --tag personal` → request log shows arguments.tag == "personal".
#[test]
fn test_query_backlog_with_tag() {
    let log_path = temp_log_path("backlog_tag");

    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "query",
            "backlog",
            "--tag",
            "personal",
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
                && v["params"]["name"].as_str() == Some("query-backlog")
        })
        .expect("must find tools/call for query-backlog");

    assert_eq!(
        call_req["params"]["arguments"]["tag"], "personal",
        "arguments.tag must be 'personal'"
    );
}
