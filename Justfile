# org-cli Justfile — developer workflow shortcuts.
#
# Context:
#   - The org-mcp Elisp source is managed in ~/.config/dotfiles/modules/home/emacs-gtd/update.sh.
#     This repo only consumes ../org-mcp/org-mcp.el for the contract parity test.
#     If the sibling repo is missing, the parity test logs and passes — it does not block builds.
#   - This repo does NOT pin org-mcp inside its flake. The CLI talks to org-mcp over stdio.

# Print available targets
default:
    just --list

# Bump cargo deps + flake inputs (org-mcp pin lives in dotfiles emacs-gtd module, NOT here)
update: update-cargo update-flake
    @echo "Reminder: org-mcp Elisp pin is managed in ~/.config/dotfiles/modules/home/emacs-gtd/update.sh"

# Bump cargo dependencies only
update-cargo:
    cargo update

# Update flake inputs only
update-flake:
    nix flake update

# Run all CI gates: fmt, clippy, tests, nix flake check
check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test
    nix flake check

# Run the live integration test suite (gated on ORG_LIVE_TEST=1; passes trivially without env)
live:
    ORG_LIVE_TEST=1 cargo test --test live_org_mcp -- --test-threads=1
