# Codex統一VR/AR AIネイティブOS 統合設計書

**日時**: 2025-11-03  
**バージョン**: v1.2.0 (Unified VR/AR OS)  
**ステータス**: 🔨 統合進行中

---

## 📊 現状分析

### ✅ 既に完了している統合

#### codex-tauriの統合状況

| コンポーネント | 統合状況 | 場所 |
|------------|---------|------|
| Rustモジュール（10ファイル） | ✅ 完了 | `codex-rs/tauri-gui/src-tauri/src/` |
| Frontendページ（3ファイル） | ✅ 完了 | `codex-rs/tauri-gui/src/pages/` |
| Componentsコンポーネント（3ファイル） | ✅ 完了 | `codex-rs/tauri-gui/src/components/` |
| Cargo.toml統合 | ✅ 完了 | `codex-core`直接依存済み |
| Workspace登録 | ✅ 完了 | `codex-rs/Cargo.toml` |

### 🔨 これから実装する統合

| コンポーネント | ステータス | 優先度 |
|------------|----------|-------|
| VR/AR機能（prism-web） | ⏳ 未実装 | 🔴 高 |
| カーネルドライバー完全実装 | ⏳ 部分実装 | 🔴 高 |
| 4D Git可視化 | ⏳ 未実装 | 🟡 中 |
| 統合ビルドスクリプト | ⏳ 未実装 | 🟢 低 |

---

## 🗂️ 統合後のディレクトリ構造

```
codex-rs/
├── tauri-gui/ ← 統合アプリケーション（メイン）
│   ├── src-tauri/ (Rust Backend)
│   │   ├── src/
│   │   │   ├── main.rs ✅ 統合済み
│   │   │   ├── tray.rs ✅ 統合済み
│   │   │   ├── watcher.rs ✅ 統合済み
│   │   │   ├── db.rs ✅ 統合済み
│   │   │   ├── autostart.rs ✅ 統合済み
│   │   │   ├── events.rs ✅ 統合済み
│   │   │   ├── codex_bridge.rs ✅ 統合済み
│   │   │   ├── shortcuts.rs ✅ 統合済み
│   │   │   ├── updater.rs ✅ 統合済み
│   │   │   ├── kernel_bridge.rs ✅ 統合済み
│   │   │   ├── core_bridge.rs 🔨 実装必要（直接依存強化）
│   │   │   └── vr_bridge.rs 🔨 実装必要（WebXR統合）
│   │   ├── Cargo.toml ✅ 依存関係統合済み
│   │   ├── tauri.conf.json ✅ 設定済み
│   │   └── build.rs ✅ ビルドスクリプト済み
│   │
│   ├── src/ (React Frontend)
│   │   ├── main.tsx ✅ 統合済み
│   │   ├── App.tsx ✅ ルーティング済み
│   │   ├── pages/
│   │   │   ├── Dashboard.tsx ✅ 統合済み
│   │   │   ├── Settings.tsx ✅ 統合済み
│   │   │   ├── Blueprints.tsx ✅ 統合済み
│   │   │   └── GitVR.tsx 🔨 実装必要（VR/AR可視化）
│   │   ├── components/
│   │   │   ├── StatusCard.tsx ✅ 統合済み
│   │   │   ├── RecentChanges.tsx ✅ 統合済み
│   │   │   ├── KernelStatus.tsx ✅ 統合済み
│   │   │   └── vr/
│   │   │       ├── Scene4D.tsx 🔨 実装必要（4D可視化）
│   │   │       ├── VRInterface.tsx 🔨 移植必要（prism-webから）
│   │   │       └── QuestOptimization.tsx 🔨 実装必要
│   │   ├── lib/
│   │   │   └── xr/
│   │   │       ├── hand-tracking.ts 🔨 移植必要
│   │   │       └── steamvr-bridge.ts 🔨 実装必要
│   │   └── styles/ ✅ CSS統合済み
│   │
│   ├── package.json ✅ 依存関係設定済み
│   ├── tsconfig.json ✅ TypeScript設定済み
│   └── vite.config.ts ✅ Vite設定済み
│
├── core/ ✅ 既存（Blueprint/Research/MCP）
├── cli/ ✅ 既存
│
└── kernel-extensions/
    └── windows/
        ├── ai_driver/
        │   ├── ai_driver.c ✅ 基本実装済み
        │   ├── gpu_integration.c ✅ 既存（361行）
        │   ├── nvapi_bridge.c ✅ 既存（152行）
        │   ├── dx12_compute.c ✅ 既存（183行）
        │   └── ioctl_handlers.c 🔨 実装必要（IOCTL処理）
        ├── codex_win_api/
        │   └── src/lib.rs ✅ FFI Wrapper実装済み
        └── etw_provider/
            └── ai_etw_provider.man ✅ 既存
```

