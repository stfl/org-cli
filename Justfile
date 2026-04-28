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

# Build the self-contained live-test Emacs env (Emacs + org-mcp + emacs-mcp-stdio.sh)
integration-test-env:
    #!/usr/bin/env bash
    if [[ ! -e "./result/bin/emacs" ]]; then
        echo ">>> Building .#live-test-env"
        nix build .#live-test-env
    fi
    ENV_OUT=$(readlink -f "./result")
    if [[ ! -x "${ENV_OUT}/bin/emacs" || ! -x "${ENV_OUT}/bin/emacs-mcp-stdio.sh" ]]; then
        echo "ERROR: live-test-env build is incomplete at ${ENV_OUT}" >&2
        exit 1
    fi

# Run the live suite including mutating (#[ignore]) tests against the disposable fixture
integration-test *ARGS: integration-test-env
    ./scripts/run-live-tests.sh --include-ignored {{ARGS}}
