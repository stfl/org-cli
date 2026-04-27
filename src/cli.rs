/// CLI argument definitions using clap derive.
///
/// # Server flag
///
/// `--server <cmd>` takes the executable path. Additional arguments to the
/// server can be passed via `--server-arg <arg>` (repeatable). This is cleaner
/// than a single string that would require shell-splitting, avoids quoting
/// ambiguities, and is explicit about what is the binary vs. what is an arg.
///
/// Example:
///   org --server emacs-mcp-stdio.sh --server-arg --socket --server-arg /tmp/mcp.sock tools list
///
/// # Discovery
///
/// When `--server` is not provided, the CLI searches `$PATH` for
/// `emacs-mcp-stdio.sh` (see `src/discovery.rs`). If not found, a usage
/// error (exit 4) is returned.
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "org", about = "Agent-first CLI for org-mcp")]
pub struct Cli {
    /// Path to the MCP server executable (e.g. emacs-mcp-stdio.sh).
    /// If omitted, PATH is searched for emacs-mcp-stdio.sh (not yet implemented).
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Additional arguments to pass to the server executable (repeatable).
    /// `allow_hyphen_values` lets values like `--socket` be passed without an
    /// equals sign — `--server-arg --socket` works, matching the doc example.
    #[arg(long = "server-arg", global = true, value_name = "ARG", allow_hyphen_values = true)]
    pub server_args: Vec<String>,

    /// Emit compact single-line JSON instead of pretty-printed output.
    #[arg(long, global = true)]
    pub compact: bool,

    /// Per-recv() timeout in seconds for the stdio transport.
    /// 0 disables the timeout. Default 30. Honors `ORG_TIMEOUT`.
    #[arg(long, global = true, env = "ORG_TIMEOUT", default_value_t = 30)]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Read an org node and its children as JSON.
    Read {
        /// Node URI, UUID, file path, or file#headline. Accepts both bare and org:// form.
        uri: String,
    },

    /// Read an org node as plain text (wrapped as JSON).
    #[command(name = "read-headline")]
    ReadHeadline {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
    },

    /// List the outline of an org file.
    Outline {
        /// Absolute path to an org file. Must NOT be an org:// URI.
        file: String,
    },

    /// Query org files with org-ql or GTD tools.
    Query(QueryArgs),

    /// Manage TODO states and create new TODO nodes.
    Todo(TodoArgs),

    /// Low-level MCP tool access.
    Tools {
        #[command(subcommand)]
        cmd: ToolsCmd,
    },

    /// Edit org node attributes (headline, body, properties, tags, priority, dates, log).
    Edit(EditArgs),

    /// Clock operations (status, in, out, add, delete, dangling).
    Clock(ClockArgs),

    /// Introspect org-mcp configuration (TODO states, tags, priority, files, clock).
    Config(ConfigArgs),

    /// Emit machine-readable schema metadata for CLI commands.
    ///
    /// With no arguments, returns metadata for ALL commands.
    /// With a command path (e.g. `org schema edit body`), returns metadata
    /// for that single command.
    ///
    /// This is a local introspection command — no --server needed.
    Schema(SchemaArgs),
}

/// Arguments for the `config` subcommand.
#[derive(Debug, clap::Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub kind: ConfigKind,
}

/// The specific config variant.
#[derive(Debug, clap::Subcommand)]
pub enum ConfigKind {
    /// Get configured TODO keyword sequences.
    Todo,
    /// Get configured tags.
    Tags,
    /// Get tag candidates (tags with usage counts).
    #[command(name = "tag-candidates")]
    TagCandidates,
    /// Get priority configuration.
    Priority,
    /// Get configured org agenda files.
    Files,
    /// Get clock configuration.
    Clock,
}

