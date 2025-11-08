# VR/AR/VRChat Integration Guide

**Version**: 2.0.0  
**Last Updated**: 2025-11-06

---

## Overview

Codex provides VR/AR runtime abstraction and VRChat world optimization tools for immersive development experiences.

### Features

- ✅ OpenXR runtime abstraction
- ✅ Quest/Vive/Index device support
- ✅ Virtual Desktop optimization
- ✅ VRChat world optimization tools
- ✅ Udon 2 support

---

## VR Runtime

### Supported Devices

| Device | Status | Features |
|--------|--------|----------|
| Meta Quest | ✅ | WebXR, Hand tracking |
| Meta Quest 2 | ✅ | 90Hz, Passthrough |
| Meta Quest 3 | ✅ | 120Hz, Enhanced passthrough |
| HTC Vive | ✅ | SteamVR |
| HTC Vive Pro | ✅ | Enhanced tracking |
| Valve Index | ✅ | 144Hz, Finger tracking |

### Installation

1. **Install OpenXR SDK** (required for native VR support)
   ```bash
   # Download from: https://www.khronos.org/openxr/
   # Or use package manager:
   # Windows: choco install openxr
   # Linux: apt-get install libopenxr-dev
   ```

2. **Enable OpenXR Feature**
   ```bash
   cargo build --features openxr
   ```

3. **Verify VR Detection**
   ```bash
   codex --check-vr
   ```

---

## Virtual Desktop Optimization

### Features

- Low latency streaming
- Bandwidth monitoring
- Quality adjustment
- Network optimization

### Usage

```bash
# Optimize for Virtual Desktop
codex --optimize-virtual-desktop

# Monitor bandwidth
codex --monitor-vr-bandwidth
```

### Quality Settings

| Quality | Resolution | Bitrate | Latency |
|---------|-----------|---------|---------|
| Low | 1080p | 50 Mbps | <30ms |
| Medium | 1440p | 100 Mbps | <25ms |
| High | 2160p | 150 Mbps | <20ms |
| Ultra | 4K | 200 Mbps | <15ms |

---

## VRChat World Optimization

### Optimization Tools

Codex provides tools for optimizing VRChat worlds:

#### Material Atlas

Combines multiple textures into a single atlas to reduce draw calls:

```bash
codex vrchat-optimize --material-atlas world_path
```

#### Post-Processing Optimization

Minimizes post-processing effects for better performance:

```bash
codex vrchat-optimize --minimize-post-processing world_path
```

#### Network Object Optimization

Optimizes network synchronization:

```bash
codex vrchat-optimize --network-objects world_path
```

#### Full Optimization

Run all optimizations:

```bash
codex vrchat-optimize --all world_path
```

### Udon 2 Support

For Udon 2 compatible worlds:

```bash
codex vrchat-optimize --udon2 world_path
```

**Benefits**:
- Better performance
- New Udon 2 features
- Improved scripting capabilities

---

## Performance Guidelines

### VRChat World Optimization

1. **Material Count**: Keep under 50 materials
2. **Polygon Count**: Optimize for target platform
3. **Post-Processing**: Minimize or make optional
4. **Network Objects**: Reduce sync frequency
5. **LOD**: Implement level-of-detail for complex objects

### VR Performance Targets

| Platform | Target FPS | Max Latency |
|----------|-----------|-------------|
| Quest 2 | 72-90 Hz | <20ms |
| Quest 3 | 90-120 Hz | <15ms |
| PCVR | 90-144 Hz | <15ms |

---

## API Usage

### Rust Code Example

```rust
use codex_vr_runtime::VrRuntime;

let runtime = VrRuntime::new()?;
let device_info = runtime.get_device_info()?;
println!("Device: {}", device_info.name);

let stats = runtime.get_device_stats().await?;
println!("FPS: {:.1}, Latency: {:.1}ms", stats.fps, stats.latency_ms);
```

### VRChat Optimizer Example

```rust
use codex_vrchat_optimizer::{VrchatOptimizer, OptimizationOptions};

let options = OptimizationOptions {
    material_atlas: true,
    minimize_post_processing: true,
    optimize_network_objects: true,
    udon2_compatible: true,
};

let optimizer = VrchatOptimizer::new(options);
let results = optimizer.optimize("path/to/world")?;

println!("Performance improvement: {:.1}%", results.performance_improvement);
```

---

## Troubleshooting

### VR Device Not Detected

1. Verify OpenXR SDK installation
2. Check device drivers
3. Ensure device is connected and powered on
4. Check OpenXR runtime is active

### Performance Issues

1. Check FPS and latency stats
2. Reduce world complexity
3. Optimize materials and textures
4. Use LOD for distant objects

### Virtual Desktop Issues

1. Check network bandwidth
2. Adjust streaming quality
3. Verify GPU performance
4. Check for network interference

---

## References

- [OpenXR Specification](https://www.khronos.org/openxr/)
- [VRChat World Optimization](https://docs.vrchat.com/worlds/optimization)
- [Udon 2 Documentation](https://docs.vrchat.com/udon)
- [Virtual Desktop Guide](https://www.vrdesktop.net/)

---

## Support

For VR/AR specific issues:
- Check OpenXR SDK installation
- Verify device compatibility
- Review performance guidelines
- Check VRChat optimization best practices











