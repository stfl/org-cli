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
///                                Enables outline/read/query/mutating tests.
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
///
/// # Mutating tests (marked `#[ignore]`)
///
/// Tests that modify org state are marked `#[ignore]` so they do not run by
/// default even with `ORG_LIVE_TEST=1`. They require an explicit opt-in.
///
/// The supported path is the disposable-fixture launcher (added in
/// org-cli-4c8), which copies the tracked fixture into a tmpdir, spawns an
/// isolated daemon against it, and tears the entire workspace down on exit:
///
/// ```sh
/// just live-env-test-mutating
/// ```
///
/// Direct invocation against an external daemon is still possible but
/// discouraged because residue (e.g. `org-add-todo` entries) is left behind
/// — `org-mcp` has no delete-headline tool, so cleanup is manual:
///
/// ```sh
/// ORG_LIVE_TEST=1 ORG_LIVE_FILES=/path/to/test.org \
///   cargo test --test live_org_mcp -- --include-ignored --test-threads=1
/// ```
///
/// ## Revert audit (org-cli-5uf)
///
/// All mutating tests use a read-then-write-then-revert pattern. Cross-test
/// interference is bounded by `--test-threads=1` (serial) plus cargo's
/// alphabetical test ordering: `live_todo_add` (only test that adds an
/// undeletable heading) and `live_edit_log_note` (append-only) sort late, so
/// their residue cannot perturb earlier tests. The disposable tmpdir is the
/// safety net for any residue that escapes the in-test revert.
///
/// Fixture constraints assumed by the audit:
///   - The first child heading must have NO `SCHEDULED:` and NO `DEADLINE:`
///     set, so `live_edit_scheduled` / `live_edit_deadline` reverting via
///     `null` correctly restores the original (unset) state.
///   - The first child must have a TODO state and be stable across the
///     suite — all mutating tests target it. `--test-threads=1` plus revert
///     keeps that target consistent.
use std::process::Command;

use org_cli::mcp::client::Client;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true when live tests are enabled.
fn live_enabled() -> bool {
    std::env::var("ORG_LIVE_TEST").as_deref() == Ok("1")
}

/// Resolve the server argv. Prefers `ORG_LIVE_SERVER` env var; otherwise
/// falls back to the same default the CLI uses (`~/.config/emacs/org-mcp-stdio.sh`).
/// Panics with a clear message if `HOME` is unset and no override is given.
fn resolve_server() -> Vec<String> {
    if let Ok(path) = std::env::var("ORG_LIVE_SERVER") {
        return vec![path];
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| {
        panic!(
            "live test: HOME unset and ORG_LIVE_SERVER not provided. \
             Set ORG_LIVE_SERVER=<path> to point at a launcher."
        )
    });
    vec![format!("{home}/.config/emacs/org-mcp-stdio.sh")]
}

/// Strip any leading `org://` prefix so URIs are bare when sent to the server.
fn bare_uri(uri: &str) -> String {
    org_cli::uri::normalize_for_tool(uri)
}

