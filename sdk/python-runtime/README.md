# Codex CLI Runtime for Python SDK

Platform-specific runtime package consumed by the published `openai-codex` SDK.

This package is staged during release so the SDK can pin an exact Codex CLI
version without checking platform binaries into the repo. The distribution name
is `openai-codex-cli-bin`, while the import module remains `codex_cli_bin`.

`openai-codex-cli-bin` is intentionally wheel-only. Do not build or publish an
sdist for this package.

Expected wheel contents:

- macOS/Linux: `codex_cli_bin/bin/codex`
- Windows: `codex_cli_bin/bin/codex.exe`,
  `codex_cli_bin/bin/codex-command-runner.exe`, and
  `codex_cli_bin/bin/codex-windows-sandbox-setup.exe`
