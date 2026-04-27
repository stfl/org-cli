# org — Agent-First CLI Plan

A synchronous Rust CLI for the local `org-mcp` Emacs MCP server.

The binary is called **`org`**. Its primary users are **LLM agents and shell pipelines**, not humans at a terminal. The CLI is therefore designed as a **deterministic machine interface** over `org-mcp`, with human-friendly rendering explicitly deferred.

> **Status:** Draft for review. No code written.

---

## 1. Product identity

`org` is not a general-purpose Org editor and not a full MCP host.

It is a small, synchronous CLI that:

- speaks MCP directly to `org-mcp`
- exposes `org-mcp` tools as stable subcommands
- emits machine-readable JSON by default
- preserves `org-mcp` as the single source of truth for Org parsing and mutation

The CLI is an **API surface** for agents.

---

## 2. Goals and non-goals

### Goals

- One binary, **`org`**, with subcommands that cover the current `org-mcp` tool surface.
- **JSON-first output.** Every v1 command emits valid JSON on stdout by default.
- **Synchronous only.** No tokio, no async runtime, no concurrency model.
- Stable shell/agent contract:
  - deterministic stdout
  - structured stderr for diagnostics only
  - meaningful exit codes
- Raw escape hatch:
  - `org tools list`
  - `org tools call <name> --args '<json>'`
- Thin compatibility layer over `org-mcp`'s current quirks where useful, without reimplementing Org semantics locally.

### Non-goals

- **Not** a full MCP host. No `roots`, `sampling`, `elicitation`, or prompt support.
- **Not** a local Org parser or editor. No `.org` parsing outside Emacs.
- **Not** a daemon. One invocation = one short-lived client session.
- **Not** a TUI or interactive prompt flow.
- **Not** a human-first pretty CLI in v1. No tables, no colors, no TTY auto-detection logic.

---

## 3. Ground truth from the current org-mcp

This plan is constrained by the local `../org-mcp` repository, not by idealized MCP assumptions.

### 3.1 Current server model

- `org-mcp` is an Emacs Lisp MCP server built on `mcp-server-lib`.
- The local repo exposes:
  - a core set of always-registered tools
  - three optional GTD tools: `query-inbox`, `query-next`, `query-backlog`
  - two resource templates: `org://{uri}` and `org-outline://{filename}`
- The documented launcher in `README.org` is an **external** `~/.emacs.d/emacs-mcp-stdio.sh` wrapper plus a pre-running Emacs MCP server.
- That wrapper is **not** in this repo, so transport details beyond documented usage must be treated as runtime-verified assumptions, not facts.

### 3.2 URI reality

`org-mcp` currently has a hard split:

- **tool inputs** use bare identifiers only
  - bare UUID
  - absolute file path
  - `file#headline/path`
- **resource URIs** use `org://...`

This is enforced in `org-mcp.el`, and tests explicitly verify rejection of `org://`-prefixed tool inputs for commands like `org-read`, `org-read-headline`, and `org-update-todo-state`.

### 3.3 Tool/result reality

- `org-read-headline` returns plain text server-side, not structured JSON.
- Most write tools return JSON via helper code that injects `success` and `uri`.
- Result shapes are useful but not fully normalized across all commands.
- JSON-RPC errors are available, but machine-readable server-side error taxonomy is thin; message text still matters.

### 3.4 Emacs session dependence

The live Emacs environment matters materially:

- `org-mcp-allowed-files` can fall back to `org-agenda-files`
- GTD tool registration is conditional on runtime configuration
- buffer/hook behavior affects correctness
- clock semantics depend on Emacs state and Org config

This means a batch-mode direct stdio launcher is a potential future optimization, not the baseline assumption for correctness.

---

## 4. Core design principles

1. **Machine-first by default**
   - stdout is a stable JSON contract
   - human formatting is optional future work

2. **Explicit beats inferred**
   - no TTY-based output switching
   - no hidden mode changes

