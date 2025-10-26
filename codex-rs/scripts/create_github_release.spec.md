## Overview
`scripts/create_github_release` automates tagging a Codex Rust release by editing `codex-rs/Cargo.toml`, creating a commit and annotated tag via the GitHub API, and optionally dry-running the process.

## Detailed Behavior
- Parses CLI flags for dry runs, alpha vs. stable releases, and an emergency version override. Exactly one publish mode must be provided.
- `main` orchestrates the workflow:
  - Determines the target version (`determine_version` or `--emergency-version-override`).
  - Early exits on dry-run after printing the chosen version.
  - Fetches the main branch head, commit tree, and current `Cargo.toml` contents via GitHub’s REST API.
  - Rewrites the `[package]` version using a regex and uploads the new blob/tree/commit.
  - Creates an annotated tag (`rust-v{version}`) and a corresponding tag ref.
- Version helpers:
  - `determine_version` inspects existing releases to increment alpha suffixes or bump the minor version for stable releases.
  - `parse_semver`/`format_version` enforce semantic version structure and convert components back to strings.
  - `list_releases`, `get_latest_release_version`, and `strip_tag_prefix` work together to interpret GitHub tag names.
- GitHub interactions all flow through `run_gh_api`, which shells out to `gh api`, optionally posting JSON payloads and returning parsed responses. Failures raise `ReleaseError`.
- Tree and commit creation helpers (`create_blob`, `create_tree`, `create_commit`, `create_tag`, `create_tag_ref`) mirror the Git data model to produce the release commit without touching the local checkout.

## Broader Context
- Used by release engineers or CI jobs to produce official Rust releases without direct git pushes. Requires an authenticated `gh` CLI environment and appropriate repository permissions.
- Keeps release automation separate from Cargo workspace scripts, focusing solely on GitHub metadata.

## Technical Debt
- Script depends on the `gh` CLI and lacks retry/backoff logic; transient API failures abort the release. Local git history is not updated, so follow-up tooling must handle syncing.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add retry/backoff around GitHub API calls (or migrate to pygithub) to tolerate transient failures during release.
related_specs: []
