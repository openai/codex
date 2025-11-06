<!-- f02b31a8-5b45-4245-b01f-711cbef26e42 b9416a3c-6a67-4ab4-af5d-a23a64362c2b -->
# Phase 2 拡張版: 完全実装プラン

## 🎯 目標

KAMUI 4D風のサイバーパンク＆テックデザインUIで、包括的なVR/AR対応と動的リソース管理を実装

## 🎨 UI/UX完全刷新（KAMUI 4D準拠）

### 1. Tauri GUIサイバーパンクデザイン

**新規ファイル**: `codex-rs/tauri-gui/src/styles/cyberpunk-theme.css`

**デザイン要素**:

- **ダークテーマ**: `#0a0a0f` (背景), `#1a1a2e` (パネル)
- **ネオンアクセント**: 
  - ブルー: `#00d4ff` (アクティブ)
  - パープル: `#a855f7` (選択)
  - グリーン: `#10b981` (成功)
  - オレンジ: `#f97316` (警告)
- **グローエフェクト**: `box-shadow: 0 0 20px rgba(0, 212, 255, 0.5)`
- **3Dネットワーク可視化**: Three.js + カスタムシェーダー

**コンポーネント構造**:

```typescript
// tauri-gui/src/App.tsx
<Layout>
  <Header>
    <Logo>CODEX 4D</Logo>
    <StatusBar> {/* CL/GX/GM/TM バージョン表示 */} </StatusBar>
    <HelpButton />
    <LicenseButton />
  </Header>
  
  <MainLayout>
    <LeftPanel>
      <FileTreeView /> {/* 322ファイル、13M行コード */}
      <NetworkGraph3D /> {/* KAMUI風3Dグラフ */}
    </LeftPanel>
    
    <CenterPanel>
      <Git4DVisualization /> {/* xyz+time可視化 */}
      <TimelineControl />
    </CenterPanel>
    
    <RightPanel>
      <TaskManager />
      <Terminal />
      <AgentStatus /> {/* AIエージェント状態 */}
    </RightPanel>
  </MainLayout>
  
  <Footer>
    <ProgressBar />
    <FileStats />
  </Footer>
</Layout>
```

### 2. ショートカットキーシステム

**実装**: `tauri-gui/src/hooks/useKeyboardShortcuts.ts`

```typescript
const shortcuts = {
  'Ctrl+C': () => copy(),
  'Ctrl+X': () => cut(),
  'Ctrl+V': () => paste(),
  'Ctrl+Z': () => undo(),
  'Ctrl+Shift+Z': () => redo(),
  'F1': () => showHelp(),
  'Ctrl+K': () => openCommandPalette(),
  'Space': () => togglePlayback(), // 4D再生
  'Ctrl+Shift+L': () => showLicense(),
  'Ctrl+Alt+V': () => enterVRMode(),
}
```

### 3. ヘルプシステム

**新規ファイル**: `tauri-gui/src/components/HelpSystem.tsx`

```typescript
export function HelpSystem() {
  return (
    <HelpOverlay>
      <Tabs>
        <Tab label="キーボードショートカット">
          <ShortcutList shortcuts={shortcuts} />
        </Tab>
        <Tab label="4D可視化の使い方">
          <Tutorial topic="git-4d" />
        </Tab>
        <Tab label="VR/ARモード">
          <VRSetupGuide devices={['Quest2', 'Quest3', 'VisionPro']} />
        </Tab>
        <Tab label="API リファレンス">
          <APIReference />
        </Tab>
      </Tabs>
    </HelpOverlay>
  )
}
```

### 4. ライセンス表示

**実装**: `tauri-gui/src/components/LicenseDialog.tsx`

```typescript
export function LicenseDialog() {
  return (
    <Dialog>
      <Title>Codex v2.0.0 ライセンス</Title>
      <Content>
        <Section>Apache License 2.0</Section>
        <Section>依存ライブラリ: {licenses.map(l => l.name)}</Section>
        <Section>OpenAI/codex ベース + zapabob拡張</Section>
      </Content>
    </Dialog>
  )
}
```

## 🔧 動的リソース管理

### 5. CPUコア動的割り当て

**実装**: `codex-rs/core/src/resources/cpu_manager.rs`

