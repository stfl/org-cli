# org

Synchronous Rust CLI for the `org-mcp` Emacs MCP server. Designed for LLM agents and shell pipelines: deterministic JSON-first stdout, structured stderr, meaningful exit codes.

> v1 status: machine interface complete. No human-pretty rendering. The authoritative contract lives in `src/contract.rs` (CLI side) and `../org-mcp/org-mcp.el` (server side).

## Install

Cargo:

```
cargo build --release
cp target/release/org ~/bin/
```

Nix flake:

```
nix build .#default        # builds and tests; binary at ./result/bin/org
nix run .#default -- --help
```

A devShell with cargo/rustc/rustfmt/clippy/rust-analyzer is available via `nix develop`.

## Usage

`--server` defaults to `~/.config/emacs/org-mcp-stdio.sh`. Pass `--server <path>`
to override.

`org-mcp-stdio.sh` is a thin wrapper around `emacs-mcp-stdio.sh` (shipped by
`mcp-server-lib`) that bakes in the org-mcp `--server-id` / `--init-function`
/ `--stop-function`, so callers don't have to. It resolves the underlying
`emacs-mcp-stdio.sh` first via co-location (same directory as the wrapper)
then via `$PATH`.

```
org tools list                                       # default launcher
org --server /path/to/org-mcp-stdio.sh tools list    # explicit override
org read org://<uuid>
org schema                                           # local; no server needed
```

For multi-arg launchers, the recommended form is `--` followed by the trailing
launcher args, then the subcommand:

```
org -- --socket /tmp/mcp.sock tools list
```

The legacy repeatable `--server-arg` form is still accepted:

```
org --server-arg --socket=/tmp/mcp.sock tools list
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

See `src/contract.rs` for the codified command-to-tool mapping and `../org-mcp/org-mcp.el` (`mcp-server-lib-register-tool` blocks) for the authoritative server-side contract.

## Discoverability

```
org schema                   # all commands as JSON
org schema edit body         # one command's contract
```

## Examples (verified by tests/)

`--server` is omitted below — auto-discovery picks `~/.config/emacs/org-mcp-stdio.sh`
(or the PATH fallbacks). Pass `--server <path>` explicitly to override.

```sh
# Read a headline (org:// prefix accepted)
org read org://abc

# Update TODO state
org todo state org://abc DONE --from TODO --note "shipped"

# Edit body
org edit body org://abc --new "new body"

# Clock in with conflict resolve
org clock in org://abc --resolve

# List all tools
org tools list

# Query org-ql (bare form or explicit `run`)
org query '(todo "TODO")'
org query run '(todo "TODO")'
```

## Live integration test

The default `cargo test` run uses an in-process mock org-mcp. To verify against a real Emacs MCP server:

```sh
ORG_LIVE_TEST=1 cargo test --test live_org_mcp -- --test-threads=1
```

Optional env vars:
- `ORG_LIVE_SERVER=<path>`      — override discovery; use this exact launcher
- `ORG_LIVE_FILES=<file.org>`   — enables outline/read tests against a real file

### Self-contained Nix env for live tests

For CI and any host without a configured Emacs daemon, the flake exposes a
repo-local environment that bundles a pinned Emacs + `org-mcp` + `org-ql`.
It does **not** read user dotfiles, system Emacs, or a pre-running daemon.
Tests that need extra Elisp (e.g. `agile-gtd` for the GTD query bindings)
load an overlay file via `emacs -l <overlay.el>`.

```sh
nix build .#live-test-env
ls result/bin                                       # emacs, emacsclient
cat result/share/org-cli-live/paths.env             # ORG_MCP_STDIO=..., EMACS_MCP_STDIO_DIR=...
```

Stable paths inside the output:
- `bin/emacs`, `bin/emacsclient`        — wrapped Emacs with all packages
- `share/org-cli-live/init.el`          — minimal init driving org-mcp
- `share/org-cli-live/paths.env`        — Nix-baked `ORG_MCP_STDIO` (absolute
                                          path to the org-mcp wrapper inside
                                          the org-mcp store path) and
                                          `EMACS_MCP_STDIO_DIR` (directory
                                          containing the patched generic shim;
                                          add to `$PATH` so the wrapper's
                                          PATH-fallback resolves)

The init.el reads `ORG_LIVE_DIR` and `ORG_LIVE_FILES` from the environment,
so the daemon launcher / rstest fixture can configure org files without
touching user state.

Daemon + launcher recipe (consumed by the rstest fixture):

```sh
# 1. Spawn an isolated daemon. -Q skips the user's ~/.config/emacs.
HOME=$TMPDIR ORG_LIVE_DIR=$TMPDIR/org ORG_LIVE_FILES=$TMPDIR/org/test.org \
  result/bin/emacs -Q --fg-daemon=NAME -l result/share/org-cli-live/init.el &

# 2. Talk to it. The wrapper bakes in --server-id / --init-function /
#    --stop-function; only the daemon socket is dynamic.
source result/share/org-cli-live/paths.env
PATH="$EMACS_MCP_STDIO_DIR:$PATH" HOME=$TMPDIR \
  "$ORG_MCP_STDIO" --socket=NAME
```

Refresh the pinned `org-mcp` / `agile-gtd` revisions with `nix/update-pins.sh`.

#### One-shot runner: `just live-env-test`

`scripts/run-live-tests.sh` is the canonical entrypoint that wires the read-only
live suite to `.#live-test-env`. It builds the env on demand, copies
`tests/live-fixtures/sample.org` into a fresh tmpdir, spawns an isolated daemon
under `HOME=$TMPDIR`, runs the live suite against it, and tears the daemon and
tmpdir down on exit (also on `INT` / `TERM`).

```sh
just live-env-test                  # default: read-only suite
just live-env-test live_handshake   # forward extra args to cargo test
```

Re-runnable from a clean checkout: no leftover daemon, no leftover tmpdir, no
shared org file.

#### Mutating live tests against the disposable fixture

The `#[ignore]`-marked mutating tests (`edit-*`, `todo state`/`todo add`,
`clock-*`, `log-note`) are safe to run end-to-end against the same launcher
because every invocation starts from a fresh tmpdir copy of the checked-in
fixture and tears the workspace down on exit:

```sh
just live-env-test-mutating
```

Residue that the in-test revert cannot undo (e.g. `org-add-todo` entries —
`org-mcp` has no delete-headline tool — and appended log notes) is contained
inside the disposable workspace and cleaned up automatically. The default
`cargo test` path is unaffected: mutating tests stay `#[ignore]`d and only
run via this dedicated entrypoint.

## Development

A `Justfile` provides curated shortcuts — run `just --list` to see all targets.

```
just check          # fmt + clippy + cargo test + nix flake check (mirrors CI)
just update         # cargo update + nix flake update
just live           # live integration test (requires running Emacs MCP server)
```

Raw cargo commands still work directly:

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

## CI

GitHub Actions workflow at `.github/workflows/ci.yml` runs `nix flake check` and `nix build .#default` on every push/PR. The workflow uses `https://cache.garnix.io` as a substituter, so it picks up artifacts already built by [Garnix](https://garnix.io) on the same flake. To enable Garnix builds (free for OSS), install the Garnix GitHub App on the repo — `garnix.yaml` already declares the build matrix.

## See also

- `src/contract.rs` — codified CLI command → tool/param mapping
- `../org-mcp/org-mcp.el` — authoritative server-side tool registrations
- `bd ready` / `bd list` — open work for this CLI
