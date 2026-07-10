# Copybara Sync

This directory is internal-only. Treat `INTERNAL_ONLY_FILES` in
`copy.bara.sky` as the source of truth for the paths that are excluded from the
public mirror surface.

## Copybara Workflows

- `internal_to_public_branch` exports the public-owned surface from
  `openai/codex-internal` to a branch in `openai/codex` when run with suitable
  local credentials.
- `internal_to_public_projection` materializes the public-owned surface into a
  local folder for CI validation and patch generation.

`openai/codex-internal` is the source of truth. Copybara synchronization only
projects changes from the internal repository to `openai/codex`; there is no
public-to-internal import path.

The internal-to-public GitHub Actions workflow does not push to `openai/codex`
or open a public PR because the repository `GITHUB_TOKEN` is scoped to
`openai/codex-internal` and no cross-repository PAT or GitHub App credential is
assumed. It uploads a `codex-public-export.patch` artifact instead and prints a
one-line command that a user can run from a local `openai/codex` clone to
download the artifact, apply it, push a branch, and open the public PR.

## Internal-Only Content

Prefer internal-only directories and files over line-level conditionals. Whole
paths are easier to audit, and `INTERNAL_ONLY_FILES` in `copy.bara.sky` is the
source of truth for those exclusions.

For Rust crates, use an internal path prefix and an internal package prefix:

- Directory: `codex-rs/internal-*`
- Cargo package name: `codex-internal-*`

For example, an internal crate named `foo` should live under
`codex-rs/internal-foo` and use `name = "codex-internal-foo"` in its
`Cargo.toml`.

Do not add an internal crate to the root workspace unless it needs to
participate in root workspace commands. If it does, mark the shared
`codex-rs/Cargo.toml` lines so the internal-to-public Copybara workflows strip
them before producing the public projection:

```toml
members = [
    "cli",
    "internal-foo", # copybara:strip-for-public
]
```

Use the same marker token in the file's native comment syntax for internal-only
dependency entries or other single-line edits in marker-eligible files. The
default marker set covers `codex-rs/Cargo.toml`, `codex-rs/**/Cargo.toml`, and
`codex-rs/**/*.rs`, excluding internal-only paths:

```toml
codex-internal-foo = { path = "internal-foo" } # copybara:strip-for-public
```

When an internal crate replaces a public crate implementation, keep consuming
crates on the public dependency name and centralize the projection difference in
`[workspace.dependencies]`. The internal side should be live TOML. Store the
public replacement as `# copybara:public` payload lines:

```toml
[workspace.dependencies]
# copybara:replace-for-public begin
codex-otel = { package = "codex-internal-otel", path = "internal-otel" }
# copybara:replace-for-public with
# copybara:public codex-otel = { path = "otel" }
# copybara:replace-for-public end
```

The internal projection sees the `codex-internal-otel` package through the
stable `codex-otel` dependency name. The public projection receives:

```toml
codex-otel = { path = "otel" }
```

Consumer crates should still depend on the workspace dependency by its stable
public name:

```toml
codex-otel = { workspace = true }
```

For multi-line edits, use a marked block:

```toml
# copybara:strip-for-public begin
[workspace.metadata.codex-internal]
foo = true
# copybara:strip-for-public end
```

Keep marked regions small and close to stable anchors in the file. If another
shared file type needs these markers, add a broad path pattern to
`INTERNAL_ONLY_MARKER_FILES` in `copy.bara.sky` so exports strip or replace the
markers. Replacement payloads may use TOML-style
`# copybara:public` or Rust-style `// copybara:public` markers.

Do not use line markers in generated files. In particular, `codex-rs/Cargo.lock`
is projection-sensitive once internal workspace crates add lockfile entries.
Keep it out of `INTERNAL_ONLY_FILES`: the internal-to-public helper command
regenerates it in the local `openai/codex` clone before creating the public PR.
Do not export a lockfile that contains
`codex-internal-*` packages or private sources.

Public-owned paths should not depend on internal-only paths unless the Copybara
transformations also remove or replace that dependency before export.
