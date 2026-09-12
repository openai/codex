---
name: bazel-hot-cache
description: Use the CI-warmed BuildBuddy cache for fast local or Applied devbox Bazel builds without uploading local artifacts.
---

# Bazel Hot Cache

Use this skill when a Codex developer wants a fast Bazel build from the
BuildBuddy cache warmed by post-merge CI, especially for
`//codex-rs/cli:codex`.

## Why This Exists

The checked-in helper `scripts/run-bazel-hot-cache-build.sh` consumes the
BuildBuddy keyspace warmed by the platform-matching `verify-release-build` lane
on successful `main` pushes. That CI lane is the relevant writer because it
builds release-shaped Rust code in Bazel `fastbuild` mode with Rust debug
assertions disabled, which matches the developer build shape this helper is
for.

The helper is read-only from a developer machine or devbox:

```text
--noremote_upload_local_results
```

Do not change this to upload laptop or devbox artifacts unless the user
explicitly asks for a cache-warming writer flow.

The helper also bounds the known remote-cache long tail: one Bazel attempt may
stall after remote-cache progress stops even though the cache keyspace is hot.
The checked-in helper times out that stalled attempt after `120s`, shuts down
that attempt's Bazel server, and retries once. Override only for diagnostics
with `CODEX_BAZEL_HOT_CACHE_TIMEOUT_SECONDS` or
`CODEX_BAZEL_HOT_CACHE_MAX_ATTEMPTS`.

## Local Flow

From a Codex checkout with `gh` auth and `BUILDBUDDY_API_KEY` in the command
environment:

```bash
hot_sha="$(scripts/run-bazel-hot-cache-build.sh --print-latest-hot-main-commit)"
git worktree add ../codex-hot-cache "${hot_sha}"
cd ../codex-hot-cache
scripts/run-bazel-hot-cache-build.sh
```

If the current checkout is already at the desired hot SHA, run the helper in
place. Pass Bazel target patterns after the script name to build something
other than the default `//codex-rs/cli:codex`.

For routine developer use, keep Bazel's normal persistent user root and
repository cache. A fresh worktree already gives Bazel a fresh workspace/output
base while preserving the shared local Bazel state that makes the workflow
predictable. Only set `BAZEL_OUTPUT_USER_ROOT` when intentionally running an
isolated diagnostic proof; that discards useful local Bazel state and can be
substantially slower even when remote cache hits are 100%.

## Applied Devbox Flow

The same helper works on a Linux Applied devbox mirror. The helper selects
`ci-linux` on Linux and `ci-macos` on macOS. Applied rsync mirrors commonly
exclude `.git`, so forward the checkout SHA as `CODEX_BAZEL_COMMIT_SHA` for
that remote command. The helper itself does not use `tmux`.

Portable shape:

```bash
commit_sha="$(git rev-parse HEAD)"
remote_repo="<remote-codex-repo>"
BUILDBUDDY_API_KEY_STDIN="$BUILDBUDDY_API_KEY" \
  ssh <devbox-host> "bash -lc 'read -r BUILDBUDDY_API_KEY; export BUILDBUDDY_API_KEY; export CODEX_BAZEL_COMMIT_SHA=${commit_sha}; cd ${remote_repo}; export PATH=\$HOME/code/openai/project/dotslash-gen/bin:\$HOME/.local/bin:\$PATH; scripts/run-bazel-hot-cache-build.sh'" \
  <<< "$BUILDBUDDY_API_KEY_STDIN"
```

Replace `<devbox-host>` and `<remote-codex-repo>` with the caller's actual host
and mirrored checkout path. Do not persist the BuildBuddy key on the devbox for
this flow.

## Cache-Key Rules

The helper intentionally owns the Bazel option order. Keep the explicit Rust
debug-assertion flags before the platform CI config. The CI config adds Rust
flags too, and Bazel action keys include generated Rust params-file bytes, so
moving `--config=ci-macos` or `--config=ci-linux` before those explicit flags
can turn a CI-hot action into a remote miss.

The helper sets:

- `--config=buildbuddy-openai-rbe`
- `--compilation_mode=fastbuild`
- Rust `-Cdebug-assertions=no` flags for target and exec Rust actions
- `--build_metadata=COMMIT_SHA=<checkout-sha>`
- `--build_metadata=TAG_job=verify-release-build`
- `--build_metadata=TAG_rust_debug_assertions=off`
- `--config=ci-macos` on macOS or `--config=ci-linux` on Linux
- `--remote_download_toplevel`
- `--noremote_upload_local_results`
- bounded retry defaults: `120s` timeout, `2` max attempts

## Reading Results

Read Bazel summaries as:

```text
cacheable_hit_rate = remote cache hit / (all processes - internal)
```

Do not include Bazel `internal` processes in the denominator; they are local
bookkeeping, not cache misses.

## Limits

- Build mode supports macOS and Linux only.
- `--print-latest-hot-main-commit` requires `gh` auth.
- Build mode requires `BUILDBUDDY_API_KEY`.
- Build mode requires `python3` for bounded retry handling.
- This is command/config specific. A successful `main` Bazel run is a good hot
  default for this helper's `verify-release-build` shape, not proof that every
  other Bazel command is hot.
- Routine latency claims should be measured from fresh worktrees using the
  normal persistent Bazel user root, not from fresh `BAZEL_OUTPUT_USER_ROOT`
  diagnostics.
