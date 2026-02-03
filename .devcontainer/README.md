# Codex devcontainer profiles

This folder now ships two profiles:

- `devcontainer.codex-dev.json` (default intent: develop the Codex repo itself)
- `devcontainer.secure.json` (default intent: run Codex in a stricter, firewall-enforced project container)

`devcontainer.json` currently mirrors `devcontainer.codex-dev.json` so VS Code opens into the Codex contributor setup by default.

## Profile 1: Codex contributor (`devcontainer.codex-dev.json`)

Use this when working on this repository:

- forces `linux/arm64` (`platform` + `runArgs`)
- uses `CARGO_TARGET_DIR=${containerWorkspaceFolder}/codex-rs/target-arm64`
- keeps firewall off by default (`CODEX_ENABLE_FIREWALL=0`) for lower friction
- still includes persistent mounts and bootstrap (`post_install.py`)

## Profile 2: Secure project (`devcontainer.secure.json`)

Use this when you want stricter egress control:

- enables firewall startup (`postStartCommand`)
- uses IPv4 allowlisting + IPv6 default-deny
- requires `NET_ADMIN` / `NET_RAW` caps
- uses project-generic Cargo target dir (`/workspace/.cache/cargo-target`)

## How to switch profiles

Option A (quick swap in repo):

```bash
cp .devcontainer/devcontainer.secure.json .devcontainer/devcontainer.json
```

or

```bash
cp .devcontainer/devcontainer.codex-dev.json .devcontainer/devcontainer.json
```

Then run **Dev Containers: Rebuild and Reopen in Container**.

Option B (CLI without copying):

```bash
devcontainer up --workspace-folder . --config .devcontainer/devcontainer.secure.json
```

or

```bash
devcontainer up --workspace-folder . --config .devcontainer/devcontainer.codex-dev.json
```

## Using Codex after opening the container

The image preinstalls the Codex CLI. In the container terminal:

```bash
codex
```

Useful checks:

```bash
which codex
codex --help
```
