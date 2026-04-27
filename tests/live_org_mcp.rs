/// Live integration tests against the real `emacs-mcp-stdio.sh` launcher.
///
/// # Environment variables
///
/// - `ORG_LIVE_TEST=1`          — Required to enable these tests. When unset,
///                                every test returns immediately (no failure).
/// - `ORG_LIVE_SERVER=<path>`   — Override discovery; use this exact launcher
///                                instead of searching $PATH. Useful when the
///                                launcher is in a non-standard location.
/// - `ORG_LIVE_FILES=<file.org>` — Absolute path to a pre-populated org file.
///                                Enables outline/read tests against a real file.
///
/// # Running
///
/// ```sh
/// ORG_LIVE_TEST=1 cargo test --test live_org_mcp -- --test-threads=1
/// ```
///
/// Optionally with overrides:
/// ```sh
/// ORG_LIVE_TEST=1 ORG_LIVE_SERVER=/path/to/emacs-mcp-stdio.sh \
///   ORG_LIVE_FILES=/path/to/test.org \
///   cargo test --test live_org_mcp -- --test-threads=1
/// ```
use std::process::Command;

use org_cli::discovery::discover_server;
use org_cli::mcp::client::Client;
use serde_json::Value;

/// Returns true when live tests are enabled.
fn live_enabled() -> bool {
    std::env::var("ORG_LIVE_TEST").as_deref() == Ok("1")
}

/// Resolve the server argv. Prefers `ORG_LIVE_SERVER` env var, falls back to
/// PATH discovery via `discover_server()`. Panics with a clear message if
/// discovery fails (see PLAN §5.2).
fn resolve_server() -> Vec<String> {
    if let Ok(path) = std::env::var("ORG_LIVE_SERVER") {
        return vec![path];
    }
    discover_server().unwrap_or_else(|e| {
        panic!(
            "live test: could not locate emacs-mcp-stdio.sh — {}.\n\
             Set ORG_LIVE_SERVER=<path> or install the launcher (see PLAN §5.2).",
            e
        )
    })
}

// ---------------------------------------------------------------------------
// Test 1: initialize handshake
// ---------------------------------------------------------------------------

/// Verify the initialize handshake completes and the server advertises tools.
#[test]
fn live_handshake() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let _client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_handshake: connect failed: {e}"));

    // Client::connect already validates that the server advertises tools capability
    // (see src/mcp/client.rs handshake()). Reaching here means it passed.
}

// ---------------------------------------------------------------------------
// Test 2: tools/list returns org tools
// ---------------------------------------------------------------------------

