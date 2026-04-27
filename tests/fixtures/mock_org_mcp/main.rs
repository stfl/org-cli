/// Mock org-mcp server for testing.
///
/// Reads newline-delimited JSON-RPC from stdin and writes newline-delimited
/// JSON-RPC to stdout. Implements the MCP initialize handshake, tools/list,
/// and tools/call.
///
/// Environment variables:
///   MOCK_TOOL_ERROR=<tool_name>      — that tool returns a JSON-RPC error
///   MOCK_TOOL_ERROR_DATA=<json>      — attach this JSON value as error.data (used with MOCK_TOOL_ERROR)
///   MOCK_NO_GTD=1                    — omit GTD tools from tools/list
///   MOCK_RECORD_REQUESTS=1           — write each received request as a JSON line to MOCK_REQUEST_LOG
///   MOCK_REQUEST_LOG=<path>          — file path for request log (used with MOCK_RECORD_REQUESTS)
///   MOCK_DIE_AFTER_HANDSHAKE=1       — close stdout immediately after sending the initialize response
///   MOCK_HANG_MS=<n>                 — sleep n ms before sending each response (covers initialize + tools/list + tools/call); used by transport-timeout tests
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

fn tool_error_name() -> Option<String> {
    std::env::var("MOCK_TOOL_ERROR").ok()
}

