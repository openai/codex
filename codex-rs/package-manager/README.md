# codex-package-manager

`codex-package-manager` is the shared installer used for versioned runtime bundles and other cached artifacts in `codex-rs`.

It owns the generic parts of package installation:

- current-platform detection
- manifest and archive fetches
- checksum and archive-size validation
- archive extraction for `.zip` and `.tar.gz`
- staging and promotion into a versioned cache directory
- cross-process install locking

Package-specific code stays behind the `ManagedPackage` trait.

## Model

The package manager is intentionally small:

1. A `ManagedPackage` implementation describes how to fetch a manifest, choose an archive for a `PackagePlatform`, and load a validated installed package from disk.
2. `PackageManager::resolve_cached()` returns a cached install for the current platform if `load_installed()` succeeds and the version matches.
3. `PackageManager::ensure_installed()` acquires a per-install lock, downloads the archive into a staging directory, extracts it, validates the staged package, and promotes it into the cache.

The default cache root is:

```text
<codex_home>/<default_cache_root_relative>
```

Callers can override that root with `PackageManagerConfig::with_cache_root(...)`.

## ManagedPackage Contract

The trait is small, but the invariants matter:

- `install_dir()` should be unique per package version and platform. If two versions or two platforms share a directory, promotion and cleanup become unsafe.
- `load_installed()` must fully validate the installed package, not just deserialize a manifest. `resolve_cached()` trusts a successful load as a valid cache hit.
- The default `detect_extracted_root()` looks for `manifest.json` at the extraction root or inside a single top-level directory. Override it if your package layout differs.
- `archive_url()` should be derived from manifest data, not recomputed from unrelated caller state, so manifest selection and download stay aligned.

## Consumer Guidance

- If your feature can install on demand, do not gate feature registration on a preinstalled-cache check alone. `resolve_cached()` only answers "is it already present?" while `ensure_installed()` is the bootstrap path.
- Keep cache-root overrides inside your manager/config surface. Separate helpers that reconstruct install paths can drift from `PackageManagerConfig`.
- Prefer surfacing package-specific validation failures from `load_installed()` when debugging. The generic manager treats failed cache loads as cache misses today.

## Security and Extraction Rules

- `.zip` extraction rejects entries that escape the extraction root and preserves Unix executable bits when the archive carries them.
- `.tar.gz` extraction rejects symlinks, hard links, sparse files, device files, and FIFOs. Only regular files and directories are promoted.
- The archive SHA-256 is always verified, and `size_bytes` is enforced when present in the manifest.

## Extending It

Typical usage looks like this:

```rust,ignore
let config = PackageManagerConfig::new(codex_home, MyPackage::new(...));
let manager = PackageManager::new(config);

let package = manager.ensure_installed().await?;
```

In practice, most packages should expose their own small wrapper config/manager types over the generic crate so the rest of the codebase does not depend on `ManagedPackage` details directly.
