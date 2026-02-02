# Codex devcontainer

This is a Codex-focused devcontainer setup adapted for this monorepo.

## Core design choices

- devcontainer schema + `init` + `updateRemoteUserUID`
- `${devcontainerId}`-scoped named volumes for per-container persistence
- read-only host `~/.gitconfig` mount with container-local `GIT_CONFIG_GLOBAL`
- explicit `workspaceMount`/`workspaceFolder`
- post-create bootstrap script (`post_install.py`) for idempotent setup

## What is Codex-specific

- Rust toolchain pinned to `1.93.0` with `clippy`, `rustfmt`, `rust-src`
- musl targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
- Node `22` + pnpm `10.28.2`
- firewall setup that allowlists domains from `OPENAI_ALLOWED_DOMAINS`
- persistent Cargo/Rustup volumes

## Lifecycle hooks

- `postCreateCommand`: `python3 /opt/post_install.py`
  - configures history files
  - fixes ownership on mounted dirs
  - writes `/home/vscode/.gitconfig.local`
- `postStartCommand`: `bash /opt/post_start.sh`
  - applies firewall rules through `init-firewall.sh`
  - optionally adds GitHub CIDR ranges from `api.github.com/meta`

## Firewall modes

- **Strict (default)**: `CODEX_ENABLE_FIREWALL=1` (or unset)
- **Permissive**: `CODEX_ENABLE_FIREWALL=0`

Optional strict-mode enhancement:

- `CODEX_INCLUDE_GITHUB_META_RANGES=1` (default) hydrates GitHub CIDRs into the allowlist.

To run in permissive mode during a session:

```bash
export CODEX_ENABLE_FIREWALL=0
```

Then restart or rebuild the container.

## Persistent volumes

- `/commandhistory`
- `/home/vscode/.codex`
- `/home/vscode/.config/gh`
- `/home/vscode/.cargo/registry`
- `/home/vscode/.cargo/git`
- `/home/vscode/.rustup`

## Local Docker smoke build

```bash
docker build -f .devcontainer/Dockerfile -t codex-devcontainer-test .
docker run --rm -it --cap-add=NET_ADMIN --cap-add=NET_RAW \
  -v "$PWD":/workspace -w /workspace codex-devcontainer-test zsh
```
