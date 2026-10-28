# `rusty_v8` Consumer Artifacts

This directory wires the `v8` crate to exact-version Bazel inputs. Bazel
consumers build V8 archives from source and generate matching Rust bindings on
Darwin, GNU/musl Linux, and Windows MSVC/GNU. `MODULE.bazel` wires
`RUSTY_V8_ARCHIVE` and `RUSTY_V8_SRC_BINDING_PATH` to the `@v8_targets` copy of
these consumer-facing selectors:

- `//third_party/v8:rusty_v8_archive_for_target`
- `//third_party/v8:rusty_v8_binding_for_target`

The Bazel `v8` crate and its native source targets enable V8's in-process
sandbox and pointer compression.

Local Cargo builds still use upstream prebuilt `rusty_v8` archives by default.
Selected Cargo CI, release, and package builds override
`RUSTY_V8_ARCHIVE`/`RUSTY_V8_SRC_BINDING_PATH` with Codex release assets.

Source targets enforce the workspace runtime and sandbox/pointer-compression
ABI. Release-pair targets enforce either the legacy or sandbox ABI plus
Chromium's pinned runtime. Target CPU follows the named artifact. These V8
settings are internal; callers do not pass separate V8 feature or libc++ flags.

Current pinned versions:

- Rust crate: `v8 = =152.2.0`
- Embedded upstream V8 source for Bazel builds: `15.2.124.1`

## Updating to a new `v8` release

Use this as the maintainer flow for a version bump:

