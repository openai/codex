<!-- d09a5b5c-e555-458f-a898-9d9a10af4906 afde11d8-0ca8-427c-ac9a-4b74aeb144b6 -->
# KAMUI 4D超え - VR/AR/VirtualDesktop完全対応版

## Phase 0: ビルドエラー修正＋警告0

### 0.1 ReasoningSummary型エラー修正

- `codex-rs/core/src/codex.rs` - `config.model_reasoning_summary`を直接渡す（unwrap不要）
- `codex-rs/core/src/git/commit_quality.rs` - 未使用import削除、未使用変数に`_`プレフィックス

### 0.2 全警告の修正

- `#[allow(unused)]`を削除し、実際に使用するか削除
- Clippy警告全修正（`just fix -p codex-core -p codex-cli`）

## Phase 1-3: 公式API統合（ReasoningEffort）

**完了済み**（前回実装）:

- AgentRuntime強化
- ParallelOrchestrator強化
- Session初期化での設定伝播
- 全AgentRuntime呼び出し更新

## Phase 4-6: 自然言語認識

**完了済み**:

- NaturalLanguageParser実装
- TUI入力前処理
- CLIは次フェーズで実装

## Phase 7: Blueprint→Plan完全リネーム

### 7.1 Coreモジュールリネーム

- `codex-rs/core/src/blueprint/` → `plan/`ディレクトリ全体を移動
- ファイルリネーム: `executor.rs`, `execution_log.rs`等

### 7.2 型定義リネーム

全ての型をリネーム:

- `BlueprintBlock` → `PlanBlock`
- `BlueprintExecutor` → `PlanExecutor`
- `BlueprintState` → `PlanState`
- `ExecutionLog`は維持（実行ログなので）

### 7.3 CLIコマンドリネーム

- `codex-rs/cli/src/blueprint_commands.rs` → `plan_commands.rs`
- `codex-rs/cli/src/blueprint_commands_impl.rs` → `plan_commands_impl.rs`
- `codex-rs/cli/src/main.rs` - `Subcommand::Blueprint` → `Plan`

## Phase 8-11: サイバーパンクUI

**完了済み**:

- cyberpunk-theme.css
- CyberpunkBackground
- ContextMenu
- Scene3Dカラフル化＋Bloom
- Orchestration.css
- App.tsxクリップボード対応

## Phase 12: セマンティックバージョンアップ v1.5.0

### 12.1 Cargo.tomlバージョン更新

- `codex-rs/Cargo.toml` - workspace.package.version = "1.5.0"
- `codex-rs/cli/Cargo.toml`
- `codex-rs/tauri-gui/src-tauri/Cargo.toml`

### 12.2 package.jsonバージョン更新

- `codex-rs/tauri-gui/package.json` - "version": "1.5.0"
- `sdk/typescript/package.json`

## Phase 13: WebXR基盤実装

### 13.1 WebXR Core実装

**新規**: `codex-rs/tauri-gui/src/components/vr/WebXRProvider.tsx`

```typescript
import { createXRStore, XR } from '@react-three/xr'

export const xrStore = createXRStore({
  hand: true,        // Hand tracking
  controller: true,  // Controllers
  anchors: true,     // AR anchors
  layers: true,      // Composition layers
})

export const WebXRProvider = ({ children }) => (
  <XR store={xrStore}>
    {children}
  </XR>
)
```

### 13.2 VRシーン拡張

`codex-rs/tauri-gui/src/components/git/SceneVR.tsx` (新規)

- Scene3Dベースに VR Controllers追加
- Hand tracking対応
- Teleportation移動
- UI Panels in 3D space

## Phase 14: ネイティブVR対応（Quest/PSVR2）

### 14.1 Quest対応ビルド

**新規**: `codex-rs/tauri-gui/src-tauri/capabilities/quest.json`

Quest向けTauriビルド設定:

- Android APKビルド
- OpenXR統合
- パフォーマンス最適化（90fps）

### 14.2 PSVR2対応

**新規**: `codex-rs/vr-native/` (Rust VRモジュール)

- OpenVR/OpenXR binding
- SteamVR統合
- PSVR2固有機能（Eye tracking, Adaptive triggers）

## Phase 15: AR機能実装

### 15.1 ARCore/ARKit統合

**新規**: `codex-rs/tauri-gui/src/components/ar/ARScene.tsx`

- 空間アンカー配置
- Plane detection（床・壁検出）
- Image tracking
- Git可視化をAR空間に配置

### 15.2 QRコードマーカー

`codex-rs/tauri-gui/src/components/ar/QRMarker.tsx`

