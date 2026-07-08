# Dependency Check Devbox Evidence

This report records the sanitized results of the dependency-check prototype
validation performed on July 7, 2026 PDT (July 8 UTC).

## Test environment

- Devbox: `depcheck-0707`
- Branch: `caseysilver/dependency-check-current-main`
- Live devbox implementation: pre-rebase commit `1afad914c2`
- Final review base: live `main` at `f1affbac5e`
- CLI: locally built `codex` from the branch
- Fixture package manager: npm 9 from `/usr/bin`
- Registry: `https://registry.npmjs.org`
- Approval policy: `on-request`
- Configured approval reviewer: `auto_review`
- Feature: `dependency_check = true`

The permission profile made the fixture workspace writable except for its root
`package.json` and `package-lock.json`, which remained read-only until the
structured tool requested exact per-command write access. Network access was
limited to the npm registry and `api.osv.dev`.

## Human approval and denial

Prompt:

```text
Add exact npm dependency zod 3.23.8 to this project and complete the install.
```

After resolution and policy evaluation, Codex displayed a human approval
prompt despite `approvals_reviewer = "auto_review"`:

```text
npm install --ignore-scripts --package-lock-only --no-audit --no-fund \
  --save-exact zod@3.23.8

Reason: Update package.json and package-lock.json only after the resolved graph
passed dependency policy.

Write permission:
/home/dev-user/codex/.tmp/depcheck-deny-latest/package.json
/home/dev-user/codex/.tmp/depcheck-deny-latest/package-lock.json
```

The request offered explicit `Yes, proceed` and `No` choices. Canceling the
request left both files byte-for-byte unchanged and did not create
`node_modules`:

```text
package.json
5abcc97473363ddc3ec6262f5858422b44ae0fa9b950e7e2ba91b12a06cc5344

package-lock.json
838707aad02f065464e1d770bd8e8955ac95b97cf209f33145a2b2d3d22117d1
```

The hashes above were identical before and after denial.

## Approved completion

The same workflow was approved in a separate fresh npm fixture. The approval
listed only these writable paths:

```text
/home/dev-user/codex/.tmp/depcheck-fixture/package.json
/home/dev-user/codex/.tmp/depcheck-fixture/package-lock.json
```

The checked operation completed the real lock update, clean install, installed
graph comparison, and `npm rebuild`. Independent verification produced:

```text
package_json_zod=3.23.8
lock_zod=3.23.8
installed_zod=3.23.8

package.json
2fec168a42eea408bbcc08c7cd410a6182c46e5cdfb3bb5936c6ac95f7e351f0

package-lock.json
a5d8d2b506043dd2738f8a41ccfb8848b87684c25b62fd48f80292b0ada183af
```

## Monorepo fail-closed result

At the Codex monorepo root, the prompt instructed Codex to call the structured
tool directly for `left-pad@1.3.0` as an exact development dependency. The tool
read the root package-manager declaration, recognized `pnpm@10.33.0`, and
returned an unsupported-project result because the prototype currently
supports npm projects only.

`git status --short` was unchanged by the attempt. The only entry was the
pre-existing untracked `.tmp/` test-fixture directory; no tracked monorepo file
was modified.

## Automated validation

- `cargo test -p codex-dependency-check`: 17 passed.
- `cargo test -p codex-core dependency_check`: focused unit and integration
  suites passed, including shell and unified-exec redirects, patch rejection,
  permission rejection, feature gating, and exact project-write permissions.
- Mocked OSV tests passed for malware blocking, ordinary advisory warnings,
  partial provider responses, and provider failure fail-closed behavior.
- Lock graph tests passed for exact graph comparison, installed graph mismatch,
  invalid artifact metadata, and unsupported v1 lockfiles.
- Devbox Linux regression test
  `exact_writable_file_does_not_create_metadata_mount_targets`: 1 passed.
- Devbox `cargo build -p codex-cli`: passed.
- `just fix -p codex-dependency-check`: passed.
- `just fix -p codex-core`: passed.
- `just fix -p codex-linux-sandbox`: passed.
- `just fmt`: passed.
- `just argument-comment-lint`: 716 targets passed.

A complete workspace `cargo test` was not run.

## Evidence handling

Sanitized TUI transcripts were retained on the devbox under
`/home/dev-user/depcheck-evidence`. Raw trace directories were deleted after
each result was extracted because trace output may contain authorization
headers.

## Proven boundary

The prototype demonstrates that Codex can route normal npm dependency work
through a structured policy check and force the final manifest mutation to a
human prompt, even when ordinary approvals use automatic review. The prompt can
be restricted to exact manifest paths; denial prevents mutation; approval can
complete the install; and unsupported package managers fail closed.

The remaining product work is broader package-manager and workspace support,
rollback after post-mutation npm failures, and defense in depth for arbitrary
shell indirection beyond the static dependency-command recognizer.
