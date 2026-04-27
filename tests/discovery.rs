/// Integration tests for server auto-discovery (ticket org-cli-qq7).
///
/// Tests cover:
///   1. Discovery succeeds when PATH contains an executable `emacs-mcp-stdio.sh`
///   2. Discovery fails (exit 4, usage kind) when PATH has no such launcher
///   3. Explicit `--server` skips discovery even when PATH is empty
///   4. `org schema` never triggers discovery (local command, no server needed)
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// Create a temp dir with a `emacs-mcp-stdio.sh` script that execs the mock binary.
fn make_launcher_dir() -> std::path::PathBuf {
    let temp = std::env::temp_dir().join(format!(
        "org_disc_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp dir");

    let script = temp.join("emacs-mcp-stdio.sh");
    let mock = env!("CARGO_BIN_EXE_mock_org_mcp");
    std::fs::write(&script, format!("#!/bin/sh\nexec {}\n", mock)).expect("write launcher script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("set executable");

    temp
}

/// Discovery succeeds when PATH contains a valid `emacs-mcp-stdio.sh`.
/// The discovered script execs the mock, so `org tools list` should succeed.
#[test]
fn discover_succeeds_when_path_has_launcher() {
    let temp = make_launcher_dir();

    let output = org_bin()
        // No --server flag
        .args(["tools", "list"])
        .env("PATH", &temp)
        .output()
        .expect("failed to run org");

    // Clean up before asserting
    let _ = std::fs::remove_dir_all(&temp);

    assert!(
        output.status.success(),
        "discovery should succeed and tools list should exit 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true, "envelope ok must be true: {}", v);
}

/// Discovery fails when PATH has no `emacs-mcp-stdio.sh` → exit 4, ok:false, kind=usage.
#[test]
fn discover_fails_when_path_missing_launcher() {
    // Use a real empty temp dir as PATH so no executables exist
    let temp = std::env::temp_dir().join(format!(
        "org_disc_empty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp dir");

    let output = org_bin()
        .args(["tools", "list"])
        .env("PATH", &temp)
        .output()
        .expect("failed to run org");

    let _ = std::fs::remove_dir_all(&temp);

    assert_eq!(
        output.status.code(),
        Some(4),
        "discovery failure must exit 4\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 4);

    let msg = v["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("emacs-mcp-stdio.sh not found"),
        "error message must mention emacs-mcp-stdio.sh not found, got: {}",
        msg
    );
}

/// Explicit `--server <mock>` bypasses discovery even when PATH is empty.
#[test]
fn explicit_server_skips_discovery() {
    let temp = std::env::temp_dir().join(format!(
        "org_disc_skip_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp dir");

    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .env("PATH", &temp) // empty PATH — discovery would fail
        .output()
        .expect("failed to run org");

    let _ = std::fs::remove_dir_all(&temp);

    assert!(
        output.status.success(),
        "explicit --server must bypass discovery\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
}

/// `org schema` does not trigger discovery — works even with empty PATH.
#[test]
fn schema_does_not_trigger_discovery() {
    let temp = std::env::temp_dir().join(format!(
        "org_disc_schema_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("create temp dir");

    let output = org_bin()
        .args(["schema"])
        .env("PATH", &temp) // empty PATH — no launcher
        .output()
        .expect("failed to run org");

    let _ = std::fs::remove_dir_all(&temp);

    assert!(
        output.status.success(),
        "schema must not trigger discovery and must exit 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true, "schema envelope ok must be true: {}", v);
}