---

## 📋 実装必要ファイルリスト

### 🔴 高優先度（Phase 3-5）

#### VR/AR統合（6ファイル）

1. **GitVR.tsx** - VR/AR Git可視化ページ
   - 移植元: `prism-web/components/visualizations/Scene3DVXR.tsx`
   - 機能: WebXR、Quest対応、Hand tracking

2. **Scene4D.tsx** - 4D可視化コンポーネント
   - 新規実装
   - 機能: 時間軸（W軸）追加、4D操作

3. **VRInterface.tsx** - VR UI
   - 移植元: `prism-web/components/visualizations/VRInterface.tsx`
   - 機能: VR空間内UI、コミット情報表示

4. **hand-tracking.ts** - Hand tracking
   - 移植元: `prism-web/lib/xr/hand-tracking.ts`
   - 対応: Quest 3/Pro、Vision Pro

5. **steamvr-bridge.ts** - SteamVR統合
   - 新規実装
   - 機能: SteamVR Input System

6. **QuestOptimization.tsx** - Quest最適化
   - 新規実装
   - 機能: 120Hz、Eye tracking、Foveated rendering

#### カーネルドライバー（1ファイル）

7. **ioctl_handlers.c** - IOCTL処理実装
   - 新規実装
   - 機能: GPU/Memory/Scheduler IOCTL実装

### 🟡 中優先度（Phase 6-7）

#### 統合強化（2ファイル）

8. **core_bridge.rs** - Core直接依存強化
   - 新規実装
   - CLI subprocess削除、直接呼び出し

9. **vr_bridge.rs** - WebXR統合
   - 新規実装
   - Tauri⇔WebXR連携

### 🟢 低優先度（Phase 8-9）

#### ビルドシステム（3ファイル）

10. **build-unified.ps1** - 統合ビルドスクリプト
11. **install-unified.ps1** - 統合インストール
12. **test-security-unified.ps1** - 統合セキュリティテスト

---

## 🔗 依存関係グラフ

```
tauri-gui
├── codex-core ✅ (Blueprint/Research/MCP)
│   └── (全サブモジュール)
├── codex-win-api 🔨 (カーネルドライバーFFI)
│   └── windows ✅ (Win32 API)
├── tauri ✅ (GUIフレームワーク)
│   ├── tauri-plugin-notification ✅
│   └── tauri-plugin-shell ✅
├── notify ✅ (ファイル監視)
├── rusqlite ✅ (データベース)
├── React 18 ✅ (Frontend)
│   ├── react-router-dom ✅
│   ├── @react-three/fiber 🔨 (VR/AR、追加必要)
│   ├── @react-three/xr 🔨 (WebXR、追加必要)
│   └── @react-three/drei 🔨 (3D helpers、追加必要)
└── three.js 🔨 (3D rendering、追加必要)
```

---

## 🎯 統合方針

### Phase 3: VR/AR統合

#### package.json更新

```json
{
  "dependencies": {
    // 既存の依存関係
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-router-dom": "^6.20.0",
    
    // VR/AR追加
    "@react-three/fiber": "^8.15.0",
    "@react-three/drei": "^9.92.0",
    "@react-three/xr": "^6.2.0",
    "three": "^0.160.0"
  }
}
```

#### ファイル移植マッピング

| 移植元 (prism-web) | 移植先 (tauri-gui) | 変更内容 |
|------------------|-------------------|---------|
| `components/visualizations/Scene3DVXR.tsx` | `src/components/vr/Scene4D.tsx` | 時間軸（W軸）追加 |
| `components/visualizations/VRInterface.tsx` | `src/components/vr/VRInterface.tsx` | Tauri統合 |
| `lib/xr/hand-tracking.ts` | `src/lib/xr/hand-tracking.ts` | そのまま移植 |
| `app/(vr)/git-vr/page.tsx` | `src/pages/GitVR.tsx` | React Router対応 |

### Phase 4: カーネルドライバー完全実装

#### IOCTL実装マッピング

| IOCTL Code | 関数 | 実装場所 |
|-----------|------|---------|
| `0x222010` | `HandleGetGpuStatus` | `ioctl_handlers.c` |
| `0x222014` | `HandleGetMemoryPool` | `ioctl_handlers.c` |
| `0x222018` | `HandleGetSchedulerStats` | `ioctl_handlers.c` |
| `0x22201C` | `HandleAllocPinned` | `ioctl_handlers.c` |
| `0x222020` | `HandleFreePinned` | `ioctl_handlers.c` |

