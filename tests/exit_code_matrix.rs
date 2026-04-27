/// Exit-code matrix verification (PLAN §5.5).
///
/// | 0 | success                          |
/// | 1 | tool error returned by org-mcp   |
/// | 2 | usage / argument error            |
/// | 3 | transport / protocol failure      |
/// | 4 | server spawn / discovery failure  |
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

// ---------------------------------------------------------------------------
// Exit code 0 — happy-path success
// ---------------------------------------------------------------------------

#[test]
fn test_exit_0_tools_list() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(0),
        "tools list must exit 0\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], true);
}

// ---------------------------------------------------------------------------
// Exit code 1 — tool error from server
// ---------------------------------------------------------------------------

#[test]
fn test_exit_1_tool_error() {
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

    assert_eq!(
        output.status.code(),
        Some(1),
        "tool error must exit 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
    assert_eq!(v["exit_code"], 1);
}

// ---------------------------------------------------------------------------
// Exit code 2 — usage / argument errors
// ---------------------------------------------------------------------------

/// Clap detects missing required positional args and exits 2.
#[test]
fn test_exit_2_clap_missing_args() {
    // `org todo state` requires <uri> and <new-state> — omit both
    let output = org_bin()
        .args(["--server", mock_bin(), "todo", "state"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "clap error must exit 2\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// CLI-detected usage error: `org outline` with an org:// URI is rejected
/// locally before the server is spawned.
#[test]
fn test_exit_2_outline_org_uri() {
    let output = org_bin()
        .args(["--server", mock_bin(), "outline", "org://abc"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "outline with org:// must exit 2\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

/// CLI-detected usage error: --args with invalid JSON exits 2.
#[test]
fn test_exit_2_invalid_args_json() {
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "tools",
            "call",
            "org-read",
            "--args",
            "not-json{{{",
        ])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid --args JSON must exit 2\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

// ---------------------------------------------------------------------------
// Exit code 3 — transport / protocol failure
// ---------------------------------------------------------------------------

/// The mock exits after sending the initialize response (MOCK_DIE_AFTER_HANDSHAKE=1).
/// The client then tries to send notifications/initialized + tools/list and gets EOF
/// on the next recv(), which is a transport error → exit 3.
#[test]
fn test_exit_3_transport_die_after_handshake() {
    let output = org_bin()
        .env("MOCK_DIE_AFTER_HANDSHAKE", "1")
        .args(["--server", mock_bin(), "tools", "list"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(3),
        "transport failure must exit 3\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit_code"], 3);
}

// ---------------------------------------------------------------------------
// Exit code 4 — server spawn failure
// (also covered by tests/error_paths.rs::test_spawn_failure_exit_code_4)
// ---------------------------------------------------------------------------

#[test]
fn test_exit_4_spawn_failure() {
    let output = org_bin()
        .args(["--server", "/nonexistent/binary/xyz", "tools", "list"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(4),
        "spawn failure must exit 4\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit_code"], 4);
}
