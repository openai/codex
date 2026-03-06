# codex-artifacts

Runtime and process-management helpers for Codex artifact generation.

This crate has two main responsibilities:

- locating, validating, and optionally downloading the pinned artifact runtime
- spawning the artifact build or render command against that runtime

## Module layout

- `src/client.rs`
  Runs build and render commands once a runtime has been resolved.
- `src/runtime/manager.rs`
  Defines the release locator and the package-manager-backed runtime installer.
- `src/runtime/installed.rs`
  Loads an extracted runtime from disk and validates its manifest and entrypoints.
- `src/runtime/js_runtime.rs`
  Chooses the JavaScript executable to use for artifact execution.
- `src/runtime/manifest.rs`
  Manifest types for release metadata and extracted runtimes.
- `src/runtime/error.rs`
  Public runtime-loading and installation errors.
- `src/tests.rs`
  Crate-level tests that exercise the public API and integration seams.

## Public API

- `ArtifactRuntimeManager`
  Resolves or installs a runtime package into `~/.codex/packages/artifacts/...`.
- `load_cached_runtime`
  Reads a previously installed runtime from a caller-provided cache root without attempting a download.
- `is_js_runtime_available`
  Checks whether artifact execution is possible with either a cached runtime or a host JS runtime.
- `ArtifactsClient`
  Executes artifact build or render requests using either a managed or preinstalled runtime.
