/// Smoke-tests that --compact works on one representative command from each
/// Phase 3–8, asserting:
///   - stdout is a single \n-terminated line (or zero if no trailing newline)
///   - the line parses as valid JSON
///   - the envelope `ok` field is present
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn assert_compact_ok(args: &[&str], label: &str) {
    let mut cmd = org_bin();
    cmd.env_clear();
    // env_clear wipes PATH too — explicitly pass the mock path (absolute) and
    // rebuild a minimal environment so the binary can be located by the OS.
    // We reconstruct PATH so the test runner can find system tools if needed.
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }

    let output = org_bin()
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{}: failed to run org: {}", label, e));

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must be a single line (compact = no pretty-printing).
    let lines: Vec<&str> = stdout.trim_end_matches('\n').lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "{}: --compact must produce exactly one line, got {} lines\nstdout: {}",
        label,
        lines.len(),
        stdout
    );

    // Must parse as JSON.
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap_or_else(|e| {
        panic!(
            "{}: --compact output is not valid JSON: {}\nline: {}",
            label, e, lines[0]
        )
    });

    // Must have ok field.
    assert!(
        v.get("ok").is_some(),
        "{}: envelope missing 'ok' field\nvalue: {}",
        label,
        v
    );
}

// Phase 3 — read
#[test]
fn test_compact_read() {
    assert_compact_ok(
        &["--server", mock_bin(), "--compact", "read", "org://abc"],
        "read",
    );
}

// Phase 3 — read-headline
#[test]
fn test_compact_read_headline() {
    assert_compact_ok(
        &[
            "--server",
            mock_bin(),
            "--compact",
            "read-headline",
            "org://abc",
        ],
        "read-headline",
    );
}

// Phase 4 — query run (ql-expr form)
#[test]
fn test_compact_query() {
    assert_compact_ok(
        &[
            "--server",
            mock_bin(),
            "--compact",
            "query",
            "run",
            "(todo \"TODO\")",
        ],
        "query",
    );
}

// Phase 5 — todo state
#[test]
fn test_compact_todo_state() {
    assert_compact_ok(
        &[
            "--server",
            mock_bin(),
            "--compact",
            "todo",
            "state",
            "org://abc",
            "DONE",
        ],
        "todo state",
    );
}

// Phase 6 — edit body
#[test]
fn test_compact_edit_body() {
    assert_compact_ok(
        &[
            "--server",
            mock_bin(),
            "--compact",
            "edit",
            "body",
            "org://abc",
            "--new",
            "updated body",
        ],
        "edit body",
    );
}

// Phase 7 — clock status
#[test]
fn test_compact_clock_status() {
    assert_compact_ok(
        &["--server", mock_bin(), "--compact", "clock", "status"],
        "clock status",
    );
}

// Phase 8 — config todo
#[test]
fn test_compact_config_todo() {
    assert_compact_ok(
        &["--server", mock_bin(), "--compact", "config", "todo"],
        "config todo",
    );
}
