# Codex Curl Installer

This folder contains the non-Rust assets for the curl-based Codex installer
and update flow. Rust code lives under `codex-rs/`.

## Goals

- Download only the binaries needed for the current platform.
- Keep the install isolated under `CODEX_HOME` (default: `~/.codex`).
- Avoid breaking npm/brew installs; shadowing is acceptable.
- Support additional helper CLIs without globally polluting `PATH`.

## Install Root (`CODEX_HOME`)

The installer treats `CODEX_HOME` as the install root:

- If `CODEX_HOME` is set, it is used as-is.
- Otherwise, the default is `~/.codex`.

All curl-managed artifacts should live under this root.

## On-Disk Layout

The layout is designed to support multiple versions, helper binaries, and
atomic updates:

- `CODEX_HOME/bin/`
- `CODEX_HOME/versions/<version>/`
- `CODEX_HOME/versions/<version>/bin/`
- `CODEX_HOME/tools/<tool>/<version>/`
- `CODEX_HOME/tools/bin/`

Key conventions:

- The user-facing entrypoint is `CODEX_HOME/bin/codex`.
- `CODEX_HOME/versions/current` is a symlink to the active version directory.
- Helper CLIs that ship with Codex (for example, Windows sandbox helpers) live
  in `CODEX_HOME/versions/<version>/bin/`.
- Third-party tools we fetch (for example, `rg`) live under
  `CODEX_HOME/tools/...`, with optional shims in `CODEX_HOME/tools/bin/`.

## PATH Strategy

We separate the user's global `PATH` from Codex's runtime `PATH`:

1. The installer ensures `CODEX_HOME/bin` is on the user's `PATH`.
2. The `codex` wrapper augments `PATH` at runtime to include:
   - `CODEX_HOME/bin`
   - `CODEX_HOME/tools/bin`
   - `CODEX_HOME/versions/current/bin`

This keeps helper CLIs available to Codex without exposing them as global
commands in every shell session.

## Versioning And Atomic Updates

Curl-managed installs should be versioned:

1. Download into a versioned directory:
   - `CODEX_HOME/versions/<version>/`
2. Link `CODEX_HOME/versions/current` to the new version atomically.
3. Keep a small number of prior versions for rollback.

Because the wrapper resolves through `versions/current`, repointing the
symlink updates the effective version without editing shell rc files again.

## Helper CLI Placement

Any additional CLIs that Codex needs at runtime should follow these rules:

- Bundled CLIs that are version-coupled to Codex:
  - Place in `CODEX_HOME/versions/<version>/bin/`
- Third-party tools that may be shared across versions:
  - Place in `CODEX_HOME/tools/<tool>/<version>/`
  - Optionally add a stable shim in `CODEX_HOME/tools/bin/`

The wrapper then makes them available during execution.

## Ripgrep (`rg`)

The preferred approach is:

- Use a system `rg` when available.
- Otherwise, allow curl-managed installs to place `rg` under
  `CODEX_HOME/tools/rg/<version>/` with a shim in `CODEX_HOME/tools/bin/rg`.

Codex CLI can optionally honor an explicit `CODEX_RG_PATH` to point directly
to a managed `rg`.

## Scripts

Planned/expected scripts in this folder:

- `installer/install.sh`
- `installer/lib.sh`
- `installer/update.sh`

The public one-liner should look like:

```sh
curl -fsSL https://raw.githubusercontent.com/openai/codex/main/installer/install.sh | bash
```

All scripts must honor `CODEX_HOME` with a fallback to `~/.codex`.
