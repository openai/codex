# ThreadManager Sample

Small one-shot binary that starts a Codex thread with `ThreadManager` from
`codex-core-api`, submits a single user turn, and prints the final assistant
message.

Authentication uses the configured credential storage under `CODEX_HOME`, including
`cli_auth_credentials_store`, `mcp_oauth_credentials_store`, and
`features.secret_auth_storage`. Local managed authentication requirements and auth
routing settings are respected. This also applies to `cargo run` builds. Other
execution settings remain the sample's built-in configuration.

```sh
cargo run -p codex-thread-manager-sample -- "Say hello"
```

Use `--model` to override the configured default model:

```sh
cargo run -p codex-thread-manager-sample -- --model gpt-5.2 "Say hello"
```

The prompt can also be piped through stdin:

```sh
printf 'Say hello\n' | cargo run -p codex-thread-manager-sample
```
