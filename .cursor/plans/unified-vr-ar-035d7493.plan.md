<!-- 035d7493-1dfd-4f5f-a8b2-e7f9e080a1af cca51a32-be2d-453b-9522-2385853af620 -->
# Unity VR/AR Git Visualization Integration Plan

## Architecture Overview

```
Codex (Rust) ──[Git Analysis]──> Unity Bridge (FFI) ──[DLL]──> Unity (C#)
                                        ↓
                                  Tauri GUI ──[IPC]──> Unity Window
```

## Implementation Steps

### 1. Unity C# Project Setup

**Create**: `codex-unity/CodexVRVisualization/`

**Structure**:

```
codex-unity/
├── CodexVRVisualization/
│   ├── Assets/
│   │   ├── Scripts/
│   │   │   ├── CodexBridge.cs          # C# FFI entry point
│   │   │   ├── GitGraphVisualizer.cs   # Main visualization
│   │   │   ├── CommitNode.cs           # Node rendering
│   │   │   ├── TimeAxisController.cs   # 4D time travel
│   │   │   └── QuestOptimization.cs    # Quest-specific optimizations
│   │   ├── Scenes/
│   │   │   └── GitVisualization.unity
│   │   └── Materials/
│   │       └── CommitNodeMaterial.mat
│   ├── Packages/
│   │   └── manifest.json               # OpenXR package
│   └── ProjectSettings/
│       └── XRSettings.asset            # Quest configuration
```

**Key Files**:

`Assets/Scripts/CodexBridge.cs`:

```csharp
using System;
using System.Runtime.InteropServices;
using UnityEngine;

public class CodexBridge : MonoBehaviour
{
    // FFI exports for Rust
    [DllExport("unity_init", CallingConvention = CallingConvention.Cdecl)]
    public static int Init() { /* ... */ }
    
    [DllExport("unity_update_commits", CallingConvention = CallingConvention.Cdecl)]
    public static void UpdateCommits(IntPtr dataPtr, int length) { /* ... */ }
    
    [DllExport("unity_render_frame", CallingConvention = CallingConvention.Cdecl)]
    public static void RenderFrame() { /* ... */ }
}
```

`Assets/Scripts/GitGraphVisualizer.cs`:

```csharp
using UnityEngine;
using UnityEngine.XR;

public class GitGraphVisualizer : MonoBehaviour
{
    // Kamui4d-style 3D graph rendering
    public GameObject commitNodePrefab;
    private List<CommitNode> nodes = new List<CommitNode>();
    
    public void UpdateGraph(CommitData[] commits) {
        // Instantiate nodes with optimized instancing
        // Apply force-directed layout
        // Handle Quest 2/3 performance constraints
    }
}
```

**Unity Build Settings**:

- Platform: Windows x64
- Scripting Backend: IL2CPP
- API Compatibility: .NET Standard 2.1
- Export as DLL: `codex-visualization.dll`

### 2. Rust FFI Bridge

**Create**: `codex-rs/unity-bridge/`

`codex-rs/unity-bridge/Cargo.toml`:

```toml
[package]
name = "codex-unity-bridge"
version = "1.2.0"
edition = "2021"

[dependencies]
codex-core = { path = "../core" }
anyhow = { workspace = true }
libloading = "0.8"
serde = { workspace = true }
serde_json = { workspace = true }

[lib]
crate-type = ["cdylib", "rlib"]
```

`codex-rs/unity-bridge/src/lib.rs`:

```rust
use libloading::{Library, Symbol};
use std::ffi::CString;

pub struct UnityBridge {
    library: Library,
}

impl UnityBridge {
    pub fn new(dll_path: &str) -> anyhow::Result<Self> {
        unsafe {
            let library = Library::new(dll_path)?;
            Ok(Self { library })
        }
    }
    
    pub fn init(&self) -> anyhow::Result<()> {
        unsafe {
            let init: Symbol<extern "C" fn() -> i32> = 
                self.library.get(b"unity_init")?;
            init();
        }
        Ok(())
    }
    
    pub fn update_commits(&self, commits: &[GitCommit]) -> anyhow::Result<()> {
        let json = serde_json::to_string(commits)?;
        // Send to Unity via FFI
    }
}
```

### 3. Tauri Integration

**Modify**: `codex-rs/tauri-gui/src-tauri/src/main.rs`

Add Unity bridge module:

```rust
mod unity_bridge;

use unity_bridge::UnityBridge;

#[tauri::command]
async fn launch_vr_visualization(commits: Vec<GitCommit>) -> Result<(), String> {
    let bridge = UnityBridge::new("./codex-visualization.dll")
        .map_err(|e| e.to_string())?;
    bridge.init().map_err(|e| e.to_string())?;
    bridge.update_commits(&commits).map_err(|e| e.to_string())?;
    Ok(())
}
```

**Modify**: `codex-rs/tauri-gui/src-tauri/Cargo.toml`

Add dependency:

```toml
codex-unity-bridge = { path = "../../unity-bridge" }
```

