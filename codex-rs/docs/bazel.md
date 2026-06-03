# Bazel in codex-rs

This repository uses Bazel to build the Rust workspace under `codex-rs`.
Cargo remains the source of truth for crates and features, while Bazel
provides hermetic builds, toolchains, and cross-platform artifacts.

As of 6/1/2026, this setup is still experimental as we stabilize it.

## High-level layout

- `../MODULE.bazel` defines Bazel dependencies and Rust toolchains.
- `rules_rs` imports third-party crates from `codex-rs/Cargo.toml` and
  `codex-rs/Cargo.lock` via `crate.from_cargo(...)` and exposes them under
  `@crates`.
- `../defs.bzl` provides `codex_rust_crate`, which wraps `rust_library`,
  `rust_binary`, and `rust_test` so Bazel targets line up with Cargo conventions.
  It provides a sane set of defaults that work for most first-party crates, but may
  need tweaks in some cases.
- Each crate in `codex-rs/*/BUILD.bazel` typically uses `codex_rust_crate` and
  makes some adjustments if the crate needs additional compile-time or runtime data,
  or other customizations.

## Running Bazel locally

The repository root `justfile` exposes the common Bazel entry points:

```bash
just bazel-test
just bazel-clippy
```

Ordinary local `bazel` and `just` invocations run locally. BuildBuddy cache,
build event upload, downloads, and remote execution are opt-in configurations.

## BuildBuddy

Codex uses BuildBuddy for a shared Bazel cache and remoted builds and tests. To use it
to speed up your builds and tests you'll need to provide an API key and select a
configuration.

### BuildBuddy API key

If you're an OpenAI employee, log in to https://openai.buildbuddy.io and use Google sign-in.

Create a BuildBuddy API key as described in BuildBuddy's [Authentication Guide][bb-auth-guide],
then add it to `~/.bazelrc`:

```bazelrc
# Local machine only; this file contains a BuildBuddy credential.
common --remote_header=x-buildbuddy-api-key=<your-buildbuddy-api-key>
```

Keeping the credential outside the workspace reduces the risk of accidentally
committing it.

If you need different API keys for different projects, put the API key in
`%workspace%/user.bazelrc` instead. The checked-in `.bazelrc` optionally imports
that file, and `.gitignore` excludes it. Do not commit or share a file containing
the credential.

[bb-auth-guide]: https://www.buildbuddy.io/docs/guide-auth/#managing-keys

### Selecting a remote build configuration

OpenAI employees should default to the OpenAI host with remote execution unless
they have a reason to choose another configuration. Add the following configuration
to `%workspace%/user.bazelrc`:

```bazelrc
common --config=buildbuddy-openai-rbe
```

OpenAI employees who don't want remote execution can use `buildbuddy-openai`. External users
should use `buildbuddy-generic-rbe` or `buildbuddy-generic`. See below for details on these
configurations.

### All remote configurations

GitHub Actions routes Bazel build and output-resolution commands through
`.github/scripts/run_bazel_with_buildbuddy.py`. Higher-level helpers such as
`.github/scripts/run-bazel-ci.sh` and `.github/scripts/rusty_v8_bazel.py`
delegate remote configuration selection to that wrapper. The wrapper reads the
GitHub Actions repository and event payload rather than relying on workflow
files to duplicate tenant-selection logic.

Loading-phase target-discovery `bazel query` commands run locally because they
only enumerate labels and do not need remote caches or execution.

The `Cache/BES` host is also used for remote downloads.

| Invocation/config | Key Required | Cache/BES | Build exec | Test exec |
| --- | --- | --- | --- | --- |
| `bazel ...` | No | None | Local | Local |
| `bazel ... --config=buildbuddy-generic` | Yes | `remote.buildbuddy.io` | Local | Local |
| `bazel ... --config=buildbuddy-generic-rbe` | Yes | `remote.buildbuddy.io` | Remote | Remote |
| `bazel ... --config=buildbuddy-openai` | Yes | `openai.buildbuddy.io` | Local | Local |
| `bazel ... --config=buildbuddy-openai-rbe` | Yes | `openai.buildbuddy.io` | Remote | Remote |

Without an API key, the wrapper removes remote CI configurations and runs
locally. With a key, workflows choose the host as follows:

| Run | Key | Uses OpenAI BuildBuddy Host |
| --- | --- | --- |
| Push to `main` in `openai/codex` | Yes | Yes |
| `workflow_dispatch` in `openai/codex` | Yes | Yes |
| Same-repository pull request in `openai/codex` | Yes | Yes |
| Fork pull request into `openai/codex` | No | No; local |
| Push or `workflow_dispatch` in a fork with a key | Yes | No; generic host |
| Pull request run in a fork repository with a key | Yes | No; generic host |

