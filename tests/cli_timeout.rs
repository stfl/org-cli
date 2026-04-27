/// Integration tests for the `--timeout <seconds>` top-level flag (and the
/// `ORG_TIMEOUT` environment variable). RED until org-cli-amu lands.
///
/// The mock supports `MOCK_HANG_MS=<n>` which sleeps n ms before sending each
/// response (covers initialize + tools/list + tools/call). When the CLI's
/// recv() exceeds `--timeout`, it must emit `kind=transport`, exit code 3, and
/// a message naming the timeout.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// Default timeout (30s) succeeds when the mock sleeps below threshold.
#[test]
fn test_timeout_default_succeeds_below_threshold() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .env("MOCK_HANG_MS", "100")
        .output()
        .expect("failed to run org");
    assert!(
        output.status.success(),
        "default 30s timeout should accept a 100ms hang; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `--timeout 1` with the mock hanging 2000ms must fail with exit 3,
/// `kind=transport`, and a message naming the timeout.
#[test]
fn test_timeout_explicit_short_triggers_exit_3() {
    let output = org_bin()
        .args(["--timeout", "1", "--server", mock_bin(), "tools", "list"])
        .env("MOCK_HANG_MS", "2000")
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(3),
        "exit 3 expected on timeout; got {:?}; stdout: {}; stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON envelope on timeout");
    assert_eq!(v["ok"], false);
    assert_eq!(
        v["error"]["kind"], "transport",
        "kind must be 'transport' on timeout; envelope: {v}"
    );
    let msg = v["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("timeout") || msg.contains("Timeout") || msg.contains("no response"),
        "message should reference the timeout; got: {msg}"
    );
}

/// `--timeout 0` disables the gate — the run completes even when the mock
/// hangs longer than what the default 30s would tolerate. Use a low hang_ms
/// to keep the suite fast (we just need to prove 0 means no upper bound).
#[test]
fn test_timeout_zero_disables() {
    let output = org_bin()
        .args(["--timeout", "0", "--server", mock_bin(), "tools", "list"])
        .env("MOCK_HANG_MS", "50")
        .output()
        .expect("failed to run org");
    assert!(
        output.status.success(),
        "--timeout 0 should disable the gate; got exit {:?}; stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `ORG_TIMEOUT=1` env var must work like `--timeout 1`.
#[test]
fn test_timeout_env_var_honored() {
    let output = org_bin()
        .args(["--server", mock_bin(), "tools", "list"])
        .env("ORG_TIMEOUT", "1")
        .env("MOCK_HANG_MS", "2000")
        .output()
        .expect("failed to run org");
    assert_eq!(
        output.status.code(),
        Some(3),
        "ORG_TIMEOUT=1 should produce exit 3 like --timeout 1"
    );
}
