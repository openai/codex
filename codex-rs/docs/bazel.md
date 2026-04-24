# Bazel in codex-rs

This repository uses Bazel to build the Rust workspace under `codex-rs`.
Cargo remains the source of truth for crates and features, while Bazel
provides hermetic builds, toolchains, and cross-platform artifacts.

As of 1/9/2026, this setup is still experimental as we stabilize it.

## Local build and test commands

Run Bazel commands from the repository root. Bazel is a good default for
build-heavy local validation because CI already uses it and the repository has
remote caching configured.

Common commands:

```bash
# Build the main CLI binary.
bazel build //codex-rs/cli:codex

# Run a crate's unit tests.
bazel test //codex-rs/tui:tui-unit-tests

# Run all tests in one crate package, including integration tests.
bazel test //codex-rs/tui:all

# Run all codex-rs tests.
bazel test //codex-rs/...
```

To discover the generated targets for a crate, query the package:

```bash
bazel query //codex-rs/tui:all
```

Cargo remains the source of truth for crates and feature definitions, so keep
using Cargo when you are validating Cargo-specific behavior or when a crate does
not yet have a matching Bazel target.

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
