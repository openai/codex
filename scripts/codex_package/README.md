# Codex package builder

This package contains the implementation behind `scripts/build_codex_package.py`.
The top-level script is the stable executable entry point; these modules keep the
package-building logic split by responsibility.

The builder creates a canonical Codex package directory:

```text
.
├── codex-package.json
├── bin
│   └── codex[.exe]
├── codex-resources
│   ├── bwrap                             # Linux only
│   ├── codex-command-runner.exe          # Windows only
│   └── codex-windows-sandbox-setup.exe   # Windows only
└── codex-path
    └── rg[.exe]
```

The package directory is the primary artifact. Archive formats such as
`.tar.gz`, `.tar.zst`, and `.zip` are serializations of that directory.

## Source-built artifacts

Artifacts built from this repository are always built by the package builder in
one grouped `cargo build` command per package:

- all targets: `codex`
- Linux targets: `bwrap`
- Windows targets: `codex-command-runner` and `codex-windows-sandbox-setup`

The default cargo profile is `dev-small` because local iteration should favor
fast, small builds. Release jobs should pass `--cargo-profile release`.

`rg` is not built from this repository, so the builder fetches it from the
DotSlash manifest at `codex-cli/bin/rg`. Downloaded archives are cached under
`$TMPDIR/codex-package/<target>-rg` and are reused only after the recorded size
and SHA-256 digest have been verified. Pass `--rg-bin` to use a local ripgrep
executable instead.
