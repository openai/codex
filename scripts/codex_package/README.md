# Codex package builder

This package contains the implementation behind `scripts/build_codex_package.py`.
The top-level script is the stable executable entry point; these modules keep the
package-building logic split by responsibility.

Run the builder through `just`:

```bash
just assemble-codex-package --help
just assemble-codex-package --variant codex-app-server
just assemble-codex-package --target x86_64-unknown-linux-gnu
```

The builder creates a canonical Codex package directory:

```text
.
├── codex-package.json
├── bin
│   ├── <entrypoint>[.exe]
│   └── codex-code-mode-host[.exe]
├── codex-resources
│   ├── bwrap                             # Linux only
│   ├── codex-command-runner.exe          # Windows only
│   └── codex-windows-sandbox-setup.exe   # Windows only
└── codex-path
    └── rg[.exe]
```

The package directory is the primary artifact. Archive formats such as
`.tar.gz`, `.tar.zst`, and `.zip` are serializations of that directory.

If `--target` is omitted, the builder uses the release target for the current
host platform. On Linux, that default is a musl target to match Codex release
artifacts; pass a GNU Linux target explicitly for native glibc local builds. If
`--package-dir` is omitted, the builder creates a new temporary directory and
prints its path after the package is built.

The `--variant` flag selects the package entrypoint. Supported variants are
`codex` and `codex-app-server`. The `--package-version` flag sets the version in
`codex-package.json`; it defaults to `[workspace.package].version` in
`codex-rs/Cargo.toml`.

## Source-built artifacts

Artifacts built from this repository are built by the package builder in one
grouped `cargo build` command per package when they are needed and no prebuilt
override was provided:

- all targets: the selected entrypoint, unless `--entrypoint-bin` is provided
- all targets: `codex-code-mode-host`, unless `--code-mode-host-bin` is provided
- Linux targets: `bwrap`, unless `--bwrap-bin` is provided
- Windows targets: `codex-command-runner` and `codex-windows-sandbox-setup`,
  unless the corresponding prebuilt helper flags are provided

The default cargo profile is `dev-small` because local iteration should favor
fast, small builds. Release jobs should pass `--cargo-profile release` and an
explicit target. Release jobs that already built and signed/notarized the
entrypoint should pass `--entrypoint-bin` so the package contains that exact
binary instead of rebuilding it.

Release jobs should likewise pass `--code-mode-host-bin` so the package contains
the signed host executable beside the signed entrypoint.

On Linux and macOS, the builder strips symbols from the **package copies** of
source-built release-profile entrypoint and code-mode host binaries. Cargo outputs
retain their symbols for debugging. The default `--strip auto` also preserves
development/profiling builds and prebuilt inputs
byte-for-byte, including signatures. Use `--strip all` to also strip prebuilt
entrypoint/host copies, or `--strip none` to keep symbols in every package copy.
Strip before production signing; `--strip all` is not appropriate for inputs
whose release signatures must be preserved. Windows MSVC symbols are separate
PDB files and are not included in the package.

macOS uses `xcrun strip -S -x`, matching the release pipeline and preserving
executable ad-hoc signatures. Linux uses `llvm-strip`, a target-prefixed GNU
strip, or native GNU strip when the host architecture matches. For cross builds,
install `llvm-strip` or pass `--strip-tool /path/to/target-strip`. Missing tools
or strip failures fail the build; use `--strip none` to deliberately opt out.
Third-party resources are copied unchanged. In particular, never strip `bwrap`
after its integrity digest has been embedded in Codex.

Release jobs that already built package resource binaries should also pass the
corresponding resource flags: `--bwrap-bin` for Linux packages, and
`--codex-command-runner-bin` plus `--codex-windows-sandbox-setup-bin` for
Windows packages. This keeps package archive creation as a pure staging step
after signing instead of rebuilding resources.

When the builder source-builds an entrypoint for a Darwin or Linux target, it
downloads and verifies the matching Codex-built V8 release pair before invoking
Cargo and sets `RUSTY_V8_ARCHIVE` plus `RUSTY_V8_SRC_BINDING_PATH` for that
build. Windows targets keep Cargo's release-build MSVC artifact path. Explicit
overrides remain authoritative when both variables are already set. Set
`V8_FROM_SOURCE=1` to leave the build with the `v8` crate source-build path.

`rg` is not built from this repository, so the builder fetches it from the
DotSlash manifest at `scripts/codex_package/rg`. Downloaded archives are cached
under `$TMPDIR/codex-package/<target>-rg` and are reused only after the recorded
size and SHA-256 digest have been verified. Pass `--rg-bin` to use a local
ripgrep executable instead.
