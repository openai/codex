# Native voice source inputs

This stage pins and prepares sources for a privately bundled, GStreamer-based
audio runtime, including its native dependencies and build tools. It does not
compile native libraries, link them into Codex or enable voice.

`sources.json` records the versions, URLs and SHA-256 digests of 11 archives:

| Purpose | Sources |
| --- | --- |
| GStreamer framework and plugin sources | `gstreamer`, `gst-plugins-base`, `gst-plugins-good` |
| Supporting native libraries | `glib`, `libffi`, `pcre2`, `zlib`, `proxy-libintl` |
| Audio codec | `opus` |
| Build tools, not runtime libraries | `meson`, `ninja` |

GLib also includes `gvdb` in its archive; it is recorded without a separate fetch.
These inputs do not include the complete platform toolchain for native builds.

Bazel uses standard `http_archive` rules to fetch, verify, unpack and cache the
archives from that manifest. Run from the repository root:

```sh
bazel build //third_party/voice:sources
```

For offline preparation without Bazel, use Python 3.12 or newer and an existing
archive directory whose filenames match the manifest:

```sh
python3 third_party/voice/prepare_sources.py --archives /path/to/archives --output /path/to/new-sources
python3 -m unittest discover -s third_party/voice -p 'test_*.py'
```

The adapter verifies archive digests and bounds, then extracts with Python's
`tarfile` data filter. It preserves links where supported and copies their archive
targets when link creation is unavailable, including on Windows. It refuses an
existing output directory and cleans up incomplete output. `prepared.json` records
successful preparation, not the integrity of later edits to the extracted tree.

Ordinary CLI builds do not run either path. The Bazel `:sources` target is
`manual`, so wildcard builds do not fetch these archives. `:source_inputs`
exports the manifest and adapter for standalone consumers; `:sources` exposes
extracted archives with Bazel build metadata. Neither target compiles libraries.

Checksums establish input identity, not security or license approval. Native
compilation, final Cargo/Bazel linking, installed packages, minimum OS support
and duplex audio validation remain separate stages. These inputs do not establish
a shared Opus build with Rust consumers or a reduced dependency count.

Rust `opus` 0.4.0 is available through Socket. Adding Rust transport dependencies
and establishing a shared Opus build remain separate integration work.

## Native development inputs

