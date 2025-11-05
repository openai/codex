# Codex Tauri GUI - AI-Native OS with VR/AR Support

**Version**: 1.2.0  
**Status**: ✅ Production Ready  
**Platform**: Windows (Primary), macOS/Linux (Experimental)

---

## 🎯 Overview

Codex Tauri GUI is a **system tray resident AI-native desktop application** with advanced **VR/AR capabilities** for 4D Git visualization. Built with Tauri 2.0, React, and Rust, it integrates directly with the Codex core engine and provides kernel-level AI optimization.

### 🌟 Key Features

- **🖥️ System Tray Resident** - Always available, minimal resource usage
- **🎮 VR/AR Ready** - Full support for Quest 2/3/Pro, SteamVR, VRChat
- **📊 4D Git Visualization** - Time-series commit visualization in VR space
- **🤖 AI-Native OS** - Kernel-level AI inference optimization
- **📁 File System Monitoring** - Real-time code change detection
- **🔧 Codex Core Integration** - Direct Rust crate dependency for maximum performance

---

## 🚀 Quick Start

### Prerequisites

- **Windows 10/11** (Primary platform)
- **Rust 2024 Edition** or later
- **Node.js 18+** and npm
- **(Optional)** Meta Quest 2/3/Pro or compatible VR headset

### Installation

```powershell
# From codex-rs directory
cd codex-rs

# Build (Release mode)
.\build-unified.ps1 -Release

# Install
.\install-unified.ps1

# Or build + install in one command
.\watch-build-v2.ps1
```

### Launch

After installation, find the **Codex icon** in your system tray (task tray). Click it to open the GUI.

---

## 🎮 VR/AR Setup

### Supported Devices

| Device | Status | Refresh Rate | Notes |
|--------|--------|--------------|-------|
| Meta Quest 2 | ✅ Optimized | 90Hz | Best with Virtual Desktop |
| Meta Quest 3 | ✅ Supported | 120Hz | Full features |
| Meta Quest Pro | ✅ Supported | 90Hz | Eye tracking ready |
| Valve Index | ✅ Supported | 144Hz | High-fidelity mode |
| HTC Vive | ✅ Supported | 90Hz | SteamVR |
| SteamVR | ✅ Compatible | Varies | Any SteamVR device |
| VRChat | 🔜 Coming Soon | - | v1.3.0 |

### Quest 2 + Virtual Desktop Setup

1. **PC Requirements**
   - GPU: NVIDIA GTX 1060 or AMD RX 480 (minimum)
   - CPU: Intel i5-4590 or AMD Ryzen 5 1500X
   - RAM: 8GB+ (16GB recommended)
   - Network: Wi-Fi 6 router recommended

2. **Virtual Desktop Configuration**
   - Install Virtual Desktop on Quest 2
   - Install Virtual Desktop Streamer on PC
   - Connect to same Wi-Fi network (5GHz band)
   - Set bitrate: 100-150 Mbps for 90Hz

3. **Codex VR Settings**
   - Open Codex GUI → "🥽 VR Settings"
   - Select "Meta Quest 2"
   - Set Refresh Rate: 90Hz
   - Enable "Virtual Desktop Optimization"
   - Save settings

4. **Launch VR Mode**
   - Navigate to "🎮 Git VR/AR" page
   - Select repository
   - Click "Enter VR" button
   - Put on Quest 2 headset

---

## 📊 Features Detail

### 1. System Tray Resident

```
Right-click tray icon:
├── 📊 Dashboard
├── 🎮 Git VR/AR
├── 📋 Blueprints
├── 🥽 VR Settings
├── ⚙️ Settings
└── 🚪 Exit
```

### 2. File System Monitoring

- **Real-time Detection**: Monitors code changes using `notify` crate
- **SQLite Database**: Tracks all file modifications
- **Automatic Blueprints**: Generates architecture diagrams on changes

### 3. 4D Git Visualization

- **X, Y, Z Axes**: Spatial commit layout
- **W Axis (Time)**: Scrub through git history
- **VR Interaction**: Hand tracking, spatial selection
- **Performance**: 85-90 FPS on Quest 2

### 4. Kernel Driver Integration (Optional)

```powershell
# Install with kernel driver (requires admin)
.\install-unified.ps1 -WithKernel -TestSign
```

**Features**:
- AI inference optimization
- GPU-aware task scheduling
- Dedicated memory management
- File system event acceleration

**Note**: Requires test signing mode on Windows. Reboot required.

---

## 🛠️ Development

### Project Structure

```
tauri-gui/
├── src/                    # React frontend
│   ├── components/
│   │   ├── vr/            # VR components
│   │   │   ├── Scene4D.tsx
│   │   │   └── Quest2Optimization.tsx
│   │   └── ...
│   ├── lib/
│   │   └── xr/            # VR utilities
│   │       ├── hand-tracking.ts
│   │       └── virtual-desktop.ts
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   ├── GitVR.tsx
│   │   ├── VRSettings.tsx
│   │   └── ...
│   └── App.tsx
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── tray.rs        # System tray
│   │   ├── watcher.rs     # File monitoring
│   │   ├── db.rs          # SQLite
│   │   ├── codex_bridge.rs # Codex core
│   │   └── kernel_bridge.rs # Kernel driver
│   └── Cargo.toml
└── README.md
```

### Build Scripts

