# OpenXR SDK Integration Guide

**Based on**: [KhronosGroup/OpenXR-SDK](https://github.com/KhronosGroup/OpenXR-SDK)  
**Last Updated**: 2025-11-06

---

## Overview

This guide explains how to integrate the official OpenXR SDK into Codex for VR/AR support.

---

## Prerequisites

### 1. Install OpenXR SDK

#### Windows

1. Download OpenXR SDK from [GitHub Releases](https://github.com/KhronosGroup/OpenXR-SDK/releases)
2. Extract to `C:\OpenXR-SDK` (or set `OPENXR_SDK_PATH` environment variable)
3. Ensure `openxr_loader.dll` is in PATH or System32

#### Linux

```bash
# Option 1: Build from source
git clone https://github.com/KhronosGroup/OpenXR-SDK.git
cd OpenXR-SDK
mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
make
sudo make install

# Option 2: Use package manager (if available)
# Ubuntu/Debian: Check for openxr packages
```

#### macOS

```bash
# Build from source
git clone https://github.com/KhronosGroup/OpenXR-SDK.git
cd OpenXR-SDK
mkdir build && cd build
cmake -G "Xcode" ..
xcodebuild -configuration Release
```

### 2. Set Environment Variable (Optional)

```bash
# Windows
set OPENXR_SDK_PATH=C:\OpenXR-SDK

# Linux/macOS
export OPENXR_SDK_PATH=/usr/local
```

---

## Building with OpenXR Support

### Enable OpenXR Feature

```bash
# Build with OpenXR support
cargo build --features openxr

# Or in codex-rs/vr-runtime directory
cd codex-rs/vr-runtime
cargo build --features openxr
```

### Verify Installation

```bash
# Check if OpenXR loader is found
cargo build --features openxr -v
# Look for: "OpenXR loader found at: ..."
```

---

## Implementation Details

### Build Script

The `build.rs` script:
1. Searches for OpenXR SDK installation
2. Configures linker paths
3. Generates Rust bindings using `bindgen` (if feature enabled)

### Bindings Generation

Bindings are generated from `openxr.h` header file:
- Types: `XrInstance`, `XrSession`, `XrSystemId`, etc.
- Functions: `xrCreateInstance`, `xrGetSystemProperties`, etc.
- Constants: `XR_SUCCESS`, `XR_ERROR_*`, etc.

### Best Practices Implementation

Following [OpenXR Best Practices](https://fredemmott.com/blog/2024/11/25/best-practices-for-openxr-api-layers.html):

1. ✅ **HKLM Registry**: Use `HKEY_LOCAL_MACHINE` for OpenXR registry
2. ✅ **Graceful Degradation**: Handle errors without crashing
3. ✅ **Extension Handling**: Attempt to enable, handle `XR_ERROR_EXTENSION_NOT_PRESENT`
4. ✅ **Runtime Detection**: Detect and support multiple runtimes

---

## Usage

### Basic Example

```rust
use codex_vr_runtime::VrRuntime;

// Create VR runtime
let runtime = VrRuntime::new()?;

// Get device information
let device_info = runtime.get_device_info()?;
println!("VR Device: {}", device_info.name);

// Get statistics
let stats = runtime.get_device_stats().await?;
println!("FPS: {:.1}, Latency: {:.1}ms", stats.fps, stats.latency_ms);
```

### Check Availability

```rust
if codex_vr_runtime::VrRuntime::is_available() {
    println!("OpenXR is available");
} else {
    println!("OpenXR not available - check SDK installation");
}
```

---

## Troubleshooting

### OpenXR Loader Not Found

**Windows**:
- Ensure `openxr_loader.dll` is in System32 or PATH
- Check registry: `HKEY_LOCAL_MACHINE\SOFTWARE\Khronos\OpenXR\1\ActiveRuntime`

**Linux**:
- Check `/usr/lib/libopenxr_loader.so` or `/usr/local/lib/libopenxr_loader.so`
- Verify with: `ldconfig -p | grep openxr`

**macOS**:
- Check `/usr/local/lib/libopenxr_loader.dylib`
- Verify with: `otool -L /usr/local/lib/libopenxr_loader.dylib`

### Build Errors

1. **bindgen not found**: Install `bindgen` dependencies
   ```bash
   # Windows: Install LLVM
   # Linux: sudo apt-get install llvm-dev libclang-dev
   # macOS: xcode-select --install
   ```

2. **Header not found**: Set `OPENXR_SDK_PATH` environment variable

3. **Linker errors**: Ensure OpenXR SDK is properly installed

---

## Next Steps

1. **Complete Implementation**: Implement actual `xrCreateInstance` calls
2. **Device Detection**: Implement `xrGetSystemProperties` for device info
3. **Session Management**: Implement `xrCreateSession` for VR sessions
4. **Frame Rendering**: Implement frame loop with `xrWaitFrame` / `xrBeginFrame` / `xrEndFrame`

---

## References

- [OpenXR SDK Repository](https://github.com/KhronosGroup/OpenXR-SDK)
- [OpenXR Specification](https://www.khronos.org/openxr/)
- [OpenXR Best Practices](https://fredemmott.com/blog/2024/11/25/best-practices-for-openxr-api-layers.html)
- [OpenXR Loader Documentation](https://github.com/KhronosGroup/OpenXR-SDK-Source)

---

## Notes

- OpenXR SDK integration requires C/C++ build tools
- Bindings are generated at build time using `bindgen`
- The loader must be available at runtime (not just build time)
- Best practices ensure compatibility with multiple runtimes and games











