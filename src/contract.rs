/// Command contract registry.
///
/// This module defines a static, compile-time-known registry of every CLI
/// command in PLAN §6 and §6.1. Each entry records the exact tool target,
/// parameter shapes, and output characteristics needed for both schema
/// introspection and future phase implementations.
///
/// The registry is the single source of truth for `org schema` output.

// ---------------------------------------------------------------------------
// Type definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// Calls an MCP tool on the server.
    Tool,
    /// Reads an MCP resource from the server.
    Resource,
    /// Handled entirely by the CLI without server IO.
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// A positional argument.
    Positional,
    /// A boolean flag (e.g. `--append`).
    Flag,
    /// A key-value option (e.g. `--note <text>`).
    KeyValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    /// An org:// URI or bare identifier — see UriRule for which forms.
    Uri,
    /// Always a bare UUID / file-path / file#path (never org://).
    BareUri,
    /// An absolute file system path.
    FilePath,
    /// A free-form string.
    String,
    /// ISO 8601 date (YYYY-MM-DD or YYYY-MM-DD HH:MM).
    IsoDate,
    /// ISO 8601 timestamp.
    IsoTimestamp,
    /// A TODO state keyword (e.g. TODO, DONE).
    TodoState,
    /// A priority letter (A, B, C).
    Priority,
    /// A boolean flag value.
    Bool,
    /// A raw JSON value (passed through as-is).
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriRule {
    /// Only bare identifiers accepted (no org:// prefix).
    BareOnly,
    /// Only org:// URIs accepted.
    OrgOnly,
    /// Either bare or org:// URI accepted.
    EitherAccepted,
    /// Not applicable (non-URI param).
    Na,
}

/// Quirk for how a parameter value is sent to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerValue {
    /// Send the value in its natural JSON type.
    Native,
    /// Send a boolean as a JSON string ("true"/"false") instead of bool.
    /// Used for clock-in --resolve per PLAN §5.6 §7.
    BoolAsString,
}

#[derive(Debug)]
pub enum ServerReturns {
    /// Server returns a structured JSON object.
    JsonObject,
    /// Server returns plain text; CLI wraps as `{"text": "..."}`.
    PlainText,
}

#[derive(Debug)]
pub enum OutputShape {
    Tool {
        server_returns: ServerReturns,
        /// Informal JSON shape descriptor for documentation.
        cli_data: &'static str,
    },
    Internal {
        cli_data: &'static str,
    },
}

#[derive(Debug)]
pub struct ParamSpec {
    /// The CLI flag/positional name (e.g. "uri", "new-state", "files").
    pub name: &'static str,
    /// The exact key used in the tool's JSON arguments sent to org-mcp.
    /// Usually matches `name` (with hyphens replaced by underscores), but
    /// differs for quirky tools like `org-edit-body` (resource_uri vs uri).
    pub server_name: &'static str,
    pub required: bool,
    /// True if the flag/option may be repeated (e.g. --tag, --files).
    pub repeated: bool,
    pub kind: ParamKind,
    pub ty: ParamType,
    pub uri_rule: UriRule,
    /// Wire-format quirk for this param's value.
    pub server_value: ServerValue,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct CommandSpec {
    /// Command path segments, e.g. &["edit", "body"] for `org edit body`.
    pub path: &'static [&'static str],
    /// One-line summary shown in schema output.
    pub summary: &'static str,
    pub kind: TargetKind,
    /// MCP tool name for Tool kind; empty string for Internal.
    pub target: &'static str,
    pub params: &'static [ParamSpec],
    pub output_shape: OutputShape,
    /// (exit_code, meaning) pairs for this command.
    pub exit_codes: &'static [(i32, &'static str)],
}

// ---------------------------------------------------------------------------
// Shared exit-code slices
// ---------------------------------------------------------------------------

const EXIT_STANDARD: &[(i32, &str)] = &[
    (0, "success"),
    (1, "tool error from org-mcp"),
    (2, "usage / argument error"),
    (3, "transport / protocol failure"),
    (4, "server spawn / discovery failure"),
];

