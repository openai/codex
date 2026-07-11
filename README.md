# openai/codex-internal

This is the source of truth for the Codex repo. It contains a mix of public and private information, so **take care when contributing to this repository to ensure private information is not leaked externally.**

Every commit on `main` in this repository is sanitized and then exported to `main` on https://github.com/openai/codex (assuming the commit is non-empty after the sanitization process). See [docs/copyberry.md](docs/copyberry.md) for the rules that govern sanitization.