fn tool_error_data() -> Option<Value> {
    std::env::var("MOCK_TOOL_ERROR_DATA")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn die_after_handshake() -> bool {
    std::env::var("MOCK_DIE_AFTER_HANDSHAKE").as_deref() == Ok("1")
}

fn hang_ms() -> u64 {
    std::env::var("MOCK_HANG_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn maybe_hang() {
    let ms = hang_ms();
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

fn no_gtd() -> bool {
    std::env::var("MOCK_NO_GTD").as_deref() == Ok("1")
}

fn record_request(req_json: &str) {
    if std::env::var("MOCK_RECORD_REQUESTS").as_deref() != Ok("1") {
        return;
    }
    if let Ok(log_path) = std::env::var("MOCK_REQUEST_LOG") {
        use std::fs::OpenOptions;
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(f, "{}", req_json);
        }
    }
}

fn make_tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

fn tools_list() -> Value {
    let uri_schema = json!({
        "type": "object",
        "properties": {
            "uri": {"type": "string", "description": "Org node URI or identifier"}
        },
        "required": ["uri"]
    });

    let mut tools = vec![
        make_tool(
            "org-read",
            "Read an org node and its children as JSON",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string", "description": "Node URI, UUID, file path, or file#headline"}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-read-headline",
            "Read an org node as plain text",
            uri_schema.clone(),
        ),
        make_tool(
            "org-read-outline",
            "List the outline of an org file",
            json!({
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "Absolute path to org file"}
                },
                "required": ["file"]
            }),
        ),
        make_tool(
            "org-update-todo-state",
            "Update the TODO state of a node",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "new_state": {"type": "string"},
                    "current_state": {"type": "string"},
                    "note": {"type": "string"}
                },
                "required": ["uri", "new_state"]
            }),
        ),
        make_tool(
            "org-add-todo",
            "Add a new TODO node",
            json!({
                "type": "object",
                "properties": {
                    "parent_uri": {"type": "string"},
                    "title": {"type": "string"},
                    "todo_state": {"type": "string"},
                    "body": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "after_uri": {"type": "string"}
                },
                "required": ["parent_uri", "title"]
            }),
        ),
        make_tool(
            "org-edit-body",
            "Edit the body of a node",
            json!({
                "type": "object",
                "properties": {
                    "resource_uri": {"type": "string"},
                    "new_body": {"type": "string"},
                    "old_body": {"type": "string"},
                    "append": {"type": "boolean"}
                },
                "required": ["resource_uri", "new_body"]
            }),
        ),
        make_tool(
            "org-rename-headline",
            "Rename a node headline",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "current_title": {"type": "string"},
                    "new_title": {"type": "string"}
                },
                "required": ["uri", "current_title", "new_title"]
            }),
        ),
        make_tool(
            "org-set-properties",
            "Edit node properties",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "properties": {"type": "object"}
                },
                "required": ["uri", "properties"]
            }),
        ),
        make_tool(
            "org-set-tags",
            "Edit node tags",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-set-priority",
            "Edit node priority",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "priority": {"type": "string", "enum": ["A", "B", "C"]}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-update-scheduled",
            "Edit node scheduled date",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "scheduled": {"type": "string"}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-update-deadline",
            "Edit node deadline date",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "deadline": {"type": "string"}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-add-logbook-note",
            "Add a log note to a node",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "note": {"type": "string"}
                },
                "required": ["uri", "note"]
            }),
        ),
        make_tool(
            "org-clock-get-active",
            "Get current clock status",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-clock-in",
            "Clock in to a node",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "start_time": {"type": "string"},
                    "resolve": {"type": "boolean"}
                },
                "required": ["uri"]
            }),
        ),
        make_tool(
            "org-clock-out",
            "Clock out",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "end_time": {"type": "string"}
                }
            }),
        ),
        make_tool(
            "org-clock-add",
            "Add a clock entry",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "start": {"type": "string"},
                    "end": {"type": "string"}
                },
                "required": ["uri", "start", "end"]
            }),
        ),
        make_tool(
            "org-clock-delete",
            "Delete a clock entry",
            json!({
                "type": "object",
                "properties": {
                    "uri": {"type": "string"},
                    "start": {"type": "string"}
                },
                "required": ["uri", "start"]
            }),
        ),
        make_tool(
            "org-clock-find-dangling",
            "List dangling clock entries",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-todo-config",
            "Get configured TODO states",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-tag-config",
            "Get configured tags",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-tag-candidates",
            "Get tag candidates",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-priority-config",
            "Get priority configuration",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-allowed-files",
            "Get configured org files",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-get-clock-config",
            "Get clock configuration",
            json!({
                "type": "object",
                "properties": {}
            }),
        ),
        make_tool(
            "org-ql-query",
            "Query org files with org-ql",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "files": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["query"]
            }),
        ),
    ];

    if !no_gtd() {
        tools.push(make_tool(
            "query-inbox",
            "Query GTD inbox",
            json!({
                "type": "object",
                "properties": {}
            }),
        ));
        tools.push(make_tool(
            "query-next",
            "Query GTD next actions",
            json!({
                "type": "object",
                "properties": {
                    "tag": {"type": "string"}
                }
            }),
        ));
        tools.push(make_tool(
            "query-backlog",
            "Query GTD backlog",
            json!({
                "type": "object",
                "properties": {
                    "tag": {"type": "string"}
                }
            }),
        ));
    }

    json!({ "tools": tools })
}

fn handle_initialize(id: Value, out: &mut impl Write) {
    let resp = Response {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "mock-org-mcp",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        })),
        error: None,
    };
    let serialized = serde_json::to_string(&resp).unwrap();
    maybe_hang();
    writeln!(out, "{}", serialized).unwrap();
    out.flush().unwrap();

    // If MOCK_DIE_AFTER_HANDSHAKE=1, close stdout now to simulate a mid-protocol
    // transport failure. The client will get EOF on its next recv() call.
    if die_after_handshake() {
        std::process::exit(0);
    }
}

fn handle_tools_list(id: Value) -> Response {
    Response {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(tools_list()),
        error: None,
    }
}

