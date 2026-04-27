/// Library entry point — exposes internal modules for integration tests.
///
/// The `org` binary is the primary deliverable; this crate also ships a
/// library target so that integration tests under `tests/` can import
/// types directly (e.g. `org_cli::contract`).
pub mod contract;
pub mod discovery;
pub mod mcp;
pub mod output;
pub mod uri;
