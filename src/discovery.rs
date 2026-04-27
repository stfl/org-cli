/// Server auto-discovery: search `$PATH` for `emacs-mcp-stdio.sh`.
///
/// PLAN §5.2 / ticket org-cli-qq7.
///
/// When `--server` is omitted, the CLI calls `discover_server()` to find the
/// launcher in PATH before spawning. Commands that don't need a server (e.g.
/// `schema`) skip discovery entirely.
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const LAUNCHER: &str = "emacs-mcp-stdio.sh";

/// Search `$PATH` for an executable named `emacs-mcp-stdio.sh`.
///
/// Returns `Ok(vec!["/path/to/emacs-mcp-stdio.sh"])` on success, or an `Err`
/// containing a human-readable message suitable for a usage error envelope.
pub fn discover_server() -> Result<Vec<String>, String> {
    let path_var = std::env::var("PATH").unwrap_or_default();

    for dir in path_var.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(LAUNCHER);
        if candidate.is_file() {
            // Check executable bit (any of owner/group/other exec bits set).
            if let Ok(meta) = std::fs::metadata(&candidate)
                && meta.permissions().mode() & 0o111 != 0
            {
                return Ok(vec![candidate.to_string_lossy().into_owned()]);
            }
        }
    }

    Err(
        "no --server specified and emacs-mcp-stdio.sh not found in $PATH; \
         pass --server <cmd> or install the launcher (see PLAN §5.2)"
            .to_string(),
    )
}