- QRコードでリポジトリ情報エンコード
- スキャンでAR Git可視化起動

## Phase 16: VirtualDesktop最適化

### 16.1 ストリーミング最適化

**新規**: `codex-rs/tauri-gui/src/utils/virtualdesktop-optimizer.ts`

```typescript
export class VirtualDesktopOptimizer {
  // VirtualDesktop検出
  detectVirtualDesktop(): boolean
  
  // レンダリング最適化
  optimizeForStreaming(): void {
    // Lower resolution rendering
    // Reduce post-processing
    // Increase compression tolerance
  }
  
  // ネットワーク最適化
  reduceNetworkLoad(): void {
    // Delta updates only
    // Aggressive caching
  }
}
```

### 16.2 自動品質調整

- VirtualDesktop検出時に自動で設定変更
- FPS上限: 72fps（Quest2 native）
- Bloom強度減少（帯域節約）
- LOD積極適用

### 16.3 手動品質設定

**新規**: `codex-rs/tauri-gui/src/components/VDQualitySettings.tsx`

```typescript
<VDQualitySettings>
  <Select label="Quality Preset">
    <Option value="ultra">Ultra (Local)</Option>
    <Option value="high">High (WiFi 6)</Option>
    <Option value="medium">Medium (VirtualDesktop)</Option>
    <Option value="low">Low (Mobile Hotspot)</Option>
  </Select>
</VDQualitySettings>
```

## Phase 17: 3Dモデルフォーマット対応（KAMUI 4D互換）

### 17.1 USD/USDZ対応

**新規**: `codex-rs/tauri-gui/src/loaders/USDLoader.ts`

- USD (Universal Scene Description) パーサー
- USDZ (AR Quick Look) 対応
- Three.js統合

### 17.2 OBJ/FBX対応

```typescript
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader'
import { FBXLoader } from 'three/examples/jsm/loaders/FBXLoader'
```

### 17.3 GLB/GLTF対応

```typescript
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader'
import { DRACOLoader } from 'three/examples/jsm/loaders/DRACOLoader'
```

## Phase 18: 空間UI実装（KAMUI 4D超え）

### 18.1 3D Task Panels

**新規**: `codex-rs/tauri-gui/src/components/vr/TaskPanel3D.tsx`

```typescript
// Floating task cards in VR space
<TaskPanel3D
  position={[x, y, z]}
  tasks={tasks}
  onTaskClick={handleTaskClick}
  cyberpunkTheme
/>
```

### 18.2 Hand Gesture Controls

**新規**: `codex-rs/tauri-gui/src/components/vr/HandGestures.tsx`

- Pinch to select
- Swipe to navigate
- Grab to move panels
- Point to raycast

### 18.3 Voice Commands

**新規**: `codex-rs/tauri-gui/src/components/vr/VoiceInput.tsx`

- Web Speech API統合
- 自然言語認識と連携
- VR内で音声コマンド実行

## Phase 19: パフォーマンス最適化

### 19.1 VR用LOD

`codex-rs/tauri-gui/src/components/git/SceneVR.tsx`

- 距離ベースLOD（3段階）
- Frustum culling
- Occlusion culling

### 19.2 Instancing最適化

- 1000+コミット対応
- GPU Instancingフル活用
- Compute shader活用（WebGPU）

### 19.3 非同期ローディング

- Progressive loading
- Web Worker活用
- Suspenseフォールバック

## Phase 20: 型定義完全化

### 20.1 TypeScript strict mode

`codex-rs/tauri-gui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true
  }
}
```

### 20.2 全コンポーネント型定義追加

- Props interface定義
- Event handler型定義
- State型定義

### 20.3 Rust型エクスポート

`codex-rs/tauri-gui/src-tauri/bindings/`

- TypeScript bindingsを自動生成
- `specta`クレート使用

## Phase 21: 依存関係追加

`codex-rs/tauri-gui/package.json`:

追加パッケージ:

- `@react-three/xr`: ^6.2.0 - WebXR
- `@react-three/usdz`: ^latest - USDZ loader
- `three-stdlib`: ^2.29.0 - Three.js loaders
- `@webxr-input-profiles/motion-controllers`: ^latest - VR controllers
- `@mediapipe/hands`: ^0.4.1646424915 - Hand tracking

## Phase 22: ビルド＋インストール

### 22.1 差分ビルド

```powershell
cd codex-rs
just fmt
just fix -p codex-core -p codex-cli
cargo build --release -p codex-cli
```

### 22.2 強制インストール