```rust
pub struct CpuManager {
    total_cores: usize,
    max_per_agent: usize, // CPUコア × 2
    current_allocation: HashMap<String, usize>,
}

impl CpuManager {
    pub fn new() -> Self {
        let total_cores = num_cpus::get();
        Self {
            total_cores,
            max_per_agent: total_cores * 2,
            current_allocation: HashMap::new(),
        }
    }
    
    pub fn allocate_for_agent(&mut self, agent_id: &str) -> Result<usize> {
        let available = self.total_cores * 2 - self.current_allocation.values().sum::<usize>();
        let allocation = available.min(self.max_per_agent);
        
        if allocation > 0 {
            self.current_allocation.insert(agent_id.to_string(), allocation);
            Ok(allocation)
        } else {
            Err(anyhow::anyhow!("No CPU cores available"))
        }
    }
    
    pub fn release(&mut self, agent_id: &str) {
        self.current_allocation.remove(agent_id);
    }
}
```

### 6. CUDA推論統合

**実装**: `codex-rs/core/src/inference/cuda_engine.rs`

```rust
#[cfg(feature = "cuda")]
pub struct CudaInferenceEngine {
    device_id: i32,
    model_path: PathBuf,
    quantization: Quantization, // INT8, INT4
}

#[cfg(feature = "cuda")]
impl CudaInferenceEngine {
    pub async fn infer(&self, prompt: &str, max_tokens: usize) -> Result<String> {
        // TensorRT-LLM or vLLM統合
        let runtime = CudaRuntime::new(self.device_id)?;
        
        // モデルロード
        let model = runtime.load_model(&self.model_path, self.quantization)?;
        
        // 推論実行
        let output = model.generate(prompt, max_tokens).await?;
        
        Ok(output)
    }
    
    pub fn estimate_memory(&self) -> Result<usize> {
        // VRAM使用量推定
        Ok(8 * 1024 * 1024 * 1024) // 8GB
    }
}
```

## 🥽 包括的VR/AR対応

### 7. Quest 2/3/3s/Pro統合

**実装**: `tauri-gui/src/vr/QuestIntegration.tsx`

```typescript
import { VRButton, XR, Controllers, Hands } from '@react-three/xr'

export function QuestVRMode() {
  const { device } = useVRDevice()
  
  return (
    <>
      <VRButton />
      <Canvas>
        <XR referenceSpace="local-floor">
          <Git4DVisualization />
          
          {/* Quest 2: コントローラー優先 */}
          {device === 'quest2' && <Controllers />}
          
          {/* Quest 3/3s/Pro: Hand Tracking */}
          {['quest3', 'quest3s', 'questpro'].includes(device) && (
            <>
              <Hands />
              <Controllers /> {/* フォールバック */}
            </>
          )}
          
          {/* Quest Pro: Eye Tracking */}
          {device === 'questpro' && <EyeTrackingGaze />}
          
          {/* カラーパススルー (Quest 3+) */}
          {['quest3', 'quest3s', 'questpro'].includes(device) && (
            <Passthrough enabled={true} />
          )}
        </XR>
      </Canvas>
    </>
  )
}
```

### 8. Apple Vision Pro対応

**新規ディレクトリ**: `codex-visionos/`

```swift
// codex-visionos/CodexVisionApp.swift
import SwiftUI
import RealityKit

@main
struct CodexVisionApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.volumetric)
        
        ImmersiveSpace(id: "Git4D") {
            Git4DVisualizationView()
        }
        .immersionStyle(selection: .constant(.full), in: .full)
    }
}

struct Git4DVisualizationView: View {
    @State private var commits: [Commit4D] = []
    
    var body: some View {
        RealityView { content in
            // Rust FFI経由でコミットデータ取得
            let entity = await loadGitVisualization()
            content.add(entity)
        }
        .gesture(SpatialTapGesture().targetedToAnyEntity())
    }
}
```

### 9. SteamVR + Virtual Desktop

**実装**: `tauri-gui/src/vr/SteamVRIntegration.tsx`

```typescript
export function SteamVRMode() {
  useEffect(() => {
    // OpenXR Runtime検出
    const runtime = detectOpenXRRuntime()
    
    if (runtime === 'SteamVR') {
      initSteamVR()
    } else if (runtime === 'VirtualDesktop') {
      initVirtualDesktop()
    }
  }, [])
  
  return (
    <Canvas>
      <XR>
        <SteamVRControllers />
        <Git4DVisualization />
      </XR>
    </Canvas>
  )
}
```

