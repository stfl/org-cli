/// Integration tests for MCP content type variants in extract_result.
use std::process::Command;

fn org_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// Baseline: text variant returns the parsed JSON object as data (existing behavior).
#[test]
fn test_text_variant_unchanged() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "test-node"])
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert!(v["data"]["title"].is_string(), "data.title must be present");
    assert!(v["data"]["uri"].is_string(), "data.uri must be present");
}

/// Image variant: content array has a single image item; data must be the typed shape.
#[test]
fn test_image_variant() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "test-node"])
        .env("MOCK_RESPONSE_KIND", "image")
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["type"], "image", "data.type must be image");
    assert_eq!(
        v["data"]["mime_type"], "image/png",
        "mime_type must be image/png (snake_case)"
    );
    assert!(
        v["data"]["data"].is_string(),
        "data.data must be base64 string"
    );
    assert!(
        !v["data"]["data"].as_str().unwrap().is_empty(),
        "data.data must not be empty"
    );
}

/// Resource variant: content array has a single resource item; data must be the typed shape.
#[test]
fn test_resource_variant() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "test-node"])
        .env("MOCK_RESPONSE_KIND", "resource")
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["type"], "resource", "data.type must be resource");
    assert_eq!(v["data"]["uri"], "org://res-xyz", "uri must match");
    assert_eq!(
        v["data"]["mime_type"], "text/plain",
        "mime_type must be text/plain (snake_case)"
    );
    assert_eq!(v["data"]["text"], "hello", "text must match");
}

/// Mixed variant: content has two items (text + image); data must be a JSON array.
#[test]
fn test_mixed_variant_is_array() {
    let output = org_bin()
        .args(["--server", mock_bin(), "read", "test-node"])
        .env("MOCK_RESPONSE_KIND", "mixed")
        .output()
        .expect("failed to run org");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(v["ok"], true);
    assert!(v["data"].is_array(), "data must be an array for mixed content");

    let arr = v["data"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "array must have exactly 2 items");

    // First item: text parsed as JSON object
    assert!(
        arr[0].is_object(),
        "first item must be the parsed JSON object from text"
    );
    assert_eq!(arr[0]["key"], "value", "first item must have key=value");

    // Second item: image shape
    assert_eq!(arr[1]["type"], "image", "second item type must be image");
    assert_eq!(arr[1]["mime_type"], "image/png", "second item mime_type");
    assert!(arr[1]["data"].is_string(), "second item data must be string");
}
