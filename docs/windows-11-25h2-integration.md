# Windows 11 25H2 Integration Guide

**Version**: 2.0.0  
**Last Updated**: 2025-11-06  
**Requirements**: Windows 11 Build 26100+ (25H2)

---

## Overview

Codex now fully supports Windows 11 25H2 with enhanced GPU acceleration, DirectML 1.13/1.14, and Copilot+ PC NPU detection.

### New Features

- ✅ DirectML 1.13/1.14 support
- ✅ WDDM 3.2 GPU scheduling optimization
- ✅ Copilot+ PC NPU detection (experimental)
- ✅ Enhanced driver compatibility checking
- ✅ Intel Arc B580 fallback support

---

## Installation

### Prerequisites

1. **Windows 11 Build 26100+** (25H2)
   ```powershell
   winver
   # Check: Build 26100.xxxx
   ```

2. **Latest GPU Drivers**
   - NVIDIA: Latest driver (check for safeguard holds)
   - Intel Arc: 31.0.101.8132+ (or rollback if issues persist)
   - AMD: Latest driver

3. **Codex Installation**
   ```powershell
   codex --version
   # Should show: codex-cli 2.0.0
   ```

---

## Usage

### Basic Windows AI Acceleration

```bash
# Use Windows AI API
codex --use-windows-ai "Analyze this codebase"

# With kernel driver acceleration
codex --use-windows-ai --kernel-accelerated "Implement feature X"
```

### Check DirectML Version

```bash
codex --version
# Look for: "DirectML 1.13" or "DirectML 1.14"
```

### Check NPU Availability

```bash
codex --check-npu
# Output: "NPU (Copilot+ PC) detected" or "NPU not available"
```

---

## Known Issues & Solutions

### Intel Arc B580 Performance Issues

**Problem**: Performance degradation on Windows 11 25H2

**Solutions**:
1. Update to driver 31.0.101.8132+
2. If issues persist, rollback to previous driver
3. Codex automatically detects and applies fallback

### NVIDIA Driver Compatibility

**Problem**: Some NVIDIA drivers have safeguard holds on Windows 11 25H2

**Solutions**:
1. Update to latest NVIDIA driver
2. Check NVIDIA website for compatibility
3. Codex warns if incompatible driver detected

### Hyper-V GPU-P Issues

**Problem**: VM freezes after KB5062553 update

**Solutions**:
1. Rollback KB5062553 update
2. Wait for Microsoft fix
3. Use native Windows 11 25H2 instead of VM

---

## DirectML 1.13/1.14 Features

### New Capabilities

- **Enhanced GPU scheduling** (WDDM 3.2+)
- **NPU support** (Copilot+ PC)
- **Improved memory management**
- **Better multi-GPU support**

### Version Detection

Codex automatically detects DirectML version:
- Build 26100-26199: DirectML 1.13
- Build 26200+: DirectML 1.14

---

## Copilot+ PC NPU Support

### Detection

Codex automatically detects NPU availability via:
1. Registry check
2. DirectML device enumeration (future)

### Usage

NPU is automatically used when available for:
- AI inference workloads
- Neural network operations
- Machine learning tasks

---

## Performance Optimization

### WDDM 3.2 GPU Scheduling

Codex automatically enables GPU-aware thread scheduling on WDDM 3.2+ systems.

**Benefits**:
- Reduced latency
- Better GPU utilization
- Improved multi-tasking

### Kernel Driver Acceleration

For maximum performance, install the AI kernel driver:

```powershell
cd kernel-extensions\windows
.\install-driver.ps1
```

**Warning**: Requires administrator privileges and test signing mode.

---

## Troubleshooting

### GPU Not Detected

1. Check Windows version: `winver`
2. Verify GPU driver is up to date
3. Check DirectML availability: `codex --check-directml`

### Performance Issues

1. Check driver compatibility
2. Verify WDDM version (should be 3.2+)
3. Check for known issues (Intel Arc B580, etc.)

### NPU Not Detected

1. Verify Copilot+ PC requirements
2. Check Windows 11 25H2 installation
3. NPU detection is experimental and may require DirectML SDK update

---

## References

- [Windows 11 25H2 Release Notes](https://support.microsoft.com/windows/windows-11-version-25h2-update-history)
- [DirectML Documentation](https://learn.microsoft.com/windows/ai/directml/)
- [NVIDIA Driver Compatibility](https://www.nvidia.com/drivers)
- [Intel Arc Driver Updates](https://www.intel.com/content/www/us/en/download-center/home.html)

---

## Support

For issues specific to Windows 11 25H2 integration:
- Check known issues section above
- Review driver compatibility
- Update to latest Codex version