/// Return the ORG_LIVE_FILES path or print a skip message and return from the
/// calling function. Usage: `let org_file = live_files_or_skip!("test_name");`
macro_rules! live_files_or_skip {
    ($name:expr) => {{
        match std::env::var("ORG_LIVE_FILES") {
            Ok(f) => f,
            Err(_) => {
                eprintln!(
                    "{}: skipping — set ORG_LIVE_FILES=/path/to/test.org to enable",
                    $name
                );
                return;
            }
        }
    }};
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
        "org-read-outline",
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

    let org_file = live_files_or_skip!("live_org_outline");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_org_outline: connect failed: {e}"));

    let result = client
        .tools_call("org-read-outline", serde_json::json!({ "file": org_file }))
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
    let candidate_core_tools = ["org-read", "org-read-outline", "org-read-headline"];
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

// ---------------------------------------------------------------------------
// Test 6: org-read against a real file
// ---------------------------------------------------------------------------

/// Call org-read with the file path as URI. Skip when ORG_LIVE_FILES is unset.
#[test]
fn live_org_read() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_org_read");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_org_read: connect failed: {e}"));

    let result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_org_read: tools_call failed: {e}"));

    eprintln!("live_org_read: result: {}", result);

    assert!(
        !result.is_null(),
        "org-read should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 7: org-read-headline against a real file
// ---------------------------------------------------------------------------

/// Call org-read-headline with the file path as URI. Skip when ORG_LIVE_FILES is unset.
#[test]
fn live_org_read_headline() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_org_read_headline");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_org_read_headline: connect failed: {e}"));

    let result = client
        .tools_call(
            "org-read-headline",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_org_read_headline: tools_call failed: {e}"));

    eprintln!("live_org_read_headline: result: {}", result);

    assert!(
        !result.is_null(),
        "org-read-headline should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 8: org-ql-query with permissive query
// ---------------------------------------------------------------------------

/// Call org-ql-query with `(todo)` against ORG_LIVE_FILES. Skip when unset.
#[test]
fn live_query_run() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_query_run");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_query_run: connect failed: {e}"));

    let result = client
        .tools_call(
            "org-ql-query",
            serde_json::json!({
                "query": "(todo)",
                "files": [org_file]
            }),
        )
        .unwrap_or_else(|e| panic!("live_query_run: tools_call failed: {e}"));

    eprintln!("live_query_run: result: {}", result);

    assert!(
        !result.is_null(),
        "org-ql-query should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 9: query-inbox (GTD — conditional on server capability)
// ---------------------------------------------------------------------------

/// Call query-inbox if the server advertises it; skip otherwise.
/// GTD tools are optional and depend on org-mcp server configuration.
#[test]
fn live_query_inbox() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_query_inbox: connect failed: {e}"));

    let has_tool = client
        .server_has_tool("query-inbox")
        .unwrap_or_else(|e| panic!("live_query_inbox: server_has_tool failed: {e}"));

    if !has_tool {
        eprintln!(
            "live_query_inbox: skipping — server does not advertise 'query-inbox' \
             (GTD tools not configured)"
        );
        return;
    }

    let result = client
        .tools_call("query-inbox", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_query_inbox: tools_call failed: {e}"));

    eprintln!("live_query_inbox: result: {}", result);

    assert!(
        !result.is_null(),
        "query-inbox should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 10: query-next (GTD — conditional on server capability)
// ---------------------------------------------------------------------------

/// Call query-next if the server advertises it; skip otherwise.
#[test]
fn live_query_next() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_query_next: connect failed: {e}"));

    let has_tool = client
        .server_has_tool("query-next")
        .unwrap_or_else(|e| panic!("live_query_next: server_has_tool failed: {e}"));

    if !has_tool {
        eprintln!(
            "live_query_next: skipping — server does not advertise 'query-next' \
             (GTD tools not configured)"
        );
        return;
    }

    let result = client
        .tools_call("query-next", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_query_next: tools_call failed: {e}"));

    eprintln!("live_query_next: result: {}", result);

    assert!(
        !result.is_null(),
        "query-next should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 11: query-backlog (GTD — conditional on server capability)
// ---------------------------------------------------------------------------

/// Call query-backlog if the server advertises it; skip otherwise.
#[test]
fn live_query_backlog() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_query_backlog: connect failed: {e}"));

    let has_tool = client
        .server_has_tool("query-backlog")
        .unwrap_or_else(|e| panic!("live_query_backlog: server_has_tool failed: {e}"));

    if !has_tool {
        eprintln!(
            "live_query_backlog: skipping — server does not advertise 'query-backlog' \
             (GTD tools not configured)"
        );
        return;
    }

    let result = client
        .tools_call("query-backlog", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_query_backlog: tools_call failed: {e}"));

    eprintln!("live_query_backlog: result: {}", result);

    assert!(
        !result.is_null(),
        "query-backlog should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 12: org-update-todo-state — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Read first heading's TODO state, change it, then revert.
/// Requires ORG_LIVE_TEST=1 AND ORG_LIVE_FILES set.
/// Run with: cargo test --test live_org_mcp -- --ignored live_todo_state
#[test]
#[ignore]
fn live_todo_state() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_todo_state");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_todo_state: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_todo_state: org-read failed: {e}"));

    eprintln!("live_todo_state: org-read result: {}", read_result);

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let (uri, original_state) = match first_child {
        Some(child) => {
            let uri = child["uri"]
                .as_str()
                .unwrap_or_else(|| panic!("live_todo_state: first child has no uri field"))
                .to_string();
            let state = child["todo"].as_str().unwrap_or("").to_string();
            (uri, state)
        }
        None => {
            eprintln!("live_todo_state: skipping — file has no child headings");
            return;
        }
    };

    eprintln!(
        "live_todo_state: uri={} original_state={}",
        uri, original_state
    );

    let bare = bare_uri(&uri);
    let new_state = if original_state == "TODO" {
        "DONE"
    } else {
        "TODO"
    };

    let update_result = client
        .tools_call(
            "org-update-todo-state",
            serde_json::json!({ "uri": bare, "new_state": new_state }),
        )
        .unwrap_or_else(|e| panic!("live_todo_state: org-update-todo-state failed: {e}"));

    eprintln!("live_todo_state: update result: {}", update_result);

    // Revert to original state.
    let revert_result = client
        .tools_call(
            "org-update-todo-state",
            serde_json::json!({ "uri": bare, "new_state": original_state }),
        )
        .unwrap_or_else(|e| panic!("live_todo_state: revert failed: {e}"));

    eprintln!("live_todo_state: revert result: {}", revert_result);

    assert!(
        !update_result.is_null(),
        "org-update-todo-state should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 13: org-add-todo — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Add a child TODO under the file root. Captures returned URI.
/// NOTE: cleanup is left to the user — org-mcp has no delete-headline tool.
/// Run with: cargo test --test live_org_mcp -- --ignored live_todo_add
#[test]
#[ignore]
fn live_todo_add() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_todo_add");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_todo_add: connect failed: {e}"));

    let parent_uri = bare_uri(&org_file);

    let add_result = client
        .tools_call(
            "org-add-todo",
            serde_json::json!({
                "parent_uri": parent_uri,
                "title": "live_todo_add test entry (safe to delete)",
                "todo_state": "TODO",
                "body": "Created by live_todo_add — disposable fixture cleans this up.",
                "tags": []
            }),
        )
        .unwrap_or_else(|e| panic!("live_todo_add: org-add-todo failed: {e}"));

    eprintln!("live_todo_add: result: {}", add_result);

    if let Some(new_uri) = add_result["uri"].as_str() {
        eprintln!(
            "live_todo_add: created heading uri={} — clean up manually if needed",
            new_uri
        );
    }

    assert!(!add_result.is_null(), "org-add-todo should return non-null");
}

// ---------------------------------------------------------------------------
// Test 14: org-rename-headline — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Rename the first child headline then revert.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_rename
#[test]
#[ignore]
fn live_edit_rename() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_rename");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_edit_rename: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_rename: org-read failed: {e}"));

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let (uri, original_title) = match first_child {
        Some(child) => {
            let uri = child["uri"]
                .as_str()
                .unwrap_or_else(|| panic!("live_edit_rename: first child has no uri"))
                .to_string();
            let title = child["title"].as_str().unwrap_or("").to_string();
            (uri, title)
        }
        None => {
            eprintln!("live_edit_rename: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);
    let temp_title = format!("{} (live_test_rename)", original_title);

    eprintln!("live_edit_rename: '{}' -> '{}'", original_title, temp_title);

    let rename_result = client
        .tools_call(
            "org-rename-headline",
            serde_json::json!({
                "uri": bare,
                "current_title": original_title,
                "new_title": temp_title
            }),
        )
        .unwrap_or_else(|e| panic!("live_edit_rename: org-rename-headline failed: {e}"));

    eprintln!("live_edit_rename: rename result: {}", rename_result);

    let revert_result = client
        .tools_call(
            "org-rename-headline",
            serde_json::json!({
                "uri": bare,
                "current_title": temp_title,
                "new_title": original_title
            }),
        )
        .unwrap_or_else(|e| panic!("live_edit_rename: revert failed: {e}"));

    eprintln!("live_edit_rename: revert result: {}", revert_result);

    assert!(
        !rename_result.is_null(),
        "org-rename-headline should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 15: org-edit-body — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Edit the body of the first child heading then revert.
/// NOTE: server param key is `resource_uri`, not `uri` (edit-body quirk).
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_body
#[test]
#[ignore]
fn live_edit_body() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_body");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_edit_body: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_body: org-read failed: {e}"));

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let (uri, original_body) = match first_child {
        Some(child) => {
            let uri = child["uri"]
                .as_str()
                .unwrap_or_else(|| panic!("live_edit_body: first child has no uri"))
                .to_string();
            let body = child["body"].as_str().unwrap_or("").to_string();
            (uri, body)
        }
        None => {
            eprintln!("live_edit_body: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);
    let temp_body = format!("{}live_test_body\n", original_body);

    // Server expects `resource_uri`, not `uri` — this is the edit-body quirk
    // documented in contract.rs (server_name = "resource_uri").
    let edit_result = client
        .tools_call(
            "org-edit-body",
            serde_json::json!({
                "resource_uri": bare,
                "new_body": temp_body,
                "old_body": original_body,
                "append": false
            }),
        )
        .unwrap_or_else(|e| panic!("live_edit_body: org-edit-body failed: {e}"));

    eprintln!("live_edit_body: edit result: {}", edit_result);

    let revert_result = client
        .tools_call(
            "org-edit-body",
            serde_json::json!({
                "resource_uri": bare,
                "new_body": original_body,
                "old_body": temp_body,
                "append": false
            }),
        )
        .unwrap_or_else(|e| panic!("live_edit_body: revert failed: {e}"));

    eprintln!("live_edit_body: revert result: {}", revert_result);

    assert!(
        !edit_result.is_null(),
        "org-edit-body should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 16: org-set-properties — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Set a test property on the first child, then unset it.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_properties
#[test]
#[ignore]
fn live_edit_properties() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_properties");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_edit_properties: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_properties: org-read failed: {e}"));

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let uri = match first_child {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_edit_properties: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_edit_properties: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    let set_result = client
        .tools_call(
            "org-set-properties",
            serde_json::json!({ "uri": bare, "properties": { "LIVE_TEST": "1" } }),
        )
        .unwrap_or_else(|e| panic!("live_edit_properties: org-set-properties failed: {e}"));

    eprintln!("live_edit_properties: set result: {}", set_result);

    // Revert: null value unsets the property.
    let revert_result = client
        .tools_call(
            "org-set-properties",
            serde_json::json!({ "uri": bare, "properties": { "LIVE_TEST": null } }),
        )
        .unwrap_or_else(|e| panic!("live_edit_properties: revert failed: {e}"));

    eprintln!("live_edit_properties: revert result: {}", revert_result);

    assert!(
        !set_result.is_null(),
        "org-set-properties should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 17: org-set-tags — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Capture original tags, add a test tag, then revert to original.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_tags
#[test]
#[ignore]
fn live_edit_tags() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_tags");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_edit_tags: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_tags: org-read failed: {e}"));

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let (uri, original_tags) = match first_child {
        Some(child) => {
            let uri = child["uri"]
                .as_str()
                .unwrap_or_else(|| panic!("live_edit_tags: first child has no uri"))
                .to_string();
            let tags: Vec<String> = child["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            (uri, tags)
        }
        None => {
            eprintln!("live_edit_tags: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);
    let mut test_tags = original_tags.clone();
    test_tags.push("live_test".to_string());

    let set_result = client
        .tools_call(
            "org-set-tags",
            serde_json::json!({ "uri": bare, "tags": test_tags }),
        )
        .unwrap_or_else(|e| panic!("live_edit_tags: org-set-tags failed: {e}"));

    eprintln!("live_edit_tags: set result: {}", set_result);

    let revert_result = client
        .tools_call(
            "org-set-tags",
            serde_json::json!({ "uri": bare, "tags": original_tags }),
        )
        .unwrap_or_else(|e| panic!("live_edit_tags: revert failed: {e}"));

    eprintln!("live_edit_tags: revert result: {}", revert_result);

    assert!(!set_result.is_null(), "org-set-tags should return non-null");
}

// ---------------------------------------------------------------------------
// Test 18: org-set-priority — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Set priority B on first child, then revert to original.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_priority
#[test]
#[ignore]
fn live_edit_priority() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_priority");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_edit_priority: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_priority: org-read failed: {e}"));

    let children = read_result["children"].as_array();
    let first_child = children.and_then(|c| c.first());

    let (uri, original_priority) = match first_child {
        Some(child) => {
            let uri = child["uri"]
                .as_str()
                .unwrap_or_else(|| panic!("live_edit_priority: first child has no uri"))
                .to_string();
            let priority = child["priority"].as_str().unwrap_or("").to_string();
            (uri, priority)
        }
        None => {
            eprintln!("live_edit_priority: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    let set_result = client
        .tools_call(
            "org-set-priority",
            serde_json::json!({ "uri": bare, "priority": "B" }),
        )
        .unwrap_or_else(|e| panic!("live_edit_priority: org-set-priority failed: {e}"));

    eprintln!("live_edit_priority: set result: {}", set_result);

    // Revert: null clears priority when original was unset.
    let revert_priority = if original_priority.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(original_priority)
    };

    let revert_result = client
        .tools_call(
            "org-set-priority",
            serde_json::json!({ "uri": bare, "priority": revert_priority }),
        )
        .unwrap_or_else(|e| panic!("live_edit_priority: revert failed: {e}"));

    eprintln!("live_edit_priority: revert result: {}", revert_result);

    assert!(
        !set_result.is_null(),
        "org-set-priority should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 19: org-update-scheduled — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Set a SCHEDULED date far in the future, then clear it.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_scheduled
#[test]
#[ignore]
fn live_edit_scheduled() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_scheduled");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_edit_scheduled: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_scheduled: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_edit_scheduled: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_edit_scheduled: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    let set_result = client
        .tools_call(
            "org-update-scheduled",
            serde_json::json!({ "uri": bare, "scheduled": "2099-12-31" }),
        )
        .unwrap_or_else(|e| panic!("live_edit_scheduled: org-update-scheduled failed: {e}"));

    eprintln!("live_edit_scheduled: set result: {}", set_result);

    let clear_result = client
        .tools_call(
            "org-update-scheduled",
            serde_json::json!({ "uri": bare, "scheduled": null }),
        )
        .unwrap_or_else(|e| panic!("live_edit_scheduled: clear failed: {e}"));

    eprintln!("live_edit_scheduled: clear result: {}", clear_result);

    assert!(
        !set_result.is_null(),
        "org-update-scheduled should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 20: org-update-deadline — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Set a DEADLINE date far in the future, then clear it.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_deadline
#[test]
#[ignore]
fn live_edit_deadline() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_deadline");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_edit_deadline: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_deadline: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_edit_deadline: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_edit_deadline: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    let set_result = client
        .tools_call(
            "org-update-deadline",
            serde_json::json!({ "uri": bare, "deadline": "2099-12-31" }),
        )
        .unwrap_or_else(|e| panic!("live_edit_deadline: org-update-deadline failed: {e}"));

    eprintln!("live_edit_deadline: set result: {}", set_result);

    let clear_result = client
        .tools_call(
            "org-update-deadline",
            serde_json::json!({ "uri": bare, "deadline": null }),
        )
        .unwrap_or_else(|e| panic!("live_edit_deadline: clear failed: {e}"));

    eprintln!("live_edit_deadline: clear result: {}", clear_result);

    assert!(
        !set_result.is_null(),
        "org-update-deadline should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 21: org-add-logbook-note — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Add a logbook note to the first child. Notes are append-only; no revert.
/// Run with: cargo test --test live_org_mcp -- --ignored live_edit_log_note
#[test]
#[ignore]
fn live_edit_log_note() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_edit_log_note");

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_edit_log_note: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_edit_log_note: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_edit_log_note: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_edit_log_note: skipping — file has no child headings");
            return;
        }
    };

    let result = client
        .tools_call(
            "org-add-logbook-note",
            serde_json::json!({
                "uri": bare_uri(&uri),
                "note": "live_test note (safe to delete)"
            }),
        )
        .unwrap_or_else(|e| panic!("live_edit_log_note: org-add-logbook-note failed: {e}"));

    eprintln!("live_edit_log_note: result: {}", result);

    assert!(
        !result.is_null(),
        "org-add-logbook-note should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 22: org-clock-get-active — read-only
// ---------------------------------------------------------------------------

/// Call org-clock-get-active to get current clock status.
#[test]
fn live_clock_status() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_clock_status: connect failed: {e}"));

    let result = client
        .tools_call("org-clock-get-active", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_clock_status: tools_call failed: {e}"));

    eprintln!("live_clock_status: result: {}", result);

    assert!(
        !result.is_null(),
        "org-clock-get-active should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 23: org-clock-find-dangling — read-only
// ---------------------------------------------------------------------------

/// Call org-clock-find-dangling to list unclosed clock entries.
#[test]
fn live_clock_dangling() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_clock_dangling: connect failed: {e}"));

    let result = client
        .tools_call("org-clock-find-dangling", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_clock_dangling: tools_call failed: {e}"));

    eprintln!("live_clock_dangling: result: {}", result);

    assert!(
        !result.is_null(),
        "org-clock-find-dangling should return a non-null result"
    );
}

// ---------------------------------------------------------------------------
// Test 24: org-clock-in — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Clock in to first child, then immediately clock out to clean up.
/// `resolve` is sent as JSON STRING "true" per the BoolAsString contract —
/// see contract.rs ServerValue::BoolAsString.
/// Run with: cargo test --test live_org_mcp -- --ignored live_clock_in
#[test]
#[ignore]
fn live_clock_in() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_clock_in");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_clock_in: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_clock_in: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_clock_in: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_clock_in: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    // `resolve` must be the STRING "true", not the boolean true.
    let clock_in_result = client
        .tools_call(
            "org-clock-in",
            serde_json::json!({ "uri": bare, "resolve": "true" }),
        )
        .unwrap_or_else(|e| panic!("live_clock_in: org-clock-in failed: {e}"));

    eprintln!("live_clock_in: clock-in result: {}", clock_in_result);

    // Immediately clock out to restore state.
    let clock_out_result = client
        .tools_call("org-clock-out", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_clock_in: org-clock-out (cleanup) failed: {e}"));

    eprintln!("live_clock_in: clock-out result: {}", clock_out_result);

    assert!(
        !clock_in_result.is_null(),
        "org-clock-in should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 25: org-clock-out — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Clock in to first child, then clock out explicitly — tests clock-out surface.
/// Run with: cargo test --test live_org_mcp -- --ignored live_clock_out
#[test]
#[ignore]
fn live_clock_out() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_clock_out");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_clock_out: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_clock_out: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_clock_out: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_clock_out: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);

    // Clock in first (setup).
    client
        .tools_call(
            "org-clock-in",
            serde_json::json!({ "uri": bare, "resolve": "true" }),
        )
        .unwrap_or_else(|e| panic!("live_clock_out: org-clock-in (setup) failed: {e}"));

    // Clock out — surface under test.
    let clock_out_result = client
        .tools_call("org-clock-out", serde_json::json!({ "uri": bare }))
        .unwrap_or_else(|e| panic!("live_clock_out: org-clock-out failed: {e}"));

    eprintln!("live_clock_out: result: {}", clock_out_result);

    assert!(
        !clock_out_result.is_null(),
        "org-clock-out should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 26: org-clock-add — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Add a historical clock entry, then delete it to clean up.
/// Run with: cargo test --test live_org_mcp -- --ignored live_clock_add
#[test]
#[ignore]
fn live_clock_add() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_clock_add");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_clock_add: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_clock_add: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_clock_add: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_clock_add: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);
    let start = "2000-01-01 00:00";
    let end = "2000-01-01 01:00";

    let add_result = client
        .tools_call(
            "org-clock-add",
            serde_json::json!({ "uri": bare, "start": start, "end": end }),
        )
        .unwrap_or_else(|e| panic!("live_clock_add: org-clock-add failed: {e}"));

    eprintln!("live_clock_add: add result: {}", add_result);

    // Delete the entry to clean up.
    let delete_result = client
        .tools_call(
            "org-clock-delete",
            serde_json::json!({ "uri": bare, "start": start }),
        )
        .unwrap_or_else(|e| panic!("live_clock_add: org-clock-delete (cleanup) failed: {e}"));

    eprintln!("live_clock_add: delete result: {}", delete_result);

    assert!(
        !add_result.is_null(),
        "org-clock-add should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 27: org-clock-delete — MUTATING (#[ignore])
// ---------------------------------------------------------------------------

/// Add then delete a clock entry — tests the delete surface specifically.
/// Run with: cargo test --test live_org_mcp -- --ignored live_clock_delete
#[test]
#[ignore]
fn live_clock_delete() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let org_file = live_files_or_skip!("live_clock_delete");

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_clock_delete: connect failed: {e}"));

    let read_result = client
        .tools_call(
            "org-read",
            serde_json::json!({ "uri": bare_uri(&org_file) }),
        )
        .unwrap_or_else(|e| panic!("live_clock_delete: org-read failed: {e}"));

    let uri = match read_result["children"].as_array().and_then(|c| c.first()) {
        Some(child) => child["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("live_clock_delete: first child has no uri"))
            .to_string(),
        None => {
            eprintln!("live_clock_delete: skipping — file has no child headings");
            return;
        }
    };

    let bare = bare_uri(&uri);
    let start = "2000-02-01 00:00";
    let end = "2000-02-01 01:00";

    // Add a clock entry to delete (setup).
    client
        .tools_call(
            "org-clock-add",
            serde_json::json!({ "uri": bare, "start": start, "end": end }),
        )
        .unwrap_or_else(|e| panic!("live_clock_delete: org-clock-add (setup) failed: {e}"));

    // Delete it — surface under test.
    let delete_result = client
        .tools_call(
            "org-clock-delete",
            serde_json::json!({ "uri": bare, "start": start }),
        )
        .unwrap_or_else(|e| panic!("live_clock_delete: org-clock-delete failed: {e}"));

    eprintln!("live_clock_delete: result: {}", delete_result);

    assert!(
        !delete_result.is_null(),
        "org-clock-delete should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 28: org-get-todo-config — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_todo() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_config_todo: connect failed: {e}"));

    let result = client
        .tools_call("org-get-todo-config", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_todo: tools_call failed: {e}"));

    eprintln!("live_config_todo: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-todo-config should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 29: org-get-tag-config — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_tags() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_config_tags: connect failed: {e}"));

    let result = client
        .tools_call("org-get-tag-config", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_tags: tools_call failed: {e}"));

    eprintln!("live_config_tags: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-tag-config should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 30: org-get-tag-candidates — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_tag_candidates() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_config_tag_candidates: connect failed: {e}"));

    let result = client
        .tools_call("org-get-tag-candidates", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_tag_candidates: tools_call failed: {e}"));

    eprintln!("live_config_tag_candidates: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-tag-candidates should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 31: org-get-priority-config — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_priority() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client = Client::connect(&argv)
        .unwrap_or_else(|e| panic!("live_config_priority: connect failed: {e}"));

    let result = client
        .tools_call("org-get-priority-config", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_priority: tools_call failed: {e}"));

    eprintln!("live_config_priority: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-priority-config should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 32: org-get-allowed-files — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_files() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_config_files: connect failed: {e}"));

    let result = client
        .tools_call("org-get-allowed-files", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_files: tools_call failed: {e}"));

    eprintln!("live_config_files: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-allowed-files should return non-null"
    );
}

// ---------------------------------------------------------------------------
// Test 33: org-get-clock-config — read-only
// ---------------------------------------------------------------------------

#[test]
fn live_config_clock() {
    if !live_enabled() {
        eprintln!("skipping live test (set ORG_LIVE_TEST=1 to run)");
        return;
    }

    let argv = resolve_server();
    let mut client =
        Client::connect(&argv).unwrap_or_else(|e| panic!("live_config_clock: connect failed: {e}"));

    let result = client
        .tools_call("org-get-clock-config", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("live_config_clock: tools_call failed: {e}"));

    eprintln!("live_config_clock: result: {}", result);

    assert!(
        !result.is_null(),
        "org-get-clock-config should return non-null"
    );
}
