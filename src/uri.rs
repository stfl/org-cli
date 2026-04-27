/// URI normalization helpers for org-cli.
///
/// Tool inputs must be **bare** (UUID, file path, or `file#headline/path`);
/// the `org://` prefix is reserved for the MCP resource layer (see
/// `org-mcp--parse-resource-uri` in ../org-mcp/org-mcp.el). The CLI accepts
/// either form for ergonomics, but strips a leading `org://` before sending.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutlineUriError {
    #[error("org outline expects a file path, not an org:// URI")]
    OrgUriNotAllowed,
}

/// Strip a leading `org://` prefix if present. Otherwise return the input
/// unchanged. This is the normalization applied before every tool call.
pub fn normalize_for_tool(input: &str) -> String {
    input.strip_prefix("org://").unwrap_or(input).to_string()
}

/// Validate that a path supplied to `org outline` is acceptable.
///
/// Rules:
/// - Reject any input starting with `org://` (outline takes a file path, not a resource URI).
/// - Otherwise pass through as-is (we do not force absolute paths on the user's behalf).
pub fn validate_outline_path(input: &str) -> Result<&str, OutlineUriError> {
    if input.starts_with("org://") {
        return Err(OutlineUriError::OrgUriNotAllowed);
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_bare_unchanged() {
        assert_eq!(normalize_for_tool("foo"), "foo");
    }

    #[test]
    fn normalize_strips_org_prefix() {
        assert_eq!(normalize_for_tool("org://abc"), "abc");
    }

    #[test]
    fn normalize_strips_org_prefix_with_path() {
        assert_eq!(normalize_for_tool("org://file#H1/H2"), "file#H1/H2");
    }

    #[test]
    fn validate_outline_absolute_path_ok() {
        assert!(validate_outline_path("/tmp/x.org").is_ok());
    }

    #[test]
    fn validate_outline_org_uri_rejected() {
        let err = validate_outline_path("org://abc").unwrap_err();
        assert!(matches!(err, OutlineUriError::OrgUriNotAllowed));
    }
}
