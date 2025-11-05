<!-- 035d7493-1dfd-4f5f-a8b2-e7f9e080a1af b618f435-befe-4a11-9ed7-ca291f8afa99 -->
# Codex OS常駐型GUIクライアント完全実装プラン

## 実装方針

**Tauri v2** を用いて、Windows常駐型セキュアGUIクライアントを構築。システムトレイ統合、ファイルシステム監視、既存Codex Rust coreとの統合により、リアルタイムAI支援を実現。

## アーキテクチャ概要

```
┌─────────────────────────────────────┐
│  Tauri Frontend (React/TypeScript)  │
│  - System Tray Menu                 │
│  - Dashboard UI                     │
│  - File Change Notifications        │
└──────────────┬──────────────────────┘
               │ IPC (invoke/emit)
┌──────────────▼──────────────────────┐
│  Tauri Rust Backend                 │
│  - Tray Management                  │
│  - File System Watcher              │
│  - Codex Core Integration           │
│  - Windows Service Bridge           │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│  Existing Codex Core                │
│  - Blueprint Executor               │
│  - MCP Server                       │
│  - Deep Research                    │
└─────────────────────────────────────┘
```

## Phase 1: Tauri基盤構築

### 1.1 Tauri v2プロジェクト初期化

**新規ディレクトリ**: `codex-tauri/`

```bash
npm create tauri-app@latest codex-tauri
# Framework: React + TypeScript
# Bundler: Vite
```

**主要ファイル**:

- `src-tauri/Cargo.toml` - Rust依存関係
- `src-tauri/tauri.conf.json` - Tauri設定
- `src-tauri/src/main.rs` - エントリーポイント
- `src/` - React frontend

**依存関係追加**:

```toml
# Cargo.toml
tauri = { version = "2.0", features = ["system-tray", "notification"] }
notify = "6.1"  # File system watcher
tokio = { version = "1.42", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
codex-core = { path = "../codex-rs/core" }  # 既存コア統合
```

### 1.2 システムトレイ統合

**実装箇所**: `src-tauri/src/tray.rs`

**機能**:

- トレイアイコン表示（Codexロゴ）
- コンテキストメニュー（Show/Hide, Start/Stop Monitoring, Quit）
- 通知表示（ファイル変更検知時）
- 左クリック: ウィンドウ表示/非表示
- 右クリック: メニュー表示

**メニュー構成**:

```
┌─────────────────────────┐
│ 📊 Dashboard を開く      │
│ ───────────────────────  │
│ ✅ ファイル監視: ON      │
│ 🔄 Codex Core: 起動中    │
│ ───────────────────────  │
│ ⚙️ Settings              │
│ 📖 Docs                  │
│ ❌ Quit                  │
└─────────────────────────┘
```

### 1.3 自動起動設定

**実装箇所**: `src-tauri/src/autostart.rs`

**機能**:

- Windows起動時に自動起動
- レジストリ登録: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`
- UI設定でON/OFF切り替え可能

## Phase 2: ファイルシステム監視

### 2.1 Windows File System Watcher

**実装箇所**: `src-tauri/src/watcher.rs`

**使用ライブラリ**: `notify` (cross-platform, Windows backend: ReadDirectoryChangesW)

**監視対象**:

- ワークスペースディレクトリ（ユーザー指定）
- `.gitignore` 尊重
- フィルター: `.rs`, `.ts`, `.py`, `.md`, `.toml`, etc.

**検知イベント**:

- `FileCreated` - 新規ファイル作成
- `FileModified` - ファイル変更
- `FileDeleted` - ファイル削除
- `FileRenamed` - ファイルリネーム

**イベント処理**:

1. Debounce (300ms) - 連続変更を1つにまとめる
2. Diff計算 - Git diff相当
3. Frontend通知 - Toast表示
4. 自動Blueprint提案（オプション）

### 2.2 変更履歴トラッキング

**実装箇所**: `src-tauri/src/change_tracker.rs`

**機能**:

- 変更履歴をSQLite DBに保存
- 統計情報: 最も変更されたファイル、時間帯別アクティビティ
- UI: Timeline表示

**DB Schema**:

```sql
CREATE TABLE file_changes (
    id INTEGER PRIMARY KEY,
    timestamp DATETIME,
    file_path TEXT,
    change_type TEXT, -- Created/Modified/Deleted
    diff_lines_added INTEGER,
    diff_lines_removed INTEGER
);
```

### 2.3 自動Blueprint生成（オプション機能）

**実装箇所**: `src-tauri/src/auto_blueprint.rs`

**トリガー条件**:

- 10ファイル以上変更
- または重要ファイル変更（`Cargo.toml`, `package.json`, `requirements.txt`）

**動作**:

1. 変更ファイル一覧取得
2. Codex Core呼び出し: Blueprint Draft作成
3. Frontend通知: "Blueprint提案があります"
4. ユーザー確認後に実行

## Phase 3: Codex Core統合

### 3.1 Core APIブリッジ

**実装箇所**: `src-tauri/src/codex_bridge.rs`

**統合方法**:

- **Option A**: 既存Codex CLIをサブプロセスで起動
- **Option B**: `codex-core` crateを直接依存（推奨）

**公開API**:

```rust
// Blueprint操作
async fn create_blueprint(description: String) -> Result<Blueprint>
async fn execute_blueprint(id: String) -> Result<ExecutionResult>
async fn list_blueprints() -> Result<Vec<Blueprint>>