### 10. VRChat対応準備

**実装**: `codex-rs/vrchat-integration/`

```rust
// vrchat-integration/src/lib.rs
pub struct VRChatIntegration {
    api_client: VRChatApiClient,
    world_id: String,
}

impl VRChatIntegration {
    pub async fn create_git_visualization_world(&self) -> Result<String> {
        // VRChat SDK連携
        // Unityプロジェクト生成
        // Git 4D可視化をVRChatワールドとして出力
        todo!("VRChat SDK統合")
    }
}
```

## 🏗️ Windows 25H2統合

### 11. Windows AI API統合

**実装**: `codex-rs/windows-ai/src/kernel_integration.rs`

```rust
#[cfg(target_os = "windows")]
pub struct WindowsAIKernel {
    ai_runtime: WindowsAIRuntime,
}

#[cfg(target_os = "windows")]
impl WindowsAIKernel {
    pub fn new() -> Result<Self> {
        // Windows.AI.MachineLearning API
        let ai_runtime = WindowsAIRuntime::initialize()?;
        Ok(Self { ai_runtime })
    }
    
    pub async fn infer_with_directml(&self, model: &Path, input: &str) -> Result<String> {
        // DirectML経由でGPU推論
        self.ai_runtime.run_inference(model, input).await
    }
    
    pub fn kernel_scheduler_priority(&self) -> Result<()> {
        // Windows 25H2 AI Scheduler統合
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        }
        Ok(())
    }
}
```

### 12. rmcp公式バージョン整合

**更新**: `codex-rs/Cargo.toml`

```toml
[dependencies]
rmcp = "0.8.5"  # 公式最新バージョンに同期

[patch.crates-io]
# rmcp = { path = "../../rust-sdk/crates/rmcp" }  # デバッグ用のみ
```

## ✅ 型安全性・警告0

### 13. 完全な型定義

**実装方針**:

```rust
// すべての関数に明示的な戻り値型
pub fn analyze_commits(repo: &Path) -> Result<Vec<CommitNode3D>> {
    // ...
}

// unwrap()禁止、?演算子使用
let data = function_that_may_fail()?;

// expect()も最小限、コンテキスト付与
let value = option.context("Failed to get value")?;
```

**Clippy設定**: `codex-rs/.cargo/config.toml`

```toml
[target.'cfg(all())']
rustflags = [
    "-D", "warnings",           # すべての警告をエラー化
    "-D", "clippy::unwrap_used",
    "-D", "clippy::expect_used",
    "-D", "clippy::panic",
]
```

### 14. CUDA機能フラグ

**更新**: `codex-rs/Cargo.toml`

```toml
[features]
default = []
cuda = ["codex-cuda-runtime", "tensorrt-rs"]
windows-ai = ["windows", "windows-ai-rs"]
vr = ["openvr", "openxr"]

[dependencies]
codex-cuda-runtime = { path = "cuda-runtime", optional = true }
tensorrt-rs = { version = "0.1", optional = true }
```

## 🚀 高速差分ビルド

### 15. ビルドスクリプト最適化

**新規**: `scripts/fast-build-install.ps1`

```powershell
# 高速差分ビルドと強制インストール
param(
    [switch]$Release,
    [switch]$Cuda,
    [switch]$WindowsAI,
    [switch]$VR
)

$env:RUSTC_WRAPPER = "sccache"
$env:CARGO_INCREMENTAL = "1"

# 並列ビルド（CPUコア数）
$cores = (Get-CimInstance Win32_Processor).NumberOfLogicalProcessors
$jobs = $cores

# 機能フラグ構築
$features = @()
if ($Cuda) { $features += "cuda" }
if ($WindowsAI) { $features += "windows-ai" }
if ($VR) { $features += "vr" }

$featureStr = if ($features.Count -gt 0) { 
    "--features " + ($features -join ",") 
} else { 
    "" 
}

Write-Host "🔨 差分ビルド開始（$jobs並列）..." -ForegroundColor Cyan

cd codex-rs

if ($Release) {
    cargo build --release -p codex-cli $featureStr --jobs $jobs
} else {
    cargo build -p codex-cli $featureStr --jobs $jobs
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ ビルド成功" -ForegroundColor Green
    Write-Host "🔧 強制インストール中..." -ForegroundColor Cyan
    
    cargo install --path cli --force $featureStr --jobs $jobs
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ インストール完了" -ForegroundColor Green
        codex --version
    }
}
```

