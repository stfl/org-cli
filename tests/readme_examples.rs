/// README examples smoke-test.
///
/// Each documented example command from README.md is run against the mock
/// server and asserted to return an ok envelope. This keeps README and
/// reality aligned.
///
/// `emacs-mcp-stdio.sh` is substituted with the mock binary path throughout.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn assert_ok_envelope(output: std::process::Output, label: &str) {
    assert!(
        output.status.success(),
        "{}: expected exit 0, got {:?}\nstdout: {}\nstderr: {}",
        label,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "{}: stdout is not valid JSON: {}\nraw: {}",
            label, e, stdout
        )
    });

    assert_eq!(
        v["ok"], true,
        "{}: envelope ok must be true\nenvelope: {}",
        label, v
    );
}

/// org --server <mock> read org://abc
#[test]
fn test_readme_read() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "org://abc"])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "read org://abc");
}

/// org --server <mock> todo state org://abc DONE --from TODO --note "shipped"
#[test]
fn test_readme_todo_state() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "todo",
            "state",
            "org://abc",
            "DONE",
            "--from",
            "TODO",
            "--note",
            "shipped",
        ])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "todo state DONE");
}

/// org --server <mock> edit body org://abc --new "new body"
#[test]
fn test_readme_edit_body() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "body",
            "org://abc",
            "--new",
            "new body",
        ])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "edit body");
}

/// org --server <mock> clock in org://abc --resolve
#[test]
fn test_readme_clock_in_resolve() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "clock",
            "in",
            "org://abc",
            "--resolve",
        ])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "clock in --resolve");
}

/// org --server <mock> tools list
#[test]
fn test_readme_tools_list() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "tools list");
}

/// org --server <mock> query run '(todo "TODO")'
#[test]
fn test_readme_query_run() {
    let output = org_bin()
        .args(["--server", mock_bin(), "query", "run", r#"(todo "TODO")"#])
        .output()
        .expect("failed to run org");
    assert_ok_envelope(output, "query run");
}