3. **Preserve org-mcp semantics**
   - the CLI normalizes command UX where valuable
   - it does not invent alternate Org behavior

4. **Small surface area**
   - hand-rolled sync MCP client
   - minimal dependencies

5. **Per-command contracts beat generic abstractions**
   - URI validation and argument rules are command-specific where necessary
   - one generic “everything is a URI” abstraction is not sufficient

---

## 5. Architecture decisions

### 5.1 Sync client, not async

Keep the client synchronous.

Why:

- `org-mcp` itself is effectively single-threaded from the CLI's perspective
- the CLI is fire-and-forget
- async crates add significant weight without clear v1 benefit

Dependencies stay small:

```toml
[dependencies]
clap       = { version = "4", features = ["derive", "env"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
anyhow     = "1"
```

### 5.2 Transport strategy for v1

The canonical v1 assumption is the currently documented `org-mcp` flow:

1. user has a working `org-mcp` setup already
2. user points `org` at a launcher command, or
3. `org` discovers a documented wrapper in a simple, explicit way

Supported modes:

1. `--server <cmd> [args...]`
   - explicit command
   - primary power-user and test path
2. default discovery
   - first look in `$PATH` for `emacs-mcp-stdio.sh`
   - otherwise fail with setup guidance
3. `--socket <path>` passthrough
   - forwarded only if the chosen launcher supports it

### 5.3 MCP framing/handshake

The client will implement the minimal MCP client flow needed for tools:

- `initialize`
- wait for initialize response
- `notifications/initialized`
- `tools/list`, `tools/call`, and resource reads as needed

The exact transport framing must be treated as **runtime-tested** against the real launcher, not just inferred from ecosystem docs.

### 5.4 Output contract

V1 defaults to JSON on stdout for **every command**.

The CLI does **not** promise “raw server result on stdout” for all commands, because that would be inconsistent across commands such as `read-headline`.

Instead, the CLI owns a small, stable envelope:

#### Success

```json
{
  "ok": true,
  "data": { }
}
```

#### Error

```json
{
  "ok": false,
  "error": {
    "kind": "tool",
    "code": -32000,
    "message": "Invalid TODO state",
    "data": null
  },
  "exit_code": 1
}
```

Rules:

- stdout contains the JSON envelope only
- stderr is reserved for launcher/diagnostic noise
- `--compact` emits single-line JSON
- a future `--human` mode is explicitly out of scope for v1

### 5.5 Exit codes

| Exit | Meaning |
|------|---------|
| 0 | success |
| 1 | tool error returned by `org-mcp` |
| 2 | usage / argument error |
| 3 | transport / protocol failure |
| 4 | server spawn / discovery failure |

### 5.6 URI normalization policy

The CLI should be generous in accepted input, but precise internally.

Rules:

- commands may accept either bare or `org://` form from the user where that is unambiguous and safe
- tool calls normalize to the exact form the underlying tool expects
- CLI outputs always prefer `org://...` where an entity URI is returned

But this is not one global rule. The implementation must respect **per-command parameter contracts** such as:

- `after_uri` for `org-add-todo` being ID-based
- `resource_uri` vs `uri`
- `org-read-outline` taking an absolute file path, not a generic resource URI

---

## 6. CLI surface

Top-level layout:

```text
org read <uri>
org read-headline <uri>
org outline <file>

org query <ql-expr> [--files <f>...]
org query inbox
org query next [--tag <t>]
org query backlog [--tag <t>]

org todo state <uri> <new-state> [--from <state>] [--note <text>]
org todo add --parent <uri> --title <t> --state <s> [--body <text>] [--tag <t>...] [--after <uri>]

org edit rename <uri> --from <t> --to <t>
org edit body <uri> --new <text> [--old <text>] [--append]
org edit properties <uri> --set <k>=<v>... [--unset <k>...]
org edit tags <uri> [--tag <t>...]
org edit priority <uri> [--priority <A|B|C>]
org edit scheduled <uri> [--date <YYYY-MM-DD[ HH:MM]>]
org edit deadline <uri> [--date <YYYY-MM-DD[ HH:MM]>]
org edit log-note <uri> --note <text>

org clock status
org clock in <uri> [--at <ts>] [--resolve]
org clock out [<uri>] [--at <ts>]
org clock add <uri> --start <ts> --end <ts>
org clock delete <uri> --at <ts>
org clock dangling

org config todo
org config tags
org config tag-candidates
org config priority
org config files
org config clock

org tools list
org tools call <name> [--args '<json>']
```

