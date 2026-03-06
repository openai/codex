# Termux Scripts

Scripts in this folder are for validating and developing Termux support in the `exomind-codex` local project branch.

## `build-safe.sh`

Builds `codex-cli` and `codex-exec` for Android/Termux with low-memory defaults:

- computes a conservative `CARGO_BUILD_JOBS` from current `MemAvailable`
- forces `release` overrides to reduce linker memory spikes:
  - `CARGO_PROFILE_RELEASE_LTO=off`
  - `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
  - `CARGO_PROFILE_RELEASE_DEBUG=0`

Usage:

```sh
scripts/termux/build-safe.sh
```

## `check-android-target.sh`

Runs Android ARM64 preflight checks and cargo compile checks for key crates.

Usage:

```sh
scripts/termux/check-android-target.sh
```