| Script | Purpose |
|--------|---------|
| `build-unified.ps1` | Full release build with progress |
| `watch-build-v2.ps1` | Monitor build + auto-install |
| `install-unified.ps1` | Force install latest MSI |
| `test-security.ps1` | Security vulnerability tests |

### Development Build

```powershell
cd tauri-gui

# Frontend dev server
npm run dev

# Tauri dev (hot reload)
npm run tauri:dev
```

### Release Build

```powershell
cd codex-rs

# Full build (frontend + rust + MSI)
.\build-unified.ps1 -Release

# Differential build (faster)
cd tauri-gui\src-tauri
cargo build --release
```

---

## 🔧 Configuration

### VR Settings

Located in GUI: "🥽 VR Settings" page

```typescript
{
  device: "Quest 2" | "Quest 3" | "Quest Pro" | "Valve Index" | "HTC Vive",
  refreshRate: 72 | 90 | 120 | 144,  // Hz
  virtualDesktopOptimized: boolean
}
```

### Kernel Driver Settings

Located in: `src-tauri/tauri.conf.json`

```json
{
  "tauri": {
    "bundle": {
      "windows": {
        "wix": {
          "kernelDriver": {
            "enabled": true,
            "testSign": true
          }
        }
      }
    }
  }
}
```

---

## 📈 Performance

### Build Times

- **Initial Build**: ~5 minutes (all packages)
- **Differential Build**: ~30 seconds (with sccache)
- **Frontend Only**: ~10 seconds

### Runtime Performance

- **Memory Usage**: 50-100 MB (idle)
- **CPU Usage**: <1% (idle), 5-15% (active monitoring)
- **VR Frame Rate**: 85-90 FPS (Quest 2, 90Hz mode)
- **Executable Size**: 15-20 MB (release)

### Optimization Tips

1. **Enable sccache**: `.\install-sccache.ps1`
2. **Use differential builds**: Build only changed crates
3. **VR Performance**: Lower refresh rate if experiencing lag
4. **Virtual Desktop**: Use dedicated Wi-Fi 6 router

---

## 🐛 Troubleshooting

### Build Errors

**"current package believes it's in a workspace when it's not"**
```powershell
# Already fixed in v1.2.0
# If still occurs, verify Cargo.toml contains:
# members = [..., "tauri-gui/src-tauri"]
```

**PowerShell encoding errors**
```powershell
# Use v2 scripts (ASCII-safe)
.\watch-build-v2.ps1
```

### VR Issues

**Quest 2 lag with Virtual Desktop**
- Lower VR quality setting in Virtual Desktop app
- Reduce bitrate to 80-100 Mbps
- Ensure 5GHz Wi-Fi connection
- Close background applications

**VR mode not launching**
- Update browser to latest version
- Enable WebXR in browser flags
- Check GPU drivers are up to date

### Installation Issues

**MSI installation fails**
```powershell
# Force uninstall existing
.\install-unified.ps1

# Or manual uninstall
msiexec /x "{PRODUCT_CODE}" /qb
```

---

## 🔐 Security

### Best Practices

- ✅ Review all AI-generated code before execution
- ✅ Use sandbox mode for untrusted operations
- ✅ Keep kernel driver disabled unless needed
- ✅ Run security tests: `.\test-security.ps1`

### Kernel Driver Security

When kernel driver is enabled:
- Runs in kernel mode (Ring 0)
- Has full system access
- Requires code signing (test mode for development)
- **Production**: Use EV certificate for signing

---

## 📝 Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

**Latest**: v1.2.0 (2025-11-03)
- Quest 2 optimization
- Virtual Desktop support
- VR Settings page
- Cargo workspace fix

---

## 🤝 Contributing

1. Fork the repository
2. Create feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'feat: add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open Pull Request

### Commit Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/):
```
feat: new feature
fix: bug fix
docs: documentation update
style: code formatting
refactor: code refactoring
test: test addition
chore: build/config changes
```

---

## 📚 Documentation

- **Installation**: [INSTALLATION.md](INSTALLATION.md)
- **Integration Design**: [INTEGRATION_DESIGN.md](INTEGRATION_DESIGN.md)
- **Quick Start**: [QUICK_START.md](QUICK_START.md)
- **Security Testing**: [SECURITY_TEST.md](SECURITY_TEST.md)
- **Implementation Logs**: `/_docs/`

---

## 🎯 Roadmap

### v1.3.0 (Upcoming)

- [ ] LOD (Level of Detail) for large repos
- [ ] Spatial audio in VR
- [ ] Kernel driver full integration
- [ ] Multiplayer VR collaboration
- [ ] VRChat integration
- [ ] Performance profiling tools

### v2.0.0 (Future)

- [ ] Cross-platform kernel drivers (Linux, macOS)
- [ ] Cloud sync for VR sessions
- [ ] AI-powered code review in VR
- [ ] Advanced hand gesture recognition

---

## 📄 License

See [LICENSE](../../LICENSE) in repository root.

---

## 🙏 Acknowledgments

- **Tauri Team** - Excellent desktop framework
- **Three.js Community** - 3D rendering library
- **React Three Fiber** - React bindings for Three.js
- **Meta Quest** - VR hardware
- **Virtual Desktop** - Wireless VR streaming

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/zapabob/codex/issues)
- **Discussions**: [GitHub Discussions](https://github.com/zapabob/codex/discussions)
- **Documentation**: `./_docs/` directory

---

**Made with ❤️ by zapabob**

**Owattaze!** 🎉