/// Arguments for the `query` subcommand.
///
/// Supports two forms:
///   - `org query run "<expr>"` — explicit subcommand form (always works)
///   - `org query "<expr>"` — bare positional form (dispatched as `QueryKind::Run`)
///
/// When both `ql_expr` (positional) and no subcommand are present, the bare
/// expr is dispatched as `QueryKind::Run`. If a known subcommand (`run`,
/// `inbox`, `next`, `backlog`) is given, it takes priority and `ql_expr`
/// must be absent (`args_conflicts_with_subcommands = true`).
#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_required = false)]
pub struct QueryArgs {
    /// org-ql query expression (bare form: `org query "<expr>"`).
    pub ql_expr: Option<String>,

    /// Org files to search (repeatable); used with the bare expression form.
    #[arg(long = "files", value_name = "FILE")]
    pub files: Vec<String>,

    #[command(subcommand)]
    pub kind: Option<QueryKind>,
}

/// The specific query variant.
#[derive(Debug, clap::Subcommand)]
pub enum QueryKind {
    /// Run an arbitrary org-ql expression.
    Run {
        /// org-ql query expression (e.g. `(todo "TODO")`).
        ql_expr: String,
        /// Org files to search (repeatable); defaults to agenda files.
        #[arg(long = "files", value_name = "FILE")]
        files: Vec<String>,
    },

    /// Query the GTD inbox (requires GTD configuration in org-mcp).
    Inbox,

    /// Query GTD next actions (requires GTD configuration in org-mcp).
    Next {
        /// Filter results by tag.
        #[arg(long)]
        tag: Option<String>,
    },

    /// Query the GTD backlog (requires GTD configuration in org-mcp).
    Backlog {
        /// Filter results by tag.
        #[arg(long)]
        tag: Option<String>,
    },
}

/// Arguments for the `todo` subcommand.
#[derive(Debug, clap::Args)]
pub struct TodoArgs {
    #[command(subcommand)]
    pub kind: TodoKind,
}

/// The specific todo variant.
#[derive(Debug, clap::Subcommand)]
pub enum TodoKind {
    /// Update the TODO state of a node.
    State {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// New TODO state keyword (e.g. TODO, DONE, NEXT).
        new_state: String,
        /// Required current state (optimistic concurrency guard).
        #[arg(long = "from")]
        from: Option<String>,
        /// Logbook note to attach with the state change.
        #[arg(long)]
        note: Option<String>,
    },
    /// Add a new TODO node.
    Add {
        /// Parent node URI. Accepts both bare and org:// form.
        #[arg(long)]
        parent: String,
        /// Headline title for the new node.
        #[arg(long)]
        title: String,
        /// Initial TODO state keyword (e.g. TODO, NEXT).
        #[arg(long)]
        state: String,
        /// Body text for the new node.
        #[arg(long)]
        body: Option<String>,
        /// Tag to attach (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Sibling node after which to insert (bare ID or org:// URI).
        #[arg(long = "after")]
        after: Option<String>,
    },
}

/// Arguments for the `edit` subcommand.
#[derive(Debug, clap::Args)]
pub struct EditArgs {
    #[command(subcommand)]
    pub kind: EditKind,
}

/// The specific edit variant.
#[derive(Debug, clap::Subcommand)]
pub enum EditKind {
    /// Rename a node headline.
    Rename {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Expected current headline (optimistic concurrency guard).
        #[arg(long)]
        from: String,
        /// New headline text.
        #[arg(long)]
        to: String,
    },

    /// Edit the body text of a node.
    Body {
        /// Node URI (sent to server as `resource_uri`; see `org-mcp--tool-edit-body` in ../org-mcp/org-mcp.el).
        uri: String,
        /// New body text.
        #[arg(long = "new")]
        new: String,
        /// Expected current body (optimistic concurrency guard).
        #[arg(long = "old")]
        old: Option<String>,
        /// Append to existing body instead of replacing.
        #[arg(long)]
        append: bool,
    },