const EXIT_INTERNAL: &[(i32, &str)] = &[(0, "success"), (2, "usage / argument error")];

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub const COMMANDS: &[CommandSpec] = &[
    // -----------------------------------------------------------------------
    // read
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["read"],
        summary: "Read an org node and its children as JSON",
        kind: TargetKind::Tool,
        target: "org-read",
        params: &[ParamSpec {
            name: "uri",
            server_name: "uri",
            required: true,
            repeated: false,
            kind: ParamKind::Positional,
            ty: ParamType::Uri,
            uri_rule: UriRule::EitherAccepted,
            server_value: ServerValue::Native,
            description: "Node URI, UUID, file path, or file#headline",
        }],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"title":"...","todo":"...","uri":"org://...","children":[]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // read-headline
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["read-headline"],
        summary: "Read an org node as plain text (wrapped as JSON)",
        kind: TargetKind::Tool,
        target: "org-read-headline",
        params: &[ParamSpec {
            name: "uri",
            server_name: "uri",
            required: true,
            repeated: false,
            kind: ParamKind::Positional,
            ty: ParamType::Uri,
            uri_rule: UriRule::EitherAccepted,
            server_value: ServerValue::Native,
            description: "Node URI or identifier",
        }],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::PlainText,
            cli_data: r#"{"text":"* TODO ...headline and body..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // outline
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["outline"],
        summary: "List the outline of an org file",
        kind: TargetKind::Tool,
        target: "org-outline",
        params: &[ParamSpec {
            name: "file",
            server_name: "file",
            required: true,
            repeated: false,
            kind: ParamKind::Positional,
            ty: ParamType::FilePath,
            uri_rule: UriRule::Na,
            server_value: ServerValue::Native,
            description: "Absolute path to an org file",
        }],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"outline":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // query
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["query"],
        summary: "Query org files with an org-ql expression",
        kind: TargetKind::Tool,
        target: "org-ql-query",
        params: &[
            ParamSpec {
                name: "ql_expr",
                server_name: "query",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "org-ql query expression",
            },
            ParamSpec {
                name: "files",
                server_name: "files",
                required: false,
                repeated: true,
                kind: ParamKind::KeyValue,
                ty: ParamType::FilePath,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Org files to search (repeatable); defaults to agenda files",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"results":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // query inbox  (GTD — optional)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["query", "inbox"],
        summary: "Query GTD inbox (requires GTD configuration in org-mcp)",
        kind: TargetKind::Tool,
        target: "query-inbox",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"results":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // query next  (GTD — optional)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["query", "next"],
        summary: "Query GTD next actions (requires GTD configuration in org-mcp)",
        kind: TargetKind::Tool,
        target: "query-next",
        params: &[ParamSpec {
            name: "tag",
            server_name: "tag",
            required: false,
            repeated: false,
            kind: ParamKind::KeyValue,
            ty: ParamType::String,
            uri_rule: UriRule::Na,
            server_value: ServerValue::Native,
            description: "Filter by tag",
        }],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"results":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // query backlog  (GTD — optional)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["query", "backlog"],
        summary: "Query GTD backlog (requires GTD configuration in org-mcp)",
        kind: TargetKind::Tool,
        target: "query-backlog",
        params: &[ParamSpec {
            name: "tag",
            server_name: "tag",
            required: false,
            repeated: false,
            kind: ParamKind::KeyValue,
            ty: ParamType::String,
            uri_rule: UriRule::Na,
            server_value: ServerValue::Native,
            description: "Filter by tag",
        }],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"results":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // todo state
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["todo", "state"],
        summary: "Update the TODO state of a node",
        kind: TargetKind::Tool,
        target: "org-update-todo-state",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "new-state",
                server_name: "new_state",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::TodoState,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "New TODO state (e.g. TODO, DONE)",
            },
            ParamSpec {
                name: "from",
                server_name: "from",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::TodoState,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Required current state (optimistic concurrency guard)",
            },
            ParamSpec {
                name: "note",
                server_name: "note",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Logbook note to attach with the state change",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // todo add
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["todo", "add"],
        summary: "Add a new TODO node",
        kind: TargetKind::Tool,
        target: "org-add-todo",
        params: &[
            ParamSpec {
                name: "parent",
                server_name: "parent_uri",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Parent node URI",
            },
            ParamSpec {
                name: "title",
                server_name: "title",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Headline title for the new node",
            },
            ParamSpec {
                name: "state",
                server_name: "state",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::TodoState,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Initial TODO state",
            },
            ParamSpec {
                name: "body",
                server_name: "body",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Body text for the new node",
            },
            ParamSpec {
                name: "tag",
                server_name: "tags",
                required: false,
                repeated: true,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Tag to attach (repeatable)",
            },
            ParamSpec {
                name: "after",
                server_name: "after_uri",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::BareUri,
                uri_rule: UriRule::BareOnly,
                server_value: ServerValue::Native,
                description: "Sibling node after which to insert (bare ID only)",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit rename
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "rename"],
        summary: "Rename a node headline",
        kind: TargetKind::Tool,
        target: "org-edit-rename",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "from",
                server_name: "from",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Expected current headline (optimistic concurrency guard)",
            },
            ParamSpec {
                name: "to",
                server_name: "to",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "New headline text",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit body
    // NOTE: server param key is `resource_uri`, not `uri` — PLAN §5.6 §7
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "body"],
        summary: "Edit the body text of a node",
        kind: TargetKind::Tool,
        target: "org-edit-body",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "resource_uri", // <— quirk documented in PLAN §7
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI (sent to server as resource_uri)",
            },
            ParamSpec {
                name: "new",
                server_name: "new_body",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "New body text",
            },
            ParamSpec {
                name: "old",
                server_name: "old_body",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Expected current body (optimistic concurrency guard)",
            },
            ParamSpec {
                name: "append",
                server_name: "append",
                required: false,
                repeated: false,
                kind: ParamKind::Flag,
                ty: ParamType::Bool,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Append to existing body instead of replacing",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit properties
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "properties"],
        summary: "Edit node properties",
        kind: TargetKind::Tool,
        target: "org-edit-properties",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "set",
                server_name: "set",
                required: false,
                repeated: true,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Property to set in k=v form (repeatable)",
            },
            ParamSpec {
                name: "unset",
                server_name: "unset",
                required: false,
                repeated: true,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Property key to remove (repeatable)",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit tags
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "tags"],
        summary: "Set tags on a node (replaces existing tags)",
        kind: TargetKind::Tool,
        target: "org-edit-tags",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "tag",
                server_name: "tags",
                required: false,
                repeated: true,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Tag to set (repeatable); omit to clear all tags",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit priority
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "priority"],
        summary: "Set the priority of a node",
        kind: TargetKind::Tool,
        target: "org-edit-priority",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "priority",
                server_name: "priority",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::Priority,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Priority letter: A, B, or C",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit scheduled
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "scheduled"],
        summary: "Set or clear the SCHEDULED date of a node",
        kind: TargetKind::Tool,
        target: "org-edit-scheduled",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "date",
                server_name: "date",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoDate,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Scheduled date in YYYY-MM-DD or YYYY-MM-DD HH:MM format",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit deadline
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "deadline"],
        summary: "Set or clear the DEADLINE date of a node",
        kind: TargetKind::Tool,
        target: "org-edit-deadline",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "date",
                server_name: "date",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoDate,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Deadline date in YYYY-MM-DD or YYYY-MM-DD HH:MM format",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // edit log-note
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["edit", "log-note"],
        summary: "Add a logbook note to a node",
        kind: TargetKind::Tool,
        target: "org-edit-log-note",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "note",
                server_name: "note",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Note text to add to the logbook",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock status
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "status"],
        summary: "Get the current clock status",
        kind: TargetKind::Tool,
        target: "org-clock-status",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"clocked_in":true,"uri":"org://...","title":"..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock in
    // NOTE: --resolve must be sent as a string, not a JSON bool — PLAN §5.6 §7
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "in"],
        summary: "Clock in to a node",
        kind: TargetKind::Tool,
        target: "org-clock-in",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "at",
                server_name: "at",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoTimestamp,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Clock-in timestamp (ISO 8601); defaults to now",
            },
            ParamSpec {
                name: "resolve",
                server_name: "resolve",
                required: false,
                repeated: false,
                kind: ParamKind::Flag,
                ty: ParamType::Bool,
                uri_rule: UriRule::Na,
                server_value: ServerValue::BoolAsString, // <— PLAN §7 quirk
                description: "Resolve dangling clocks before clocking in (sent as string to server)",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true,"uri":"org://..."}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock out
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "out"],
        summary: "Clock out of the active clock",
        kind: TargetKind::Tool,
        target: "org-clock-out",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: false,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI (optional; defaults to active clock)",
            },
            ParamSpec {
                name: "at",
                server_name: "at",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoTimestamp,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Clock-out timestamp (ISO 8601); defaults to now",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock add
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "add"],
        summary: "Add a clock entry to a node",
        kind: TargetKind::Tool,
        target: "org-clock-add",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "start",
                server_name: "start",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoTimestamp,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Clock entry start timestamp (ISO 8601)",
            },
            ParamSpec {
                name: "end",
                server_name: "end",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoTimestamp,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Clock entry end timestamp (ISO 8601)",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock delete
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "delete"],
        summary: "Delete a clock entry from a node",
        kind: TargetKind::Tool,
        target: "org-clock-delete",
        params: &[
            ParamSpec {
                name: "uri",
                server_name: "uri",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::Uri,
                uri_rule: UriRule::EitherAccepted,
                server_value: ServerValue::Native,
                description: "Node URI or identifier",
            },
            ParamSpec {
                name: "at",
                server_name: "at",
                required: true,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::IsoTimestamp,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Timestamp identifying the clock entry to delete",
            },
        ],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"success":true}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // clock dangling
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["clock", "dangling"],
        summary: "List dangling (unclosed) clock entries",
        kind: TargetKind::Tool,
        target: "org-clock-dangling",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"entries":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config todo
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "todo"],
        summary: "Get configured TODO states",
        kind: TargetKind::Tool,
        target: "org-config-todo",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"states":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config tags
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "tags"],
        summary: "Get configured tags",
        kind: TargetKind::Tool,
        target: "org-config-tags",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"tags":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config tag-candidates
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "tag-candidates"],
        summary: "Get tag candidates",
        kind: TargetKind::Tool,
        target: "org-config-tag-candidates",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"candidates":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config priority
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "priority"],
        summary: "Get priority configuration",
        kind: TargetKind::Tool,
        target: "org-config-priority",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"priorities":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config files
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "files"],
        summary: "Get configured org files",
        kind: TargetKind::Tool,
        target: "org-config-files",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"files":[...]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // config clock
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["config", "clock"],
        summary: "Get clock configuration",
        kind: TargetKind::Tool,
        target: "org-config-clock",
        params: &[],
        output_shape: OutputShape::Tool {
            server_returns: ServerReturns::JsonObject,
            cli_data: r#"{"config":{...}}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // tools list  (Internal — passes through to MCP)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["tools", "list"],
        summary: "List all tools exposed by the MCP server",
        kind: TargetKind::Internal,
        target: "",
        params: &[],
        output_shape: OutputShape::Internal {
            cli_data: r#"{"tools":[{"name":"...","description":"...","inputSchema":{}}]}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // tools call  (Internal — raw escape hatch)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["tools", "call"],
        summary: "Call a named MCP tool with optional JSON arguments",
        kind: TargetKind::Internal,
        target: "",
        params: &[
            ParamSpec {
                name: "name",
                server_name: "name",
                required: true,
                repeated: false,
                kind: ParamKind::Positional,
                ty: ParamType::String,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "Name of the MCP tool to call",
            },
            ParamSpec {
                name: "args",
                server_name: "arguments",
                required: false,
                repeated: false,
                kind: ParamKind::KeyValue,
                ty: ParamType::Json,
                uri_rule: UriRule::Na,
                server_value: ServerValue::Native,
                description: "JSON object of arguments to pass to the tool",
            },
        ],
        output_shape: OutputShape::Internal {
            cli_data: r#"{"tool":"...","result":{...}}"#,
        },
        exit_codes: EXIT_STANDARD,
    },
    // -----------------------------------------------------------------------
    // schema  (Internal)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["schema"],
        summary: "Emit machine-readable metadata for all CLI commands",
        kind: TargetKind::Internal,
        target: "",
        params: &[],
        output_shape: OutputShape::Internal {
            cli_data: r#"{"version":1,"commands":[...]}"#,
        },
        exit_codes: EXIT_INTERNAL,
    },
    // -----------------------------------------------------------------------
    // schema <path>  (Internal)
    // -----------------------------------------------------------------------
    CommandSpec {
        path: &["schema", "<path>"],
        summary: "Emit machine-readable metadata for a single CLI command",
        kind: TargetKind::Internal,
        target: "",
        params: &[ParamSpec {
            name: "path",
            server_name: "",
            required: true,
            repeated: false,
            kind: ParamKind::Positional,
            ty: ParamType::String,
            uri_rule: UriRule::Na,
            server_value: ServerValue::Native,
            description: "Command path segments (e.g. 'edit body')",
        }],
        output_shape: OutputShape::Internal {
            cli_data: r#"{"command":{...}}"#,
        },
        exit_codes: EXIT_INTERNAL,
    },
];

