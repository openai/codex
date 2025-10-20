# AGENTS.md Discovery

Codex uses [`AGENTS.md`](https://agents.md/) files to gather helpful guidance before it starts assisting you. This page explains how those files are discovered and combined, so you can decide where to place your instructions.

## Global Instructions (`~/.codex`)

- Codex looks for global guidance in your Codex home directory (usually `~/.codex`; set `CODEX_HOME` to change it). For a quick overview, see the [Memory with AGENTS.md section](../docs/getting-started.md#memory-with-agentsmd) in the getting started guide.
- If an `AGENTS.override.md` file exists there, it takes priority. If not, Codex falls back to `AGENTS.md`.
- Only the first non-empty file is used. Other filenames, such as `instructions.md`, have no effect unless Codex is specifically instructed to use them.
- Whatever Codex finds here stays active for the whole session, and Codex combines it with any project-specific instructions it discovers.

## Project Instructions (per-repository)

When you work inside a project, Codex builds on those global instructions by collecting project docs:

- The search starts at the repository root and continues down to your current directory. If a Git root is not found, only the current directory is checked.
- In each directory along that path, Codex looks for `AGENTS.override.md` first, then `AGENTS.md`, and then any fallback names listed in your Codex configuration (see [`project_doc_fallback_filenames`](../docs/config.md#project_doc_fallback_filenames)). At most one file per directory is included.
- Files are read in order from root to leaf and joined together with blank lines. Empty files are skipped, and very large files are truncated once the combined size reaches 32 KiB (the default [`project_doc_max_bytes`](../docs/config.md#project_doc_max_bytes) limit). If you need more space, split guidance across nested directories or raise the limit in your configuration.

## How They Come Together

Before Codex gets to work, the instructions are ingested in precedence order: global guidance from `~/.codex` comes first, then each project doc from the repository root down to your current directory. Guidance in deeper directories overrides earlier layers, so the most specific file controls the final behavior.

### Priority Summary

1. Global `AGENTS.override.md` (if present), otherwise global `AGENTS.md`.
2. For each directory from the repository root to your working directory: `AGENTS.override.md`, then `AGENTS.md`, then configured fallback names.

Only these filenames are considered. To use a different name, add it to the fallback list in your Codex configuration or rename the file accordingly.

## Fallback Filenames

Codex can look for additional instruction filenames beyond the two defaults if you add them to `project_doc_fallback_filenames` in your Codex configuration. Each fallback is checked after `AGENTS.override.md` and `AGENTS.md` in every directory along the search path.

Example: suppose your configuration lists `["TEAM_GUIDE.md", ".agents.md"]`. Inside each directory Codex will look in this order:

1. `AGENTS.override.md`
2. `AGENTS.md`
3. `TEAM_GUIDE.md`
4. `.agents.md`

If the repository root contains `TEAM_GUIDE.md` and the `backend/` directory contains `AGENTS.override.md`, the overall instructions will combine the root `TEAM_GUIDE.md` (because no override or default file was present there) with the `backend/AGENTS.override.md` file (which takes precedence over the fallback names).

You can configure those fallbacks in `~/.codex/config.toml` (or another profile) like this:

```toml
[project]
project_doc_fallback_filenames = ["TEAM_GUIDE.md", ".agents.md"]
```

For additional configuration details, see [Config](../docs/config.md) and revisit the [Memory with AGENTS.md guide](../docs/getting-started.md#memory-with-agentsmd) for practical usage tips.