    /// Edit node properties (set and/or unset key-value pairs).
    Properties {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Property to set in k=v form (repeatable).
        #[arg(long = "set")]
        sets: Vec<String>,
        /// Property key to remove (repeatable).
        #[arg(long = "unset")]
        unsets: Vec<String>,
    },

    /// Set tags on a node (replaces existing tags; omit --tag to clear all).
    Tags {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Tag to set (repeatable); omit to clear all tags.
        #[arg(long = "tag")]
        tags: Vec<String>,
    },

    /// Set the priority of a node (omit --priority to clear).
    Priority {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Priority letter: A, B, or C. Omit to clear priority.
        #[arg(long)]
        priority: Option<String>,
    },

    /// Set or clear the SCHEDULED date of a node.
    Scheduled {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Scheduled date in YYYY-MM-DD or YYYY-MM-DD HH:MM format. Omit to clear.
        #[arg(long)]
        date: Option<String>,
    },

    /// Set or clear the DEADLINE date of a node.
    Deadline {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Deadline date in YYYY-MM-DD or YYYY-MM-DD HH:MM format. Omit to clear.
        #[arg(long)]
        date: Option<String>,
    },

    /// Add a logbook note to a node.
    #[command(name = "log-note")]
    LogNote {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Note text to add to the logbook.
        #[arg(long)]
        note: String,
    },
}

/// Arguments for the `clock` subcommand.
#[derive(Debug, clap::Args)]
pub struct ClockArgs {
    #[command(subcommand)]
    pub kind: ClockKind,
}

/// The specific clock variant.
#[derive(Debug, clap::Subcommand)]
pub enum ClockKind {
    /// Get the current clock status.
    Status,

    /// Clock in to a node.
    In {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Clock-in timestamp (ISO 8601); defaults to now.
        #[arg(long)]
        at: Option<String>,
        /// Resolve dangling clocks before clocking in.
        /// NOTE: sent to server as string "true"/"false", not JSON bool — see `ServerValue::BoolAsString` in contract.rs.
        #[arg(long)]
        resolve: bool,
    },

    /// Clock out of the active clock.
    Out {
        /// Node URI (optional; omit to clock out current active clock).
        uri: Option<String>,
        /// Clock-out timestamp (ISO 8601); defaults to now.
        #[arg(long)]
        at: Option<String>,
    },

    /// Add a clock entry to a node.
    Add {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Clock entry start timestamp (ISO 8601).
        #[arg(long)]
        start: String,
        /// Clock entry end timestamp (ISO 8601).
        #[arg(long)]
        end: String,
    },

    /// Delete a clock entry from a node.
    Delete {
        /// Node URI or identifier. Accepts both bare and org:// form.
        uri: String,
        /// Timestamp identifying the clock entry to delete (ISO 8601).
        #[arg(long)]
        at: String,
    },

    /// List dangling (unclosed) clock entries.
    Dangling,
}

/// Arguments for the `schema` subcommand.
#[derive(Debug, clap::Args)]
pub struct SchemaArgs {
    /// Command path segments to look up (e.g. `edit body`).
    /// Omit to return all commands.
    #[arg(value_name = "PATH")]
    pub path: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ToolsCmd {
    /// List all tools exposed by the MCP server.
    List,

    /// Call a named MCP tool with optional JSON arguments.
    Call {
        /// Name of the tool to call.
        name: String,

        /// JSON object of arguments to pass to the tool.
        /// Defaults to `{}` if omitted.
        #[arg(long, value_name = "JSON")]
        args: Option<String>,
    },
}

impl Cli {
    /// Build the argv list for spawning the server process.
    /// Returns an error string if no server is configured.
    /// Discovery (PATH search) is handled in `main.rs::run()` before this is called.
    pub fn server_argv(&self) -> Result<Vec<String>, String> {
        match &self.server {
            Some(cmd) => {
                let mut argv = vec![cmd.clone()];
                argv.extend(self.server_args.clone());
                Ok(argv)
            }
            None => Err("no --server specified".to_string()),
        }
    }
}
