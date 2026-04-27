/// Integration tests for `org config *` subcommands (Phase 8).
///
/// All 6 config commands are pure read/introspection: they send `arguments: {}`
/// and return a stable JSON envelope.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_config_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

fn find_tools_call<'a>(log: &'a str, tool: &str) -> serde_json::Value {
    log.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call") && v["params"]["name"].as_str() == Some(tool)
        })
        .unwrap_or_else(|| panic!("must find a tools/call for {tool}"))
}

// ---------------------------------------------------------------------------
// org config todo
// ---------------------------------------------------------------------------

/// Happy path: config todo → ok envelope, data.keywords includes "TODO".
#[test]
fn test_config_todo_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "todo"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    let keywords = v["data"]["keywords"]
        .as_array()
        .expect("data.keywords must be an array");
    assert!(
        keywords.iter().any(|k| k.as_str() == Some("TODO")),
        "data.keywords must include 'TODO', got: {keywords:?}"
    );
}

/// config todo sends empty arguments object.
#[test]
fn test_config_todo_empty_arguments() {
    let log_path = temp_log("todo_args");
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "todo"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-config-todo");
    let args = &req["params"]["arguments"];
    assert!(
        args.as_object().map(|o| o.is_empty()).unwrap_or(false),
        "config todo must send empty arguments object, got: {args}"
    );
}

/// Tool error on config todo → ok:false, exit 1.
#[test]
fn test_config_todo_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "todo"])
        .env("MOCK_TOOL_ERROR", "org-config-todo")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}

// ---------------------------------------------------------------------------
// org config tags
// ---------------------------------------------------------------------------

/// Happy path: config tags → ok envelope, data.tags is an array.
#[test]
fn test_config_tags_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "tags"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    let tags = v["data"]["tags"]
        .as_array()
        .expect("data.tags must be an array");
    assert!(
        tags.iter().any(|t| t.as_str() == Some("work")),
        "data.tags must include 'work', got: {tags:?}"
    );
}

// ---------------------------------------------------------------------------
// org config tag-candidates
// ---------------------------------------------------------------------------

/// Happy path: config tag-candidates → ok envelope, data.candidates is an array.
#[test]
fn test_config_tag_candidates_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "tag-candidates"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    let candidates = v["data"]["candidates"]
        .as_array()
        .expect("data.candidates must be an array");
    assert!(
        !candidates.is_empty(),
        "data.candidates must have at least one entry"
    );
}

// ---------------------------------------------------------------------------
// org config priority
// ---------------------------------------------------------------------------

/// Happy path: config priority → ok envelope, data.default == "B".
#[test]
fn test_config_priority_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "priority"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert_eq!(
        v["data"]["default"], "B",
        "data.default must be 'B', got: {}",
        v["data"]["default"]
    );
}

// ---------------------------------------------------------------------------
// org config files
// ---------------------------------------------------------------------------

/// Happy path: config files → ok envelope, data.agenda_files has at least 1 entry.
#[test]
fn test_config_files_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "files"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    let files = v["data"]["agenda_files"]
        .as_array()
        .expect("data.agenda_files must be an array");
    assert!(
        !files.is_empty(),
        "data.agenda_files must have at least 1 entry"
    );
}

// ---------------------------------------------------------------------------
// org config clock
// ---------------------------------------------------------------------------

/// Happy path: config clock → ok envelope, data.persist is present.
#[test]
fn test_config_clock_ok() {
    let output = org_bin()
        .args(["--server", mock_bin(), "config", "clock"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "exit 0 expected; stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert!(
        v["data"]["persist"].is_boolean(),
        "data.persist must be a boolean, got: {}",
        v["data"]["persist"]
    );
    assert_eq!(
        v["data"]["idle_minutes"], 15,
        "data.idle_minutes must be 15"
    );
}

// ---------------------------------------------------------------------------
// Envelope sanity check: all 6 produce {ok:true, data:<object>}
// ---------------------------------------------------------------------------

/// All 6 config subcommands produce a well-formed ok envelope with a data object.
#[test]
fn test_all_config_commands_produce_ok_envelope() {
    let subcommands = [
        "todo",
        "tags",
        "tag-candidates",
        "priority",
        "files",
        "clock",
    ];

    for sub in subcommands {
        let output = org_bin()
            .args(["--server", mock_bin(), "config", sub])
            .output()
            .unwrap_or_else(|_| panic!("failed to run org config {sub}"));

        assert!(
            output.status.success(),
            "org config {sub} must exit 0; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let v: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| panic!("org config {sub} stdout must be valid JSON"));

        assert_eq!(v["ok"], true, "org config {sub} must have ok:true");
        assert!(
            v["data"].is_object(),
            "org config {sub} must have data as an object, got: {}",
            v["data"]
        );
    }
}
