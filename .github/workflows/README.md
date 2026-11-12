# Workflow Strategy

The workflows in this directory are split so that pull requests get fast, review-friendly signal while `main` still gets the full cross-platform verification pass.

## Pull Requests

- Required checks run against GitHub's synthetic merge commit, not the pull
  request head alone. This includes changes already on `main` and catches
  conflicts before they reach the branch.
- `bazel.yml` is the main pre-merge verification path for Rust code.
  It runs Bazel `test` and Bazel `clippy` on the supported Bazel targets,
  including the generated Rust test binaries needed to lint inline `#[cfg(test)]`
  code.
- `rust-ci.yml` keeps the Cargo-native PR checks intentionally small:
  - `cargo fmt --check`
  - `cargo shear`
  - `argument-comment-lint` on Linux, macOS, and Windows
  - `tools/argument-comment-lint` package tests when the lint or its workflow wiring changes

## Post-Merge On `main`

- `bazel.yml` also runs on pushes to `main`.
  This re-verifies the merged Bazel path and populates the public GCS cache.
- `rust-ci-full.yml` is the full Cargo-native verification workflow.
  It keeps the heavier checks off the PR path while still validating them after merge:
  - the full Cargo `clippy` matrix
  - the full Cargo `nextest` matrix via per-platform archive-backed shards
  - Windows ARM64 nextest archives cross-compiled on Windows x64, then replayed on native Windows ARM64 shards
  - release-profile Cargo builds
  - cross-platform `argument-comment-lint`
  - Linux remote-env tests

## Rule Of Thumb

- If a build/test/clippy check can be expressed in Bazel, prefer putting the PR-time version in `bazel.yml`.
- Keep `rust-ci.yml` fast enough that it usually does not dominate PR latency.
- Reserve `rust-ci-full.yml` for heavyweight Cargo-native coverage that Bazel does not replace yet.

### Public Bazel action cache

Set `CODEX_BAZEL_GCS_BUCKET` and `CODEX_BAZEL_GCS_WORKLOAD_IDENTITY_PROVIDER`
as GitHub Actions secrets in `openai/codex`. They identify the bucket and its
GitHub OIDC provider. The job exchanges its identity for temporary Google
credentials. Protected main pushes populate the cache. Protected main
dispatches and Rust release tags can read it. Pull requests build without
GCS credentials.

After a main push populates the cache and a separate runner reports a remote
cache hit, set the repository variable `CODEX_BAZEL_RELEASES` to `true` to
build the additional Bazel artifacts on Rust release tags. The release
workflow selects one build-system matrix for the build, signing, and
packaging jobs. Cargo artifacts remain the default.

Once public Bazel CI passes, require the `Bazel result` check in the main
branch ruleset. It reports the result of the full Bazel workflow.
