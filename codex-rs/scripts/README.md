# Build Scripts

## smart-build.sh

Intelligent build wrapper that automatically detects and fixes common Cargo cache issues.

### Problem

When developing on the codex-rs workspace, incremental compilation can sometimes leave stale artifacts after struct fields are added or modified. This manifests as compilation errors like:

```
error[E0609]: no field `spec` on type `codex_tui::Cli`
```

Even though the field exists in the source code, Cargo's incremental cache references an older version of the struct.

### Solution

The `smart-build.sh` script wraps `cargo build` and:

1. Attempts the build normally
2. If it detects "no field...on type" errors, automatically:
   - Extracts the affected crate name from the error
   - Runs `cargo clean -p <affected-crate>`
   - Retries the build
3. Falls back to full `cargo clean` if crate detection fails

### Usage

```bash
# Build codex binary (default)
./scripts/smart-build.sh

# Pass custom cargo build args
./scripts/smart-build.sh build --release --bin codex
./scripts/smart-build.sh test -p codex-tui
```

### When to Use

- After pulling changes that modify struct definitions
- When encountering "no field" compilation errors
- As a first troubleshooting step for mysterious build failures
- Can be used as a drop-in replacement for `cargo build` in CI/CD

### Limitations

- Only detects "no field...on type" errors (most common cache issue)
- Requires bash shell
- Assumes workspace is in working order (won't fix actual code errors)