The private native CI job also emits `sdk.tar.gz` from the same inspected prefix.
It contains headers (including the target's GLib configuration), development
library names, import/static libraries and pkg-config metadata. `sdk.json`
records the target, source commit, pinned manifest and every exported file hash.
Shared-library bytes must match the native inspection receipt; other development
files are hashed during export. This is provenance, not an authenticity check.

Meson generates relocatable pkg-config metadata using its standard option.
Consumers must restrict `PKG_CONFIG_LIBDIR` to the SDK, clear `PKG_CONFIG_PATH`,
and use `pkg-config --define-prefix` for libffi/PCRE2/zlib metadata too. Only the
required native metadata is exported; capture Opus uses `opusic-sys`. Native
library loader paths are not changed by SDK export. These build inputs do not replace the
separate runtime projection and are never copied into users' Codex packages.
Cargo/Bazel native providers, final helper linkage and moved-package execution
remain separate integration work; exporting an SDK does not enable voice.

## Native build recipe

`build_native.py` runs the unmodified upstream build systems in a new output
directory, using the same archives. Specify the target and existing compiler,
CMake, make, pkg-config and shell paths explicitly. It requires a matching
native host: GNU Linux, macOS, or Windows MSVC, on x64 or ARM64.

On macOS, specify the existing release deployment target with
`--deployment-target`; the host OS version is not an acceptable default.
Windows requires the normal Visual Studio SDK environment, Cygwin GNU make,
bash/cygpath and Automake 1.18's standard `ar-lib` for upstream libffi,
native Windows pkgconf, and `--bootstrap-make` pointing to NMake.
The recipe does not install these build prerequisites or patch upstream sources.
The private CI bootstrap verifies the official Cygwin installer and native pkgconf
MSI hashes before use. It also verifies a retained Cygwin package snapshot against
pinned archive and member hashes before installing it offline using signed
metadata. The installed package/version set must exactly match the snapshot
manifest.
The MSI is administratively extracted into job storage without a system install.
Cygwin runs under x64 emulation on ARM64; the compiler probes and emitted DLLs
must still match the real native target. Native pkgconf relocates libffi's POSIX
prefix metadata; CI rejects residual Cygwin paths. These are build prerequisites,
not shipped runtime components or evidence of working voice.

CMake libraries use relative install runpaths (`$ORIGIN` on Linux and
`@loader_path` on Mac), with `@rpath` install names on Mac. Linux Meson links
use `$ORIGIN:$ORIGIN/..`, matching libraries in `lib/` and plugins in
`lib/gstreamer-1.0/`. Mac Meson and libffi still need packaging-time fixups;
these options do not make every Mac library relocatable at installation.

Outputs are under `prefix/`, build tools under `tools/`, and logs beside them.
`build-state.json` records completed commands and failures; `built.json` exists
only when every build/install command succeeds. Failed builds retain their logs
and must use a new output directory on retry. CMake compiler-identification logs
and the recorded tool/configuration inputs remain part of the build provenance.

The recipe disables optional plugins and Meson fallback dependency resolution,
with pkg-config restricted to this prefix. Only system ABI libraries/frameworks
may remain external; runtime closure inspection must verify that independently.
`//third_party/voice:build_inputs` exposes the recipe and source inputs to Bazel.
Neither this filegroup nor a successful prefix build proves final Cargo/Bazel
linkage, safe private runtime loading, or an installed voice-capable Codex package.

## Private macOS runtime projection

`macos_runtime.py --prefix <extracted-prefix> --receipts <native-ci-receipts>
--target <macOS-triple> --output <fresh-directory>` verifies the native receipt,
source manifest, per-file digests and Mach-O architecture before projecting the
seven explicit plugins and their declared library dependencies. It removes SDK
aliases and build-machine runpaths, rewrites private imports relative to each
loader, and regenerates only development ad-hoc signatures. Inputs are untouched.
The output must be new and outside the input directories; failures remove only
that new output. `runtime.json` records source and transformed file identities.
Xcode's `xcrun llvm-objdump` inspects Mach-O headers and load commands; the
preparer reads its output and enforces the package dependency policy rather than
decoding binary structures itself. `install_name_tool` still rewrites paths.
`runtime.py` owns shared receipt checks, dependency selection, verified copying,
and cleanup; each platform owns its binary format and loader changes. Output
containment uses filesystem identity, and copied bytes are checked again before
transformation so input changes cannot silently invalidate the source receipt.

This is a development-only payload, not a signed distribution package or proof of
audio behavior. Dynamic-only dependencies, native helper linkage, LGPL notices,
production signing/notarization, Windows/Linux loading and security approval remain
separate requirements. No microphone, device, plugin scanner or backend is started
by projection. Run its native relocation tests on macOS with Python 3.12 or newer.

## Private GNU Linux runtime preparation

`linux_runtime.py` takes the same prefix, receipts, target and output arguments.
It reads bounded ELF64 headers, segments and dynamic tables directly and accepts
x64/ARM64 GNU Linux libraries. The shared Python coordinator selects the seven
plugins and their declared dependencies without changing their bytes. The native
build must have emitted package-relative runpaths; older absolute paths are
rejected with a rebuild instruction. Output preserves `lib/gstreamer-1.0/` so
those relative paths remain valid. Loader audit/filter dependencies and
path-bearing imports are rejected. Native tests require Python 3.12, a C compiler
and `patchelf`; the latter constructs malformed inputs and is not needed during
preparation or shipped in the runtime. The output is development-only, uses the
host glibc, and does not establish musl or minimum-glibc support, dynamic-only
dependency closure, helper loading policy or working voice.

## Private Windows runtime preparation

`windows_runtime.py` takes the same arguments for x64/ARM64 MSVC build prefixes.
MSVC's existing `dumpbin` reads PE headers, dependencies and exports; the Python
adapter applies package policy without walking binary structures.
It checks bounded PE32+ import tables, uses case-insensitive DLL identities, and
copies the seven plugins and their declared dependencies into one private `bin/`
directory without changing DLL bytes. Delayed imports, managed DLLs and forwarded
exports are unsupported and rejected. Native tests require MSVC and Python 3.12;
they load the moved DLLs using only the DLL directory and System32 search flags.
This development payload expects the Windows Universal CRT and the matching
Microsoft Visual C++ runtime (`VCRUNTIME140.dll`) already installed. The latter
is not a guaranteed OS component. Release redistribution/licensing, Authenticode
policy and actual helper loading remain separate requirements; this script does
not install or redistribute Microsoft runtime files or enable voice.