// Deep Research
async fn research(query: String, depth: u8) -> Result<ResearchReport>

// MCP Server操作
async fn list_mcp_tools() -> Result<Vec<Tool>>
async fn invoke_mcp_tool(name: String, args: Value) -> Result<Value>
```

### 3.2 リアルタイム進捗通知

**実装箇所**: `src-tauri/src/events.rs`

**Tauri Event System使用**:

```rust
// Backend → Frontend
app.emit_all("blueprint:progress", progress_data);
app.emit_all("file:changed", file_change_data);
app.emit_all("notification", notification_data);
```

**Frontend購読**:

```typescript
import { listen } from '@tauri-apps/api/event'

listen('blueprint:progress', (event) => {
  // Progress bar更新
})
```

### 3.3 セキュリティサンドボックス

**Tauri設定**: `tauri.conf.json`

```json
{
  "tauri": {
    "allowlist": {
      "fs": {
        "scope": ["$APPDATA/codex/*", "$WORKSPACE/*"]
      },
      "shell": {
        "sidecar": false,
        "execute": false
      },
      "protocol": {
        "asset": true,
        "assetScope": ["$RESOURCE/*"]
      }
    },
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  }
}
```

## Phase 4: Frontend UI実装

### 4.1 Dashboard画面

**実装箇所**: `codex-tauri/src/pages/Dashboard.tsx`

**コンポーネント**:

- **Status Card**: Codex Core起動状態、ファイル監視状態
- **Recent Changes**: 最近の変更ファイル一覧（最大20件）
- **Blueprints**: 実行中/完了Blueprint一覧
- **Quick Actions**: "New Blueprint", "Research", "Open Workspace"

**デザイン**: Tailwind CSS + shadcn/ui

### 4.2 Settings画面

**実装箇所**: `codex-tauri/src/pages/Settings.tsx`

**設定項目**:

- ワークスペースパス選択
- 自動起動 ON/OFF
- ファイル監視 ON/OFF
- 通知設定（トースト表示、音声）
- テーマ（Light/Dark/System）
- 監視除外パターン（.gitignore追加）

### 4.3 Blueprints管理画面

**実装箇所**: `codex-tauri/src/pages/Blueprints.tsx`

**機能**:

- Blueprint一覧表示（Pending/Approved/Executing/Completed）
- 新規作成フォーム
- 実行ボタン + リアルタイム進捗表示
- ロールバック機能
- 既存prism-webと同等機能

## Phase 5: Windows統合強化

### 5.1 Windows通知統合

**実装箇所**: `src-tauri/src/notifications.rs`

**使用**: Tauri `notification` feature + Windows Toast API

**通知種類**:

- ファイル変更検知
- Blueprint実行完了/失敗
- エラー発生

**動作**:

```rust
use tauri::api::notification::Notification;

Notification::new("com.codex.app")
    .title("Blueprint実行完了")
    .body("機能追加のBlueprintが正常に完了しました")
    .show()?;