**使用例**:

```powershell
# 基本ビルド
.\scripts\fast-build-install.ps1

# すべての機能有効
.\scripts\fast-build-install.ps1 -Release -Cuda -WindowsAI -VR

# CUDA + VRのみ
.\scripts\fast-build-install.ps1 -Release -Cuda -VR
```

## 📋 実装マイルストーン

### Week 1-2: UI/UX基盤

- サイバーパンクテーマCSS
- ショートカットキーシステム
- ヘルプ・ライセンスダイアログ

### Week 3-4: リソース管理

- CPUコア動的割り当て
- CUDA推論エンジン
- Windows 25H2統合

### Week 5-6: VR基本対応

- Quest 2/3基本実装
- WebXR統合
- SteamVR対応

### Week 7-8: VR拡張

- Quest 3s/Pro機能
- Vision Pro基本実装
- Virtual Desktop統合

### Week 9-10: 品質向上

- 型安全性100%
- 警告0達成
- パフォーマンス最適化

### Week 11-12: 統合テスト

- 全VRデバイステスト
- CUDA推論テスト
- リリース準備

## 🎯 完了基準

- ✅ KAMUI 4D風UI完成
- ✅ ショートカットキー実装
- ✅ ヘルプ・ライセンス表示
- ✅ 動的リソース管理（CPUコア×2上限）
- ✅ CUDA推論動作
- ✅ Quest 2/3/3s/Pro対応
- ✅ Vision Pro基本対応
- ✅ SteamVR + Virtual Desktop対応
- ✅ Windows 25H2機能統合
- ✅ rmcp公式バージョン整合
- ✅ 型定義完全・警告0
- ✅ 高速差分ビルド確立

## 📚 技術スタック

### Frontend

- React 18 + TypeScript
- Three.js + React Three Fiber
- @react-three/xr (WebXR)
- Tailwind CSS + カスタムサイバーパンクテーマ

### Backend

- Rust 2024 Edition
- Tauri 2.0
- git2-rs
- rmcp 0.8.5

### VR/AR

- WebXR API
- OpenXR (SteamVR)
- visionOS SDK (Swift)
- VRChat SDK (Unity, 将来)

### GPU

- CUDA 12.x
- TensorRT / vLLM
- DirectML (Windows AI)

### Build

- sccache
- cargo incremental
- 並列ビルド（全コア活用）

### To-dos

- [ ] KAMUI 4D風サイバーパンクテーマCSS実装
- [ ] ショートカットキーシステム（Ctrl+C/X/V/Z, F1等）
- [ ] ヘルプシステム（チュートリアル、APIリファレンス）
- [ ] ライセンス表示ダイアログ
- [ ] 3Dネットワークグラフ可視化（KAMUI風）
- [ ] CPUコア動的割り当て（コア×2上限）
- [ ] CUDA推論エンジン統合（TensorRT/vLLM）
- [ ] Windows 25H2 AI API統合（DirectML）
- [ ] Windowsカーネルスケジューラー優先度制御
- [ ] Quest 2基本対応（WebXR、コントローラー）
- [ ] Quest 3 Hand Tracking + カラーパススルー
- [ ] Quest 3s対応
- [ ] Quest Pro Eye Tracking + Face Tracking
- [ ] Apple Vision Pro対応（visionOS + RealityKit）
- [ ] SteamVR統合（OpenXR）
- [ ] Virtual Desktop連携
- [ ] VRChat対応準備（SDK統合設計）
- [ ] 型定義100%完全化（unwrap/expect排除）
- [ ] 警告0達成（Clippy厳格設定）
- [ ] rmcp公式バージョン整合（0.8.5）
- [ ] CUDA機能フラグ整備
- [ ] 高速差分ビルドスクリプト（sccache + 並列）
- [ ] 強制上書きインストールスクリプト
- [ ] TUI 4D可視化完成（TimelineControl + 再生）
- [ ] Tauri GUI 3D可視化完成（Three.js）
- [ ] 統合テスト（全VRデバイス + CUDA + Windows AI）
- [ ] パフォーマンス最適化（60fps保証）
- [ ] 完全ドキュメント（VR/AR/CUDA/Windows統合）