// ---------------------------------------------------------------------------
// Serializer: CommandSpec → serde_json::Value
// ---------------------------------------------------------------------------

use serde_json::{Value, json};

fn serialize_target_kind(k: &TargetKind) -> &'static str {
    match k {
        TargetKind::Tool => "tool",
        TargetKind::Resource => "resource",
        TargetKind::Internal => "internal",
    }
}

fn serialize_param_kind(k: &ParamKind) -> &'static str {
    match k {
        ParamKind::Positional => "positional",
        ParamKind::Flag => "flag",
        ParamKind::KeyValue => "key_value",
    }
}

fn serialize_param_type(t: &ParamType) -> &'static str {
    match t {
        ParamType::Uri => "uri",
        ParamType::BareUri => "bare_uri",
        ParamType::FilePath => "file_path",
        ParamType::String => "string",
        ParamType::IsoDate => "iso_date",
        ParamType::IsoTimestamp => "iso_timestamp",
        ParamType::TodoState => "todo_state",
        ParamType::Priority => "priority",
        ParamType::Bool => "bool",
        ParamType::Json => "json",
    }
}

fn serialize_uri_rule(r: &UriRule) -> &'static str {
    match r {
        UriRule::BareOnly => "bare_only",
        UriRule::OrgOnly => "org_only",
        UriRule::EitherAccepted => "either",
        UriRule::Na => "n/a",
    }
}