CI configurations determine whether builds and tests execute remotely:

| CI config | Remote config | Build exec | Test exec |
| --- | --- | --- | --- |
| `ci-linux` | `*-rbe` | Remote host | Remote host |
| `ci-v8` | `*-rbe` | Remote host | Remote host |
| `ci-macos` | `*-rbe` | Remote host | Local |
| `ci-windows-cross` | `*-rbe` | Remote host | Local |
| `ci-windows` | non-RBE | Local | Local |
| Keyless CI fallback | none | Local | Local |

To exercise the generic remote configuration with your key:

```bash
BUILDBUDDY_API_KEY=... GITHUB_REPOSITORY=my-fork/codex \
  ./.github/scripts/run_bazel_with_buildbuddy.py \
  build --config=ci-linux //codex-rs/cli:codex
```

The wrapper selects the OpenAI host only inside GitHub Actions for a trusted
run in `openai/codex`. A missing or malformed pull request event
payload fails closed to the generic host. For local OpenAI host access, use
the `user.bazelrc` configuration above.

### Use the CI-warmed developer cache

The platform-matching `verify-release-build` Bazel lane on `main` warms a
developer fastbuild keyspace for release-shaped Rust builds with debug
assertions off. On a local Mac or Linux Applied devbox with
`BUILDBUDDY_API_KEY` set in that command's environment, use
`scripts/run-bazel-hot-cache-build.sh` to consume that cache without uploading
locally built artifacts:

```bash
hot_sha="$(scripts/run-bazel-hot-cache-build.sh --print-latest-hot-main-commit)"
git worktree add ../codex-hot-cache "${hot_sha}"
cd ../codex-hot-cache
scripts/run-bazel-hot-cache-build.sh
```

If the current checkout is already at a known hot commit, run the script in
place instead of creating another worktree. On an Applied Linux devbox, run
the same script from the synced mirror with `BUILDBUDDY_API_KEY` and
`CODEX_BAZEL_COMMIT_SHA` forwarded only for that remote command; it selects the
Linux `verify-release-build` cache keyspace instead of the macOS one. Pass
Bazel target patterns after the script name to build something other than the
default `//codex-rs/cli:codex`.

For routine developer use, keep Bazel's normal persistent user root and
repository cache. A fresh worktree already gives Bazel a fresh workspace/output
base while preserving the shared local Bazel state that makes this workflow
predictable. Set `BAZEL_OUTPUT_USER_ROOT` only for isolated diagnostic proofs;
that discards useful local Bazel state and can be much slower even when remote
cache hits are 100%.

Keep the script-owned option order. Bazel action keys include the generated
Rust params-file bytes, so moving the platform CI config before the explicit
Rust debug-assertion flags can turn a hot CI action into a local cache miss.

## Evolving the setup

When you add or change Rust dependencies, update the Cargo.toml/Cargo.lock as normal.
Then refresh the Bzlmod lockfile from the repo root:

```bash
just bazel-lock-update
```

This runs `bazel mod deps --lockfile_mode=update` and updates `MODULE.bazel.lock` if needed.
Commit the lockfile changes along with your Cargo lockfile update.

To verify lockfile alignment locally (the same check CI runs), use:

```bash
just bazel-lock-check
```

In some cases, an upstream crate may need a patch or a `crate.annotation` in `../MODULE.bzl`
to have it build in Bazel's sandbox or make it cross-compilation-friendly. If you see issues,
feel free to ping zbarsky or mbolin.

When you add a new crate or binary:

1. Add it to the Cargo workspace as usual.
2. Create a `BUILD.bazel` that calls `codex_rust_crate` (see nearby crates for
   examples).
3. If a dependency needs special handling (compile/runtime data, additional binaries
   for integration tests, env vars, etc) you may need to adjust the parameters to
   `codex_rust_crate` to configure it.
   One common customization is setting `test_tags = ["no-sandbox]` to run the test
   unsandboxed. Prefer to avoid it, but it is necessary in some cases such as when the
   test itself uses Seatbelt (the sandbox does as well, and it cannot be nested).
   To limit the blast radius, consider isolating such tests to a separate crate.

If you see build issue and are not sure how to apply the proper customizations, feel free to ping zbarsky or mbolin.

## References

- Bazel overview: https://bazel.build/
- Bzlmod (module system): https://bazel.build/external/overview
- rules_rust: https://github.com/bazelbuild/rules_rust
- rules_rs: https://github.com/bazelbuild/rules_rs