### 6.1 Additional machine-facing commands

Because agents are the primary consumer, v1 should also plan for discoverability commands:

```text
org schema
org schema <command-path>
```

These emit machine-readable command metadata, including:

- command path
- accepted flags
- required/optional params
- expected JSON output shape
- exit semantics

This is higher value for agents than prose `--help` alone.

---

## 7. Command contract table

The plan must treat this table as a core artifact, not a note.

Each command needs an exact mapping to:

- underlying MCP method/resource
- exact parameter names
- parameter types
- URI expectations
- output transformation rules

Examples of why this matters:

- `org edit body` maps to a tool using `resource_uri`, not `uri`
- `org clock in --resolve` may need to map to the server's string-shaped expectation rather than a native JSON boolean
- `org outline` is file-path oriented and should not be treated like generic `org://` resource access

This table should be implemented as both documentation and tests.

---

## 8. Output schemas

### 8.1 General rule

Every command returns the CLI-owned envelope:

- `ok: true|false`
- `data` on success
- `error` on failure

### 8.2 Command-specific data shapes

Examples:

#### `org read`

```json
{
  "ok": true,
  "data": {
    "title": "Review PR",
    "todo": "TODO",
    "uri": "org://...",
    "children": []
  }
}
```

#### `org read-headline`

Even though the server returns plain text, the CLI wraps it as JSON:

```json
{
  "ok": true,
  "data": {
    "text": "* TODO Review PR\nBody..."
  }
}
```

#### `org tools call`

For the raw escape hatch:

```json
{
  "ok": true,
  "data": {
    "tool": "org-read",
    "result": { }
  }
}
```

### 8.3 Token-efficiency considerations

Agent use means output size matters.

V1 should therefore support:

- `--compact`
- predictable field ordering where practical

Likely v1.1 / upstream work:

- `--fields` / projection on CLI commands
- pagination support where server-side data can get large, especially `org-ql-query`

---

## 9. Project structure

```text
org/
├── Cargo.toml
├── PLAN.md
├── README.md
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── uri.rs
│   ├── output.rs
│   ├── contract.rs
│   └── mcp/
│       ├── mod.rs
│       ├── client.rs
│       ├── transport.rs
│       └── error.rs
└── tests/
    ├── fixtures/
    │   └── echo_server.rs
    ├── handshake.rs
    ├── cli_parse.rs
    ├── contract_mapping.rs
    ├── integration.rs
    └── live_org_mcp.rs
```

Notes:

- no `human.rs` renderer in v1
- `contract.rs` holds command→tool/resource mapping logic and schema metadata

---

## 10. Testing strategy

This project should be built test-first.

### 10.1 Test layers

| Layer | What | How |
|------|------|-----|
| Unit | URI normalization, JSON envelope, parameter shaping | plain `#[test]` |
| Parse | clap argument parsing and invalid usage | `cli_parse.rs` |
| Contract | command→tool/resource mapping and exact JSON params | mock/fixture tests |
| Protocol | initialize + request/response handling | fixture stdio server |
| Live integration | real launcher + real `org-mcp` | opt-in local/live test |

### 10.2 Critical test requirements

The following are mandatory because they cover the highest-risk assumptions:

1. **Live launcher contract test**
   - verify the chosen real launcher works with the client's framing assumptions
2. **Uniform JSON output test**
   - prove even plain-text server commands become valid CLI JSON