```powershell
cargo install --path cli --force
codex --version  # v1.5.0確認
```

### 22.3 GUI依存関係インストール

```powershell
cd tauri-gui
npm install
npm run build
```

## Phase 23: 機能テスト

### 23.1 自然言語テスト

- TUI: "会話を圧縮して" → `/compact`
- TUI: "計画を作成して" → `/plan`

### 23.2 VR/ARテスト

- WebXR Session起動
- Hand tracking動作確認
- AR plane detection

### 23.3 VirtualDesktop最適化テスト

- VD経由で接続
- FPS測定（目標: 72fps安定）
- レイテンシ測定

---

## 実装ファイル一覧（Phase 0-23全体）

### Phase 0: エラー修正

1. `codex-rs/core/src/codex.rs` ✓
2. `codex-rs/core/src/git/commit_quality.rs` ✓

### Phase 1-11: 既存（完了済み）

3-22. （前回実装分）

### Phase 12: バージョン更新

23. `codex-rs/Cargo.toml`
24. `codex-rs/cli/Cargo.toml`
25. `codex-rs/tauri-gui/src-tauri/Cargo.toml`
26. `codex-rs/tauri-gui/package.json`

### Phase 13-15: WebXR/VR/AR

27. `codex-rs/tauri-gui/src/components/vr/WebXRProvider.tsx` (新規)
28. `codex-rs/tauri-gui/src/components/git/SceneVR.tsx` (新規)
29. `codex-rs/tauri-gui/src/components/ar/ARScene.tsx` (新規)
30. `codex-rs/tauri-gui/src/components/ar/QRMarker.tsx` (新規)
31. `codex-rs/vr-native/` (新規Rustモジュール)

### Phase 16: VirtualDesktop

32. `codex-rs/tauri-gui/src/utils/virtualdesktop-optimizer.ts` (新規)
33. `codex-rs/tauri-gui/src/components/VDQualitySettings.tsx` (新規)

### Phase 17: 3Dフォーマット

34. `codex-rs/tauri-gui/src/loaders/USDLoader.ts` (新規)
35. `codex-rs/tauri-gui/src/loaders/ModelLoader.ts` (新規)

### Phase 18: 空間UI

36. `codex-rs/tauri-gui/src/components/vr/TaskPanel3D.tsx` (新規)
37. `codex-rs/tauri-gui/src/components/vr/HandGestures.tsx` (新規)
38. `codex-rs/tauri-gui/src/components/vr/VoiceInput.tsx` (新規)

### Phase 20: 型定義

39. `codex-rs/tauri-gui/tsconfig.json` - strict mode有効化
40. 全`.tsx`ファイルに型定義追加

---

## 期待される効果

### KAMUI 4D超え要素

- USD/USDZ/OBJ/FBX/GLB全対応（KAMUI未対応含む）
- WebXR + ネイティブVR（Quest, PSVR2, Vive）
- AR対応（空間アンカー、平面検出）
- VirtualDesktop最適化（72fps@Quest2）
- Hand tracking + Voice control
- 3D空間でのタスク管理

### 技術的優位性

- 型安全（TypeScript strict + Rust）
- 警告0
- パフォーマンス（1000+コミット @ 90fps VR）
- サイバーパンクデザイン
- 自然言語操作

### バージョン情報

- v1.4.0 → v1.5.0
- 後方互換性維持
- 段階的VR/AR有効化可能

### To-dos

- [ ] Phase 0: ビルドエラー＋警告修正（ReasoningSummary、unused imports）
- [ ] Phase 7: Blueprint→Plan完全リネーム（Core、CLI、GUI全体）
- [ ] Phase 12: セマンティックバージョンv1.5.0へアップ
- [ ] Phase 13: WebXR基盤実装（WebXRProvider、SceneVR）
- [ ] Phase 14: ネイティブVR対応（Quest APK、OpenXR）
- [ ] Phase 15: AR機能実装（空間アンカー、平面検出、QRマーカー）
- [ ] Phase 16: VirtualDesktop最適化（ストリーミング、品質調整）
- [ ] Phase 17: 3Dモデル対応（USD/USDZ/OBJ/FBX/GLB）
- [ ] Phase 18: 空間UI実装（3Dパネル、Hand gesture、Voice）
- [ ] Phase 19: VR用パフォーマンス最適化（LOD、GPU Instancing）
- [ ] Phase 20: 型定義完全化（TypeScript strict mode、警告0）
- [ ] Phase 21: VR/AR依存関係追加（@react-three/xr等）
- [ ] Phase 22: 差分ビルド＋強制インストール＋テスト