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
update: update-cargo update-flake
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

# Run the live integration test suite (gated on ORG_LIVE_TEST=1; passes trivially without env)
live:
    ORG_LIVE_TEST=1 cargo test --test live_org_mcp -- --test-threads=1

# Build the self-contained live-test Emacs env (Emacs + org-mcp + emacs-mcp-stdio.sh)
live-env:
    nix build .#live-test-env

# Run the read-only live suite end-to-end against an isolated daemon from live-test-env
live-env-test *ARGS:
    ./scripts/run-live-tests.sh {{ARGS}}
