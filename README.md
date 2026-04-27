# org

Synchronous Rust CLI for the `org-mcp` Emacs MCP server. Designed for LLM agents and shell pipelines: deterministic JSON-first stdout, structured stderr, meaningful exit codes.

> v1 status: machine interface complete. No human-pretty rendering — see PLAN §2.

## Install

```
cargo build --release
cp target/release/org ~/bin/
```

## Usage

Always supply a launcher with `--server <cmd>` (auto-discovery is a follow-up — see PLAN §5.2).

```
org --server emacs-mcp-stdio.sh tools list
org --server emacs-mcp-stdio.sh read org://<uuid>
org --server emacs-mcp-stdio.sh schema
```

## JSON envelope

Success:
```json
{"ok": true, "data": {...}}
```

Error:
```json
{"ok": false, "error": {"kind": "tool|transport|usage", "code": -32000, "message": "...", "data": null}, "exit_code": 1}
```

`--compact` emits single-line JSON. stdout is the envelope only; stderr is reserved for diagnostics.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | success |
| 1 | tool error returned by org-mcp |
| 2 | usage / argument error |
| 3 | transport / protocol failure |
| 4 | server spawn / discovery failure |

## Command surface

| Command | Description |
|---------|-------------|
| `read <uri>` | Read an org node and its children as JSON |
| `read-headline <uri>` | Read an org node as plain text, wrapped as JSON |
| `outline <file>` | List the outline of an org file (absolute path required) |
| `query run <ql-expr>` | Run an arbitrary org-ql expression |
| `query inbox` | Query the GTD inbox (requires GTD config in org-mcp) |
| `query next` | Query GTD next actions |
| `query backlog` | Query GTD backlog |
| `todo state <uri> <state>` | Update a TODO state |
| `todo add --parent <uri> --title <t> --state <s>` | Create a new TODO node |
| `edit rename <uri> --from <t> --to <t>` | Rename a headline |
| `edit body <uri> --new <text>` | Replace or append to a node body |
| `edit properties <uri> --set k=v` | Set/unset node properties |
| `edit tags <uri> --tag <t>` | Set node tags |
| `edit priority <uri> --priority <A\|B\|C>` | Set node priority |
| `edit scheduled <uri> --date <date>` | Set scheduled date |
| `edit deadline <uri> --date <date>` | Set deadline date |
| `edit log-note <uri> --note <text>` | Add a log note |
| `clock status` | Get current clock status |
| `clock in <uri>` | Clock in to a node |
| `clock out [<uri>]` | Clock out |
| `clock add <uri> --start <ts> --end <ts>` | Add a clock entry |
| `clock delete <uri> --at <ts>` | Delete a clock entry |
| `clock dangling` | List dangling clock entries |
| `config todo` | Get configured TODO states |
| `config tags` | Get configured tags |
| `config tag-candidates` | Get tag candidates |
| `config priority` | Get priority configuration |
| `config files` | Get configured org files |
| `config clock` | Get clock configuration |
| `schema` | All commands as JSON |
| `schema <command-path>` | One command's contract |
| `tools list` | Raw tools/list from the server |
| `tools call <name> --args <json>` | Raw tools/call escape hatch |

See PLAN.md §6 for the canonical command list and §7 for the parameter contract table.

## Discoverability

```
org schema                   # all commands as JSON
org schema edit body         # one command's contract
```

## Examples (verified by tests/)

```sh
# Read a headline (org:// prefix accepted)
org --server emacs-mcp-stdio.sh read org://abc

# Update TODO state
org --server emacs-mcp-stdio.sh todo state org://abc DONE --from TODO --note "shipped"

# Edit body
org --server emacs-mcp-stdio.sh edit body org://abc --new "new body"

# Clock in with conflict resolve
org --server emacs-mcp-stdio.sh clock in org://abc --resolve

# List all tools
org --server emacs-mcp-stdio.sh tools list

# Query org-ql
org --server emacs-mcp-stdio.sh query run '(todo "TODO")'
```

## Live integration test

The default `cargo test` run uses an in-process mock org-mcp. To verify against a real Emacs MCP server:

```sh
ORG_LIVE_TEST=1 cargo test --test live_org_mcp -- --test-threads=1
```

Optional env vars:
- `ORG_LIVE_SERVER=<path>`      — override discovery; use this exact launcher
- `ORG_LIVE_FILES=<file.org>`   — enables outline/read tests against a real file

## Development

```
cargo test          # full suite (mock-only; live tests are gated on ORG_LIVE_TEST=1)
cargo clippy -- -D warnings
cargo fmt
```

The mock org-mcp server lives at `tests/fixtures/mock_org_mcp/main.rs` and is spawned via `env!("CARGO_BIN_EXE_mock_org_mcp")`. It supports knobs:

- `MOCK_TOOL_ERROR=<tool>` — returns JSON-RPC error for that tool
- `MOCK_TOOL_ERROR_DATA=<json>` — attaches server data to the error (used with `MOCK_TOOL_ERROR`)
- `MOCK_NO_GTD=1` — omit GTD trio from tools/list
- `MOCK_RECORD_REQUESTS=1` + `MOCK_REQUEST_LOG=<path>` — log incoming requests for assertions
- `MOCK_DIE_AFTER_HANDSHAKE=1` — exit mid-protocol to test transport failure (exit code 3)

## See also

PLAN.md — the v1 contract (read this before modifying anything).
