mod cli;
mod mcp;
mod uri;

use clap::Parser;
use serde_json::{Value, json};

use cli::{Cli, Commands, QueryArgs, QueryKind, SchemaArgs, TodoArgs, TodoKind, ToolsCmd};
use mcp::client::Client;
use mcp::error::McpError;
use org_cli::contract;
use org_cli::output::{ErrorKind, print_error, print_success};

fn main() {
    let cli = Cli::parse();
    let compact = cli.compact;

    let code = run(cli, compact);
    std::process::exit(code);
}

fn run(cli: Cli, compact: bool) -> i32 {
    // Schema is a local introspection command — no server needed.
    if let Commands::Schema(ref args) = cli.command {
        return cmd_schema(args, compact);
    }

    // Outline validation is also local — reject org:// paths before spawning server.
    if let Commands::Outline { file } = &cli.command
        && let Err(e) = uri::validate_outline_path(file)
    {
        return print_error(ErrorKind::Usage, 2, e.to_string(), json!(null), 2, compact);
    }

    // Resolve server argv for all other commands.
    let argv = match cli.server_argv() {
        Ok(a) => a,
        Err(msg) => {
            return print_error(ErrorKind::Usage, 2, msg, json!(null), 2, compact);
        }
    };

    match &cli.command {
        Commands::Read { uri } => cmd_read(&argv, uri, compact),
        Commands::ReadHeadline { uri } => cmd_read_headline(&argv, uri, compact),
        Commands::Outline { file } => cmd_outline(&argv, file, compact),
        Commands::Query(query_args) => cmd_query(&argv, query_args, compact),
        Commands::Todo(todo_args) => cmd_todo(&argv, todo_args, compact),
        Commands::Tools { cmd } => match cmd {
            ToolsCmd::List => cmd_tools_list(&argv, compact),
            ToolsCmd::Call { name, args } => {
                // Parse --args JSON if provided
                let arguments: Value = match args {
                    None => json!({}),
                    Some(s) => match serde_json::from_str(s) {
                        Ok(v) => v,
                        Err(e) => {
                            return print_error(
                                ErrorKind::Usage,
                                2,
                                format!("--args is not valid JSON: {}", e),
                                json!(null),
                                2,
                                compact,
                            );
                        }
                    },
                };
                cmd_tools_call(&argv, name, arguments, compact)
            }
        },
        // Schema already handled above.
        Commands::Schema(_) => unreachable!(),
    }
}

fn cmd_schema(args: &SchemaArgs, compact: bool) -> i32 {
    if args.path.is_empty() {
        // `org schema` — return all commands
        let data = contract::serialize_all();
        print_success(data, compact);
        0
    } else {
        // `org schema <path...>` — return single command
        let path_refs: Vec<&str> = args.path.iter().map(String::as_str).collect();
        match contract::serialize_one(&path_refs) {
            Some(data) => {
                print_success(data, compact);
                0
            }
            None => {
                let msg = format!("unknown command path: {}", args.path.join(" "));
                print_error(ErrorKind::Usage, 2, msg, json!(null), 2, compact)
            }
        }
    }
}

fn connect(argv: &[String], compact: bool) -> Result<Client, i32> {
    Client::connect(argv).map_err(|e| handle_mcp_error(e, compact))
}

fn handle_mcp_error(e: McpError, compact: bool) -> i32 {
    let exit_code = e.exit_code();
    let kind = match e.kind_str() {
        "tool" => ErrorKind::Tool,
        _ => ErrorKind::Transport,
    };
    print_error(
        kind,
        e.rpc_code(),
        e.to_string(),
        json!(null),
        exit_code,
        compact,
    )
}

