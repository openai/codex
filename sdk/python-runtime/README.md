# Codex App Server Runtime for Python SDK

Platform-specific runtime package consumed by the published `codex-app-server-sdk`.

This package is staged during release so the SDK can pin an exact Codex app-server
version without checking platform binaries into the repo.

`openai-codex-app-server-bin` is intentionally wheel-only. Do not build or publish an
sdist for this package.