3. **Per-command parameter contract tests**
   - especially `resource_uri` vs `uri`, special URI rules, and odd parameter types
4. **Optional tool discovery test**
   - GTD tools must not be treated as guaranteed

### 10.3 Evidence rule

No phase is complete unless:

- tests for that phase were written first or alongside the contract definition
- tests pass
- a real or fixture-backed command path demonstrates the intended JSON envelope

---

## 11. Implementation phases

The work should be split into small, claimable pieces so multiple agents can pick it up later.

### Phase 1 — MCP client foundation

- create project skeleton
- implement sync transport/client
- support `org tools list` and `org tools call`
- add fixture server
- add handshake/protocol tests

**Exit criterion:** `org tools list` returns valid JSON through the CLI envelope.

### Phase 2 — command contract layer

- introduce command contract table / metadata
- implement `org schema` and `org schema <command>`
- codify command→tool/resource mappings

**Exit criterion:** machine-readable schema exists for all planned commands.

### Phase 3 — read commands

- `org read`
- `org read-headline`
- `org outline`

**Exit criterion:** all read commands produce stable JSON envelopes.

### Phase 4 — query commands

- `org query`
- optional GTD query commands, surfaced based on real tool discovery behavior

**Exit criterion:** core query path works; GTD capability handling is explicit and tested.

### Phase 5 — todo commands

- `org todo state`
- `org todo add`

**Exit criterion:** state changes and creation commands are wired and contract-tested.

### Phase 6 — edit commands

- rename/body/properties/tags/priority/scheduled/deadline/log-note

**Exit criterion:** all edit commands map correctly to server params and return stable JSON.

### Phase 7 — clock commands

- status/in/out/add/delete/dangling

**Exit criterion:** command contracts cover all clock operations and edge-case parameter shaping.

### Phase 8 — config commands

- todo/tags/tag-candidates/priority/files/clock

**Exit criterion:** introspection commands are available and schema-documented.

### Phase 9 — error handling and polish

- finalize exit-code matrix
- preserve raw server error detail in JSON envelope
- add `--compact`
- write README examples

**Exit criterion:** CLI-level contract is documented and consistent.

---

## 12. Proposed upstream changes to org-mcp

These are not required for v1, but some are high leverage.

### High priority

1. **Accept both bare and `org://` URI forms for tool inputs**
   - simplest way to remove a major class of CLI/agent mistakes

2. **Pagination / projection for `org-ql-query`**
   - important for agent token budgets and large Org setups

3. **Small server-info / capability tool**
   - useful for connection checks and runtime introspection

### Medium priority

4. **Normalize result shapes more consistently**
   - especially across reads, writes, and “soft empty” cases

### Lower priority / defer carefully

5. **Direct stdio batch entrypoint**
   - useful only if it preserves correctness with live Emacs configuration
   - must not replace the current daemon/wrapper model by assumption

---

## 13. Open questions

1. Should v1 include `org schema`, or should that land immediately after `tools list/call`?
2. Should GTD commands appear unconditionally in the CLI, or be surfaced only when discovered from the live server?
3. Should the CLI accept both bare and `org://` forms everywhere it safely can, or stay strict until upstream URI normalization lands?
4. Is `--compact` enough for v1 token control, or should field projection be moved into the initial implementation set?

---

## 14. Explicitly out of scope

- local `.org` parsing
- daemon/background process model in the CLI
- interactive prompts
- TTY-driven format switching
- human-pretty rendering in v1
- Windows support beyond what the existing launcher setup naturally permits

---

## 15. Delivery plan for a team of agents

The implementation should be translated into beads issues after this plan is accepted.

Recommended issue shape:

- one issue for plan acceptance / contract finalization
- one issue per phase in §11
- dependencies linking the phases in execution order
- explicit acceptance criteria on each issue tied to tests and command contract evidence

This keeps the work small, parallelizable where possible, and easy for a team to pick up safely.
