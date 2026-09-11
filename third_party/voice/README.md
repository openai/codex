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
Final helper linkage and moved-package execution remain separate integration
work; exporting an SDK does not enable voice.

The Rust GStreamer bindings also link GLib's GIO library. Its pkg-config metadata
is included in the SDK, and runtime preparation treats its loader identity as an
explicit dependency root alongside the seven plugins. GIO comes from the same
pinned GLib build; it is not another source package or a GStreamer plugin. Its
transitive imports must satisfy the same private-library and system-import checks.

## Native build recipe

`bazel build //third_party/voice:native_prefix` runs this same recipe with
Bazel-declared source archives, C/C++ compiler and SDK flags, LLVM ranlib,
CMake, Make, pkg-config and Python. It supports matching native macOS and Linux
GNU 2.28 toolchain targets on x64/ARM64; Windows continues to use the standalone
recipe below. The macOS minimum comes from the selected CcToolchain. This manual
target is not part of ordinary wildcard CLI builds.
Mac libffi partial links require Apple's `/usr/bin/ld` from Command Line Tools;
the LLVM compiler and normal final-link flags remain in use.

The root registers the existing pinned built-Make toolchain for all foreign_cc
Make consumers, including pkg-config's bootstrap, instead of selecting host Make.
The recipe still needs the host's standard Unix utilities and `/bin/bash`;
it is not a fully hermetic build.

The target exports raw `prefix.tar` and a `built.json` receipt (the `receipt`
output group). These are build inputs, not a relocatable SDK or runtime package.
No Rust GStreamer build-script annotations or helper loader-path changes are
provided by this target. Upstream build failure logs appear in the action output.

`build_native.py` runs the unmodified upstream build systems in a new output
directory, using the same archives. Unix builds accept repeated `--c-flag=...`,
`--cxx-flag=...`, `--link-flag=...` and optional `--ar`/`--ranlib` inputs.
These overrides are rejected on Windows; ambient flags remain ignored.
Libffi uses compiler response files to preserve literal definitions through
configure, recursive Make and libtool. Response-file paths must not require shell
quoting. Its
Autoconf recipe cannot preserve flags containing whitespace; those are rejected.
Declared archiver and ranlib paths must not require shell quoting for libffi.
Specify the target and existing compiler,
CMake, make, pkg-config and shell paths explicitly. It requires a matching
native host: GNU Linux, macOS, or Windows MSVC, on x64 or ARM64.

On macOS, specify the existing release deployment target with
`--deployment-target`; the host OS version is not an acceptable default.
Windows requires the normal Visual Studio SDK environment, Cygwin GNU make,
bash/cygpath and Automake 1.18's standard `ar-lib` for upstream libffi,
native Windows pkgconf, and `--bootstrap-make` pointing to NMake.
The recipe does not install these build prerequisites or patch upstream sources.
The optional `--windows-build-inputs <json>` argument records an explicit selection
of these tools, the target-specific MSVC assembler, linker, library manager,
inspector, Windows SDK resource/manifest tools, Python, and include/library roots.
The existing private CI driver supplies this input from its provisioned VS/Cygwin
setup. In this mode the recipe checks that named tools resolve to the selected
files, puts MSVC ahead of Cygwin's different `link.exe`, and excludes unrelated
inherited PATH and SDK entries. The exact selection is retained in build receipts.
The JSON uses `schemaVersion: 1`, `target`, `tools` (role to absolute file path),
`systemRoot`, and `INCLUDE`/`LIB` arrays of absolute directories. The CLI tool
arguments must agree with the recorded selection. Without this argument, the
standalone recipe keeps using the normal Visual Studio environment.

This selects already installed inputs; it does not hash their support files,
sandbox the build, supply a Bazel Windows provider, or publish build tools.
Those still require a complete declared compiler/bootstrap closure and approved
public-readable inputs. Existing private Cygwin release assets do not satisfy
public self-build access. No Windows support is disabled to hide that gap.
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

## Prepare Bazel's native build output

`bazel build //third_party/voice:native_runtime` prepares the selected Mac or GNU
Linux prefix archive using the existing platform inspection and preparation code.
It checks the completed build receipt, records the current workspace build commit,
inspects physical libraries, and prepares verified copies. These receipts describe
build inputs and inspection; they are not signatures or release approval.

This manual target requires the same host inspection/signing tools as the standalone
platform preparer. It does not link Rust, change Windows builds, assemble a CLI
package, or enable voice. The prepared runtime is the input to those later steps.

