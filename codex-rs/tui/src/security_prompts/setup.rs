pub(crate) const SECURITY_REVIEW_SETUP_PROMPT: &str = r#"
You are preparing a setup script for the security review connectors (linear, google-workspace-mcp, notion, secbot). Follow these rules and produce commands the Codex agent can run:
- Always ask the user for confirmation before running any command that installs node or gh. If either is missing, show a copy/paste install command for the detected platform (macOS: brew install node gh; Debian/Ubuntu: sudo apt-get update && sudo apt-get install -y nodejs npm gh) and stop if the user declines.
- Verify required CLIs exist by running `command -v node` and `gh --version`; fail with the install instructions when absent.
- Use ~/.codex/config.toml as the example config; update MCP server entries without disturbing unrelated settings.
- For OAuth MCP servers, run `codex mcp add <server-name>` so the browser flow can complete (skip when already configured). Do this for linear, notion, google-workspace-mcp, and secbot.
- For google-workspace-mcp, prefer an existing binary. If ~/.codex/plugins/google-workspace-mcp/google-workspace-mcp is missing, try downloading the published binary from the user's Google Drive, place it in that directory, and chmod +x it. If you cannot retrieve it, ask the user to upload or point to a local path.
- After binaries and config are in place, validate logins and connectivity (for example, `codex mcp list` and, when applicable, `codex-rmcp-client login --server <name>`). Report the exact steps taken and any manual actions required.
"#;