```

### 5.2 ショートカットキー

**実装箇所**: `src-tauri/src/shortcuts.rs`

**グローバルホットキー**:

- `Ctrl+Shift+C` - Dashboard表示/非表示
- `Ctrl+Shift+B` - 新規Blueprint作成
- `Ctrl+Shift+R` - Deep Research起動

**使用**: `tauri-plugin-global-shortcut`

### 5.3 コンテキストメニュー統合（将来）

**実装箇所**: `kernel-extensions/windows/context_menu/`

**機能**: Explorerのコンテキストメニューに「Codexで解析」追加

**技術**: Windows Shell Extension (COM)

## Phase 6: パッケージング & デプロイ

### 6.1 Tauri Bundler設定

**設定箇所**: `src-tauri/tauri.conf.json`

```json
{
  "bundle": {
    "identifier": "com.codex.app",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.ico"
    ],
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": ""
    },
    "externalBin": ["codex-core"]
  }
}
```

### 6.2 インストーラー作成

**出力形式**: `.msi` (Windows Installer)

**ビルドコマンド**:

```bash
cd codex-tauri
npm run tauri build
```

**出力先**: `src-tauri/target/release/bundle/msi/Codex_0.1.0_x64.msi`

### 6.3 自動更新機能

**使用**: `tauri-plugin-updater`

**動作**:

- 起動時にGitHub Releasesチェック
- 新バージョンあり → 通知表示
- ユーザー承認後に自動ダウンロード & インストール

## 重要ファイル一覧

### 新規作成ファイル（27ファイル想定）

**Rust Backend**:

- `codex-tauri/src-tauri/src/main.rs` - エントリーポイント
- `codex-tauri/src-tauri/src/tray.rs` - システムトレイ
- `codex-tauri/src-tauri/src/watcher.rs` - ファイル監視
- `codex-tauri/src-tauri/src/change_tracker.rs` - 変更履歴
- `codex-tauri/src-tauri/src/auto_blueprint.rs` - 自動Blueprint
- `codex-tauri/src-tauri/src/codex_bridge.rs` - Core統合
- `codex-tauri/src-tauri/src/events.rs` - イベント管理
- `codex-tauri/src-tauri/src/autostart.rs` - 自動起動
- `codex-tauri/src-tauri/src/notifications.rs` - 通知
- `codex-tauri/src-tauri/src/shortcuts.rs` - ショートカット
- `codex-tauri/src-tauri/src/db.rs` - SQLite DB
- `codex-tauri/src-tauri/Cargo.toml` - 依存関係

**Frontend**:

- `codex-tauri/src/main.tsx` - Reactエントリー
- `codex-tauri/src/App.tsx` - ルートコンポーネント
- `codex-tauri/src/pages/Dashboard.tsx` - ダッシュボード
- `codex-tauri/src/pages/Settings.tsx` - 設定
- `codex-tauri/src/pages/Blueprints.tsx` - Blueprint管理
- `codex-tauri/src/components/StatusCard.tsx` - ステータス表示
- `codex-tauri/src/components/RecentChanges.tsx` - 変更一覧
- `codex-tauri/src/hooks/useTauriEvents.ts` - イベント購読
- `codex-tauri/src/hooks/useCodexAPI.ts` - API呼び出し
- `codex-tauri/src/lib/api.ts` - API定義

**設定**:

- `codex-tauri/src-tauri/tauri.conf.json` - Tauri設定
- `codex-tauri/package.json` - npm依存関係
- `codex-tauri/vite.config.ts` - Vite設定
- `codex-tauri/tsconfig.json` - TypeScript設定
- `codex-tauri/README.md` - ドキュメント

## セキュリティ考慮事項

### 1. サンドボックス化

- Tauri allowlistで許可されたAPIのみ使用
- ファイルアクセススコープ制限（ワークスペース + AppData のみ）
- shell実行禁止

### 2. 通信暗号化

- IPC通信は内部（セキュア）
- 外部API呼び出し時はHTTPS必須

### 3. 権限管理

- 管理者権限不要（通常ユーザーで動作）
- レジストリ書き込みはHKCU（Current User）のみ

### 4. コード署名（本番環境）

- Windows Authenticode署名
- 証明書取得 → `tauri.conf.json`に設定

## パフォーマンス目標

| 指標 | 目標 |

|------|------|

| 起動時間 | < 2秒 |

| メモリ使用量 | < 150MB |

| CPU使用率（アイドル） | < 1% |

| ファイル変更検知遅延 | < 500ms |

| UI応答速度 | < 100ms |

## 既存コンポーネント活用

- `codex-rs/core` - Blueprint, MCP, Research機能
- `codex-rs/cli` - CLIコマンド（参考）
- `prism-web` - UI デザイン参考
- `.cursorrules` - 開発規約遵守

## 次のステップ（実装後）

1. Tauri初期化 & トレイ統合テスト
2. ファイル監視実装 & Debounceテスト
3. Codex Core統合 & Blueprint実行テスト
4. Frontend UI実装 & デザイン調整
5. Windows通知 & ショートカットテスト
6. ビルド & MSIインストーラー作成
7. 実機テスト（Windows 11）
8. ドキュメント完成 & リリース

## Phase 7: AIネイティブOSカーネル統合

### 7.1 Windowsカーネルドライバー実装

**実装箇所**: `kernel-extensions/windows/ai_driver/`

**既存ファイル活用**:

- `ai_driver.c` - カーネルドライバー本体
- `ai_driver.inf` - ドライバー情報ファイル
- `sources` - ビルド設定

**新規実装機能**:

#### AI Scheduler Driver

```c
// GPU-aware プロセススケジューリング
NTSTATUS AiSchedulerSetPriority(HANDLE ProcessId, AI_PRIORITY Priority);
NTSTATUS AiSchedulerGetGpuStatus(GPU_STATUS *Status);
```

**機能**:

- AI推論プロセスの自動検出（CUDA/DirectML API呼び出し監視）
- GPU利用率に基づく動的優先度調整
- リアルタイムスケジューリングクラス適用

#### AI Memory Manager

```c
// Pinned Memory Pool for GPU Direct Access
NTSTATUS AiMemAllocPinned(SIZE_T Size, PVOID *Address);
NTSTATUS AiMemFreePinned(PVOID Address);
```

**機能**:

- 256MB Pinned Memory Pool（GPU直接アクセス可能）
- 4KB ブロック単位管理
- 断片化防止アルゴリズム

#### GPU Direct Access

```c
// カーネル空間からGPU制御
NTSTATUS AiGpuExecuteKernel(GPU_KERNEL_DESC *Desc);
NTSTATUS AiGpuGetUtilization(FLOAT *Utilization);
```

**統合**:

- NVIDIA CUDA Driver API
- DirectX 12 Compute
- Windows Display Driver Model (WDDM)

### 7.2 ETW (Event Tracing for Windows) 統合

**実装箇所**: `kernel-extensions/windows/etw_provider/`

**既存ファイル**: `ai_etw_provider.man` - ETW マニフェスト

**トレースイベント**:

- AI推論開始/完了
- GPU利用率変化
- メモリアロケーション
- スケジューリング決定

**使用方法**:

```bash
# ETWセッション開始
logman create trace AICodex -p {GUID} -o ai_trace.etl