#### GPU統合方法

```c
// gpu_integration.c (既存361行) の関数を利用
NTSTATUS HandleGetGpuStatus(PIRP Irp) {
    GPU_STATUS status;
    DxGetGpuUtilization(&status.utilization);  // DirectX 12
    CudaGetDeviceInfo(&status.cuda_info);      // CUDA
    NvGetTemperature(&status.temperature);     // NVAPI
    // ... Irpに書き込み
}
```

---

## 🚀 実装優先順位

### Sprint 1: VR/AR統合（Phase 3）

**期間**: 1-2日  
**目標**: WebXR対応Git可視化の動作確認

**タスク**:
1. package.json更新（Three.js関連追加）
2. Scene3DVXR移植 → Scene4D.tsx作成
3. VRInterface.tsx移植
4. hand-tracking.ts移植
5. GitVR.tsx作成（VRページ）
6. App.tsx更新（ルート追加）

### Sprint 2: カーネルドライバー実装（Phase 4）

**期間**: 2-3日  
**目標**: 実ドライバーとの通信成功

**タスク**:
1. ioctl_handlers.c実装
2. ai_driver.c更新（IOCTL分岐追加）
3. テスト署名＆インストール
4. kernel_bridge.rs更新（実データ取得）
5. KernelStatus.tsx更新（リアルタイム表示）

### Sprint 3: 4D可視化（Phase 5）

**期間**: 1-2日  
**目標**: 時間軸操作の実装

**タスク**:
1. Scene4D.tsx に時間軸（W軸）追加
2. VRコントローラー対応
3. Timeline操作実装
4. アニメーション実装

### Sprint 4: ビルドシステム（Phase 7-8）

**期間**: 1日  
**目標**: 差分ビルド＆自動インストール

**タスク**:
1. build-unified.ps1作成
2. install-unified.ps1作成
3. test-security-unified.ps1作成

---

## 📦 統合後の機能マップ

### Desktop Mode（通常使用）

```
システムトレイ常駐
├── ファイル監視（リアルタイム）
├── 変更履歴トラッキング（SQLite）
├── Blueprint管理（AI支援）
├── Deep Research
├── MCP Tools
├── Kernel Status（GPU/Memory/Scheduler）
└── 設定（自動起動/テーマ）
```

### VR Mode（Quest 3/SteamVR）

```
WebXR起動
├── 4D Git可視化（時間軸追加）
├── VRコントローラー操作
│   ├── Thumbstick: 時間軸移動
│   ├── Trigger: コミット選択
│   ├── Grip: 空間移動
│   └── Buttons: ブランチ切り替え
├── Hand Tracking（Quest 3 Pro）
├── Spatial Audio（3D音響）
└── VR UI Panel（Blueprint/Settings）
```

### AR Mode（Quest 3 Passthrough）

```
Passthrough AR
├── Git可視化オーバーレイ
├── 現実空間にコミットノード配置
├── Hand Gestureでコミット操作
└── AI支援情報表示
```

---

## 🔗 API統合方針

### codex-core直接依存（CLI subprocess削除）

**Before (codex_bridge.rs)**:
```rust
// CLI subprocess呼び出し
let output = Command::new("codex")
    .arg("blueprint").arg("create")
    .output()?;
```

**After (core_bridge.rs)**:
```rust
// 直接依存
use codex_core::blueprint::BlueprintExecutor;

let executor = BlueprintExecutor::new()?;
let blueprint = executor.create(description)?;
```

**メリット**:
- ✅ 高速化（プロセス起動オーバーヘッド削減）
- ✅ 型安全性（Rust型システム活用）
- ✅ メモリ効率向上

### カーネルドライバー統合

**Before (kernel_bridge.rs)**:
```rust
// シミュレーションデータ
fn get_simulated_gpu_status() -> GpuStatus {
    GpuStatus { utilization: 45.2, ... }
}
```

**After (kernel_bridge.rs)**:
```rust
// 実ドライバー通信
use codex_win_api::AiDriverHandle;

let driver = AiDriverHandle::open()?;
let gpu_status = driver.get_gpu_status()?;  // リアルデータ
```

**前提条件**:
- ドライバーインストール済み
- `codex-win-api`のIOCTL実装完了

---

## 🎮 VR/AR技術スタック

