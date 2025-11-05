<!-- 035d7493-1dfd-4f5f-a8b2-e7f9e080a1af 81408f7a-1d97-4283-be2b-59e100c02917 -->
# Codex AI-Native OS VR/AR統合実装プラン

## 実装方針

既存の`codex-rs/tauri-gui`と`prism-web`の3D Git可視化（Scene3DInstanced）を統合し、**WebXR + Unity VR + AR overlay**を一気に実装。Quest 3、Apple Vision Pro対応で、カーネルドライバー統合によるAI Native OS常駐型VR/AR Readyアプリケーションを完成。

## アーキテクチャ全体図

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri Desktop Client (Windows常駐)                         │
│  ├── System Tray                                            │
│  ├── File Watcher                                           │
│  ├── Codex Core Bridge                                      │
│  └── WebView (React)                                        │
│      └── WebXR Integration                                  │
└─────────────────┬───────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬──────────────────┐
    │             │             │                  │
┌───▼────┐   ┌───▼────┐   ┌───▼────┐      ┌─────▼──────┐
│ WebXR  │   │ Unity  │   │  AR    │      │  Kernel    │
│ (Web)  │   │  VR    │   │Overlay │      │  Driver    │
│        │   │ Client │   │Quest/  │      │ (GPU/Mem)  │
│Three.js│   │        │   │Vision  │      │            │
└────────┘   └────────┘   └────────┘      └────────────┘
```

## Phase 1: codex-rs/tauri-guiとprism-web統合

### 1.1 Tauri WebView内でprism-web起動

**実装箇所**: `codex-rs/tauri-gui/src-tauri/src/main.rs`

**変更点**:

- prism-web devサーバーを自動起動
- Tauri WebViewでlocalhost:3000を読み込み
- または、prism-webをTauri内に静的バンドル

**選択肢**:

- **Option A**: prism-web devサーバー起動（開発時）
- **Option B**: prism-webビルド成果物をTauriに埋め込み（本番）

### 1.2 codex-core統合強化

**実装箇所**: `codex-rs/tauri-gui/src-tauri/src/codex_bridge.rs`

**変更点**:

- CLI subprocess → Direct crate依存に変更
- `use codex_core::blueprint::BlueprintExecutor;`
- パフォーマンス向上（IPC削減）

### 1.3 カーネルドライバー統合

**実装箇所**: `codex-rs/tauri-gui/src-tauri/src/kernel_bridge.rs`

**統合**:

- `kernel-extensions/windows/codex_win_api`を依存関係に追加
- 実ドライバーとの通信実装
- GPU/Memory/Scheduler統計の実データ取得

## Phase 2: WebXR統合（Three.js VRモード）

### 2.1 prism-web WebXR対応

**実装箇所**: `prism-web/components/visualizations/Scene3DVXR.tsx`

**新規ファイル**: WebXR対応版Scene3D

**機能**:

- `@react-three/xr`統合
- VRコントローラー対応（Quest 3触覚フィードバック）
- VR空間でのコミットノード探索
- Hand tracking対応（Quest 3 Pro/Vision Pro）
- Spatial audio（コミット位置に応じた3D音響）

**コントローラーマッピング**:

- **Trigger**: コミット選択
- **Grip**: 移動モード
- **Thumbstick**: ナビゲーション
- **A/X Button**: Timeline操作
- **B/Y Button**: ブランチ切り替え

### 2.2 VRナビゲーションシステム

**実装箇所**: `prism-web/lib/visualization/vr-navigator.ts`

**機能**:

- テレポーテーション移動
- Smooth locomotion
- Snap turning
- 快適性オプション（VR酔い対策）

### 2.3 VR UI

**実装箇所**: `prism-web/components/visualizations/VRInterface.tsx`

**機能**:

- 3D空間内のUI Panel（ワールド空間固定）
- Hand menu（手のひらメニュー）
- Gaze-based selection
- コミット詳細パネル（VR空間内浮遊）

## Phase 3: Unity VRネイティブクライアント

### 3.1 Unityプロジェクト作成

**新規ディレクトリ**: `codex-rs/unity-vr-client/`

**Unityバージョン**: 2022.3 LTS

**プラットフォーム**:

- Meta Quest 2/3/Pro
- PCVR (SteamVR)
- Apple Vision Pro（visionOS）

### 3.2 Git可視化Unityシーン

**実装箇所**: `unity-vr-client/Assets/Scripts/GitVisualization.cs`

**機能**:

- JSON読み込み（prism-webと同じデータ）
- コミットノードの3D配置
- InstancedMesh（GPU Instancing）
- LODシステム（3段階）
- Occlusion Culling

### 3.3 VRインタラクション

**実装箇所**: `unity-vr-client/Assets/Scripts/VRController.cs`

**機能**:

- XR Interaction Toolkit統合
- コミットノードのGrab/Release
- レーザーポインター選択
- 物理ベース移動
- ハプティックフィードバック

### 3.4 Codex Core通信

**実装箇所**: `unity-vr-client/Assets/Scripts/CodexBridge.cs`

**統合方法**:

- WebSocket通信（Tauri app-server経由）
- または、HTTP REST API
- Blueprint実行、Research起動

## Phase 4: AR Overlay（Quest 3/Vision Pro）

### 4.1 Passthrough AR（Quest 3）

**実装箇所**: `unity-vr-client/Assets/Scripts/ARPassthrough.cs`

**機能**:

- Meta Quest 3 Passthrough API
- 実世界の上にGit可視化を重ねる
- Depth API（障害物認識）
- Spatial anchors（位置固定）

### 4.2 Vision Pro対応

**実装箇所**: `unity-vr-client/Assets/Scripts/VisionOSBridge.swift`

**機能**:

- visionOS RealityKit統合
- Window groups
- Volumes（3D空間）
- Immersive spaces

### 4.3 ARインタラクション

**機能**:

- Hand tracking（素手操作）
- Eye tracking（視線選択）
- Spatial gesture
- 実コードファイルの上にGit履歴表示

## Phase 5: 4D可視化強化（時間軸 + AR）

### 5.1 Timeline拡張

**実装箇所**: `prism-web/components/visualizations/Timeline4D.tsx`

**機能**:

- アニメーション再生（既存機能強化）
- タイムスライダー（VR/AR空間内）
- コミット間のモーフィング
- ブランチ分岐アニメーション

### 5.2 AR Code Overlay

**実装箇所**: `unity-vr-client/Assets/Scripts/ARCodeOverlay.cs`

**機能**:

- VSCode/Cursor画面認識（画像認識 or API）
- コード行の上にGit blame情報表示
- コミット履歴のホログラム表示
- Author情報のAR表示

### 5.3 Spatial Audio

**実装箇所**: `unity-vr-client/Assets/Scripts/SpatialAudio.cs`

**機能**:

- コミット位置に応じた3D音響
- Author別の音色
- ブランチマージ時の効果音
- VR空間の没入感向上

## Phase 6: カーネルドライバー完全統合

### 6.1 Windows AI Driver実装

**実装箇所**: `kernel-extensions/windows/ai_driver/ai_driver.c`

**新規IOCTL実装**:

```c
// GPU Status取得
case IOCTL_AI_GET_GPU_STATUS:
    // NVAPI統合
    // DirectX 12統合
    // 戻り値: GpuStatus構造体

