# org-cli Justfile — developer workflow shortcuts.
#
# Context:
#   - The CLI talks to org-mcp over stdio. The contract parity test consumes
#     ../org-mcp/org-mcp.el from a sibling checkout (logs & passes if absent).
#   - The flake's `live-test-env` output pins its own copy of org-mcp;
#     refresh it with `just update-pins` (or nix/update-pins.sh).

# Print available targets
default:
    just --list

# Bump cargo deps + flake inputs (live-test-env Elisp pins refresh via `just update-pins`)
update: update-cargo update-flake update-pins
    @echo "Reminder: refresh live-test-env Elisp pins with 'just update-pins' if needed"

# Bump cargo dependencies only
update-cargo:
    cargo update

# Update flake inputs only
update-flake:
    nix flake update

# Refresh repo-local Elisp pins (org-mcp) used by live-test-env
update-pins:
    ./nix/update-pins.sh

# Run all CI gates: fmt, clippy, tests, nix flake check
check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test
    nix flake check

# Build the self-contained live-test Emacs env (Emacs + org-mcp).
# Both stdio shims are exposed via deterministic paths in
# share/org-cli-live/paths.env (see nix/live-test-env.nix).
integration-test-env:
    #!/usr/bin/env bash
    if [[ ! -e "./result/bin/emacs" ]]; then
        echo ">>> Building .#live-test-env"
        nix build .#live-test-env
    fi
    ENV_OUT=$(readlink -f "./result")
    if [[ ! -x "${ENV_OUT}/bin/emacs" ]]; then
        echo "ERROR: live-test-env build is incomplete at ${ENV_OUT}" >&2
        exit 1
    fi
    PATHS_ENV="${ENV_OUT}/share/org-cli-live/paths.env"
    if [[ ! -f "$PATHS_ENV" ]]; then
        echo "ERROR: $PATHS_ENV missing" >&2
        exit 1
    fi
    # shellcheck source=/dev/null
    source "$PATHS_ENV"
    if [[ ! -x "$ORG_MCP_STDIO" ]]; then
        echo "ERROR: ORG_MCP_STDIO=$ORG_MCP_STDIO not executable." >&2
        echo "Update the org-mcp pin (nix/update-pins.sh) to a rev that ships scripts/org-mcp-stdio.sh." >&2
        exit 1
    fi

# Run the live suite including mutating (#[ignore]) tests against the disposable fixture
integration-test *ARGS: integration-test-env
    ./scripts/run-live-tests.sh --include-ignored {{ARGS}}
