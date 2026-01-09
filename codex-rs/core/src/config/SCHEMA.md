# Config JSON Schema

We generate a JSON Schema for `~/.codex/config.toml` from the `ConfigToml` type
and commit it at `docs/config.schema.json` for editor integration.

When you change any fields included in `ConfigToml` (or nested config types),
regenerate the schema and update the fixture:

```
just write-config-schema
cargo test -p codex-core config_schema_matches_fixture
```