# リアルタイム監視
tracerpt ai_trace.etl
```

### 7.3 Rust FFI Wrapper

**実装箇所**: `kernel-extensions/windows/codex_win_api/src/lib.rs`

**既存実装拡張**:

```rust
pub struct AiDriver {
    handle: HANDLE,
}

impl AiDriver {
    pub fn new() -> Result<Self>;
    
    // AI Scheduler
    pub fn set_process_priority(&self, pid: u32, priority: AiPriority) -> Result<()>;
    pub fn get_gpu_status(&self) -> Result<GpuStatus>;
    
    // AI Memory
    pub fn alloc_pinned(&self, size: usize) -> Result<*mut u8>;
    pub fn free_pinned(&self, ptr: *mut u8) -> Result<()>;
    
    // GPU Control
    pub fn get_gpu_utilization(&self) -> Result<f32>;
    pub fn execute_kernel(&self, kernel: &GpuKernelDesc) -> Result<()>;
}
```

### 7.4 Tauri統合

**実装箇所**: `codex-tauri/src-tauri/src/kernel_bridge.rs`

**Tauri Command**:

```rust
#[tauri::command]
async fn kernel_get_gpu_status() -> Result<GpuStatus> {
    let driver = AiDriver::new()?;
    driver.get_gpu_status()
}

#[tauri::command]
async fn kernel_optimize_process(pid: u32) -> Result<()> {
    let driver = AiDriver::new()?;
    driver.set_process_priority(pid, AiPriority::High)
}
```

**Frontend呼び出し**:

```typescript
import { invoke } from '@tauri-apps/api/tauri'