fn cmd_tools_list(argv: &[String], compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };

    match client.tools_list() {
        Ok(tools) => {
            print_success(json!({ "tools": tools }), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_tools_call(argv: &[String], tool_name: &str, arguments: Value, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };

    match client.tools_call(tool_name, arguments) {
        Ok(content) => {
            // Parse the result: content is an array of content items.
            // Extract text from the first item; try to parse as JSON, else wrap as string.
            let result = extract_result(content);
            print_success(json!({ "tool": tool_name, "result": result }), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_read(argv: &[String], uri: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    match client.tools_call("org-read", json!({ "uri": bare })) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_read_headline(argv: &[String], uri: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    match client.tools_call("org-read-headline", json!({ "uri": bare })) {
        Ok(content) => {
            // Server returns plain text; extract_result wraps it as {"text": "..."}
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_outline(argv: &[String], file: &str, compact: bool) -> i32 {
    // validate_outline_path was already checked in run() before server spawn;
    // this is here for completeness / future direct calls.
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match client.tools_call("org-outline", json!({ "file": file })) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_query(argv: &[String], args: &QueryArgs, compact: bool) -> i32 {
    match &args.kind {
        QueryKind::Run { ql_expr, files } => cmd_query_run(argv, ql_expr, files, compact),
        QueryKind::Inbox => cmd_query_gtd(argv, "query-inbox", None, compact),
        QueryKind::Next { tag } => cmd_query_gtd(argv, "query-next", tag.as_deref(), compact),
        QueryKind::Backlog { tag } => cmd_query_gtd(argv, "query-backlog", tag.as_deref(), compact),
    }
}

fn cmd_query_run(argv: &[String], ql_expr: &str, files: &[String], compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let args = json!({ "ql_expr": ql_expr, "files": files });
    match client.tools_call("org-ql-query", args) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

/// Run a GTD query tool after checking capability discovery.
/// If the tool is not advertised by the server, returns a usage error (exit 2).
fn cmd_query_gtd(argv: &[String], tool_name: &str, tag: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // Capability check — GTD tools are optional and conditional on runtime config.
    match client.server_has_tool(tool_name) {
        Ok(true) => {}
        Ok(false) => {
            return print_error(
                ErrorKind::Usage,
                -1,
                format!(
                    "GTD tool '{}' not advertised by server \
                     (org-mcp GTD support is optional and conditional on runtime config)",
                    tool_name
                ),
                json!(null),
                2,
                compact,
            );
        }
        Err(e) => return handle_mcp_error(e, compact),
    }

    let arguments = match tag {
        Some(t) => json!({ "tag": t }),
        None => json!({}),
    };

    match client.tools_call(tool_name, arguments) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_todo(argv: &[String], args: &TodoArgs, compact: bool) -> i32 {
    match &args.kind {
        TodoKind::State {
            uri,
            new_state,
            from,
            note,
        } => cmd_todo_state(
            argv,
            uri,
            new_state,
            from.as_deref(),
            note.as_deref(),
            compact,
        ),
        TodoKind::Add {
            parent,
            title,
            state,
            body,
            tags,
            after,
        } => cmd_todo_add(
            argv,
            parent,
            title,
            state,
            body.as_deref(),
            tags,
            after.as_deref(),
            compact,
        ),
    }
}

fn cmd_todo_state(
    argv: &[String],
    uri: &str,
    new_state: &str,
    from: Option<&str>,
    note: Option<&str>,
    compact: bool,
) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare_uri = uri::normalize_for_tool(uri);

    // Build arguments: always include uri and new_state; omit from/note when None.
    let mut arguments = json!({
        "uri": bare_uri,
        "new_state": new_state,
    });
    if let Some(f) = from {
        arguments["from"] = json!(f);
    }
    if let Some(n) = note {
        arguments["note"] = json!(n);
    }

    match client.tools_call("org-update-todo-state", arguments) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_todo_add(
    argv: &[String],
    parent: &str,
    title: &str,
    state: &str,
    body: Option<&str>,
    tags: &[String],
    after: Option<&str>,
    compact: bool,
) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    // NOTE: parent_uri is the server param name — defensive choice consistent with after_uri.
    // The real server may use "parent" instead; update here and in contract.rs if confirmed.
    let bare_parent = uri::normalize_for_tool(parent);

    // Build arguments: parent_uri, title, state are always sent.
    // tags is always sent as an array (empty [] if none) for deterministic shape.
    // body and after_uri are omitted when None.
    let mut arguments = json!({
        "parent_uri": bare_parent,
        "title": title,
        "state": state,
        "tags": tags,
    });
    if let Some(b) = body {
        arguments["body"] = json!(b);
    }
    if let Some(a) = after {
        // after_uri is ID-based per PLAN §5.6 — strip org:// prefix.
        let bare_after = uri::normalize_for_tool(a);
        arguments["after_uri"] = json!(bare_after);
    }

    match client.tools_call("org-add-todo", arguments) {
        Ok(content) => {
            let data = extract_result(content);
            print_success(data, compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

/// Extract a usable result value from an MCP content array.
///
/// MCP tool responses contain a `content` array of typed items. For the
/// tools/call escape hatch we return a single result value by taking the
/// first text item and attempting to parse it as JSON (for structured tools),
/// or falling back to a `{"text": ...}` wrapper (for plain-text tools like
/// org-read-headline). If the content is not an array, return it as-is.
fn extract_result(content: Value) -> Value {
    let items = match content.as_array() {
        Some(a) => a,
        None => return content,
    };

    if items.is_empty() {
        return json!(null);
    }

    // Take the first text item
    if let Some(text) = items[0].get("text").and_then(Value::as_str) {
        // Try to parse as JSON first
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            return parsed;
        }
        // Plain text — wrap as {text: ...}
        return json!({ "text": text });
    }

    // No text field — return the raw item
    items[0].clone()
}
