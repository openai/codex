# Pre-Main Hardening Benchmark

This benchmark demonstrates the impact of the pre-main hardening routine on Linux systems that rely on `LD_LIBRARY_PATH` for custom library loading.

## Problem

The current pre-main hardening in release builds unconditionally strips all `LD_*` environment variables before `main()` runs. While this provides security against library injection attacks, it breaks legitimate workflows:

- **Conda environments**: MKL/OpenBLAS loaded via `LD_LIBRARY_PATH`
- **CUDA workloads**: `libcublas`, `libcudart` often require custom paths
- **Enterprise deployments**: Oracle clients, custom database drivers
- **Development environments**: Custom-compiled libraries

## Test Setup

```bash
# Create isolated library directory
mkdir -p /tmp/isolated_libs
cp benchmark/lib/libfastmath.so /tmp/isolated_libs/libcustom.so

# Set LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/tmp/isolated_libs
```

## Expected Results

| Build | LD_LIBRARY_PATH | Library Load | Status |
|-------|-----------------|--------------|--------|
| Debug | Preserved | Success | PASS |
| Release (current) | STRIPPED | Fail | BROKEN |
| Release (with fix) | Preserved | Success | PASS |
| Release + CODEX_SECURE_MODE=1 | STRIPPED | Fail | Expected |

## Running the Benchmark

```bash
# Build release binary
cargo build --release -p codex-cli

# Run comparison (requires both fixed and unfixed binaries)
python3 benchmark/run_codex_comparison.py \
    --cod3x-bin target/release/codex \
    --stock-bin /path/to/unfixed/codex
```

## Fix

The fix makes pre-main hardening opt-in via the `CODEX_SECURE_MODE=1` environment variable:

```rust
#[ctor::ctor]
#[cfg(not(debug_assertions))]
fn pre_main_hardening() {
    let enabled = matches!(std::env::var("CODEX_SECURE_MODE").as_deref(), Ok("1"));
    if enabled {
        codex_process_hardening::pre_main_hardening();
    }
}
```

Users who need maximum security can still enable it explicitly.

## References

- [Ghosts in the Codex Machine](https://docs.google.com/document/d/1fDJc1e0itJdh0MXMFJtkRiBcxGEFtye6Xc6Ui7eMX4o) - OpenAI's investigation into Codex performance
