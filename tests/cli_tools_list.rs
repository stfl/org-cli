/// Integration test: invoke the compiled `org` binary with --server <mock> tools list.
/// Parses stdout as JSON and asserts envelope shape and tool count > 0.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_tools_list_envelope_shape() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit code should be 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true, "envelope ok must be true");
    let tools = v["data"]["tools"]
        .as_array()
        .expect("data.tools must be array");
    assert!(!tools.is_empty(), "tools array must not be empty");
}

#[test]
fn test_tools_list_has_required_tools() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .output()
        .expect("failed to run org");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let tools = v["data"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(names.contains(&"org-read"), "must include org-read");
    assert!(names.contains(&"org-ql-query"), "must include org-ql-query");
    assert!(names.len() >= 26, "expect at least 26 tools (core + GTD)");
}

#[test]
fn test_tools_list_compact_flag() {
    let output = org_bin()
        .args(["--server", mock_bin(), "--compact", "tools", "list"])
        .output()
        .expect("failed to run org");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    // Compact = single line
    assert_eq!(
        trimmed.lines().count(),
        1,
        "--compact must produce single-line output"
    );
    // Still valid JSON
    let _: serde_json::Value = serde_json::from_str(trimmed).expect("must be valid JSON");
}
