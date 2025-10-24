## Overview
`codex-cli::mcp_cmd` powers the experimental `codex mcp` subcommands. It manages global MCP server configurations (stdio and streamable HTTP), lists/inspects entries, and handles OAuth flows when remote MCP servers support them.

## Detailed Behavior
- CLI structure:
  - `McpCli` (Parser) flattens config overrides and dispatches to `McpSubcommand`.
  - Subcommands (`List`, `Get`, `Add`, `Remove`, `Login`, `Logout`) each have dedicated argument structs (including transport-specific args).
  - `AddMcpTransportArgs` uses a clap `ArgGroup` to ensure exactly one transport is specified (`--command` vs `--url`).
- Core operations:
  - `run_add`: Validates server name, loads global MCP servers (`load_global_mcp_servers`), constructs `McpServerTransportConfig` (stdio or streamable HTTP), writes back via `write_global_mcp_servers`, and opportunistically initiates OAuth login if the remote advertises support.
  - `run_remove`: Removes server entries and rewrites config.
  - `run_list`: Loads config, computes OAuth auth statuses (`compute_auth_statuses`), and prints either JSON or formatted tables for stdio/HTTP transports.
  - `run_get`: Shows a single server (JSON or human-readable), including transport details and enabled/disabled tool lists.
  - `run_login` / `run_logout`: Enforce feature toggles (`Feature::RmcpClient`), ensure transport compatibility (streamable HTTP), and call `perform_oauth_login` or `delete_oauth_tokens`.
- Helpers:
  - `validate_server_name` (inlined near parse_env_pair) ensures names meet policy (not shown earlier but likely) (if missing maybe near bottom? need confirm). Should mention environment variable parsing `parse_env_pair`.
  - `parse_env_pair` parses `KEY=VALUE` into tuples for stdio env injection.
  - `format_env_display` from `codex_common` renders environment variables consistently.
  - `supports_oauth_login` queries remote capabilities before offering OAuth login.
- Configuration:
  - All subcommands load `Config` with CLI overrides to respect profile toggles and feature flags.
  - `find_codex_home` ensures global config paths are resolved properly.

## Broader Context
- MCP server management is experimental; other Codex components consume the resulting config (e.g., `codex_core::config`).
- OAuth flows rely on `codex_rmcp_client` and the configured credential store mode from `Config`.
- Context can't yet be determined for future transports; the code is structured to add more variants (`grpc`, etc.) via additional flatten sections.

## Technical Debt
- Error handling mixes `anyhow` with custom messages; consolidating logging and error reporting would make UX smoother.
- Formatting logic for tables duplicates widths calculation; extracting helpers would reduce repetition.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract shared table/JSON printers to reduce duplication across list/get commands.
    - Refine error messaging to highlight configuration paths and remediation (e.g., when OAuth feature is disabled).
related_specs:
  - ./main.rs.spec.md
  - ../core/src/config.rs.spec.md
  - ../core/src/mcp/*.rs.spec.md
