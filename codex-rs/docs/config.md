# Config Warnings for Unstable Features

When `config.toml` explicitly enables a feature whose stage is **under development**, Codex core emits a non-fatal warning event at session
startup.

- Under-development features are incomplete and may behave unpredictably.

To suppress this warning, set the following in `~/.codex/config.toml`:

```toml
suppress_unstable_features_warning = true
```

Example enabling an experimental feature and suppressing the warning:

```toml
[features]
collaboration_modes = true

suppress_unstable_features_warning = true
```
