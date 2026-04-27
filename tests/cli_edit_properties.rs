/// Integration tests for `org edit properties`.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

fn temp_log(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "org_cli_edit_props_{}_{}.jsonl",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

fn find_tools_call(log: &str, tool: &str) -> serde_json::Value {
    log.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .find(|v: &serde_json::Value| {
            v["method"].as_str() == Some("tools/call") && v["params"]["name"].as_str() == Some(tool)
        })
        .unwrap_or_else(|| panic!("must find a tools/call for {tool}"))
}

/// --set foo=bar --set baz=qux → arguments.set == {"foo":"bar","baz":"qux"}.
#[test]
fn test_properties_set_pairs_forwarded_as_object() {
    let log_path = temp_log("set_pairs");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "properties",
            "org://abc",
            "--set",
            "foo=bar",
            "--set",
            "baz=qux",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(
        output.status.success(),
        "stderr: {}; stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-properties");
    let args = &req["params"]["arguments"];
    let set = &args["set"];
    assert!(set.is_object(), "set must be an object; got: {set}");
    assert_eq!(set["foo"].as_str(), Some("bar"));
    assert_eq!(set["baz"].as_str(), Some("qux"));
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("set"));
    assert!(args.as_object().unwrap().contains_key("unset"));
}

/// --unset a --unset b → arguments.unset == ["a","b"].
#[test]
fn test_properties_unset_forwarded_as_array() {
    let log_path = temp_log("unset_array");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "properties",
            "abc",
            "--unset",
            "a",
            "--unset",
            "b",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-properties");
    let args = &req["params"]["arguments"];
    let unset = &args["unset"];
    assert!(unset.is_array(), "unset must be array");
    let arr: Vec<&str> = unset
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(arr, vec!["a", "b"]);
}

/// Combined --set and --unset together.
#[test]
fn test_properties_set_and_unset_combined() {
    let log_path = temp_log("combined");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "properties",
            "abc",
            "--set",
            "x=1",
            "--unset",
            "y",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-properties");
    let args = &req["params"]["arguments"];
    assert_eq!(args["set"]["x"].as_str(), Some("1"));
    let unset_arr = args["unset"].as_array().unwrap();
    assert_eq!(unset_arr.len(), 1);
    assert_eq!(unset_arr[0].as_str(), Some("y"));
}

/// Empty (no flags) → set == {}, unset == [].
#[test]
fn test_properties_empty_sends_empty_object_and_array() {
    let log_path = temp_log("empty");
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "properties", "abc"])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-properties");
    let args = &req["params"]["arguments"];
    assert!(args["set"].is_object(), "set must be object");
    assert!(
        args["set"].as_object().unwrap().is_empty(),
        "set must be {{}} when no --set"
    );
    assert!(args["unset"].is_array(), "unset must be array");
    assert!(
        args["unset"].as_array().unwrap().is_empty(),
        "unset must be [] when no --unset"
    );
}

/// Malformed --set noequals → exit 2 (usage error), NO server spawn.
#[test]
fn test_properties_malformed_set_exits_2_before_spawn() {
    // Use a log path but do NOT set MOCK_RECORD_REQUESTS so we can assert it's never written.
    let log_path = temp_log("malformed");
    // We still pass --server but it should not be spawned.
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "properties",
            "abc",
            "--set",
            "noequals",
        ])
        .output()
        .expect("failed to run org");
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit 2 expected for malformed --set; stdout: {}",
        String::from_utf8_lossy(&output.stdout),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "usage");
    // Log must not have been created (server was never spawned)
    assert!(
        !log_path.exists(),
        "log must not exist — server must not have been spawned"
    );
}

/// Value with equals: --set foo=bar=baz → arguments.set.foo == "bar=baz".
#[test]
fn test_properties_value_with_equals_splits_on_first() {
    let log_path = temp_log("value_equals");
    let output = org_bin()
        .args([
            "--server",
            mock_bin(),
            "edit",
            "properties",
            "abc",
            "--set",
            "foo=bar=baz",
        ])
        .env("MOCK_RECORD_REQUESTS", "1")
        .env("MOCK_REQUEST_LOG", log_path.to_str().unwrap())
        .output()
        .expect("failed to run org");
    assert!(output.status.success());
    let log = std::fs::read_to_string(&log_path).expect("log must exist");
    let _ = std::fs::remove_file(&log_path);
    let req = find_tools_call(&log, "org-edit-properties");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["set"]["foo"].as_str(),
        Some("bar=baz"),
        "split must happen on first = only"
    );
}

/// Tool error → ok:false, exit 1.
#[test]
fn test_properties_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "properties", "abc"])
        .env("MOCK_TOOL_ERROR", "org-edit-properties")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
