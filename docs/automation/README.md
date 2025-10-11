# Automation Config Samples

These example TOML files are used by automation scripts when private configs under `local/automation/` are not present. Copy the examples and fill in secrets via GitHub Actions secrets or environment variables.

Example usage:
- `local/automation/agent_bus.toml` ← copy from `docs/automation/agent_bus.example.toml`
- `local/automation/review_watch_config.toml` ← copy from `docs/automation/review_watch.example.toml`
- `local/automation/monitors.toml` ← copy from `docs/automation/monitors.example.toml`

Do not commit real secrets. Use `${ENV:VARIABLE}` placeholders and map them from workflow `env:` using repo secrets.
