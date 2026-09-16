# `rusty_v8` Consumer Artifacts

This directory wires the `v8` crate to exact-version Bazel inputs.
Bazel consumer builds use:

- Codex sandbox release archives on Windows MSVC
- source-built V8 archives on Darwin, GNU Linux, musl Linux, and Windows GNU

Local Cargo builds still use upstream prebuilt `rusty_v8` archives by default.
Selected Cargo CI, release, and package builds override
`RUSTY_V8_ARCHIVE`/`RUSTY_V8_SRC_BINDING_PATH` with Codex release assets. Bazel
sets those variables independently in `MODULE.bazel` to select source-built
local archives and bindings for its consumer builds.

The Bazel `v8` crate feature selection enables V8's in-process sandbox for
Darwin, Linux, and Windows, including MSVC.

Current pinned versions:

- Rust crate: `v8 = =152.2.0`
- Embedded upstream V8 source for Bazel-produced release builds: `15.2.124.1`

Source builds apply the [M152 fix for CVE-2026-85046](https://github.com/v8/v8/commit/94c34b200ae9caf83d06c167a070746a1c7e825c)
from `patches/v8_array_sort_elements_kind.patch` in both Bazel and the Windows GN
workflows. It changes compiler internals and preserves the `152.2.0` bindings.
Use a patched Codex canary or release archive and its matching binding for local
Cargo validation; the default upstream archive does not contain this backport.

The 152.2.0 candidate still needs published Windows sandbox archives.
Before merging, publish the matching Codex release and replace the old Windows
archive declarations and checksum manifest with the generated 152.2.0 pins.
The new Windows selectors deliberately require those exact-version repositories.

## Updating to a new `v8` release

Use this as the maintainer flow for a version bump:

1. Bump the `v8` crate version and refresh `codex-rs/Cargo.lock`.
2. Update the Bazel versioned inputs in `MODULE.bazel`, then refresh the
   matching checksum manifest and generated checksums as described below.
3. Publish a release-candidate PR and validate that `v8-canary` passes.
4. If the canary is green, publish the release tag and release build.
5. Independently verify the published Codex-built checksum manifests and record
   their SHA-256 digests in
   `third_party/v8/rusty_v8_<version>_release_manifests.sha256`.
6. Once the release build completes, rerun the build on the candidate branch
   and verify that the final artifact builds and tests pass.

When changing the remaining prebuilt `rusty_v8` `http_file` inputs, keep the
checked-in checksum manifest and `MODULE.bazel` in sync:

```bash
python3 .github/scripts/rusty_v8_bazel.py update-module-bazel
python3 .github/scripts/rusty_v8_bazel.py check-module-bazel
```

The commands default to the single `rusty_v8_*` `http_file` version still
present in `MODULE.bazel` and validate every matching entry. CI runs the check
command to block checksum drift.

The consumer-facing selectors are:

- `//third_party/v8:rusty_v8_archive_for_target`
- `//third_party/v8:rusty_v8_binding_for_target`

Published release assets are expected at the tag:

- `rusty-v8-v<crate_version>`

with these raw asset names:

- `librusty_v8_release_<target>.a.gz`
- `src_binding_release_<target>.rs`

During the sandbox rollout, sandbox-enabled assets are published alongside those
current assets on the same tag, with the Rust crate's sandbox feature suffix in
their raw names:

- `librusty_v8_ptrcomp_sandbox_release_<target>.a.gz`
- `rusty_v8_ptrcomp_sandbox_release_<target>.lib.gz` on Windows MSVC
- `src_binding_ptrcomp_sandbox_release_<target>.rs`

The dedicated publishing workflow is `.github/workflows/rusty-v8-release.yml`.
Tagged runs build release artifacts from the Bazel graph itself:

- `//third_party/v8:rusty_v8_release_pair_x86_64_apple_darwin`
- `//third_party/v8:rusty_v8_release_pair_aarch64_apple_darwin`
- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_release_pair_aarch64_unknown_linux_musl`

The same run also builds the matching sandbox pair targets:

- `//third_party/v8:rusty_v8_sandbox_release_pair_x86_64_apple_darwin`
- `//third_party/v8:rusty_v8_sandbox_release_pair_aarch64_apple_darwin`
- `//third_party/v8:rusty_v8_sandbox_release_pair_x86_64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_sandbox_release_pair_aarch64_unknown_linux_gnu`
- `//third_party/v8:rusty_v8_sandbox_release_pair_x86_64_unknown_linux_musl`
- `//third_party/v8:rusty_v8_sandbox_release_pair_aarch64_unknown_linux_musl`

The workflow also builds sandbox-enabled
`x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` archive/binding pairs
from upstream `rusty_v8` source. Those ABI-specific outputs cannot be produced
by Codex's Bazel Windows GNU toolchain.

The Bazel graph pins the same libc++, libc++abi, and llvm-libc source revisions
used by `rusty_v8 v152.2.0`, compiles published artifact targets with
`--config=rusty-v8-upstream-libcxx`, and folds the matching runtime objects into
the final static archive so consumers can link it with the `v8` crate's default
`use_custom_libcxx` feature. The config keeps the object files and the bundled
runtime on Chromium's `std::__Cr` ABI namespace instead of mixing those objects
with the toolchain libc++ default namespace. Bazel consumers use these
source-built targets directly; Cargo release and package builds use the
published copies.

MSVC is not part of the Bazel-produced matrix yet. The repository's current
hermetic Windows C++ platform is `windows-gnullvm`/`x86_64-w64-windows-gnu`, so
it cannot truthfully reproduce upstream's `*-pc-windows-msvc` archives until we
add a real MSVC-targeting C++ toolchain to the Bazel graph.

Release and CI Cargo builds for Darwin and Linux use `RUSTY_V8_ARCHIVE` plus a
downloaded `RUSTY_V8_SRC_BINDING_PATH` to point at those `openai/codex` release
assets directly, using the `rusty-v8-v<crate_version>` release tag.

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version in `codex-rs/Cargo.lock`.
