# Dependency Check

`dependency_check` is an experimental npm-first tool that separates dependency
resolution and policy from package lifecycle execution.

Enable it in `config.toml`:

```toml
[features]
dependency_check = true
```

The model-visible request uses exact package intent:

```json
{
  "ecosystem": "npm",
  "dependencies": [{ "name": "zod", "version": "3.23.8" }],
  "dependency_kind": "development"
}
```

Version ranges and tags are rejected. A request may contain at most twenty
direct dependencies.

## Execution flow

1. Copy `package.json`, an existing `package-lock.json`, and project `.npmrc`
   into a temporary directory.
2. Run `npm install --package-lock-only --ignore-scripts` there to resolve the
   complete graph without running package lifecycle code.
3. Parse the npm v2/v3 `package-lock.json` graph and require every package to
   have an HTTPS artifact URL and integrity value.
4. Query the OSV batch API for every unique exact npm package coordinate.
5. Update the real project lock graph through Codex's normal sandbox and
   approval path, then require it to exactly match the checked graph.
6. Run a clean `npm ci --ignore-scripts`, verify the installed artifact graph
   matches the checked graph, and only then run `npm rebuild`.

Known `MAL-*` advisories block the operation. Ordinary vulnerability advisories
are reported as warnings. Provider failures, partial responses, unsupported
sources, missing integrity values, and graph changes fail closed before
lifecycle scripts run.

An OSV allow result means only that OSV returned no matching advisory. It is
not a general assertion that a package is trustworthy.

## Enforcement

When the feature is enabled, Codex rejects recognizable direct dependency-add
commands such as `npm install package@version`, `pnpm add`, `yarn add`, and
`bun add`, directing the model to `dependency_check`. It also rejects direct
`apply_patch` changes to JavaScript package manifests and lockfiles. Generic
shell tools cannot request full sandbox bypass or additional write permissions
that overlap the active project, and `request_permissions` cannot grant write
access to dependency manifests.

The real project mutation still uses the existing shell sandbox and approval
orchestrator. Its request is limited to `package.json` and `package-lock.json`
and is always routed to the user, even when the session normally uses automatic
approval review. A permission profile should keep the root manifest and
lockfile read-only inside the ambient sandbox so only the structured tool can
request that exact write access:

```toml
default_permissions = "dependency-check"

[permissions.dependency-check]
description = "Workspace write access with npm manifests read-only."

[permissions.dependency-check.filesystem]
":minimal" = "read"
":tmpdir" = "write"
"/path/to/node/runtime" = "read"

[permissions.dependency-check.filesystem.":workspace_roots"]
"." = "write"
"package.json" = "read"
"package-lock.json" = "read"

[permissions.dependency-check.network]
enabled = true

[permissions.dependency-check.network.domains]
"registry.npmjs.org" = "allow"
"api.osv.dev" = "allow"
```

Replace `/path/to/node/runtime` with the directory containing the configured
Node.js and npm binaries, and ensure that directory's `bin` is on `PATH` when
Codex starts. List only lockfiles that already exist: on Linux, a missing
read-only path can be materialized as an empty bind-mount placeholder. Add
`npm-shrinkwrap.json` only when the project already contains it.

This profile protects only those paths at the workspace root. Managed policy
must add equivalent rules for nested package roots when they need the same
filesystem enforcement. The tool grants its own temporary resolution
directory as a preapproved, per-command write path; that does not widen access
to the project. The checked project update separately requests per-command
write access to `package.json` and `package-lock.json`; with `on-request`
approval, that request produces the human prompt. Denial stops before the
project command runs.

## Current limitations

- Only non-workspace npm projects are supported.
- pnpm, Yarn, and Bun dependency additions are redirected but return an
  explicit unsupported-project result from the tool.
- Git, file, workspace, HTTP, and other non-HTTPS package sources are blocked.
- Static command recognition covers common direct commands. Arbitrary shell
  indirection is outside the redirect boundary and must be constrained by the
  filesystem sandbox policy.
- The install phase may leave `package.json`, `package-lock.json`, or unpacked
  packages changed if npm fails after the approved project mutation. Lifecycle
  scripts remain disabled in those failure cases.
