# Copybara Sync

This directory is internal-only. Treat `INTERNAL_ONLY_FILES` in
`copy.bara.sky` as the source of truth for the paths that are excluded from the
public mirror surface.

## Copybara Workflows

- `public_to_internal` imports `openai/codex` into `openai/codex-internal`,
  preserves internal-only files, and runs in `SQUASH` mode so `merge_import`
  has a merge baseline.
- `internal_to_public_branch` exports the public-owned surface from
  `openai/codex-internal` to a branch in `openai/codex` when run with suitable
  local credentials.
- `internal_to_public_projection` materializes the public-owned surface into a
  local folder for CI validation and patch generation.

The GitHub Actions workflows use the same public-to-internal and
internal-to-public direction names, but they are not a strict 1:1 wrapper around
Copybara workflows. Some Actions workflows perform CI checks or use the
projection workflow as an intermediate step.

The public-to-internal Actions workflow advances one first-parent public change
at a time. Its internal PR title is the associated `openai/codex` PR title, and
the internal PR body starts with a link to the original public PR before
including the original PR body. The PR body and generated sync commit also
include a `Codex-Public-RevId` trailer so later runs can identify the last
imported public revision.

The public-to-internal Copybara workflow uses `merge_import` for shared
marker-eligible files matched by `INTERNAL_ONLY_MARKER_FILES`. Copybara runs a
three-way merge for those files so destination-only internal lines can survive
public imports. After Copybara creates the sync branch, the Actions workflow
restores the previous internal `codex-rs/Cargo.lock` unless the public change
touched that file, then asks Cargo to resolve the workspace from that baseline
and amends the generated sync commit. This keeps internal workspace crate
entries available without eagerly refreshing unrelated third-party package
versions.

If multiple public changes are pending, the workflow opens one internal sync PR
for the oldest pending public change and merges it immediately, without waiting
for internal CI. After each merge, the script fetches the updated internal
`main` branch and continues importing pending public changes in the same
workflow job, so the backlog drains without repeating checkout and Java setup.

The immediate merge path assumes the repository `GITHUB_TOKEN` is allowed to
merge these sync PRs, including bypassing branch protections if they would
otherwise require internal checks.

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

Keep marked regions small and close to stable anchors in the file. Copybara uses
`merge_import` for marker-eligible files, so public-to-internal imports should
preserve destination-only marked lines unless the public change edits the same
hunk. If that happens, resolve the merge conflict by keeping the public change
and reapplying the marked internal lines. If another shared file type needs
these markers, add a broad path pattern to `INTERNAL_ONLY_MARKER_FILES` in
`copy.bara.sky` so imports three-way merge that file class and exports strip or
replace the markers. Replacement payloads may use TOML-style
`# copybara:public` or Rust-style `// copybara:public` markers.

Do not use line markers in generated files. In particular, `codex-rs/Cargo.lock`
is projection-sensitive once internal workspace crates add lockfile entries.
Keep it out of `INTERNAL_ONLY_FILES`: public-to-internal imports resolve it for
the internal projection from the prior internal lockfile when possible, and the
internal-to-public helper command regenerates it in the local `openai/codex`
clone before creating the public PR. Do not export a lockfile that contains
`codex-internal-*` packages or private sources.

Public-owned paths should not depend on internal-only paths unless the Copybara
transformations also remove or replace that dependency before export.
