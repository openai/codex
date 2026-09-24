# `rusty_v8` Consumer Artifacts

This directory wires the `v8` crate to exact-version Bazel inputs.
Bazel consumer builds use:

- Codex-published sandbox archive/binding pairs on Darwin and GNU Linux (x64
  and arm64), with checksums pinned from the trusted release manifests
- the existing Codex-published Windows MSVC archives
- source-built V8 archives on musl Linux and Windows GNU

Local Cargo builds still use upstream prebuilt `rusty_v8` archives by default.
Selected Cargo CI, release, and package builds override
`RUSTY_V8_ARCHIVE`/`RUSTY_V8_SRC_BINDING_PATH` with Codex release assets. Bazel
sets those variables independently in `MODULE.bazel`, selecting the pair above
for its consumers. All Bazel compilation modes use the same published V8
release archive on supported platforms.

The Bazel `v8` crate feature selection enables V8's in-process sandbox.
Darwin/GNU consumers select prebuilts only when both the V8 sandbox and pointer
compression settings match the published artifact. For source instrumentation
or custom V8 C++ settings, use `--//:rusty_v8_from_source=true`; the archive
and binding then both come from the source path. The published release archive
cannot incorporate local V8 C++ flags or native debug/sanitizer settings.

Current pinned versions:

- Rust crate: `v8 = =150.4.0`
- Embedded upstream V8 source for Bazel-produced release builds: `15.0.245.2`

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

When changing the prebuilt `rusty_v8` `http_file` inputs, keep the
checked-in checksum manifest and `MODULE.bazel` in sync:

```bash
python3 .github/scripts/rusty_v8_bazel.py update-module-bazel
python3 .github/scripts/rusty_v8_bazel.py check-module-bazel
```

For the Darwin/GNU pairs, verify each published
`rusty_v8_ptrcomp_sandbox_release_<target>.sha256` against the committed
`rusty_v8_<version>_release_manifests.sha256` first. Copy the verified archive
and binding checksums into `rusty_v8_<version>.sha256`, then run these
commands. They validate every matching `http_file` entry, and CI blocks
checksum drift.

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
used by `rusty_v8 v150.4.0`, compiles published artifact targets with
`--config=rusty-v8-upstream-libcxx`, and folds the matching runtime objects into
the final static archive so consumers can link it with the `v8` crate's default
`use_custom_libcxx` feature. The config keeps the object files and the bundled
runtime on Chromium's `std::__Cr` ABI namespace instead of mixing those objects
with the toolchain libc++ default namespace. Bazel consumers use these
published archives for supported Darwin/GNU platforms, as do Cargo release and
package builds. On GNU Linux, Bazel decompresses to a private output and weakens
the ten shared `std::logic_error` and `std::runtime_error` constructors and
assignment entry points listed in `gnu_libcxx_shared_exception_symbols.txt`.
libc++ keeps these exception functions in `std::` across inline ABI namespaces;
the toolchain's definitions then take precedence if both runtimes are linked.
Constructors accepting Chromium's `std::__Cr::string` remain strong in the V8
archive. The verified input and the producer's archive are never mutated.
This depends on libc++'s shared exception-object ABI; updates to either libc++
revision must keep the native GNU link/runtime checks passing. The Bazel pair targets above still point directly to source
targets so a new release or canary never depends on an older published copy.

MSVC is not part of the Bazel-produced matrix yet. The repository's current
hermetic Windows C++ platform is `windows-gnullvm`/`x86_64-w64-windows-gnu`, so
it cannot truthfully reproduce upstream's `*-pc-windows-msvc` archives until we
add a real MSVC-targeting C++ toolchain to the Bazel graph.

Release and CI Cargo builds for Darwin and Linux use `RUSTY_V8_ARCHIVE` plus a
downloaded `RUSTY_V8_SRC_BINDING_PATH` to point at those `openai/codex` release
assets directly. We do not use `RUSTY_V8_MIRROR` because the upstream `v8` crate
hardcodes a `v<crate_version>` tag layout, while our artifacts are published
under `rusty-v8-v<crate_version>`.

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version in `codex-rs/Cargo.lock`.