const gpuStatus = await invoke('kernel_get_gpu_status')
console.log(`GPU使用率: ${gpuStatus.utilization}%`)
```

### 7.5 UI: カーネルステータス表示

**実装箇所**: `codex-tauri/src/components/KernelStatus.tsx`

**表示情報**:

- ドライバーステータス（Loaded/Not Loaded）
- GPU使用率リアルタイムグラフ
- AI Memory Pool使用状況（256MB中XX MB使用）
- スケジューリング統計（AI優先度プロセス数）

**デザイン**:

```tsx
<Card>
  <h3>AIネイティブOS - カーネル統合</h3>
  <div className="status">
    {driverLoaded ? '✅ ドライバー起動中' : '❌ ドライバー未起動'}
  </div>
  <ProgressBar 
    label="GPU使用率" 
    value={gpuUtilization} 
    max={100}
  />
  <ProgressBar 
    label="AI Memory Pool" 
    value={memUsed} 
    max={256}
  />
</Card>
```

### 7.6 ドライバーインストール & 署名

**実装箇所**: `kernel-extensions/windows/install.ps1`

**インストール手順**:

#### 開発環境（テスト署名）

```powershell
# テストモード有効化
bcdedit /set testsigning on

# ドライバーインストール
pnputil /add-driver ai_driver.inf /install

# サービス開始
sc start AiDriver
```

#### 本番環境（正式署名）

```powershell
# EV証明書でコード署名
signtool sign /v /ac "MSCV-VSClass3.cer" /s MY /n "YourCert" /t http://timestamp.digicert.com ai_driver.sys

# Windows Hardware Quality Labs (WHQL) 認証
# → Microsoft Partner Center経由で申請
```

**注意事項**:

- Windows 10/11 はDriver署名必須
- Secure Boot環境では正式署名が必要
- 開発時は`bcdedit /set testsigning on`で回避可能

### 7.7 セキュリティ & 安定性

**カーネルパニック対策**:

- 入力検証徹底（ユーザーモードからの全IOCTLリクエスト）
- メモリ境界チェック（Buffer Overflow防止）
- 例外ハンドラー完備

**権限管理**:

- ドライバーロードには管理者権限必須
- Tauri側で`require_admin` manifest設定

**ロールバック**:

- ドライバー障害時の自動無効化
- Event Logへのエラー記録
- UI上で警告表示

### 7.8 パフォーマンス測定

**ベンチマーク**: `kernel-extensions/windows/benchmarks/`

**測定項目**:

- IOCTL呼び出しオーバーヘッド（< 10μs目標）
- Pinned Memory アロケーション速度（vs. VirtualAlloc）
- AI推論スループット（Driverあり/なし比較）

**期待効果**:

- 推論レイテンシ **30-50%削減**
- スループット **2-3倍向上**
- GPU利用率 **15-20%向上**

### 7.9 アーキテクチャ全体図（カーネル統合版）

```
┌─────────────────────────────────────┐
│  Tauri Frontend (React/TypeScript)  │
│  - Dashboard + Kernel Status        │
│  - GPU Utilization Graph            │
└──────────────┬──────────────────────┘
               │ Tauri IPC
┌──────────────▼──────────────────────┐
│  Tauri Rust Backend                 │
│  - kernel_bridge.rs                 │
│  - codex_bridge.rs                  │
└──────┬───────────────────────┬──────┘
       │                       │
       │ FFI                   │ In-process
       │                       │
┌──────▼─────────┐    ┌────────▼──────┐
│ codex_win_api  │    │  Codex Core   │
│ (Rust Wrapper) │    │  (Blueprint)  │
└──────┬─────────┘    └───────────────┘
       │ DeviceIoControl
       │
┌──────▼──────────────────────────────┐
│  Windows Kernel (Ring 0)            │
│  ┌──────────────────────────────┐   │
│  │  ai_driver.sys               │   │
│  │  - AI Scheduler              │   │
│  │  - AI Memory Manager         │   │
│  │  - GPU Direct Access         │   │
│  └──────────────────────────────┘   │
│  ┌──────────────────────────────┐   │
│  │  ETW Provider                │   │
│  └──────────────────────────────┘   │
└──────┬──────────────────────────────┘
       │
┌──────▼──────────────────────────────┐
│  Hardware                            │
│  - CPU (Scheduler Integration)      │
│  - GPU (RTX 3080)                   │
│  - Memory (Pinned Pool)             │
└─────────────────────────────────────┘
```

## 実装ログ保存先

`_docs/2025-11-03_AI-Native-OS-Implementation.md`