The `native_sdk` output exports the existing development SDK from that same
inspected build. Unix Bazel Rust bindings keep their upstream pkg-config version
checks, restricted to this SDK and the declared pkg-config executable. The
supported `system-deps` search-path override directs linking to `native_link`'s
prepared libraries; its GStreamer linker-flag override removes the SDK's absolute
rpath. Standard CcInfo supplies relative Bazel runpaths and explicit runfiles.
Canonical ABI names and development aliases stay together, including transitive
native dependencies. No host library fallback or version-probe bypass is used.
Plugins and their manifest are exported beside those same canonical libraries;
bindings and plugin imports must resolve to one physical copy of each library.
Cargo still consumes an explicitly supplied SDK; Windows MSVC and final installed
helper loader paths remain separate packaging steps. This does not enable voice.

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
# Windows Bazel inputs and actions

The named `native_prefix_windows_{x86_64,aarch64}`,
`native_runtime_windows_{x86_64,aarch64}` and
`native_link_windows_{x86_64,aarch64}` targets use the existing native recipe,
runtime inspection and SDK export. These targets are MSVC-only. They require
native Windows execution of the matching architecture; they are not cross builds.
The generic Rust-consumer aliases are connected separately, after these inputs.

Provide the complete installed Cygwin/pkgconf repository explicitly. For a public
Windows build, `.github/scripts/setup-voice-windows.ps1 -Target
x86_64-pc-windows-msvc -SnapshotArchive <path>` (under `public/` in
codex-internal) verifies the pinned archive and every input size and SHA-512
digest, then runs the offline installer against its signed metadata. ARM64 uses
`aarch64-pc-windows-msvc` and the same Cygwin x64 tools under emulation. The
installed tool tree stays in the CI temporary directory and is never added to
a Codex package. The upstream mirror's signed metadata changes over time, so
the archived snapshot must be supplied separately. Private CI validates this
public bootstrap against its existing pinned archive. Public release CI obtains
the same hash-pinned build inputs from the public `openai/codex` release named
by `voice-cygwin-snapshot.json`. That release also makes the corresponding
upstream source archives available under `cygwin-build-sources.tar`, with its
own size and SHA-256 pin. These Cygwin tools run only on the build runner;
neither archive is included in the user's Codex package.

The default
`windows_installed_tools` label setting is empty and fails if a Windows action
needs it. This keeps ordinary public dependency queries independent of private
provisioning; it does not silently omit tools from a requested Windows build.
The module does not download that installed tree or accept compiler licenses.

After provisioning, a native PowerShell invocation is:

```powershell
$hostArch = $env:PROCESSOR_ARCHITEW6432
if (-not $hostArch) { $hostArch = $env:PROCESSOR_ARCHITECTURE }
bazel build //third_party/voice:native_link_windows_x86_64 `
  --platforms=//:local_windows_msvc `
  --inject_repository="voice_windows_tools=$env:VOICE_WINDOWS_BAZEL_REPOSITORY" `
  --//third_party/voice:windows_installed_tools=@voice_windows_tools//:tools `
  --action_env="SystemRoot=$env:SystemRoot" --host_action_env="SystemRoot=$env:SystemRoot" `
  --action_env="PROCESSOR_ARCHITECTURE=$hostArch" --host_action_env="PROCESSOR_ARCHITECTURE=$hostArch"
```

Use `native_link_windows_aarch64` on native ARM64 Windows. Existing MSVC license
acceptance requirements still apply. `SystemRoot` must be a fixed action value,
not the inherited form `--action_env=SystemRoot`; Bazel analysis cannot inspect
that inherited value. The action validates the Windows command directory and
constructs its tool search path from declared inputs, not developer PATH entries.

Compiler, SDK, Python and CMake inputs use the existing pinned repositories.
CMake and Cygwin execute as x64 under emulation on ARM64; compiler, SDK tools,
Python and pkgconf match the native architecture. Complete support, include and
library files are declared, and installed entrypoints must match the supplied
target/architecture manifest and belong to that declared tree. This does not
turn caller-provided files into authenticated inputs: provisioning retains that
responsibility. These build tools must never enter shipped Codex packages.

Windows link inputs pair SDK import libraries with the corresponding prepared
DLLs. DLLs, plugins and the receipt remain under the normal native-link runfiles
layout; Windows receives no ELF or Mach-O runtime-search flags. This capability
still needs real native x64/ARM64 Bazel execution and consumer validation before
it establishes complete Windows voice support. The existing direct native recipe
passing on Windows does not prove this new Bazel input and execution path.
