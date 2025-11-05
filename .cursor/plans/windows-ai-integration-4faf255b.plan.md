<!-- f1b32a57-ba87-4227-8742-cf20de5a579c 200feb15-e3e8-4d4a-86f9-d9cae7c87914 -->
# モック実装→本番実装移行計画

## 対象モック実装

### 1. Git操作（src-tauri/src/main.rs）

**現状**:

```rust
#[tauri::command]
async fn get_git_commits_3d(repo_path: String, limit: usize) -> Result<Vec<Commit3D>, String> {
    // TODO: 実際のGit操作実装（git2クレート使用）
    // 現在はモックデータを返す
    for i in 0..limit.min(1000) { /* mock data */ }
}
```

**本番実装**:

- git2クレート使用
- `codex-rs/cli/src/git_cuda.rs`のロジック流用
- 実リポジトリから3D座標計算
- エラーハンドリング強化

### 2. CUDA解析（src-tauri/src/main.rs）

**現状**:

```rust
#[tauri::command]
async fn analyze_with_cuda(commits: Vec<Commit3D>) -> Result<Vec<Commit3D>, String> {
    // TODO: CUDA並列解析実装
    // 現在はそのまま返す
    Ok(commits)
}
```

**本番実装**:

- `codex_cuda_runtime::CudaRuntime`直接使用
- 並列座標計算・最適化
- パフォーマンス統計収集

### 3. CLI統合（src-tauri/src/main.rs）

**現状**:

```rust
#[tauri::command]
async fn execute_cli_command(cmd: String, args: Vec<String>) -> Result<String, String> {
    // TODO: codex CLI統合実装
    Ok(format!("Command executed: ..."))
}
```

**本番実装**:

- `std::process::Command`でcodex CLI起動
- stdout/stderrキャプチャ
- 非同期実行（tokio）
- タイムアウト処理

## 実装手順

### Step 1: git2クレート依存追加

**codex-rs/tauri-gui/src-tauri/Cargo.toml**:

```toml
[dependencies]
git2 = { workspace = true }
codex-cuda-runtime = { path = "../../../cuda-runtime", optional = true }

[features]
cuda = ["codex-cuda-runtime"]
```

### Step 2: Git解析モジュール作成

**新規**: `codex-rs/tauri-gui/src-tauri/src/git_analyzer.rs`:

```rust
use git2::{Repository, Oid};
use crate::Commit3D;

pub struct GitAnalyzer {
    repo: Repository,
}

impl GitAnalyzer {
    pub fn new(repo_path: &str) -> Result<Self>;
    pub fn get_commits_3d(&self, limit: usize) -> Result<Vec<Commit3D>>;
    fn calculate_3d_position(/* ... */) -> (f64, f64, f64);
}
```

CLIの`git_cuda.rs`ロジックを移植。

### Step 3: main.rs本番実装

**get_git_commits_3d()書き換え**:

```rust
#[tauri::command]
async fn get_git_commits_3d(repo_path: String, limit: usize) -> Result<Vec<Commit3D>, String> {
    use git_analyzer::GitAnalyzer;
    
    let analyzer = GitAnalyzer::new(&repo_path).map_err(|e| e.to_string())?;
    analyzer.get_commits_3d(limit).map_err(|e| e.to_string())
}
```

**analyze_with_cuda()書き換え**:

```rust
#[tauri::command]
async fn analyze_with_cuda(commits: Vec<Commit3D>) -> Result<Vec<Commit3D>, String> {
    #[cfg(feature = "cuda")]
    {
        use codex_cuda_runtime::CudaRuntime;
        let cuda = CudaRuntime::new(0).map_err(|e| e.to_string())?;
        // CUDA並列計算実装
    }
}
```

**execute_cli_command()書き換え**:

```rust
#[tauri::command]
async fn execute_cli_command(cmd: String, args: Vec<String>) -> Result<String, String> {
    use tokio::process::Command;
    
    let output = Command::new("codex")
        .arg(&cmd)
        .args(&args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}
```

### Step 4: GitVR.tsxフォールバック削除

**src/pages/GitVR.tsx**:

- `loadMockCommits()`は開発用に残す
- 本番では常にTauri IPC使用
- エラー時のみモックにフォールバック

## 主要ファイル

**新規作成**:

- `codex-rs/tauri-gui/src-tauri/src/git_analyzer.rs` (~300行)

**変更**:

- `codex-rs/tauri-gui/src-tauri/Cargo.toml` - git2依存追加
- `codex-rs/tauri-gui/src-tauri/src/main.rs` - 3関数を本番実装に置換
- `codex-rs/tauri-gui/src-tauri/src/lib.rs` - git_analyzerモジュール追加

## 品質保証

- 型エラー0（git2型定義完備）
- 警告0（clippy準拠）
- エラーハンドリング完全
- テスト追加（unit + integration）

### To-dos

- [ ] Phase 1.1: Babylon.js依存関係追加（package.json更新、npm install）
- [ ] Phase 1.2: BabylonGitScene.tsx実装（10万コミット対応、動的LOD、PBRマテリアル）
- [ ] Phase 1.2: babylon-git-engine.ts実装（エンジン初期化、コミット読み込み、ノード選択）
- [ ] Phase 1.3: babylon-optimizer.ts実装（GPU統計、動的品質調整、Virtual Desktop対応）
- [ ] Phase 1: GitVR.tsxをBabylon.jsに統合、Three.js削除、型チェック・警告0確認
- [ ] Phase 2.1: Tauri IPC拡張（execute_cli_command, get_git_commits_3d, analyze_with_cuda）