fn serialize_server_value(v: &ServerValue) -> Option<&'static str> {
    match v {
        ServerValue::Native => None, // omit from JSON
        ServerValue::BoolAsString => Some("bool_as_string"),
    }
}

pub fn serialize_param(p: &ParamSpec) -> Value {
    let mut obj = json!({
        "name": p.name,
        "server_name": p.server_name,
        "required": p.required,
        "repeated": p.repeated,
        "kind": serialize_param_kind(&p.kind),
        "type": serialize_param_type(&p.ty),
        "uri_rule": serialize_uri_rule(&p.uri_rule),
        "description": p.description,
    });
    if let Some(sv) = serialize_server_value(&p.server_value) {
        obj.as_object_mut()
            .unwrap()
            .insert("server_value".to_string(), json!(sv));
    }
    obj
}

pub fn serialize_output_shape(s: &OutputShape) -> Value {
    match s {
        OutputShape::Tool {
            server_returns,
            cli_data,
        } => {
            let sr = match server_returns {
                ServerReturns::JsonObject => "json_object",
                ServerReturns::PlainText => "plain_text",
            };
            json!({
                "kind": "tool",
                "server_returns": sr,
                "cli_data": cli_data,
            })
        }
        OutputShape::Internal { cli_data } => {
            json!({
                "kind": "internal",
                "cli_data": cli_data,
            })
        }
    }
}

