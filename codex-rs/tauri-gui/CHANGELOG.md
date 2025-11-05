# Changelog - Codex Tauri GUI

All notable changes to the Codex Tauri GUI project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.2.0] - 2025-11-03

### 🎮 Added - VR/AR Support

- **Quest 2 Optimization**
  - `Quest2Optimization.tsx` component for Quest 2 specific optimizations
  - 90Hz refresh rate support
  - Shadow mapping disabled for performance
  - LOD (Level of Detail) preparation

- **Virtual Desktop Support**
  - `VirtualDesktopOptimizer` class for wireless VR streaming
  - Bandwidth optimization algorithms
  - Dynamic bitrate adjustment
  - Predictive tracking compensation
  - Network quality monitoring

- **VR Settings Page**
  - New `VRSettings.tsx` page for VR device configuration
  - VR device selection (Quest 2/3/Pro, Valve Index, HTC Vive)
  - Refresh rate settings (72/90/120/144 Hz)
  - Virtual Desktop optimization toggle
  - Performance tips section

### 📊 Enhanced - 4D Git Visualization

- **4D Scene Component**
  - `Scene4D.tsx` - 4D space commit node placement
  - Time axis (W-axis) slider implementation
  - Hand tracking support integration
  - Spatial audio preparation

### 🔧 Improved - Build System

- **Differential Build Scripts**
  - `watch-build-v2.ps1` - Build monitoring & auto-installation
  - `build-unified.ps1` - Unified build script with progress
  - sccache integration for faster incremental builds
  - Build progress visualization

- **Force Installation**
  - Automatic uninstallation of existing versions
  - MSI auto-installation
  - Kernel driver integration option

### 🛠️ Fixed

- **Cargo Workspace Configuration**
  - Added `tauri-gui/src-tauri` to workspace members
  - Fixed "current package believes it's in a workspace when it's not" error

- **PowerShell Encoding Issues**
  - Created ASCII-safe scripts to avoid encoding problems
  - Fixed Japanese character corruption in build scripts

### 📦 Dependencies

- Added `@react-three/fiber@^8.15.0` - 3D rendering
- Added `@react-three/xr@^6.2.0` - WebXR integration
- Added `@react-three/drei@^9.92.0` - 3D utilities
- Added `three@^0.160.0` - 3D engine

### 🔄 Changed

- Updated version from 1.1.0 to 1.2.0
- Updated UI version display in `App.tsx`

---

## [1.1.0] - 2025-11-02

### 🎨 Added - Core Features

- **System Tray Resident Application**
  - System tray icon with context menu
  - Auto-startup on Windows login
  - Minimize to tray functionality

- **File System Monitoring**
  - Real-time file change detection using `notify` crate
  - SQLite database for tracking changes
  - Automatic blueprint generation on code changes

- **Codex Core Integration**
  - Direct Rust crate dependency on `codex-core`
  - Blueprint generation API
  - Deep research integration
  - MCP tools integration

- **3D Git Visualization (Initial)**
  - Basic 3D scene with Three.js
  - WebXR support for VR/AR devices
  - Git commit graph visualization

### 🔒 Security

- **Kernel Driver Integration (Windows)**
  - AI inference optimization at kernel level
  - GPU-aware scheduling
  - Dedicated memory management
  - Test signing mode support

### 📱 UI Pages

- **Dashboard** - System status and recent changes
- **Blueprints** - Generated blueprints viewer
- **Settings** - Application configuration
- **Git VR/AR** - 3D Git visualization

### 🎨 Styling

- Modern dark theme UI
- Responsive layout
- CSS custom properties for theming

---

## [1.0.0] - 2025-11-01

### 🎉 Initial Release

- Basic Tauri application structure
- React + TypeScript frontend
- Rust backend with Tauri 2.0
- Project scaffolding and initial setup

---

## Upcoming in [1.3.0]

### 🚀 Planned Features

- **LOD (Level of Detail) Implementation**
  - Distance-based quality adjustment
  - Performance improvement for large commit graphs

- **Spatial Audio Integration**
  - 3D sound for each commit node
  - Hand tracking audio feedback

- **Kernel Driver Full Integration**
  - AI inference optimization
  - GPU-aware scheduling complete

- **Multiplayer Support**
  - Multi-user collaborative visualization
  - WebRTC synchronization

---

## Version History

- **v1.2.0** (2025-11-03) - Quest 2/Virtual Desktop support + VR/AR enhancements
- **v1.1.0** (2025-11-02) - Core features + Kernel integration
- **v1.0.0** (2025-11-01) - Initial release

---

## Contributing

See [README.md](README.md) for contribution guidelines.

## License

See [LICENSE](../../LICENSE) for license information.

---

**Owattaze!** 🎉

