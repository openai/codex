# Contributing to this Fork

Languages: [English](CONTRIBUTING.md) | [Chinese (zh-CN)](CONTRIBUTING.zh-CN.md)

This document describes how to develop and release this fork while staying
close to the upstream project: https://github.com/openai/codex

The goal is to keep a clear separation between upstream versions and the
fork-specific changes, while making it easy to sync and publish.

## Branching model

- `main`: the primary branch for this fork. It is based on upstream release
  tags (see "Syncing with upstream") plus fork changes.
- `sync/...`: temporary branches created when syncing from upstream. Open a PR
  from these into `main`.

You can use either merge or rebase when integrating upstream. Merge is the
default and is usually simpler for shared branches.

## Versioning and tags

- Fork releases mirror upstream version numbers with a `-fork.N` suffix.
  Example: `0.21.0-fork.1`, `0.21.0-fork.2`, ...
- Release tags must be named `rust-v<version>` and match `Cargo.toml` exactly.
  Example: `rust-v0.21.0-fork.1`.

Why this scheme?

- It is obvious which upstream version a fork build is based on.
- The GitHub Actions release pipeline triggers on `rust-v*` tags and will
  build multi-platform artifacts automatically for this fork.

## Syncing with upstream

There are two complementary paths: a manual local script and an automated
GitHub Action.

### Manual (local script)

Use `scripts/sync_upstream.sh` from the repo root. It requires a clean working
tree.

List latest upstream tags:

```
scripts/sync_upstream.sh list-tags --limit 10
```

Merge upstream `main` into your branch (creates `sync/...` branch by default):

```
scripts/sync_upstream.sh merge-main --branch main --push
``;

Merge a specific upstream tag into your branch:

```
scripts/sync_upstream.sh merge-tag rust-v0.21.0 --branch main --push
```

Options:

- `--rebase`: rebase instead of merge.
- `--no-branch`: operate directly on the target branch.
- `--upstream-url`: override the default upstream URL (openai/codex).
- `--push`: push the result to `origin` (useful for opening a PR).
- `--tag-baseline`: when merging a specific upstream rust tag (e.g., `rust-v0.21.0`),
  also create a baseline tag (default name: `base-rust-v0.21.0`) pointing at the
  merge result. Use `--push-tags` to push such tags to `origin`. Baseline tags do
  NOT trigger releases because they don't start with `rust-v`.
- `--dry-run`: preview the actions (branch create/checkout, merges/rebases, baseline tags,
  pushes) without changing the repository. Dry-run performs a quick connectivity probe
  and warns if the network is restricted, then uses locally cached tags (no fetch).
  Run without `--dry-run` to fetch the latest upstream tags; if fetch fails, the script
  will warn and proceed with the local tag view.
- `--force-tags`: when fetching from upstream, force-update local tags to match upstream
  (equivalent to adding `--force` to `git fetch upstream --tags --prune`). Use this only
  if you intentionally want to overwrite diverged local tags.

Identify current baseline:

```
scripts/sync_upstream.sh current-baseline
```
This prints the nearest baseline tag (like `base-rust-v0.21.0`) if present, or
falls back to the last "Merge upstream rust-v..." commit.

Merge a series of upstream tags automatically:

```
scripts/sync_upstream.sh merge-series --branch main --push --push-tags --limit 10
```

Behavior:

- Detects the current baseline (`base-rust-v*`) and merges subsequent upstream `rust-v*`
  tags one by one (by default, stable-only), tagging each successful step as a new baseline.
- On conflicts, the script stops at the offending tag. Resolve conflicts and rerun
  the same command to continue from the next tag.
- Use `--from <rust-vX.Y.Z>` to explicitly set the baseline when none is detected; use
  `--to <rust-vX.Y.Z>` to stop at a specific tag.
 - Add `--dry-run` to preview the full merge plan without making changes.
 - Tag selection:
   - Default: stable-only (matches `rust-vX.Y.Z`)
   - `--include-pre`: include pre-release tags (e.g., `-alpha/-beta/-rc`)
   - `--pre-only`: only pre-release tags

Initialize a baseline on a fresh fork (no baseline yet):

```
scripts/sync_upstream.sh init-baseline rust-v0.21.0 --push-tags
```

This records that your current `HEAD` is based on upstream `rust-v0.21.0` by creating
`base-rust-v0.21.0`. After that, `merge-series` can discover the baseline automatically.

### Automated (GitHub Action)

Workflow: `.github/workflows/upstream-sync.yml`.

- Scheduled (daily): finds the newest `rust-v*` tag upstream, creates a
  `sync/...` branch that merges it into `main`, and opens a PR.
- Manual trigger: go to GitHub Actions, choose `upstream-sync` and provide:
  - `upstream_repo` (optional; defaults to `openai/codex`)
  - `ref` (optional; a tag like `rust-v0.21.0` or a branch like `upstream/main`)
  - `mode` (`merge` or `rebase`, default `merge`)

## Releasing fork builds

1) One‑command release (auto compute next -fork.N)

```
scripts/release_fork.sh --dry-run               # preview: baseline + next version
scripts/release_fork.sh                         # release from main; runs codex-rs/scripts/create_github_release.sh
scripts/release_fork.sh --baseline 0.30.0       # override baseline if needed
scripts/release_fork.sh --version 0.30.0-fork.2 # fully explicit
```

2) Manual (equivalent)

- Ensure `main` is at the desired upstream baseline and fork changes are merged.
- Update `codex-rs/Cargo.toml` with the new fork version (`X.Y.Z-fork.N`).
- Create and push the tag `rust-vX.Y.Z-fork.N`.

Triggering the `rust-release` workflow:

- Validates the tag format and that it matches `Cargo.toml`.
- Builds multi-platform binaries and uploads artifacts.
- Creates a GitHub Release (fork suffixes are published as normal releases;
  only `-alpha/-beta/-rc` are marked pre-release).
- Stages an npm tarball artifact (publishing to npm is manual; see below).

Optional: publish to npm using the helper script (requires access):

```
VERSION=0.21.0-fork.1
# For this fork (default repo: yoyooyooo/codex):
./scripts/publish_to_npm.py "$VERSION"
# If you ever need upstream assets instead:
./scripts/publish_to_npm.py "$VERSION" --repo openai/codex
```

## Coding, CI, and docs notes

- Keep changes minimal and focused; avoid diverging from upstream unless
  necessary.
- Follow upstream build and test instructions in `docs/`.
- The root README is ASCII-only (CI enforces this) and does not require a ToC
  block. If you add a ToC, use the `<!-- Begin ToC -->`/`<!-- End ToC -->`
  markers so CI can verify it.

## Questions

If something in this guide is unclear or missing, open an issue or PR against
this fork with a proposed update.
