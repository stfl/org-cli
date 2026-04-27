/// Contract registry tests — written RED first (Phase 2 TDD).
///
/// These tests assert that `contract::COMMANDS` contains the full planned CLI
/// surface with consistent, correct metadata. They must FAIL until
/// `src/contract.rs` is implemented.
use org_cli::contract::{
    COMMANDS, CommandSpec, OutputShape, ParamKind, ParamType, ServerReturns, ServerValue,
    TargetKind, UriRule,
};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn find_command(path: &[&str]) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.path == path)
}

fn find_param<'a>(cmd: &'a CommandSpec, name: &str) -> Option<&'a org_cli::contract::ParamSpec> {
    cmd.params.iter().find(|p| p.name == name)
}

// ---------------------------------------------------------------------------
// Registry shape
// ---------------------------------------------------------------------------

#[test]
fn test_registry_has_expected_count() {
    // Expected count based on the CLI command surface (src/contract.rs COMMANDS):
    // read, read-headline, outline                           = 3
    // query, query inbox, query next, query backlog          = 4
    // todo state, todo add                                   = 2
    // edit rename, edit body, edit properties, edit tags,
    //   edit priority, edit scheduled, edit deadline,
    //   edit log-note                                        = 8
    // clock status, clock in, clock out, clock add,
    //   clock delete, clock dangling                         = 6
    // config todo, config tags, config tag-candidates,
    //   config priority, config files, config clock          = 6
    // tools list, tools call                                 = 2
    // schema, schema <path>                                  = 2
    // Total                                                  = 33
    assert_eq!(
        COMMANDS.len(),
        33,
        "registry must have exactly 33 entries; got {}",
        COMMANDS.len()
    );
}

#[test]
fn test_all_paths_are_unique() {
    let mut seen: Vec<&[&str]> = Vec::new();
    for cmd in COMMANDS {
        assert!(!seen.contains(&cmd.path), "duplicate path {:?}", cmd.path);
        seen.push(cmd.path);
    }
}

#[test]
fn test_tool_kind_commands_have_non_empty_target() {
    for cmd in COMMANDS {
        if matches!(cmd.kind, TargetKind::Tool) {
            assert!(
                !cmd.target.is_empty(),
                "Tool command {:?} must have a non-empty target",
                cmd.path
            );
        }
    }
}

