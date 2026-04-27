/// Verify that server error.data is preserved in the CLI envelope.
///
/// Uses MOCK_TOOL_ERROR=<tool> + MOCK_TOOL_ERROR_DATA=<json> knobs.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_error_data_is_preserved_in_envelope() {
    let error_data = r#"{"detail":"row 42 lock conflict"}"#;

    let output = org_bin()
        .env("MOCK_TOOL_ERROR", "org-read")
        .env("MOCK_TOOL_ERROR_DATA", error_data)
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

    // Must still exit 1 (tool error)
    assert_eq!(
        output.status.code(),
        Some(1),
        "tool error with data must exit 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
    assert_eq!(
        v["error"]["data"],
        serde_json::json!({"detail": "row 42 lock conflict"}),
        "error.data must be preserved from the server response\nenvelope: {}",
        v
    );
}

#[test]
fn test_error_data_is_null_when_absent() {
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        v["error"]["data"],
        serde_json::Value::Null,
        "error.data must be null when server sends no data\nenvelope: {}",
        v
    );
}
