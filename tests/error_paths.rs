/// Tests for error paths: tool errors, bad args, transport errors.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_tool_error_returns_ok_false_envelope() {
    let output = org_bin()
        .env("MOCK_TOOL_ERROR", "org-read")
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-read",
            "--args",
            r#"{"uri":"foo"}"#,
        ])
        .output()
        .expect("failed to run org");

    // Exit code 1 = tool error
    assert_eq!(
        output.status.code(),
        Some(1),
        "tool error must produce exit code 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout must be valid JSON even on error");

    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
    assert!(!v["error"]["message"].as_str().unwrap_or("").is_empty());
    assert_eq!(v["exit_code"], 1);
}

#[test]
fn test_spawn_failure_exit_code_4() {
    let output = org_bin()
        .args([
            "--server",
            "/nonexistent/binary/that/does/not/exist",
            "tools",
            "list",
        ])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(4),
        "spawn failure must produce exit code 4\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "transport");
    assert_eq!(v["exit_code"], 4);
}

#[test]
fn test_invalid_args_json_exit_code_2() {
    // --args with invalid JSON
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-read",
            "--args",
            "not-valid-json{{{",
        ])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid JSON args must produce exit code 2\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}
