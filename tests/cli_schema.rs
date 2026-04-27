/// Integration tests for `org schema` and `org schema <command-path>`.
///
/// `schema` is an internal/local command — no --server needed.
/// Written RED first (Phase 2 TDD).
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

// ---------------------------------------------------------------------------
// org schema (all commands)
// ---------------------------------------------------------------------------

#[test]
fn test_schema_all_returns_ok_envelope() {
    let output = org_bin()
        .args(["schema"])
        .output()
        .expect("failed to run org schema");

    assert!(
        output.status.success(),
        "org schema must exit 0\nstderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("org schema must output valid JSON");

    assert_eq!(v["ok"], true, "envelope ok must be true");
    assert_eq!(v["data"]["version"], 1, "version must be 1");

    let commands = v["data"]["commands"]
        .as_array()
        .expect("data.commands must be an array");
    assert!(!commands.is_empty(), "commands array must not be empty");
}

#[test]
fn test_schema_all_contains_read_entry() {
    let v = run_schema_all();
    let commands = v["data"]["commands"].as_array().unwrap();
    let read = commands
        .iter()
        .find(|c| c["path"] == serde_json::json!(["read"]))
        .expect("must have a [\"read\"] entry");

    assert_eq!(read["target"]["kind"], "tool");
    assert_eq!(read["target"]["name"], "org-read");
    assert!(read["params"].as_array().is_some());
    assert!(read["exit_codes"].as_array().is_some());
}

#[test]
fn test_schema_all_contains_edit_body_entry() {
    let v = run_schema_all();
    let commands = v["data"]["commands"].as_array().unwrap();
    let edit_body = commands
        .iter()
        .find(|c| c["path"] == serde_json::json!(["edit", "body"]))
        .expect("must have [\"edit\", \"body\"] entry");

    assert_eq!(edit_body["target"]["kind"], "tool");
    assert_eq!(edit_body["target"]["name"], "org-edit-body");
}

#[test]
fn test_schema_all_contains_clock_in_entry() {
    let v = run_schema_all();
    let commands = v["data"]["commands"].as_array().unwrap();
    let cmd = commands
        .iter()
        .find(|c| c["path"] == serde_json::json!(["clock", "in"]))
        .expect("must have [\"clock\", \"in\"] entry");

    assert_eq!(cmd["target"]["kind"], "tool");
    assert_eq!(cmd["target"]["name"], "org-clock-in");
}

#[test]
fn test_schema_all_contains_query_inbox_entry() {
    let v = run_schema_all();
    let commands = v["data"]["commands"].as_array().unwrap();
    commands
        .iter()
        .find(|c| c["path"] == serde_json::json!(["query", "inbox"]))
        .expect("must have [\"query\", \"inbox\"] entry");
}

// ---------------------------------------------------------------------------
// org schema <path> (single command)
// ---------------------------------------------------------------------------

#[test]
fn test_schema_single_edit_body_returns_command_envelope() {
    let output = org_bin()
        .args(["schema", "edit", "body"])
        .output()
        .expect("failed to run org schema edit body");

    assert!(
        output.status.success(),
        "exit code must be 0\nstderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("must output valid JSON");

    assert_eq!(v["ok"], true);
    let cmd = &v["data"]["command"];
    assert_eq!(cmd["path"], serde_json::json!(["edit", "body"]));
    assert_eq!(cmd["target"]["name"], "org-edit-body");
}

#[test]
fn test_schema_single_edit_body_shows_resource_uri_server_name() {
    let v = run_schema_path(&["schema", "edit", "body"]);
    let cmd = &v["data"]["command"];
    let params = cmd["params"].as_array().expect("params must be array");
    let uri_param = params
        .iter()
        .find(|p| p["server_name"] == "resource_uri")
        .expect("edit body must have a param with server_name = resource_uri");
    assert_eq!(uri_param["name"], "uri");
}

#[test]
fn test_schema_single_clock_in_shows_bool_as_string_quirk() {
    let v = run_schema_path(&["schema", "clock", "in"]);
    let cmd = &v["data"]["command"];
    let params = cmd["params"].as_array().expect("params must be array");
    let resolve = params
        .iter()
        .find(|p| p["name"] == "resolve")
        .expect("clock in must have resolve param");
    assert_eq!(
        resolve["server_value"], "bool_as_string",
        "resolve must surface bool_as_string quirk"
    );
}

#[test]
fn test_schema_unknown_path_returns_usage_error() {
    let output = org_bin()
        .args(["schema", "does", "not", "exist"])
        .output()
        .expect("failed to run org");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown schema path must exit 2\nstdout: {}",
        String::from_utf8_lossy(&output.stdout),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["exit_code"], 2);
}

#[test]
fn test_schema_compact_flag() {
    let output = org_bin()
        .args(["--compact", "schema"])
        .output()
        .expect("failed to run org --compact schema");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert_eq!(
        trimmed.lines().count(),
        1,
        "--compact must produce single-line output"
    );
    let _: serde_json::Value = serde_json::from_str(trimmed).expect("must be valid JSON");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_schema_all() -> serde_json::Value {
    let output = org_bin()
        .args(["schema"])
        .output()
        .expect("failed to run org schema");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("org schema must output valid JSON")
}

fn run_schema_path(args: &[&str]) -> serde_json::Value {
    let output = org_bin()
        .args(args)
        .output()
        .expect("failed to run org schema <path>");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("org schema <path> must output valid JSON")
}
