mod cli;
mod discovery;
mod mcp;
mod uri;

use clap::Parser;
use serde_json::{Value, json};

use cli::{
    Cli, ClockArgs, ClockKind, Commands, ConfigArgs, ConfigKind, EditArgs, EditKind, QueryArgs,
    QueryKind, SchemaArgs, TodoArgs, TodoKind, ToolsCmd,
};
use mcp::client::Client;
use mcp::error::McpError;
use org_cli::argv::split_sentinel;
use org_cli::contract;
use org_cli::output::{ErrorKind, print_error, print_success};

const SUBCOMMAND_NAMES: &[&str] = &[
    "read",
    "read-headline",
    "outline",
    "query",
    "todo",
    "edit",
    "clock",
    "config",
    "schema",
    "tools",
];

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let (cleaned, extra) = split_sentinel(raw, SUBCOMMAND_NAMES);
    let mut cli = Cli::parse_from(cleaned);
    cli.server_args.extend(extra);
    let compact = cli.compact;

    let code = run(cli, compact);
    std::process::exit(code);
}

fn run(cli: Cli, compact: bool) -> i32 {
    RECV_TIMEOUT_SECS.store(cli.timeout, std::sync::atomic::Ordering::Relaxed);

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
    // If --server is omitted, try auto-discovery of emacs-mcp-stdio.sh in PATH.
    let argv = if cli.server.is_some() || !cli.server_args.is_empty() {
        match cli.server_argv() {
            Ok(a) => a,
            Err(msg) => {
                return print_error(ErrorKind::Usage, 2, msg, json!(null), 2, compact);
            }
        }
    } else {
        match discovery::discover_server() {
            Ok(a) => a,
            Err(msg) => {
                return print_error(ErrorKind::Usage, 2, msg, json!(null), 4, compact);
            }
        }
    };

    match &cli.command {
        Commands::Read { uri } => cmd_read(&argv, uri, compact),
        Commands::ReadHeadline { uri } => cmd_read_headline(&argv, uri, compact),
        Commands::Outline { file } => cmd_outline(&argv, file, compact),
        Commands::Query(query_args) => cmd_query(&argv, query_args, compact),
        Commands::Todo(todo_args) => cmd_todo(&argv, todo_args, compact),
        Commands::Edit(edit_args) => cmd_edit(&argv, edit_args, compact),
        Commands::Clock(clock_args) => cmd_clock(&argv, clock_args, compact),
        Commands::Config(config_args) => cmd_config(&argv, config_args, compact),
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

/// Per-recv() timeout in seconds. 0 disables the gate. Set once in `run()`
/// from `--timeout` / `ORG_TIMEOUT`; read by every `connect()` call.
static RECV_TIMEOUT_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(30);

fn connect(argv: &[String], compact: bool) -> Result<Client, i32> {
    let secs = RECV_TIMEOUT_SECS.load(std::sync::atomic::Ordering::Relaxed);
    let timeout = if secs == 0 {
        None
    } else {
        Some(std::time::Duration::from_secs(secs))
    };
    Client::connect_with_timeout(argv, timeout).map_err(|e| handle_mcp_error(e, compact))
}

fn handle_mcp_error(e: McpError, compact: bool) -> i32 {
    let exit_code = e.exit_code();
    let kind = match e.kind_str() {
        "tool" => ErrorKind::Tool,
        _ => ErrorKind::Transport,
    };
    let data = e.rpc_data();
    print_error(kind, e.rpc_code(), e.to_string(), data, exit_code, compact)
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
    match client.tools_call("org-read-outline", json!({ "file": file })) {
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
        Some(QueryKind::Run { ql_expr, files }) => cmd_query_run(argv, ql_expr, files, compact),
        Some(QueryKind::Inbox) => cmd_query_gtd(argv, "query-inbox", None, compact),
        Some(QueryKind::Next { tag }) => cmd_query_gtd(argv, "query-next", tag.as_deref(), compact),
        Some(QueryKind::Backlog { tag }) => {
            cmd_query_gtd(argv, "query-backlog", tag.as_deref(), compact)
        }
        None => {
            // Bare form: `org query "<expr>"` — dispatched as QueryKind::Run
            if let Some(expr) = &args.ql_expr {
                cmd_query_run(argv, expr, &args.files, compact)
            } else {
                // `org query` with no args or subcommand — usage error
                print_error(
                    ErrorKind::Usage,
                    2,
                    "org query requires an expression or subcommand (run/inbox/next/backlog)"
                        .to_string(),
                    json!(null),
                    2,
                    compact,
                )
            }
        }
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

/// Run a GTD query tool. GTD support is optional and conditional on runtime
/// config in org-mcp, so the server may not advertise these tools.
///
/// Strategy: try-call first; if the server returns JSON-RPC method-not-found
/// (-32601), fall back to a usage error envelope. This costs ONE round-trip
/// in the happy path — calling tools/list as a pre-check would always cost
/// two.
fn cmd_query_gtd(argv: &[String], tool_name: &str, tag: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };

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
        Err(McpError::ToolError { code: -32601, .. }) => print_error(
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
        ),
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

    // Build arguments: always include uri and new_state; omit current_state/note when None.
    let mut arguments = json!({
        "uri": bare_uri,
        "new_state": new_state,
    });
    if let Some(f) = from {
        arguments["current_state"] = json!(f);
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

    // Build arguments: parent_uri, title, todo_state are always sent.
    // tags is always sent as an array (empty [] if none) for deterministic shape.
    // body and after_uri are omitted when None.
    let mut arguments = json!({
        "parent_uri": bare_parent,
        "title": title,
        "todo_state": state,
        "tags": tags,
    });
    if let Some(b) = body {
        arguments["body"] = json!(b);
    }
    if let Some(a) = after {
        // after_uri is ID-based (bare form only) — strip org:// prefix if present.
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

/// Parse `k=v` strings into a JSON object. Splits on the first `=` only.
/// Returns Err with a usage-error message if any pair has no `=`.
fn parse_set_pairs(pairs: &[String]) -> Result<serde_json::Map<String, Value>, String> {
    let mut map = serde_json::Map::new();
    for pair in pairs {
        match pair.find('=') {
            Some(idx) => {
                let key = pair[..idx].to_string();
                let val = pair[idx + 1..].to_string();
                map.insert(key, json!(val));
            }
            None => {
                return Err(format!(
                    "--set value {:?} is malformed: expected k=v form",
                    pair
                ));
            }
        }
    }
    Ok(map)
}

fn cmd_edit(argv: &[String], args: &EditArgs, compact: bool) -> i32 {
    match &args.kind {
        EditKind::Rename { uri, from, to } => cmd_edit_rename(argv, uri, from, to, compact),
        EditKind::Body {
            uri,
            new,
            old,
            append,
        } => cmd_edit_body(argv, uri, new, old.as_deref(), *append, compact),
        EditKind::Properties { uri, sets, unsets } => {
            // Parse k=v pairs before spawning server — usage error exits early.
            let set_map = match parse_set_pairs(sets) {
                Ok(m) => m,
                Err(msg) => {
                    return print_error(ErrorKind::Usage, 2, msg, json!(null), 2, compact);
                }
            };
            cmd_edit_properties(argv, uri, set_map, unsets, compact)
        }
        EditKind::Tags { uri, tags } => cmd_edit_tags(argv, uri, tags, compact),
        EditKind::Priority { uri, priority } => {
            cmd_edit_priority(argv, uri, priority.as_deref(), compact)
        }
        EditKind::Scheduled { uri, date } => {
            cmd_edit_scheduled(argv, uri, date.as_deref(), compact)
        }
        EditKind::Deadline { uri, date } => cmd_edit_deadline(argv, uri, date.as_deref(), compact),
        EditKind::LogNote { uri, note } => cmd_edit_log_note(argv, uri, note, compact),
    }
}

fn cmd_edit_rename(argv: &[String], uri: &str, from: &str, to: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    let arguments = json!({
        "uri": bare,
        "current_title": from,
        "new_title": to,
    });
    match client.tools_call("org-rename-headline", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_body(
    argv: &[String],
    uri: &str,
    new: &str,
    old: Option<&str>,
    append: bool,
    compact: bool,
) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    // NOTE: server param key is `resource_uri` (not `uri`) — see `org-mcp--tool-edit-body` in ../org-mcp/org-mcp.el
    // NOTE: --new maps to `new_body`, --old maps to `old_body`
    let mut arguments = json!({
        "resource_uri": bare,
        "new_body": new,
        "append": append,
    });
    if let Some(o) = old {
        arguments["old_body"] = json!(o);
    }
    match client.tools_call("org-edit-body", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_properties(
    argv: &[String],
    uri: &str,
    set_map: serde_json::Map<String, Value>,
    unsets: &[String],
    compact: bool,
) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    // Merge --set k=v (value) and --unset k (null) into a single `properties` object.
    let mut properties = set_map;
    for key in unsets {
        properties.insert(key.clone(), Value::Null);
    }
    let arguments = json!({
        "uri": bare,
        "properties": properties,
    });
    match client.tools_call("org-set-properties", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_tags(argv: &[String], uri: &str, tags: &[String], compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    let arguments = json!({
        "uri": bare,
        "tags": tags,
    });
    match client.tools_call("org-set-tags", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_priority(argv: &[String], uri: &str, priority: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    // Send explicit null when no --priority so server distinguishes "clear" from "missing".
    let priority_val: Value = match priority {
        Some(p) => json!(p),
        None => Value::Null,
    };
    let arguments = json!({
        "uri": bare,
        "priority": priority_val,
    });
    match client.tools_call("org-set-priority", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_scheduled(argv: &[String], uri: &str, date: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    // Send explicit null when no --date so server distinguishes "clear" from "missing".
    let date_val: Value = match date {
        Some(d) => json!(d),
        None => Value::Null,
    };
    let arguments = json!({
        "uri": bare,
        "scheduled": date_val,
    });
    match client.tools_call("org-update-scheduled", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_deadline(argv: &[String], uri: &str, date: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    // Send explicit null when no --date so server distinguishes "clear" from "missing".
    let date_val: Value = match date {
        Some(d) => json!(d),
        None => Value::Null,
    };
    let arguments = json!({
        "uri": bare,
        "deadline": date_val,
    });
    match client.tools_call("org-update-deadline", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_edit_log_note(argv: &[String], uri: &str, note: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    let arguments = json!({
        "uri": bare,
        "note": note,
    });
    match client.tools_call("org-add-logbook-note", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock(argv: &[String], args: &ClockArgs, compact: bool) -> i32 {
    match &args.kind {
        ClockKind::Status => cmd_clock_status(argv, compact),
        ClockKind::In { uri, at, resolve } => {
            cmd_clock_in(argv, uri, at.as_deref(), *resolve, compact)
        }
        ClockKind::Out { uri, at } => cmd_clock_out(argv, uri.as_deref(), at.as_deref(), compact),
        ClockKind::Add { uri, start, end } => cmd_clock_add(argv, uri, start, end, compact),
        ClockKind::Delete { uri, at } => cmd_clock_delete(argv, uri, at, compact),
        ClockKind::Dangling => cmd_clock_dangling(argv, compact),
    }
}

fn cmd_clock_status(argv: &[String], compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match client.tools_call("org-clock-get-active", json!({})) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock_in(argv: &[String], uri: &str, at: Option<&str>, resolve: bool, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);

    // Build arguments: uri and resolve are always sent.
    // at is omitted when None.
    // CRITICAL: resolve is sent as a JSON STRING "true"/"false", NOT a JSON boolean.
    // The org-mcp server expects a string-shaped value here (see `org-mcp--tool-clock-in`
    // in ../org-mcp/org-mcp.el and contract.rs ServerValue::BoolAsString). Sending a native bool would silently
    // break server-side resolution logic.
    let resolve_str = if resolve { "true" } else { "false" };
    let mut arguments = json!({
        "uri": bare,
        "resolve": resolve_str,
    });
    if let Some(ts) = at {
        arguments["start_time"] = json!(ts);
    }

    match client.tools_call("org-clock-in", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock_out(argv: &[String], uri: Option<&str>, at: Option<&str>, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // Both uri and at are optional — omit keys entirely when None.
    // Omitting uri tells the server to use the currently active clock.
    let mut arguments = json!({});
    if let Some(u) = uri {
        let bare = uri::normalize_for_tool(u);
        arguments["uri"] = json!(bare);
    }
    if let Some(ts) = at {
        arguments["end_time"] = json!(ts);
    }

    match client.tools_call("org-clock-out", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock_add(argv: &[String], uri: &str, start: &str, end: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    let arguments = json!({
        "uri": bare,
        "start": start,
        "end": end,
    });
    match client.tools_call("org-clock-add", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock_delete(argv: &[String], uri: &str, at: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let bare = uri::normalize_for_tool(uri);
    let arguments = json!({
        "uri": bare,
        "start": at,
    });
    match client.tools_call("org-clock-delete", arguments) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_clock_dangling(argv: &[String], compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match client.tools_call("org-clock-find-dangling", json!({})) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

fn cmd_config(argv: &[String], args: &ConfigArgs, compact: bool) -> i32 {
    match &args.kind {
        ConfigKind::Todo => cmd_config_call(argv, "org-get-todo-config", compact),
        ConfigKind::Tags => cmd_config_call(argv, "org-get-tag-config", compact),
        ConfigKind::TagCandidates => cmd_config_call(argv, "org-get-tag-candidates", compact),
        ConfigKind::Priority => cmd_config_call(argv, "org-get-priority-config", compact),
        ConfigKind::Files => cmd_config_call(argv, "org-get-allowed-files", compact),
        ConfigKind::Clock => cmd_config_call(argv, "org-get-clock-config", compact),
    }
}

fn cmd_config_call(argv: &[String], tool_name: &str, compact: bool) -> i32 {
    let mut client = match connect(argv, compact) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match client.tools_call(tool_name, json!({})) {
        Ok(content) => {
            print_success(extract_result(content), compact);
            0
        }
        Err(e) => handle_mcp_error(e, compact),
    }
}

/// Extract a usable result value from an MCP content array.
///
/// Walks the content array and maps each item by its `type` field:
///   "text"              → JSON-parse the text value; if non-JSON, wrap as {text: <string>}
///   "image"             → {type:"image", mime_type:<mimeType>, data:<base64>}
///   "resource" /
///   "embedded_resource" → {type:"resource", uri, mime_type, text (if present)}
///   "resource_link"     → {type:"resource_link", uri, name, mime_type, description} (snake_cased)
///   unknown             → {type:<type>, raw:<original item>}
///
/// Single-item arrays return a scalar (preserves existing test shapes).
/// Multi-item arrays return a JSON array.
/// Non-array content is returned as-is (legacy defensive path).
fn extract_result(content: Value) -> Value {
    let items = match content.as_array() {
        Some(a) => a,
        None => return content,
    };

    if items.is_empty() {
        return json!(null);
    }

    let mapped: Vec<Value> = items.iter().map(map_content_item).collect();

    if mapped.len() == 1 {
        mapped.into_iter().next().unwrap()
    } else {
        Value::Array(mapped)
    }
}

fn map_content_item(item: &Value) -> Value {
    let type_str = item.get("type").and_then(Value::as_str).unwrap_or("");

    match type_str {
        "text" => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    return parsed;
                }
                return json!({ "text": text });
            }
            json!({ "type": "text", "raw": item })
        }
        "image" => {
            let mime_type = item
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("");
            let data = item.get("data").and_then(Value::as_str).unwrap_or("");
            json!({
                "type": "image",
                "mime_type": mime_type,
                "data": data
            })
        }
        "resource" | "embedded_resource" => {
            let res = item.get("resource").unwrap_or(&Value::Null);
            let uri = res.get("uri").and_then(Value::as_str).unwrap_or("");
            let mime_type = res.get("mimeType").and_then(Value::as_str).unwrap_or("");
            let mut out = json!({
                "type": "resource",
                "uri": uri,
                "mime_type": mime_type
            });
            if let Some(text) = res.get("text").and_then(Value::as_str) {
                out["text"] = json!(text);
            }
            out
        }
        "resource_link" => {
            let uri = item.get("uri").and_then(Value::as_str).unwrap_or("");
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            let mime_type = item.get("mimeType").and_then(Value::as_str).unwrap_or("");
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            json!({
                "type": "resource_link",
                "uri": uri,
                "name": name,
                "mime_type": mime_type,
                "description": description
            })
        }
        _ => {
            json!({ "type": type_str, "raw": item })
        }
    }
}
