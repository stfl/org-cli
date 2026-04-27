/// Integration tests for `org outline`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

#[test]
fn test_outline_ok_envelope() {
    let output = org_bin()
        .args(["--server", mock_bin(), "outline", "/tmp/foo.org"])
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

    assert_eq!(v["ok"], true, "envelope ok must be true");
    assert_eq!(
        v["data"]["file"], "/tmp/foo.org",
        "data.file must echo the requested path"
    );
    let outline = v["data"]["outline"]
        .as_array()
        .expect("data.outline must be array");
    assert!(!outline.is_empty(), "outline array must not be empty");
}

/// `org outline org://abc` must fail with kind=usage, exit 2.
/// Importantly this must NOT spawn the server — the validation is local.
#[test]
fn test_outline_org_uri_rejected() {
    // No --server flag: if validation is correct the server is never spawned
    // and the process exits 2 with a usage error envelope.
    let output = org_bin()
        .args(["outline", "org://abc"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for org:// outline path, got {:?}\nstdout: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}
