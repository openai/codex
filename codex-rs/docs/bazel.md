# Bazel in codex-rs

This repository uses Bazel to build the Rust workspace under `codex-rs`.
Cargo remains the source of truth for crates and features, while Bazel
provides hermetic builds, toolchains, and cross-platform artifacts.

As of 1/9/2026, this setup is still experimental as we stabilize it.

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
just bazel-remote-test
```

Ordinary local `bazel` and `just` invocations run locally. BuildBuddy cache,
build event upload, downloads, and remote execution are opt-in configurations.

## `user.bazelrc`

The checked-in `.bazelrc` optionally imports `%workspace%/user.bazelrc`, and
`.gitignore` excludes that file. You do not need a `user.bazelrc` for local
builds or GitHub Actions.

Create `user.bazelrc` only if you have a BuildBuddy API key and want direct
local `bazel` and `just` commands to use remote services. Choose the generic
host, or `buildbuddy-openai-rbe` if you are authorized for the OpenAI host:

```bazelrc
# Local only; this file contains a BuildBuddy credential.
common --config=buildbuddy-generic-rbe
common --remote_header=x-buildbuddy-api-key=<your-buildbuddy-api-key>
```

Use `buildbuddy-generic` or `buildbuddy-openai` without the `-rbe` suffix if
you want cache, build event upload, and downloads without remote execution.
The repository's `--config=remote` settings do not select a host; the `-rbe`
configuration above supplies the executor when a command uses them.

`user.bazelrc` is ignored by Git but still contains a credential; do not commit
or share it.

## BuildBuddy remote configurations

GitHub Actions routes Bazel traffic through
`.github/scripts/run_bazel_with_buildbuddy.py`. Higher-level helpers such as
`.github/scripts/run-bazel-ci.sh` and `.github/scripts/rusty_v8_bazel.py`
delegate remote configuration selection to that wrapper. The wrapper reads the
GitHub Actions repository and event payload rather than relying on workflow
files to duplicate tenant-selection logic.

The `Cache/BES` host is also used for remote downloads.

| Invocation/config | Key Required | Cache/BES | Build exec | Test exec |
| --- | --- | --- | --- | --- |
| `bazel ...` | No | None | Local | Local |
| `bazel ... --config=remote` | - | None | Requires `*-rbe` | Requires `*-rbe` |
| `bazel ... --config=buildbuddy-generic` | Yes | `remote.buildbuddy.io` | Local | Local |
| `bazel ... --config=remote --config=buildbuddy-generic-rbe` | Yes | `remote.buildbuddy.io` | Remote | Remote |
| `bazel ... --config=buildbuddy-openai` | Yes | `openai.buildbuddy.io` | Local | Local |
| `bazel ... --config=remote --config=buildbuddy-openai-rbe` | Yes | `openai.buildbuddy.io` | Remote | Remote |

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
  build --config=remote //codex-rs/cli:codex
```

The wrapper selects the OpenAI host only inside GitHub Actions for a trusted
run in `openai/codex`. A missing or malformed pull request event
payload fails closed to the generic host. For local OpenAI host access, use
the `user.bazelrc` configuration above.

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