/// Verify tools/list returns at least 5 tools and at least one contains "org".
///
/// Note: exact tool names may vary between org-mcp versions. This test is
/// intentionally tolerant. If names differ from expectations, the eprintln!
/// output will guide you on what to update in a follow-up.
#[test]
fn live_tools_list_returns_org_tools() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_tools_list_returns_org_tools: connect failed: {e}"));

    let tools = client
        .tools_list()
        .unwrap_or_else(|e| panic!("live_tools_list_returns_org_tools: tools/list failed: {e}"));

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();

    eprintln!(
        "live_tools_list_returns_org_tools: server advertises {} tools: {:?}",
        names.len(),
        names
    );

    assert!(
        names.len() >= 5,
        "expected at least 5 tools, got {}: {:?}",
        names.len(),
        names
    );

    let has_org_tool = names.iter().any(|n| n.contains("org"));
    assert!(
        has_org_tool,
        "expected at least one tool name containing 'org', got: {:?}",
        names
    );

    // Log any tools that don't match the expected names so a future user can
    // update assertions if the real org-mcp uses different names.
    let expected_names = [
        "org-read",
        "org-read-headline",
        "org-outline",
        "org-update-todo-state",
        "org-add-todo",
    ];
    for expected in &expected_names {
        if !names.contains(expected) {
            eprintln!(
                "live_tools_list_returns_org_tools: NOTE: expected tool '{}' not found in server \
                 response — update assertions if the real org-mcp uses a different name.",
                expected
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 3: org-outline against a real file
// ---------------------------------------------------------------------------

/// Call org-outline against the file pointed to by `ORG_LIVE_FILES`.
/// If `ORG_LIVE_FILES` is unset, skip with an informational message.
#[test]
fn live_org_outline() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = match std::env::var("ORG_LIVE_FILES") {
        Ok(f) => f,
        Err(_) => {
            eprintln!(
                "live_org_outline: skipping — set ORG_LIVE_FILES=/path/to/test.org to enable"
            );
            return;
        }
    };

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_org_outline: connect failed: {e}"));

    let result = client
        .tools_call("org-outline", serde_json::json!({ "file": org_file }))
        .unwrap_or_else(|e| panic!("live_org_outline: tools_call failed: {e}"));

    eprintln!("live_org_outline: result: {}", result);

    // The result should be a non-null, non-error response (content array or object).
    assert!(
        !result.is_null(),
        "org-outline should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 4: server_has_tool capability discovery
// ---------------------------------------------------------------------------

/// Verify that server_has_tool correctly returns false for unknown tools and
/// true for known core tools.
#[test]
fn live_capability_discovery() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_capability_discovery: connect failed: {e}"));

    let has_nonexistent = client
        .server_has_tool("tools/list-doesnt-exist-tool")
        .unwrap_or_else(|e| {
            panic!("live_capability_discovery: server_has_tool (nonexistent) failed: {e}")
        });
    assert!(
        !has_nonexistent,
        "server_has_tool should return false for a nonexistent tool"
    );

    // Try known core tools (tolerant: accept any of these)
    let candidate_core_tools = ["org-read", "org-outline", "org-read-headline"];
    let mut found_core = false;
    for tool in &candidate_core_tools {
        match client.server_has_tool(tool) {
            Ok(true) => {
                eprintln!("live_capability_discovery: found core tool '{tool}'");
                found_core = true;
                break;
            }
            Ok(false) => {
                eprintln!("live_capability_discovery: core tool '{tool}' not found (trying next)");
            }
            Err(e) => {
                panic!("live_capability_discovery: server_has_tool({tool}) failed: {e}")
            }
        }
    }
    assert!(
        found_core,
        "server_has_tool should return true for at least one of {:?}",
        candidate_core_tools
    );
}

// ---------------------------------------------------------------------------
// Test 5: envelope round-trip via the compiled org binary
// ---------------------------------------------------------------------------

/// Invoke the compiled `org` binary with the discovered server and `tools list`.
/// Assert the stdout is a valid `{"ok":true,"data":{"tools":[...]}}` envelope.
#[test]
fn live_envelope_round_trip() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    // argv[0] is the server path; remaining elements are extra args (rare).
    let server_path = &argv[0];

    let output = Command::new(env!("CARGO_BIN_EXE_org"))
        .args(["--server", server_path, "tools", "list"])
        .output()
        .unwrap_or_else(|e| panic!("live_envelope_round_trip: failed to spawn org binary: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!(
        "live_envelope_round_trip: exit={:?} stderr={}",
        output.status.code(),
        stderr
    );

    assert!(
        output.status.success(),
        "org binary should exit 0, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout,
        stderr
    );

    let v: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("live_envelope_round_trip: stdout is not valid JSON: {e}\nstdout: {stdout}")
    });

    assert_eq!(
        v["ok"], true,
        "envelope ok must be true\nfull response: {v}"
    );

    let tools = v["data"]["tools"].as_array().unwrap_or_else(|| {
        panic!("live_envelope_round_trip: data.tools must be an array\nfull response: {v}")
    });

    assert!(
        !tools.is_empty(),
        "data.tools must not be empty\nfull response: {v}"
    );

    eprintln!(
        "live_envelope_round_trip: received {} tools via org binary",
        tools.len()
    );
}
