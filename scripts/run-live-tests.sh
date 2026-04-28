#!/usr/bin/env bash
# Run the live integration suite against an isolated, repo-local Emacs daemon.
#
# Brings up an ephemeral org-mcp daemon from the .#live-test-env Nix output,
# points the test suite at it via ORG_LIVE_SERVER / ORG_LIVE_FILES, and tears
# everything down on exit. Touches no user state: HOME, XDG_RUNTIME_DIR, and
# org files all live in a tmpdir that is rm -rf'd on exit.
#
# Usage:
#   scripts/run-live-tests.sh                  # run the read-only live suite
#   scripts/run-live-tests.sh -- live_handshake  # forward extra args to cargo test
#
# This is the canonical entrypoint future CI will invoke (org-cli-4c8).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

if [[ ! -e "${REPO_ROOT}/result/bin/emacs" ]]; then
    echo "ERROR: expected Nix output not found at ${REPO_ROOT}/result" >&2
    echo "Run 'nix build .#live-test-env' from the repo root to build it." >&2
    exit 1
fi
ENV_OUT=$(readlink -f "./result")

FIXTURE_SRC="$REPO_ROOT/tests/live-fixtures/sample.org"
if [[ ! -f "$FIXTURE_SRC" ]]; then
    echo "ERROR: fixture not found at $FIXTURE_SRC" >&2
    exit 1
fi

# 2. Create the disposable workspace. HOME is pinned here so the daemon never
#    reads ~/.config/emacs or any other user dotfiles.
WORKSPACE="$(mktemp -d -t org-cli-live.XXXXXXXX)"
DAEMON_NAME="org-cli-live-$$"
LAUNCHER="$WORKSPACE/mcp-launcher.sh"
ORG_DIR="$WORKSPACE/org"
ORG_FILE="$ORG_DIR/sample.org"
DAEMON_LOG="$WORKSPACE/daemon.log"
RUNTIME_DIR="$WORKSPACE/run"

mkdir -p "$ORG_DIR" "$RUNTIME_DIR"
chmod 700 "$RUNTIME_DIR"
cp "$FIXTURE_SRC" "$ORG_FILE"

echo ">>> Workspace: $WORKSPACE"
echo ">>> Fixture:   $ORG_FILE"
echo ">>> Daemon:    $DAEMON_NAME"

# 3. Cleanup trap: stop the daemon and remove the workspace. Idempotent so it
#    can fire from EXIT after we've already explicitly stopped the daemon.
DAEMON_PID=""
cleanup() {
    local rc=$?
    set +e
    if [[ -n "$DAEMON_PID" ]] && kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo ">>> Stopping daemon (pid $DAEMON_PID)"
        HOME="$WORKSPACE" XDG_RUNTIME_DIR="$RUNTIME_DIR" \
            "$ENV_OUT/bin/emacsclient" -s "$DAEMON_NAME" \
                -e '(kill-emacs)' >/dev/null 2>&1
        wait "$DAEMON_PID" 2>/dev/null
    fi
    if [[ -n "${WORKSPACE:-}" && -d "$WORKSPACE" ]]; then
        rm -rf "$WORKSPACE"
    fi
    exit "$rc"
}
trap cleanup EXIT INT TERM

# 4. Spawn the daemon. --fg-daemon does not fork; we background it with & so
#    the script keeps control. The init.el reads ORG_LIVE_DIR / ORG_LIVE_FILES
#    from the environment.
echo ">>> Spawning daemon"
HOME="$WORKSPACE" \
ORG_LIVE_DIR="$ORG_DIR" \
ORG_LIVE_FILES="$ORG_FILE" \
XDG_RUNTIME_DIR="$RUNTIME_DIR" \
    "$ENV_OUT/bin/emacs" -Q \
        --fg-daemon="$DAEMON_NAME" \
        -l "$ENV_OUT/share/org-cli-live/init.el" \
        >"$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!

# 5. Wait for readiness via emacsclient probe. Cap at ~30s; bail early if the
#    daemon process has already died.
echo ">>> Waiting for daemon readiness"
ready=0
for _ in $(seq 1 60); do
    if HOME="$WORKSPACE" XDG_RUNTIME_DIR="$RUNTIME_DIR" \
       "$ENV_OUT/bin/emacsclient" -s "$DAEMON_NAME" \
           -e 't' >/dev/null 2>&1; then
        ready=1
        break
    fi
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo "ERROR: daemon exited before becoming ready" >&2
        echo "--- daemon log ---" >&2
        cat "$DAEMON_LOG" >&2 || true
        exit 1
    fi
    sleep 0.5
done
if [[ "$ready" -ne 1 ]]; then
    echo "ERROR: daemon did not become ready within 30s" >&2
    echo "--- daemon log ---" >&2
    cat "$DAEMON_LOG" >&2 || true
    exit 1
fi
echo ">>> Daemon ready"

# 6. Write a launcher wrapper that bakes in the --socket / org-mcp args. The
#    test harness reads ORG_LIVE_SERVER as a single path with no extra argv
#    slots, so the wrapper is the contract surface.
cat >"$LAUNCHER" <<WRAPPER
#!/usr/bin/env bash
export PATH="$ENV_OUT/bin:\$PATH"
export HOME="$WORKSPACE"
export XDG_RUNTIME_DIR="$RUNTIME_DIR"
exec "$ENV_OUT/bin/emacs-mcp-stdio.sh" \\
    --socket="$DAEMON_NAME" \\
    --server-id=org-mcp \\
    --init-function=org-mcp-enable \\
    --stop-function=org-mcp-disable \\
    "\$@"
WRAPPER
chmod +x "$LAUNCHER"

# 7. Run the read-only live suite. Mutating tests are gated on --ignored and
#    are NOT included here (tracked separately).
echo ">>> Running live test suite"
ORG_LIVE_TEST=1 \
ORG_LIVE_SERVER="$LAUNCHER" \
ORG_LIVE_FILES="$ORG_FILE" \
HOME="$WORKSPACE" \
XDG_RUNTIME_DIR="$RUNTIME_DIR" \
    cargo test --test live_org_mcp -- --test-threads=1 "$@"

echo ">>> Live test suite complete"
