<!-- f02b31a8-5b45-4245-b01f-711cbef26e42 cd2bfab8-020e-4aab-9a21-39952ff0c172 -->
# Ollama統合修正計画（公式リポジトリ整合性維持）

## 方針

- ✅ **codex-ollamaを保持**（公式リポジトリとの整合性）
- ❌ **core/inference削除**（循環依存の原因）
- ✅ **TUI/EXECで直接codex-ollama使用**（シンプルな実装）

---

## Step 1: core/inferenceの削除とロールバック

### 1.1 inference/ディレクトリ削除

```bash
rm -rf codex-rs/core/src/inference/
```

### 1.2 core/lib.rs からinference参照削除

**ファイル**: `codex-rs/core/src/lib.rs`

**削除する行**:

```rust
#[cfg(feature = "ollama")]
pub mod inference;
```

### 1.3 core/Cargo.toml からollama feature削除

**ファイル**: `codex-rs/core/Cargo.toml`

**削除**:

- `[features]` の `ollama = []`
- `tokio-stream = { workspace = true }` （他で使ってなければ）

---

## Step 2: workspace設定の復元

### 2.1 Cargo.tomlにcodex-ollama復元

**ファイル**: `codex-rs/Cargo.toml`

**復元**:

```toml
members = [
    # ...
    "ollama",  # この行を追加
    # ...
]

[workspace.dependencies]
codex-ollama = { path = "ollama" }  # この行を追加
```

### 2.2 tui/Cargo.toml復元

**ファイル**: `codex-rs/tui/Cargo.toml`

**復元**:

```toml
[dependencies]
codex-ollama = { workspace = true }
```

### 2.3 exec/Cargo.toml復元

**ファイル**: `codex-rs/exec/Cargo.toml`

**復元**:

```toml
[dependencies]
codex-ollama = { workspace = true }
```

---

## Step 3: CLI統合（既存フラグを活用）

### 3.1 CLI main.rs確認

**ファイル**: `codex-rs/cli/src/main.rs`

**既に実装済みのフラグを確認**:

```rust
/// Use Ollama for local inference
#[clap(long, global = true)]
pub use_ollama: bool,

/// Ollama model name
#[clap(long, global = true, default_value = "gpt-oss:20b")]
pub ollama_model: String,

/// Ollama server URL
#[clap(long, global = true)]
pub ollama_url: Option<String>,
```

**→ このフラグはそのまま保持（TUI/EXECに渡す）**

---

## Step 4: ビルド確認

### 4.1 全体ビルド

```bash
cd codex-rs
cargo build --all-features
```

**期待結果**: エラー0、警告0

### 4.2 個別クレートビルド

```bash
cargo build -p codex-core
cargo build -p codex-ollama
cargo build -p codex-tui
cargo build -p codex-exec
cargo build -p codex-cli
```

---

## Step 5: 実装ログ更新

**ファイル**: `_docs/2025-11-06_phase2-ollama-implementation.md`

**更新内容**:

- 方針変更の記録
- codex-ollama保持の理由（公式リポジトリ整合性）
- 循環依存解決方法の変更

---

## 完成基準

- [ ] `core/src/inference/` 完全削除
- [ ] `core/lib.rs` からinference参照削除
- [ ] `codex-rs/Cargo.toml` にcodex-ollama復元
- [ ] `tui/Cargo.toml` にcodex-ollama復元
- [ ] `exec/Cargo.toml` にcodex-ollama復元
- [ ] ビルド成功（警告0、エラー0）
- [ ] 実装ログ更新

---

## 実装順序

1. **core/inference削除** (2分)
2. **workspace設定復元** (3分)
3. **ビルド確認** (5分)
4. **実装ログ更新** (2分)

**推定所要時間**: 12分

---

## 備考

- この方針により、公式codexリポジトリとの整合性を保持
- 既存のcodex-ollama実装をそのまま活用
- 循環依存を完全に回避
- 最小限の変更で問題解決

### To-dos

- [x] 全コードをLLMOps/AIエンジニア/ソフトウェア工学観点でレビュー
- [x] 評価ログ作成 (_docs/2025-11-06_code-review-evaluation.md)
- [x] 改善方針ロードマップ作成
- [x] README.md v2.0.0改訂（時系列、インストール手順）
- [x] architecture-v2.0.0.mmd作成
- [x] PNG変換（X: 1200x630, LinkedIn: 1200x627）
- [x] TUI Git 4D可視化実装 (xyz+t) - 基礎完成
- [x] npmパッケージ化 (@zapabob/codex-cli)
- [x] render_timelineメソッド実装