pub fn serialize_command(cmd: &CommandSpec) -> Value {
    let path: Vec<Value> = cmd.path.iter().map(|s| json!(s)).collect();
    let params: Vec<Value> = cmd.params.iter().map(serialize_param).collect();
    let exit_codes: Vec<Value> = cmd
        .exit_codes
        .iter()
        .map(|(code, meaning)| json!({"code": code, "meaning": meaning}))
        .collect();

    let target = if matches!(cmd.kind, TargetKind::Internal) {
        json!({ "kind": "internal" })
    } else {
        json!({ "kind": serialize_target_kind(&cmd.kind), "name": cmd.target })
    };

    json!({
        "path": path,
        "summary": cmd.summary,
        "target": target,
        "params": params,
        "output": serialize_output_shape(&cmd.output_shape),
        "exit_codes": exit_codes,
    })
}

/// Serialize the full registry as the `org schema` data payload.
pub fn serialize_all() -> Value {
    let commands: Vec<Value> = COMMANDS.iter().map(serialize_command).collect();
    json!({
        "version": 1,
        "commands": commands,
    })
}

/// Look up a command by path segments and serialize it, or return None.
pub fn serialize_one(path: &[&str]) -> Option<Value> {
    COMMANDS
        .iter()
        .find(|c| c.path == path)
        .map(|cmd| json!({ "command": serialize_command(cmd) }))
}