### WebXR（ブラウザベースVR）

| ライブラリ | バージョン | 用途 |
|----------|----------|------|
| `@react-three/fiber` | ^8.15.0 | React + Three.js統合 |
| `@react-three/xr` | ^6.2.0 | WebXR API |
| `@react-three/drei` | ^9.92.0 | 3D helpers |
| `three` | ^0.160.0 | 3Dレンダリング |

### 対応デバイス

| デバイス | 対応機能 | 実装状況 |
|---------|---------|---------|
| **Meta Quest 3** | WebXR, 120Hz, Passthrough AR | 🔨 実装予定 |
| **Meta Quest 3 Pro** | + Eye tracking, Face tracking | 🔨 実装予定 |
| **SteamVR** | Index, Vive, Pimax | 🔨 実装予定 |
| **Apple Vision Pro** | Hand tracking, Eye tracking | 🔨 将来対応 |
| **VRChat** | Avatar, World | 🔨 将来対応 |

---

## 📊 パフォーマンス設計

### メモリバジェット

| モード | 目標メモリ | 内訳 |
|--------|----------|------|
| **Desktop** | < 150MB | Tauri(50MB) + React(40MB) + DB(10MB) + その他(50MB) |
| **VR (10Kコミット)** | < 250MB | Desktop(150MB) + Three.js(60MB) + VR(40MB) |
| **VR (100Kコミット)** | < 500MB | Desktop(150MB) + InstancedMesh(250MB) + VR(100MB) |

### VRフレームレート目標

| デバイス | 目標FPS | 最低FPS | 実装技術 |
|---------|---------|---------|---------|
| Quest 3 | 120Hz | 90Hz | Foveated rendering, Dynamic LOD |
| Quest Pro | 90Hz | 72Hz | Eye tracking, Reprojection |
| SteamVR (Index) | 144Hz | 90Hz | Async reprojection |

### ビルド時間目標

| ビルド種類 | 初回 | 差分 | 技術 |
|-----------|------|------|------|
| **Rust (release)** | 10-15分 | 1-2分 | sccache, incremental |
| **Frontend** | 30秒 | 5秒 | Vite HMR |
| **MSI生成** | 1分 | 30秒 | キャッシュ |
| **合計** | ~15分 | ~2分 | 並列ビルド |

---

## 🔒 セキュリティ設計

### WebXR Permission Model

```typescript
// XR Session要求
const session = await navigator.xr.requestSession('immersive-vr', {
  requiredFeatures: ['local-floor', 'hand-tracking'],
  optionalFeatures: ['eye-tracking', 'face-tracking']
})

// ユーザー許可が必要
// → セキュリティ確保
```

### カーネルドライバーサンドボックス

```c
// 入力検証
NTSTATUS ValidateIoctlInput(PVOID InputBuffer, ULONG InputLength) {
    if (!InputBuffer || InputLength == 0) return STATUS_INVALID_PARAMETER;
    if (InputLength > MAX_IOCTL_SIZE) return STATUS_BUFFER_TOO_SMALL;
    
    // Probeメモリアクセス
    ProbeForRead(InputBuffer, InputLength, 1);
    
    return STATUS_SUCCESS;
}
```

---

## 🎊 統合完了後の機能

### ✅ 実現される機能

1. **Windows常駐型AIアシスタント**
   - システムトレイ常駐
   - ファイル変更リアルタイム監視
   - 自動Blueprint提案

2. **VR/AR Git可視化**
   - Quest 3でGit履歴を4D空間で探索
   - Hand trackingでコミット操作
   - 時間軸をVRコントローラーで操作

3. **AIネイティブOS**
   - カーネルレベルでGPU最適化
   - AI推論高速化（30-50%削減）
   - リアルタイムGPU統計

4. **統合開発環境**
   - コード変更検知 → AI分析 → Blueprint提案
   - VRでコード構造可視化
   - AI支援デバッグ

---

## 📈 実装ロードマップ

### Week 1（現在）
- ✅ codex-tauri→tauri-gui統合完了
- 🔨 VR/AR機能移植開始

### Week 2
- 🔨 カーネルドライバー完全実装
- 🔨 4D可視化実装

### Week 3
- 🔨 ビルドシステム完成
- 🔨 セキュリティテスト

### Week 4
- 🔨 パフォーマンス最適化
- 🔨 ドキュメント完成
- 🎉 v1.2.0リリース

---

**作成日**: 2025-11-03  
**ステータス**: 🔨 統合進行中  
**次回更新**: VR/AR統合完了時

