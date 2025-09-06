<h1 align="center">Codex CLI (Fork)</h1>

<p align="center">Languages: <a href="README.md">English</a> | <a href="README.zh-CN.md">Chinese (zh-CN)</a></p>

<p align="center">
  <a href="https://github.com/openai/codex/releases/tag/rust-v0.30.0">
    <img alt="Upstream" src="https://img.shields.io/badge/upstream-0.30.0-blue" />
  </a>
  &nbsp;
  <a href="https://github.com/openai/codex/releases">
    <img alt="Upstream Releases" src="https://img.shields.io/badge/upstream-releases-555" />
  </a>
  &nbsp;
  <a href="https://github.com/openai/codex">
    <img alt="Upstream Repo" src="https://img.shields.io/badge/source-openai%2Fcodex-555" />
  </a>

</p>

This repository is a maintained fork of the upstream project at:
https://github.com/openai/codex

For product docs, installation, and general usage, please refer to the
upstream README and documentation. This fork focuses on a small set of
exclusive features and a separate sync/release workflow while staying close to upstream.

> Upstream baseline: **0.30.0** - based on upstream tag
> [`rust-v0.30.0`](https://github.com/openai/codex/releases/tag/rust-v0.30.0)

## Fork-Specific Features

- TUI: Esc behavior optimized for fast editing/backtracking - see details in [TUI - Esc](#tui--esc-clear-input-or-backtrack)

### TUI - Esc: clear input or backtrack

In the TUI composer, Esc adapts to context:

- When the composer has text: press Esc once to show a one-second window to clear. The footer adds a subtle `Esc clear` indicator and a second line "Please Escape again to clear"; press Esc again within 1s to clear. If you don't, the hint hides automatically.
- When the composer is empty: press Esc to prime backtrack; press Esc again to open "Backtrack to User Messages" and pick an earlier user message to fork from (Up/Down, Enter). The transcript overlay (`Ctrl+T`) remains available and continues to support Esc-to-step, Enter-to-confirm.

## Install & CLI Name (Fork)

- npm package: `@jojoyo/codex`
- global binary: `jcodex` (renamed to avoid conflicts with the upstream `codex`)
  - install: `npm i -g @jojoyo/codex`
  - run: `jcodex`

## Sync and Release Workflow

- Versioning: releases follow upstream versions with a "-fork.N" suffix, e.g. "0.21.0-fork.1".
- Tags: publishing is triggered by tags named "rust-v<version>", including fork suffixes (e.g., "rust-v0.21.0-fork.1").
- Release pipeline: only "-alpha/-beta/-rc" are marked as pre-release; "-fork.*" are published as normal releases.
- Upstream sync tooling: local helper script at "scripts/sync_upstream.sh" and a scheduled/hand-run GitHub Action to open PRs that sync from upstream.
- Tag selection for sync: by default only stable tags ("rust-vX.Y.Z"). Use `--include-pre` to include pre-release tags (e.g., `-alpha/-beta/-rc`) or `--pre-only` for pre-releases only.

Getting started with this fork:

- Download builds from this fork's GitHub Releases page.
- See CONTRIBUTING.md for the fork-specific contribution and release guide.

License:

Apache-2.0, same as upstream (see LICENSE).
