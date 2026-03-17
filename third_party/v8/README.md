# `rusty_v8` Musl Artifacts

This directory contains the Bazel packaging used to build and stage musl
`rusty_v8` release artifacts for `codex-rs/v8-poc`.

Current pinned versions:

- Rust crate: `v8 = =142.2.0`
- Embedded upstream V8 source: `14.2.231.17`

The generated musl release pairs are:

- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_musl`

Each release pair contains:

- a static library built from source
- a Rust binding file copied from the matching GNU Linux binding shipped in the
  exact same `v8` crate version

Musl consumers in this repo should be wired with explicit paths:

- `RUSTY_V8_ARCHIVE`
- `RUSTY_V8_SRC_BINDING_PATH`

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version from `codex-rs/Cargo.lock`.

The dedicated publishing workflow is:

- `.github/workflows/rusty-v8-release.yml`

That workflow stages:

- `librusty_v8_release_x86_64-unknown-linux-musl.a.gz`
- `librusty_v8_release_aarch64-unknown-linux-musl.a.gz`
- `src_binding_release_x86_64-unknown-linux-musl.rs`
- `src_binding_release_aarch64-unknown-linux-musl.rs`
