# Codex devcontainer

Use this devcontainer when you want to run Codex inside your own project container.

## Who this is for

- developers using Codex on application repos
- teams that want a consistent, secure dev environment
- contributors working on this repo (also supported)

## Quick start

1. Put this `.devcontainer/` folder in your project.
2. Open the project in VS Code.
3. Run **Dev Containers: Rebuild and Reopen in Container**.
4. In the container terminal, run `codex`.

If you prefer API key auth, set `OPENAI_API_KEY` in your host environment before opening the container.

## What you get by default

- `codex` CLI preinstalled (`@openai/codex` via npm)
- Node `22` + pnpm `10.28.2`
- Python 3 + pip
- Rust `1.92.0` with `clippy`, `rustfmt`, `rust-src`
- musl targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
- common tools: git, zsh, rg, fd, fzf, jq, curl
- persistent state volumes for history, auth/config, Cargo cache, and Rustup

## How to use Codex after opening the container

Basic flow:

```bash
codex
```

Useful checks:

```bash
codex --help
which codex
```

Typical usage is from your project root (`/workspace`), so Codex can inspect and edit files directly.

## Firewall and network policy

Strict mode is the default (`CODEX_ENABLE_FIREWALL=1`):

- outbound traffic is allowlisted by domain via `OPENAI_ALLOWED_DOMAINS`
- IPv4 is enforced with `iptables` + `ipset`
- IPv6 is explicitly default-deny via `ip6tables` (prevents bypass)

Default allowlist includes:

- OpenAI: `api.openai.com`, `auth.openai.com`
- GitHub: `github.com`, `api.github.com`, `codeload.github.com`, `raw.githubusercontent.com`, `objects.githubusercontent.com`
- registries: `registry.npmjs.org`, `crates.io`, `index.crates.io`, `static.crates.io`, `static.rust-lang.org`, `pypi.org`, `files.pythonhosted.org`

You can temporarily disable strict mode:

```bash
export CODEX_ENABLE_FIREWALL=0
```

Then rebuild/restart the container.

## Adding more languages or tooling

For project-specific stacks (Go, Java, .NET, etc.), add Dev Container features in `devcontainer.json`.

Example:

```json
{
  "features": {
    "ghcr.io/devcontainers/features/go:1": { "version": "1.24" },
    "ghcr.io/devcontainers/features/java:1": { "version": "21" }
  }
}
```

## Local Docker smoke build

```bash
docker build -f .devcontainer/Dockerfile -t codex-devcontainer-test .
docker run --rm -it --cap-add=NET_ADMIN --cap-add=NET_RAW \
  -v "$PWD":/workspace -w /workspace codex-devcontainer-test zsh
```