1. Bump the `v8` crate version and refresh `codex-rs/Cargo.lock`.
2. Update the Bazel versioned inputs in `MODULE.bazel` and the source pins
   in `third_party/v8/dependencies.bzl` and `abseil_repository.bzl` using
   [the source dependency update procedure](#updating-source-dependencies).
   Verify the imported source groups by querying the source archive and generating Torque
   and snapshots for each supported platform. If V8 changes its BUILD section
   layout, the importer fails instead of silently using an old inventory;
   update that boundary and review its fixed feature and CPU selections.
   Then refresh the matching checksum manifest and generated checksums below.
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

For all eight published pairs, verify each published
`rusty_v8_ptrcomp_sandbox_release_<target>.sha256` against the committed
`rusty_v8_<version>_release_manifests.sha256` first. Copy the verified archive
and binding checksums into `rusty_v8_<version>.sha256`, then run these
commands. They validate every matching `http_file` entry, and CI blocks
checksum drift.

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
used by `rusty_v8 v152.2.0`. The named release and sandbox-release pair targets
select those headers and fold the matching runtime objects into the final static
archive so consumers can link it with the `v8` crate's default
`use_custom_libcxx` feature. The profile keeps the object files and the bundled
runtime on Chromium's `std::__Cr` ABI namespace instead of mixing those objects
with the toolchain libc++ default namespace. Selected Cargo release and package
builds use these published archives. The explicit Bazel prebuilt archive targets
remain available; on Linux, they decompress to a private output and weaken
the ten shared `std::logic_error` and `std::runtime_error` constructors and
assignment entry points listed in `gnu_libcxx_shared_exception_symbols.txt`.
libc++ keeps these exception functions in `std::` across inline ABI namespaces;
the toolchain's definitions then take precedence if both runtimes are linked.
Constructors accepting Chromium's `std::__Cr::string` remain strong in the V8
archive. The verified input and the producer's archive are never mutated.
This depends on libc++'s shared exception-object ABI; updates to either libc++
revision must keep the native GNU link/runtime checks passing. The Bazel pair
targets above point directly to source targets so a new release or canary never
depends on an older published copy.

Native Windows x64 and ARM64 hosts can build sandbox archives and matching
Rust bindings with pinned clang-cl and the installed Microsoft SDK using
`//third_party/v8:v8_152_2_0_x86_64_pc_windows_msvc_bazel` and
`//third_party/v8:v8_152_2_0_aarch64_pc_windows_msvc_bazel`.
The producer selectors `//third_party/v8:rusty_v8_native_windows_archive_for_target`
and `//third_party/v8:rusty_v8_native_windows_binding_for_target` select source
outputs directly. Both use the same sandbox, pointer compression, target CPU,
and Chromium libc++ profile. The bindings use V8's compiled headers and defines
so their sizes follow the archive, including CppGC members. Bazel MSVC consumers
use the newly built archive and bindings together.

The owned engine policy preserves the GN MSVC defaults for GC-managed microtask
queues, random Wasm module generation, disabled imminent-deprecation warnings,
and external TypedArray backing stores. Other source targets retain the published
Unix policy, including a 64-byte inline TypedArray threshold. Generated Rust
bindings use that same threshold; never pair a native 64-byte threshold with
GN's size-zero binding constant. These are fixed platform choices, not additional
developer build switches.

Use a native Visual Studio Developer PowerShell with the C++ tools and Windows
SDK available. The SDK supplies headers, libraries, `dumpbin.exe`, and the
assembler. From the repo root, build the archive for that host:

```powershell
bazel build --config=ci-windows-msvc `
  --host_platform=//:local_windows_msvc --platforms=//:local_windows_msvc `
  //third_party/v8:rusty_v8_native_windows_archive_for_target `
  //third_party/v8:rusty_v8_native_windows_binding_for_target
```

Bazel fetches and verifies LLVM 22.1.8 for the host, the same major and patch
version as the libclang that generates bindings. The V8 artifact profile selects
clang-cl, lld-link, llvm-lib and compiler-rt only for its Windows MSVC source
archive and bindings, together with the installed SDK. Bazel consumers now
cross this same boundary automatically. Other C++
dependencies in the same `ci-windows-msvc` build keep the installed MSVC
compiler; no whole-build V8 compiler flag is needed.

This is a native Windows build, not an x64-to-ARM64 or remote Linux build:
V8's snapshot generator must execute on the target architecture. The repository
inputs pinned in #4009 are MSVC/SDK files, not a registered clang-cl toolchain;
making them replace the installed SDK is the separate hermetic MSVC migration.

Release and CI Cargo builds for Darwin and Linux use `RUSTY_V8_ARCHIVE` plus a
downloaded `RUSTY_V8_SRC_BINDING_PATH` to point at those `openai/codex` release
assets directly. We do not use `RUSTY_V8_MIRROR` because the upstream `v8` crate
hardcodes a `v<crate_version>` tag layout, while our artifacts are published
under `rusty-v8-v<crate_version>`.

Do not mix artifacts across crate versions. The archive and binding must match
the exact resolved `v8` crate version in `codex-rs/Cargo.lock`.

## Source dependency ownership

The Darwin, Linux GNU/musl and Windows source targets consume Codex-owned
Abseil, Highway, simdutf, ICU, fast_float, dragonbox and fp16 repositories.
`dependencies.bzl` declares the pinned imports; the `*.BUILD.bazel` overlays
here declare the targets V8 uses. Abseil, Highway and simdutf apply their C++ runtime
settings through `dependency_config.bzl`; ICU defines its settings in its own
overlay.

`engine_repository.bzl` fetches the V8 source pin and installs Codex's engine
BUILD into `@v8`. Source filegroups are read from the pinned upstream BUILD
each fetch, including added, removed or reordered groups. The importer rejects
missing boundaries, malformed or empty sections, and new compiled targets. Only
the two known Dragonbox and FP16 declarations are skipped, since the dependency
overlays already supply them. `v8_binary` is valid V8 syntax, but compiled targets
must be reviewed against the owned engine BUILD on an upgrade.
The upstream `bazel/config` platform and CPU package is reused; only the CPU
setting changes from a CLI flag to an internal setting. `v8_defs.patch` records
that change and the Codex differences to upstream's `bazel/defs.bzl`:
internal ABI/runtime settings, their visibility, libc++ headers/runtime, Windows
compiler and linker settings, and snapshot generation. All engine rules,
including Torque and V8's test configuration helpers, load from that file.
The engine BUILD owns feature policy and dependency targets. ABI, runtime and
CPU settings are selected together by `artifact_profile.bzl` for source and
release-pair targets.

V8's private Abseil is built from the pinned 20250814.1 sources and uses
`codex_v8_lts_20250814` for its supported inline-namespace option. Protobuf
keeps the regular shared Abseil module and its `lts_20250814` namespace.
Both can link into one binary without interposing functions built for different
C++ runtimes. No V8 dependency needs a hook in shared `rules_cc` libraries.

### Updating source dependencies

These are reviewed with each V8 source upgrade. There is no source-pin
autoupdater: the prebuilt checksum commands above and their CI check validate
published archive/binding pairs, not these source revisions. Bazel verifies
the declared download hashes and Git revisions; it does not choose them for us.

Use the V8 tag in the root `MODULE.bazel` `engine_repository(name = "v8")`
(currently `15.2.124.1`) as the input. Read that tag's **unpatched** `DEPS`
and `MODULE.bazel`; `@v8` deliberately does not include upstream's MODULE.
On an upgrade, also review `v8_defs.patch` against the new `bazel/defs.bzl`
and run the V8 source canary. A cleanly applying patch does not by itself
verify new upstream defaults or code-generation behavior.

```bash
v8_tag=15.2.124.1 # Replace with the V8 source tag selected for this upgrade.
gh api "repos/v8/v8/contents/DEPS?ref=$v8_tag" \
  -H 'Accept: application/vnd.github.raw+json' |
  rg -n -A 2 "'third_party/(highway/src|icu|dragonbox/src|fp16/src|fast_float/src|simdutf)'"
gh api "repos/v8/v8/contents/MODULE.bazel?ref=$v8_tag" \
  -H 'Accept: application/vnd.github.raw+json' | rg 'bazel_dep\(name = "abseil-cpp"'
```

- `v8_highway`, `v8_icu`, `v8_dragonbox`, `v8_fp16`: copy the respective
  DEPS commits. For Highway, use that commit in the archive URL and strip prefix.
- `v8_fast_float`: use the DEPS commit. If selecting a release-tag archive,
  verify the tag resolves to that commit, e.g.
  `gh api repos/fastfloat/fast_float/git/ref/tags/v8.2.10 --jq '.object'`
  (dereference annotated tags). Current v8.2.10 equals DEPS commit `34164f5`.
- `v8_simdutf`: DEPS points at a Chromium vendoring repo, not the simdutf
  source repo. Read the metadata at that DEPS commit:

  ```bash
  simdutf_chromium=f7356eed293f8208c40b3c1b344a50bd70971983 # from new DEPS
  curl -fsSL "https://chromium.googlesource.com/chromium/src/third_party/simdutf/+/$simdutf_chromium/README.chromium?format=TEXT" |
    base64 -d
  ```

  Match its `Version:` to the upstream release tag and inspect Chromium's
  local changes before accepting the source form. Chromium vendors amalgamated
  files; we build upstream `src/simdutf.cpp`. At the current pin it reports
  7.7.0, but its `Revision: da645ece` predates the v7.7.0 tag. This is a
  version match, not an assertion that the repositories' commits or bytes match.
- `v8_abseil`: follow `abseil-cpp` in V8's unpatched MODULE, not its Chromium
  DEPS vendor revision. Refresh `abseil_repository.bzl` release archive and
  its guarded inline namespace together; the V8 namespace must stay private.

For each `http_archive`/`download_and_extract` source, hash the exact new
URL, not a different archive of the same commit:

```bash
source_url='https://github.com/google/highway/archive/2607d3b5b0113992fe84d3848859eae13b3b52c1.tar.gz'
curl -fsSL "$source_url" | shasum -a 256
# For an integrity= field (Abseil) instead:
curl -fsSL "$source_url" | openssl dgst -sha256 -binary | openssl base64 -A
```

Record the hex digest in `sha256`, or prefix the base64 digest with `sha256-`
for `integrity`. Review each matching BUILD overlay against its stated source,
then build the V8 generator and source/archive targets on supported platforms.
The regular V8 canary/release checks validate the resulting engine; they do not
prove upstream-version equivalence for a substituted source.
