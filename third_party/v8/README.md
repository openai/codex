# `rusty_v8` Release Artifacts

This directory contains the Bazel packaging used to build and stage
target-specific `rusty_v8` release artifacts for `codex-rs/v8-poc`.

Current pinned versions:

- Rust crate: `v8 = =146.4.0`
- Embedded upstream V8 source: `14.6.202.11`

The generated release pairs include:

- `//third_party/v8:rusty_v8_release_pair_x86_64_apple_darwin`
- `//third_party/v8:rusty_v8_release_pair_aarch64_apple_darwin`
- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_x86_64_pc_windows_msvc`
- `//third_party/v8:rusty_v8_release_pair_aarch64_pc_windows_msvc`

Each release pair contains:

- a static library built from source
- a Rust binding file copied from the exact same `v8` crate version for that
  target

Consumers in this repo should be wired with explicit paths:

- `RUSTY_V8_ARCHIVE`
- `RUSTY_V8_SRC_BINDING_PATH`

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version from `codex-rs/Cargo.lock`.

The target-select aliases used by the `v8` build script are:

- `//third_party/v8:rusty_v8_archive_for_target`
- `//third_party/v8:rusty_v8_binding_for_target`

The dedicated publishing workflow is:

- `.github/workflows/rusty-v8-release.yml`

That workflow currently stages musl artifacts:

- `librusty_v8_release_x86_64-unknown-linux-musl.a.gz`
- `librusty_v8_release_aarch64-unknown-linux-musl.a.gz`
- `src_binding_release_x86_64-unknown-linux-musl.rs`
- `src_binding_release_aarch64-unknown-linux-musl.rs`
