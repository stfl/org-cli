/// Integration tests for `org read-headline`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_read_headline_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read-headline", "foo"])
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

    assert_eq!(v["ok"], true);
    let text = v["data"]["text"]
        .as_str()
        .expect("data.text must be a string");
    assert!(!text.is_empty(), "data.text must not be empty");
    // The mock returns plain text starting with "* TODO"
    assert!(
        text.contains("TODO"),
        "mock plain-text response should contain TODO; got: {text}"
    );
}

/// With MOCK_RECORD_REQUESTS=1, verify org:// prefix is stripped.
#[test]
fn test_read_headline_strips_org_prefix() {
    let log_path = std::env::temp_dir().join(format!(
        "org_cli_headline_req_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));

    let output = org_bin()
        .args(["--server", mock_bin(), "read-headline", "org://mynode"])
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
                && v["params"]["name"].as_str() == Some("org-read-headline")
        })
        .expect("must find tools/call for org-read-headline in log");

    let sent_uri = call_req["params"]["arguments"]["uri"]
        .as_str()
        .expect("uri must be string");
    assert_eq!(
        sent_uri, "mynode",
        "CLI must strip org:// before sending; got: {sent_uri}"
    );
}