### 4. Git Data Pipeline

**Modify**: `codex-rs/core/src/git/`

Create Git analysis adapter:

```rust
// codex-rs/core/src/git/unity_adapter.rs
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct GitCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub position: Position3D,
    pub parents: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn analyze_repository(repo_path: &str) -> anyhow::Result<Vec<GitCommit>> {
    // Use existing git2 integration
    // Calculate force-directed layout positions
    // Return structured data for Unity
}
```

### 5. Quest Optimization

**Unity C#**: `Assets/Scripts/QuestOptimization.cs`

```csharp
public class QuestOptimization : MonoBehaviour
{
    void Start() {
        // Detect Quest 2/3
        if (XRSettings.loadedDeviceName == "Oculus") {
            // Apply optimizations
            Application.targetFrameRate = 90;  // Quest 2: 90Hz, Quest 3: 120Hz
            QualitySettings.SetQualityLevel(1); // Low quality for Quest 2
            
            // Reduce draw calls with instancing
            // Disable shadows
            // Use LOD system
        }
    }
}
```

### 6. Fix Existing TypeScript Errors (Temporary)

**Modify**: `codex-rs/tauri-gui/src/components/vr/Scene4D.tsx`

Simplify to stub implementation:

```typescript
// Remove all VR-specific code temporarily
// Keep basic 3D visualization for desktop
// Add "Launch VR" button that calls unity_bridge

export default function Scene4D({ commits }) {
  const handleLaunchVR = async () => {
    await invoke('launch_vr_visualization', { commits });
  };
  
  return (
    <div>
      <button onClick={handleLaunchVR}>Launch Unity VR</button>
      {/* Basic desktop 3D view */}
    </div>
  );
}
```

### 7. Build Configuration

**Create**: `codex-rs/build-with-unity.ps1`

```powershell
# 1. Build Unity DLL
cd codex-unity/CodexVRVisualization
& "C:\Program Files\Unity\Hub\Editor\2022.3.x\Editor\Unity.exe" `
  -quit -batchmode -projectPath . -buildTarget Win64 -executeMethod BuildScript.Build

# 2. Copy DLL to Tauri resources
Copy-Item "Build/codex-visualization.dll" "../../codex-rs/tauri-gui/src-tauri/resources/"

# 3. Build Tauri
cd ../../codex-rs/tauri-gui
npm run build
npx tauri build

# 4. Install
cd ../
.\install-unified.ps1
```

## File Changes Summary

**New Files**:

- `codex-unity/CodexVRVisualization/` (entire Unity project)
- `codex-rs/unity-bridge/` (Rust FFI crate)
- `codex-rs/core/src/git/unity_adapter.rs`
- `codex-rs/build-with-unity.ps1`

**Modified Files**:

- `codex-rs/Cargo.toml` (add unity-bridge member)
- `codex-rs/tauri-gui/src-tauri/Cargo.toml` (add dependency)
- `codex-rs/tauri-gui/src-tauri/src/main.rs` (add Unity commands)
- `codex-rs/tauri-gui/src/components/vr/Scene4D.tsx` (simplify to stub)
- `codex-rs/tauri-gui/src-tauri/tauri.conf.json` (add resources)

**Deleted/Replaced**:

- `codex-rs/tauri-gui/src/lib/xr/*` (React Three Fiber XR utilities - replaced by Unity)
- TypeScript type errors resolved by removing complex XR code

## Dependencies to Install

**Unity**:

- Unity 2022.3 LTS or later
- OpenXR Plugin (via Package Manager)
- XR Interaction Toolkit
- IL2CPP Build Support (Windows)

**Rust**:

```bash
cargo install cargo-expand  # For debugging FFI
```

**Windows**:

- Visual Studio 2022 with C++ tools (for IL2CPP compilation)

## Testing Strategy

1. **Unit Tests**: Rust FFI bridge calls
2. **Integration Tests**: Codex Git analysis → Unity data transfer
3. **VR Tests**: Quest 2/3 device testing via Oculus Link
4. **Performance**: 90Hz stable on Quest 2 with 1000+ commits

## Next Steps (v1.3.0+)

- VRChat SDK integration
- SteamVR support
- Hand tracking gestures
- Multiplayer collaboration
- Time-travel UI improvements

### To-dos

- [ ] 既存コードベースレビュー: tauri-gui/prism-web/kernel-extensions確認
- [ ] 統合設計書作成: ディレクトリ構造、依存関係、移植リスト
- [ ] Rustモジュール移植: tray/watcher/db/kernel_bridge等をtauri-guiへ
- [ ] Frontend統合: Dashboard/Settings/Blueprints/KernelStatusをtauri-guiへ
- [ ] VR/AR統合: Scene3DVXR/VRInterface/hand-tracking移植
- [ ] 4D Git可視化実装: 時間軸追加、VR操作、空間アニメーション
- [ ] 統合ビルドシステム: workspace設定、差分ビルド、強制インストール
- [ ] 統合セキュリティテスト: VR/AR含む全機能テスト