// Memory Pool管理
case IOCTL_AI_GET_MEMORY_POOL:
    // Pool統計取得
    
case IOCTL_AI_ALLOC_PINNED:
    // Pinned Memory確保
    
case IOCTL_AI_FREE_PINNED:
    // Pinned Memory解放

// Scheduler統計
case IOCTL_AI_GET_SCHEDULER_STATS:
    // AI Process数、レイテンシ等
```

### 6.2 NVAPI統合（GPU Direct Access）

**実装箇所**: `kernel-extensions/windows/ai_driver/nvapi_integration.c`

**機能**:

- NVIDIA GPU統計取得
- CUDA利用率監視
- DirectX 12 Compute統合
- VR rendering最適化

### 6.3 Tauri実統合

**実装箇所**: `codex-rs/tauri-gui/src-tauri/src/kernel_bridge.rs`

**変更点**:

- シミュレーションモード → 実ドライバー呼び出し
- `codex_win_api::AiDriverHandle::open()`
- リアルデータ表示

### 6.4 VRパフォーマンス最適化

**カーネルドライバーによる最適化**:

- VRレンダリングプロセスの優先度UP
- GPU利用率の動的調整
- Pinned Memoryでフレームバッファ管理
- レイテンシ削減（Motion-to-Photon < 20ms）

## Phase 7: 統合ビルドシステム

### 7.1 Cargoワークスペース統合

**実装箇所**: `codex-rs/Cargo.toml`

**既存ワークスペースメンバー追加確認**:

- `tauri-gui`（既に存在）

**依存関係追加**:

```toml
[workspace.dependencies]
tauri = "2.0"
notify = "6.1"
rusqlite = "0.32"
three-d = "0.17"  # 3D rendering
```

### 7.2 統合ビルドスクリプト

**実装箇所**: `codex-rs/build-all.ps1`

**機能**:

- codex-cli差分ビルド
- tauri-gui差分ビルド
- prism-webビルド
- unity-vr-clientビルド（オプション）
- カーネルドライバービルド（オプション）
- tqdm風進捗表示
- 残り時間推定

### 7.3 強制インストールスクリプト

**実装箇所**: `codex-rs/force-install-all.ps1`

**機能**:

1. codex-cli強制インストール（`cargo install --path cli --force`）
2. tauri-gui MSI強制インストール
3. カーネルドライバーインストール（管理者権限）
4. 統合テスト実行
5. 完了音声再生 🔊

## Phase 8: パフォーマンス最適化

### 8.1 VR 90fps保証

**目標**:

- Quest 3: 90fps（推奨120fps）
- Vision Pro: 90fps
- PCVR: 120fps

**最適化手法**:

- GPU Instancing（既存Scene3DInstanced活用）
- Frustum Culling（既存LOD活用）
- Dynamic LOD（VR用に調整）
- Foveated Rendering（Vision Pro）
- カーネルドライバーによるGPU優先度制御

### 8.2 メモリ最適化

**目標**:

- VRモード: < 2GB
- ARモード: < 1.5GB

**手法**:

- Asset bundling
- Texture streaming
- カーネルPinned Memory活用

## 重要ファイル一覧

### 新規作成ファイル（約50ファイル、推定8,000行以上）

#### WebXR統合（prism-web拡張）

- `prism-web/components/visualizations/Scene3DVXR.tsx` (~350行)
- `prism-web/components/visualizations/VRInterface.tsx` (~200行)
- `prism-web/components/visualizations/VRControls.tsx` (~150行)
- `prism-web/lib/visualization/vr-navigator.ts` (~200行)
- `prism-web/lib/xr/hand-tracking.ts` (~180行)
- `prism-web/lib/xr/spatial-audio.ts` (~120行)
- `prism-web/app/(vr)/git-vr/page.tsx` (~150行)

#### Unity VRクライアント

- `codex-rs/unity-vr-client/Assets/Scripts/GitVisualization.cs` (~400行)
- `codex-rs/unity-vr-client/Assets/Scripts/VRController.cs` (~300行)
- `codex-rs/unity-vr-client/Assets/Scripts/CodexBridge.cs` (~250行)
- `codex-rs/unity-vr-client/Assets/Scripts/ARPassthrough.cs` (~200行)
- `codex-rs/unity-vr-client/Assets/Scripts/ARCodeOverlay.cs` (~350行)
- `codex-rs/unity-vr-client/Assets/Scripts/SpatialAudio.cs` (~150行)
- `codex-rs/unity-vr-client/Assets/Scripts/HandTracking.cs` (~180行)
- `codex-rs/unity-vr-client/Assets/Scenes/GitVR.unity`
- `codex-rs/unity-vr-client/ProjectSettings/*`

#### カーネルドライバー完全実装

- `kernel-extensions/windows/ai_driver/ioctl_handlers.c` (~500行)
- `kernel-extensions/windows/ai_driver/gpu_integration.c` (~400行)
- `kernel-extensions/windows/ai_driver/nvapi_bridge.c` (~300行)
- `kernel-extensions/windows/ai_driver/dx12_compute.c` (~350行)
- `kernel-extensions/windows/codex_win_api/src/gpu.rs` (~250行)
- `kernel-extensions/windows/codex_win_api/src/memory.rs` (~200行)
- `kernel-extensions/windows/codex_win_api/src/scheduler.rs` (~180行)

#### 統合ビルドシステム

- `codex-rs/build-all.ps1` (~500行、tqdm風進捗表示）
- `codex-rs/force-install-all.ps1` (~400行)
- `codex-rs/test-vr-ar.ps1` (~300行、VR/ARテスト）
- `codex-rs/deploy-production.ps1` (~250行）

#### ドキュメント

- `codex-rs/VR_AR_GUIDE.md` (~600行)
- `codex-rs/unity-vr-client/README.md` (~400行)
- `_docs/2025-11-03_VR-AR-Complete-Integration.md` (~1,200行)

### 更新ファイル（約30ファイル）

- `codex-rs/Cargo.toml` - tauri-gui統合確認
- `codex-rs/tauri-gui/src-tauri/src/main.rs` - prism-web統合
- `codex-rs/tauri-gui/src-tauri/src/kernel_bridge.rs` - 実ドライバー統合
- `codex-rs/tauri-gui/src-tauri/Cargo.toml` - codex-core直接依存
- `prism-web/components/visualizations/Scene3DInstanced.tsx` - WebXR拡張
- `prism-web/package.json` - WebXR依存関係追加

## 技術スタック

### WebXR

- `@react-three/xr` - React Three Fiber XR統合
- `three` - Three.js（VRモード）
- WebXR Device API
- WebXR Gamepads Module

### Unity

- Unity 2022.3 LTS
- XR Interaction Toolkit
- OpenXR
- Meta Quest SDK
- Apple visionOS SDK

### AR

- Meta Spatial SDK
- ARCore（Android）
- ARKit（iOS/visionOS）
- Hand Tracking 2.0（Quest）
- Eye Tracking（Vision Pro）

### カーネル

- Windows WDK
- NVAPI（NVIDIA）
- DirectX 12
- CUDA Driver API

## セキュリティ考慮事項

### VR/AR固有のセキュリティ

- カメラアクセス権限管理
- Passthrough映像の保護
- Eye tracking dataの暗号化
- Hand tracking dataの匿名化

### カーネルドライバーセキュリティ

- 入力検証徹底（全IOCTL）
- Buffer overflow対策
- カーネルパニック対策
- Rate limiting

## パフォーマンス目標

| プラットフォーム | FPS | Latency | Memory |

|----------------|-----|---------|--------|

| Quest 3 | 90fps+ | <20ms | <2GB |

| Vision Pro | 90fps+ | <15ms | <1.5GB |

| PCVR | 120fps | <15ms | <2GB |

| WebXR (PC) | 60fps+ | <30ms | <1GB |

## 実装順序（一気に実装）

1. **カーネルドライバー完全実装** (Phase 6)
2. **WebXR統合** (Phase 2)
3. **Unity VRクライアント** (Phase 3)
4. **AR Overlay** (Phase 4)
5. **統合ビルドシステム** (Phase 7)
6. **パフォーマンス最適化** (Phase 8)
7. **テスト＆デプロイ**

## 実装ログ保存先

`_docs/2025-11-03_VR-AR-Native-OS-Complete.md`

### To-dos

- [ ] カーネルドライバー完全実装: IOCTL handlers, GPU integration, NVAPI, DX12
- [ ] WebXR統合: Scene3DVXR, VRナビゲーション, VR UI, コントローラー対応
- [ ] Unity VRクライアント: プロジェクト作成, Git可視化, VRインタラクション, Codex通信
- [ ] AR Overlay: Quest 3 Passthrough, Vision Pro対応, ARコードオーバーレイ
- [ ] 4D可視化: Timeline4D, Spatial Audio, Hand/Eye tracking
- [ ] 統合ビルドシステム: build-all.ps1, force-install-all.ps1, tqdm風進捗表示
- [ ] VRパフォーマンス最適化: 90fps保証, メモリ最適化, カーネル統合
- [ ] テスト＆デプロイ: VR/ARテスト, 実機確認, ドキュメント完成