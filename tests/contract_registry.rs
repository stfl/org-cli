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
    // Count from PLAN §6 + §6.1:
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
