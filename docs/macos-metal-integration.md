# macOS Metal Integration Guide

**Version**: 2.0.0  
**Last Updated**: 2025-11-06  
**Requirements**: macOS with Apple Silicon (M1/M2/M3)

---

## Overview

Codex now supports macOS Metal API for GPU acceleration on Apple Silicon chips, providing native performance optimization.

### Features

- ✅ Metal 3 support
- ✅ Apple Silicon chip detection (M1/M2/M3 series)
- ✅ Metal Performance Shaders (MPS) support
- ✅ Neural Engine detection
- ✅ Automatic chip optimization

---

## Installation

### Prerequisites

1. **macOS with Apple Silicon**
   - M1, M1 Pro, M1 Max, M1 Ultra
   - M2, M2 Pro, M2 Max, M2 Ultra
   - M3, M3 Pro, M3 Max, M3 Ultra

2. **Codex Installation**
   ```bash
   codex --version
   # Should show: codex-cli 2.0.0
   ```

3. **Enable Metal Feature**
   ```bash
   cargo build --features metal
   ```

---

## Usage

### Basic Metal Acceleration

```bash
# Use Metal acceleration (automatic on macOS)
codex "Analyze this codebase"
# Metal is automatically selected when available

# Explicitly check Metal availability
codex --check-metal
```

### Check Chip Information

```bash
codex --chip-info
# Output: "Apple M3 (10 CPU cores, 10 GPU cores, 16 Neural Engine cores)"
```

---

## Chip Detection

Codex automatically detects Apple Silicon chips using `sysctl`:

### Supported Chips

| Chip | CPU Cores | GPU Cores | Neural Engine |
|------|-----------|-----------|---------------|
| M1 | 8 | 8 | 16 |
| M1 Pro | 10 | 14 | 16 |
| M1 Max | 10 | 32 | 16 |
| M1 Ultra | 20 | 64 | 32 |
| M2 | 8 | 10 | 16 |
| M2 Pro | 12 | 19 | 16 |
| M2 Max | 12 | 38 | 16 |
| M2 Ultra | 24 | 76 | 32 |
| M3 | 8 | 10 | 16 |
| M3 Pro | 12 | 18 | 16 |
| M3 Max | 16 | 40 | 16 |
| M3 Ultra | 32 | 80 | 32 |

---

## Metal Performance Shaders (MPS)

MPS provides optimized operations for:
- Matrix multiplication
- Neural network inference
- Image processing
- Signal processing

### Usage

MPS is automatically enabled when available:

```bash
# MPS is used automatically for supported operations
codex --use-metal "Process large dataset"
```

---

## Performance Optimization

### Chip-Specific Optimizations

Codex automatically applies optimizations based on detected chip:

- **M1/M2**: Standard optimizations
- **M1 Pro/M2 Pro**: Enhanced multi-core utilization
- **M1 Max/M2 Max/M3 Max**: Maximum GPU utilization
- **M1 Ultra/M2 Ultra/M3 Ultra**: Full chip utilization

### Memory Management

Metal automatically manages GPU memory:
- Unified memory architecture
- Automatic memory pooling
- Efficient memory transfers

---

## Troubleshooting

### Metal Not Available

1. Verify macOS version (should be recent)
2. Check chip detection: `codex --chip-info`
3. Ensure Metal feature is enabled: `--features metal`

### Performance Issues

1. Check chip type (M1 vs M3 performance differs)
2. Verify MPS availability
3. Check GPU utilization

### Unknown Chip Detected

If chip is detected as "Unknown":
1. Check `sysctl machdep.cpu.brand_string`
2. Report chip type for support
3. Codex will use default optimizations

---

## API Usage

### Rust Code Example

```rust
use codex_metal_runtime::MetalRuntime;

let runtime = MetalRuntime::new()?;
let chip_info = runtime.get_chip_info()?;
println!("Chip: {}", chip_info.chip_type.label());

if runtime.is_mps_available() {
    println!("MPS available");
}

let stats = runtime.get_gpu_stats().await?;
println!("GPU Utilization: {:.1}%", stats.utilization);
```

---

## References

- [Metal Documentation](https://developer.apple.com/metal/)
- [Metal Performance Shaders](https://developer.apple.com/documentation/metalperformanceshaders)
- [Apple Silicon Architecture](https://www.apple.com/mac/)

---

## Support

For Metal-specific issues:
- Check chip detection
- Verify macOS version
- Review Metal API documentation











