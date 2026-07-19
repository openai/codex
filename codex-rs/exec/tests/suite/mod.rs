// Aggregates all former standalone integration tests as modules.
mod add_dir;
mod agents_md;
mod apply_patch;
mod approval_policy;
mod auth_env;
#[path = "completion_backfill_tests.rs"]
mod completion_backfill;
mod ephemeral;
mod hooks;
mod mcp_required_exit;
mod originator;
mod output_schema;
mod prompt_stdin;
mod resume;
mod sandbox;
mod server_error_exit;