#[test]
fn test_internal_kind_commands_have_empty_target() {
    for cmd in COMMANDS {
        if matches!(cmd.kind, TargetKind::Internal) {
            assert!(
                cmd.target.is_empty(),
                "Internal command {:?} must have empty target, got {:?}",
                cmd.path,
                cmd.target
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Specific command contracts
// ---------------------------------------------------------------------------

#[test]
fn test_edit_body_uses_resource_uri_server_name() {
    let cmd = find_command(&["edit", "body"]).expect("edit body must be in registry");
    let uri_param = cmd
        .params
        .iter()
        .find(|p| p.server_name == "resource_uri")
        .expect("edit body must have a param with server_name = resource_uri");
    assert_eq!(uri_param.name, "uri", "the CLI name should be 'uri'");
}

#[test]
fn test_clock_in_resolve_is_bool_as_string() {
    let cmd = find_command(&["clock", "in"]).expect("clock in must be in registry");
    let resolve = find_param(cmd, "resolve").expect("clock in must have --resolve param");
    assert!(
        matches!(resolve.server_value, ServerValue::BoolAsString),
        "--resolve must be ServerValue::BoolAsString, got {:?}",
        resolve.server_value
    );
}

#[test]
fn test_outline_uses_file_path_type() {
    let cmd = find_command(&["outline"]).expect("outline must be in registry");
    let file_param = find_param(cmd, "file").expect("outline must have a 'file' param");
    assert!(
        matches!(file_param.ty, ParamType::FilePath),
        "outline file param must be FilePath, got {:?}",
        file_param.ty
    );
}

#[test]
fn test_read_headline_has_plain_text_server_returns() {
    let cmd = find_command(&["read-headline"]).expect("read-headline must be in registry");
    match &cmd.output_shape {
        OutputShape::Tool { server_returns, .. } => {
            assert!(
                matches!(server_returns, ServerReturns::PlainText),
                "read-headline must have ServerReturns::PlainText"
            );
        }
        other => panic!("read-headline must be OutputShape::Tool, got {:?}", other),
    }
}

#[test]
fn test_gtd_trio_present() {
    find_command(&["query", "inbox"]).expect("query inbox must be in registry");
    find_command(&["query", "next"]).expect("query next must be in registry");
    find_command(&["query", "backlog"]).expect("query backlog must be in registry");
}

#[test]
fn test_read_accepts_either_uri_form() {
    let cmd = find_command(&["read"]).expect("read must be in registry");
    let uri_param = find_param(cmd, "uri").expect("read must have a 'uri' param");
    assert!(
        matches!(uri_param.uri_rule, UriRule::EitherAccepted),
        "read uri param must be EitherAccepted, got {:?}",
        uri_param.uri_rule
    );
}

#[test]
fn test_query_params() {
    let cmd = find_command(&["query"]).expect("query must be in registry");
    let ql_param = find_param(cmd, "ql_expr").expect("query must have ql_expr param");
    assert!(matches!(ql_param.kind, ParamKind::Positional));
    let files_param = find_param(cmd, "files").expect("query must have --files param");
    assert!(files_param.repeated, "--files must be repeated");
}

#[test]
fn test_todo_add_after_is_bare_only() {
    let cmd = find_command(&["todo", "add"]).expect("todo add must be in registry");
    let after = find_param(cmd, "after").expect("todo add must have --after param");
    assert!(
        matches!(after.uri_rule, UriRule::BareOnly),
        "--after must be BareOnly, got {:?}",
        after.uri_rule
    );
}

#[test]
fn test_schema_commands_are_internal() {
    let schema = find_command(&["schema"]).expect("schema must be in registry");
    assert!(matches!(schema.kind, TargetKind::Internal));
    let schema_path =
        find_command(&["schema", "<path>"]).expect("schema <path> must be in registry");
    assert!(matches!(schema_path.kind, TargetKind::Internal));
}

// ---------------------------------------------------------------------------
// Parity with upstream org-mcp.el (sibling repo) — see ticket org-cli-j8z
// ---------------------------------------------------------------------------
//
// The CLI's `target` and `server_name` fields must match the real org-mcp
// Emacs server. These tests parse ../org-mcp/org-mcp.el and assert parity.
// If the sibling repo is missing (CI without it), tests log a warning and
// pass — they must not block builds.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn org_mcp_el_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is .../org-cli, sibling is .../org-mcp/org-mcp.el
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(|p| p.join("org-mcp").join("org-mcp.el"))
        .unwrap_or_else(|| PathBuf::from("../org-mcp/org-mcp.el"))
}

fn read_org_mcp_el() -> Option<String> {
    let p = org_mcp_el_path();
    match std::fs::read_to_string(&p) {
        Ok(s) => Some(s),
        Err(_) => {
            eprintln!(
                "warning: ../org-mcp/org-mcp.el not found at {:?} — \
                 skipping parity test (sibling repo absent)",
                p
            );
            None
        }
    }
}

/// Collect all `:id "..."` values from the elisp source.
fn collect_tool_ids(src: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    for line in src.lines() {
        if let Some(start) = line.find(":id \"") {
            let after = &line[start + 5..];
            if let Some(end) = after.find('"') {
                ids.insert(after[..end].to_string());
            }
        }
    }
    ids
}

/// Collect every `(defun org-mcp--tool-NAME (arglist)` and return a map
/// of defun name -> Vec<arg name> (without `&optional` / `&rest` markers).
fn collect_defun_arglists(src: &str) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let prefix = "(defun org-mcp--tool-";
    let mut search = 0;
    while let Some(pos) = src[search..].find(prefix) {
        let abs = search + pos;
        let after_prefix = abs + prefix.len();
        // Read symbol suffix up to whitespace or '('
        let suffix_end = src[after_prefix..]
            .find(|c: char| c.is_whitespace() || c == '(')
            .map(|i| after_prefix + i);
        let Some(se) = suffix_end else {
            break;
        };
        let suffix = &src[after_prefix..se];
        let defun = format!("org-mcp--tool-{}", suffix);
        // Find the arglist's opening paren
        if let Some(paren_off) = src[se..].find('(') {
            let arg_start = se + paren_off + 1;
            if let Some(end_off) = src[arg_start..].find(')') {
                let arglist = &src[arg_start..arg_start + end_off];
                let args: Vec<String> = arglist
                    .split_whitespace()
                    .filter(|w| !w.starts_with('&'))
                    .map(String::from)
                    .collect();
                out.insert(defun, args);
            }
        }
        search = after_prefix;
    }
    out
}

/// Build {tool-id -> arglist} map by walking `mcp-server-lib-register-tool`
/// blocks in the elisp source, joining each `#'org-mcp--tool-NAME` to the
/// `:id "..."` that immediately follows.
fn collect_id_arglists(src: &str) -> HashMap<String, Vec<String>> {
    let defuns = collect_defun_arglists(src);
    let mut out = HashMap::new();
    let prefix = "#'org-mcp--tool-";
    let mut search = 0;
    while let Some(pos) = src[search..].find(prefix) {
        let abs = search + pos;
        let after_prefix = abs + prefix.len();
        let suffix_end = src[after_prefix..]
            .find(|c: char| c.is_whitespace() || c == ')')
            .map(|i| after_prefix + i);
        let Some(se) = suffix_end else {
            break;
        };
        let suffix = &src[after_prefix..se];
        let defun = format!("org-mcp--tool-{}", suffix);
        // Look ahead a small window for `:id "..."`
        let win_end = (se + 600).min(src.len());
        let win = &src[se..win_end];
        if let Some(id_off) = win.find(":id \"") {
            let id_start = id_off + 5;
            if let Some(id_end) = win[id_start..].find('"') {
                let id = win[id_start..id_start + id_end].to_string();
                if let Some(args) = defuns.get(&defun) {
                    out.insert(id, args.clone());
                }
            }
        }
        search = after_prefix;
    }
    out
}

#[test]
fn parity_with_org_mcp_el() {
    let Some(src) = read_org_mcp_el() else {
        return;
    };
    let ids = collect_tool_ids(&src);
    let mut missing = Vec::new();
    for cmd in COMMANDS {
        if matches!(cmd.kind, TargetKind::Tool) && !ids.contains(cmd.target) {
            missing.push((cmd.path, cmd.target));
        }
    }
    assert!(
        missing.is_empty(),
        "{} contract Tool target(s) are not registered in ../org-mcp/org-mcp.el:\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(p, t)| format!("  {:?} -> target={:?}", p, t))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[test]
fn param_names_match() {
    let Some(src) = read_org_mcp_el() else {
        return;
    };
    let id_args = collect_id_arglists(&src);
    let mut bad: Vec<String> = Vec::new();
    for cmd in COMMANDS {
        if !matches!(cmd.kind, TargetKind::Tool) {
            continue;
        }
        let Some(args) = id_args.get(cmd.target) else {
            // Missing :id is reported by parity_with_org_mcp_el; skip here.
            continue;
        };
        for p in cmd.params {
            if !args.iter().any(|a| a == p.server_name) {
                bad.push(format!(
                    "  {:?} param {:?} server_name={:?} not in defun arglist {:?}",
                    cmd.path, p.name, p.server_name, args
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} param-name mismatch(es) against ../org-mcp/org-mcp.el:\n{}",
        bad.len(),
        bad.join("\n"),
    );
}
