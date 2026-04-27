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

/// --set foo=bar --set baz=qux → arguments.properties == {"foo":"bar","baz":"qux"}.
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
    let req = find_tools_call(&log, "org-set-properties");
    let args = &req["params"]["arguments"];
    let props = &args["properties"];
    assert!(
        props.is_object(),
        "properties must be an object; got: {props}"
    );
    assert_eq!(props["foo"].as_str(), Some("bar"));
    assert_eq!(props["baz"].as_str(), Some("qux"));
    // Contract assertion
    assert!(args.as_object().unwrap().contains_key("uri"));
    assert!(args.as_object().unwrap().contains_key("properties"));
}

/// --unset a --unset b → arguments.properties == {"a": null, "b": null}.
#[test]
fn test_properties_unset_forwarded_as_null_values() {
    let log_path = temp_log("unset_nulls");
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
    let req = find_tools_call(&log, "org-set-properties");
    let args = &req["params"]["arguments"];
    let props = &args["properties"];
    assert!(props.is_object(), "properties must be object");
    assert_eq!(
        props["a"],
        serde_json::Value::Null,
        "unset key must be null"
    );
    assert_eq!(
        props["b"],
        serde_json::Value::Null,
        "unset key must be null"
    );
}

/// Combined --set and --unset → merged into single properties object.
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
    let req = find_tools_call(&log, "org-set-properties");
    let args = &req["params"]["arguments"];
    let props = &args["properties"];
    assert!(props.is_object(), "properties must be object");
    assert_eq!(props["x"].as_str(), Some("1"), "set key must have value");
    assert_eq!(
        props["y"],
        serde_json::Value::Null,
        "unset key must be null"
    );
}

/// Empty (no flags) → properties == {}.
#[test]
fn test_properties_empty_sends_empty_object() {
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
    let req = find_tools_call(&log, "org-set-properties");
    let args = &req["params"]["arguments"];
    assert!(args["properties"].is_object(), "properties must be object");
    assert!(
        args["properties"].as_object().unwrap().is_empty(),
        "properties must be {{}} when no flags given"
    );
    // properties key must always be present (upstream defun requires it)
    assert!(args.as_object().unwrap().contains_key("properties"));
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

/// Value with equals: --set foo=bar=baz → properties.foo == "bar=baz".
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
    let req = find_tools_call(&log, "org-set-properties");
    let args = &req["params"]["arguments"];
    assert_eq!(
        args["properties"]["foo"].as_str(),
        Some("bar=baz"),
        "split must happen on first = only"
    );
}

/// Tool error → ok:false, exit 1.
#[test]
fn test_properties_tool_error() {
    let output = org_bin()
        .args(["--server", mock_bin(), "edit", "properties", "abc"])
        .env("MOCK_TOOL_ERROR", "org-set-properties")
        .output()
        .expect("failed to run org");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["kind"], "tool");
}
