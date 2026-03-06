# Termux Compatibility Project

## Goal

Make `exomind-team/codex` buildable and usable on Android Termux (ARM64) without regressing existing Windows/Linux behavior.

## Local Project Setup

- Local repo: `/data/data/com.termux/files/home/projects/exomind-codex`
- Working branch: `feat/termux-compat-project-init`
- Reference fork: `https://github.com/exomind-team/codex-termux`
- Tracking issue: `https://github.com/exomind-team/codex/issues/3`

## Scope

1. Runtime compatibility for Termux:

- login browser open path (`termux-open-url`)
- update version parsing for termux suffix
- update command routing for termux package line

2. Build/package compatibility for Termux:

- Android target compile path (`aarch64-linux-android`)
- Termux runtime launcher/dynamic library handling

3. Keep cross-platform safety:

- no behavior change for Windows/Linux default path
- keep current `@openai/codex` update/install path outside Termux

## Milestones

1. M1: Android target compile check passes for core CLI crates.
2. M2: Termux login and update behavior patched behind Android/Termux conditions.
3. M3: Termux packaging/launcher path integrated (without breaking existing packaging).
4. M4: smoke test on real Termux (`codex --version`, `codex login`, `codex exec`).

## Immediate Backlog

1. Add Android build checks and preflight script (this branch).
2. Port minimal runtime patches from `codex-termux`:

- `codex-rs/login/src/server.rs`
- `codex-rs/tui/src/updates.rs`
- `codex-rs/tui/src/update_action.rs`

3. Add targeted tests for version parsing and update action selection.
4. Define packaging strategy for Termux release line.
