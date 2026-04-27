/// Integration test: org tools call command.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_tools_call_org_read_returns_ok_envelope() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-read",
            "--args",
            r#"{"uri":"test-uuid-1234"}"#,
        ])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["tool"], "org-read");
    assert!(!v["data"]["result"].is_null(), "result must be present");
}

#[test]
fn test_tools_call_org_read_headline_returns_text_as_json() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-read-headline",
            "--args",
            r#"{"uri":"test-uuid-1234"}"#,
        ])
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["tool"], "org-read-headline");
    // The result should be the text content from the server
    assert!(!v["data"]["result"].is_null());
}

#[test]
fn test_tools_call_without_args_flag() {
    // tools call without --args should still work (empty args)
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-clock-get-active",
        ])
        .output()
        .expect("failed to run org");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], true);
}