fn handle_tools_call(id: Value, params: Option<Value>) -> Response {
    let params = params.unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // Check if this tool should error
    if let Some(err_tool) = tool_error_name()
        && err_tool == tool_name
    {
        return Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code: -32000,
                message: "Invalid input".to_string(),
                data: tool_error_data(),
            }),
        };
    }

    // Under MOCK_NO_GTD=1, GTD tools were stripped from tools/list. If the
    // client calls one anyway, behave like a real MCP server: return JSON-RPC
    // method-not-found (-32601). Mirrors what org-mcp's mcp-server-lib emits
    // for unknown tools.
    if no_gtd() && matches!(tool_name.as_str(), "query-inbox" | "query-next" | "query-backlog") {
        return Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code: -32601,
                message: format!("Tool not found: {}", tool_name),
                data: None,
            }),
        };
    }

    let content = match tool_name.as_str() {
        "org-read" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            json!([{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "title": "Mock Title",
                    "todo": "TODO",
                    "uri": format!("org://{}", uri),
                    "children": []
                })).unwrap()
            }])
        }
        "org-read-headline" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            json!([{
                "type": "text",
                "text": format!("* TODO Mock Title\nMock body for {}", uri)
            }])
        }
        "org-read-outline" => {
            let file = arguments
                .get("file")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            json!([{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "file": file,
                    "outline": [{"title": "Top", "level": 1, "children": []}]
                })).unwrap()
            }])
        }
        "org-update-todo-state" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let new_state = arguments
                .get("new_state")
                .and_then(Value::as_str)
                .unwrap_or("");
            // Optional fields: include as JSON null when absent so response shape is stable.
            let current_state = arguments
                .get("current_state")
                .cloned()
                .unwrap_or(Value::Null);
            let note = arguments.get("note").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": format!("org://{}", uri),
                "new_state": new_state,
                "current_state": current_state,
                "note": note,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-add-todo" => {
            let parent_uri = arguments.get("parent_uri").cloned().unwrap_or(Value::Null);
            let title = arguments.get("title").cloned().unwrap_or(Value::Null);
            let todo_state = arguments.get("todo_state").cloned().unwrap_or(Value::Null);
            let body = arguments.get("body").cloned().unwrap_or(Value::Null);
            let tags = arguments.get("tags").cloned().unwrap_or(json!([]));
            let after_uri = arguments.get("after_uri").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": "org://new-id-12345",
                "parent_uri": parent_uri,
                "title": title,
                "todo_state": todo_state,
                "tags": tags,
                "after_uri": after_uri,
                "body": body,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-ql-query" => {
            let ql_expr = arguments
                .get("ql_expr")
                .and_then(Value::as_str)
                .unwrap_or("");
            let payload = if ql_expr.contains("empty") {
                json!({"matches": [], "count": 0})
            } else {
                json!({
                    "matches": [
                        {"uri": "org://q1", "title": "Result 1"},
                        {"uri": "org://q2", "title": "Result 2"}
                    ],
                    "count": 2
                })
            };
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "query-inbox" => {
            let payload = json!({
                "matches": [{"uri": "org://inbox-item", "title": "Inbox 1"}],
                "count": 1
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "query-next" => {
            let tag = arguments.get("tag").and_then(Value::as_str);
            let mut entry = json!({"uri": "org://next-item", "title": "Next 1", "tags": ["work"]});
            if let Some(t) = tag {
                entry
                    .as_object_mut()
                    .unwrap()
                    .insert("filtered_tag".to_string(), json!(t));
            }
            let payload = json!({"matches": [entry], "count": 1});
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "query-backlog" => {
            let tag = arguments.get("tag").and_then(Value::as_str);
            let mut entry = json!({"uri": "org://backlog-item", "title": "Backlog 1"});
            if let Some(t) = tag {
                entry
                    .as_object_mut()
                    .unwrap()
                    .insert("filtered_tag".to_string(), json!(t));
            }
            let payload = json!({"matches": [entry], "count": 1});
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-get-active" => {
            let payload = json!({
                "clocked_in": true,
                "current": {
                    "uri": "org://current-task",
                    "title": "Working",
                    "since": "2026-04-27T08:00:00Z"
                }
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-in" => {
            // Echo arguments back so tests can assert exact wire format.
            // Critically: resolve is echoed exactly as received (string or bool)
            // so tests can assert it arrived as a JSON STRING "true"/"false".
            let uri = arguments.get("uri").and_then(Value::as_str).unwrap_or("");
            let start_time = arguments.get("start_time").cloned().unwrap_or(Value::Null);
            let resolve = arguments.get("resolve").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": format!("org://{}", uri),
                "start_time": start_time,
                "resolve": resolve,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-out" => {
            // Echo uri if present, else use "current" to indicate no-uri case.
            let uri_val = arguments.get("uri").and_then(Value::as_str);
            let uri_str = uri_val
                .map(|u| format!("org://{}", u))
                .unwrap_or_else(|| "org://current".to_string());
            let end_time = arguments.get("end_time").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": uri_str,
                "end_time": end_time,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-add" => {
            let uri = arguments.get("uri").and_then(Value::as_str).unwrap_or("");
            let start = arguments.get("start").cloned().unwrap_or(Value::Null);
            let end = arguments.get("end").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": format!("org://{}", uri),
                "start": start,
                "end": end,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-delete" => {
            let uri = arguments.get("uri").and_then(Value::as_str).unwrap_or("");
            let start = arguments.get("start").cloned().unwrap_or(Value::Null);
            let payload = json!({
                "success": true,
                "uri": format!("org://{}", uri),
                "start": start,
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-clock-find-dangling" => {
            let payload = json!({
                "dangling": [{
                    "uri": "org://orphan-1",
                    "title": "Unterminated clock",
                    "start": "2026-04-26T22:00:00Z"
                }],
                "count": 1
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-todo-config" => {
            let payload = json!({
                "keywords": ["TODO", "NEXT", "WAITING", "DONE", "CANCELLED"],
                "groups": [
                    {"name": "todo", "keywords": ["TODO", "NEXT", "WAITING"]},
                    {"name": "done", "keywords": ["DONE", "CANCELLED"]}
                ]
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-tag-config" => {
            let payload = json!({
                "tags": ["work", "home", "urgent", "read"]
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-tag-candidates" => {
            let payload = json!({
                "candidates": [
                    {"tag": "work", "count": 42},
                    {"tag": "home", "count": 17}
                ]
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-priority-config" => {
            let payload = json!({
                "priorities": ["A", "B", "C"],
                "default": "B"
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-allowed-files" => {
            let payload = json!({
                "agenda_files": [
                    "/home/user/.org/inbox.org",
                    "/home/user/.org/projects.org"
                ]
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        "org-get-clock-config" => {
            let payload = json!({
                "persist": true,
                "resolve_strategy": "prompt",
                "idle_minutes": 15
            });
            json!([{
                "type": "text",
                "text": serde_json::to_string(&payload).unwrap()
            }])
        }
        _ => {
            // Generic: echo args + success
            let uri = arguments
                .get("uri")
                .or_else(|| arguments.get("resource_uri"))
                .cloned()
                .unwrap_or(json!("synthetic-uri"));
            json!([{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "success": true,
                    "uri": uri,
                    "tool": tool_name,
                    "arguments": arguments
                })).unwrap()
            }])
        }
    };

    Response {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(json!({ "content": content })),
        error: None,
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                writeln!(out, "{}", serde_json::to_string(&err).unwrap()).unwrap();
                out.flush().unwrap();
                continue;
            }
        };

        // Record the raw request line for tests that need to inspect what was sent.
        record_request(&line);

        // Notifications have no id — no response
        if req.id.is_none() && req.method.starts_with("notifications/") {
            continue;
        }

        let id = req.id.unwrap_or(Value::Null);

        // initialize writes directly (and may exit for MOCK_DIE_AFTER_HANDSHAKE).
        if req.method == "initialize" {
            handle_initialize(id, &mut out);
            continue;
        }

        let response = match req.method.as_str() {
            "tools/list" => handle_tools_list(id),
            "tools/call" => handle_tools_call(id, req.params),
            other => Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(RpcError {
                    code: -32601,
                    message: format!("Method not found: {}", other),
                    data: None,
                }),
            },
        };

        let serialized = serde_json::to_string(&response).unwrap();
        maybe_hang();
        writeln!(out, "{}", serialized).unwrap();
        out.flush().unwrap();
    }
}
