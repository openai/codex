# ⚡ Codex Rust ビルド高速化ガイド

**最終更新**: 2025-10-15  
**対象バージョン**: codex v0.48.0  
**推定効果**: フルビルド 15〜25分 → **3〜7分**、差分ビルド **30秒〜2分**

---

## 🚀 即効性のある高速化（今すぐ実行可能）

### 1. sccache導入（最優先）

**効果**: 2回目以降のビルドが **70〜90%高速化**

```powershell
# 自動インストール&セットアップ
cd codex-rs
.\install-sccache.ps1

# 手動インストール
cargo install sccache
$env:RUSTC_WRAPPER = "sccache"

# PowerShellプロファイルに追加（永続化）
Add-Content $PROFILE "`n`$env:RUSTC_WRAPPER = 'sccache'"
```

**確認**:
```powershell
sccache --show-stats

# 出力例:
# Compile requests: 150
# Cache hits: 120 (80%)
# Cache misses: 30 (20%)
```

---

### 2. 並列ビルドジョブ数最適化

**効果**: CPU使用率最適化、ビルド時間 **10〜20%短縮**

すでに `.cargo/config.toml` に設定済み：

```toml
[build]
jobs = 8                    # RTX3080システム（論理16コア）で最適値
```

**カスタマイズ**:
```powershell
# CPU論理コア数確認
Get-WmiObject Win32_Processor | Select-Object NumberOfLogicalProcessors

# 推奨値: 論理コア数 - (2〜4)
# 例: 16コア → jobs = 8〜12
```

---

### 3. 増分コンパイル有効化

**効果**: 差分ビルド **50〜70%高速化**

すでに `.cargo/config.toml` に設定済み：

```toml
[build]
incremental = true          # 開発時の差分ビルド高速化
```

**注意**: リリースビルド（`lto = "fat"`）とは併用不可（自動で無効化される）

---

## 🎯 開発フロー別の推奨設定

### パターンA: 日常開発（最速）

```powershell
# 開発用プロファイル（LTO無効）
cargo build -p codex-cli

# ビルド時間: 初回 3〜7分、差分 30秒〜2分
```

**Cargo.toml設定**（既に適用済み）:
```toml
[profile.dev]
opt-level = 0
lto = false
codegen-units = 16
incremental = true
```

---

### パターンB: テスト実行

```powershell
# テスト用プロファイル（最適化なし）
cargo test -p codex-core

# ビルド時間: 2〜5分
```

**Cargo.toml設定**（既に適用済み）:
```toml
[profile.test]
opt-level = 0
lto = false
```

---

### パターンC: リリースビルド（最適化優先）

```powershell
# リリース用プロファイル（フルLTO）
cargo build --release -p codex-cli
cargo install --path cli --force

# ビルド時間: 15〜25分（sccache有効で2回目以降は2〜5分）
```

**Cargo.toml設定**（元から存在）:
```toml
[profile.release]
lto = "fat"                 # 全crate跨ぎ最適化
codegen-units = 1           # 最小バイナリサイズ
strip = "symbols"           # デバッグシンボル削除
```

---

## 📊 ビルド時間比較（実測推定値）

| シナリオ | 現状（最適化前） | sccache有効 | 開発プロファイル | 両方適用 |
|----------|-----------------|------------|-----------------|----------|
| **初回フルビルド** | 15〜25分 | 15〜25分 | 3〜7分 | 3〜7分 |
| **2回目フルビルド** | 15〜25分 | **2〜5分** ⚡ | 3〜7分 | **1〜3分** ⚡ |
| **差分ビルド（1ファイル変更）** | 2〜5分 | **30秒〜1分** ⚡ | 30秒〜2分 | **10〜30秒** ⚡ |
| **cargo clean後** | 15〜25分 | 2〜5分 | 3〜7分 | 1〜3分 |

---

## 🔍 ビルド時間計測方法

### 基本計測

```powershell
# 時間計測（キャッシュクリア）
Measure-Command { 
    cargo clean
    cargo build --release -p codex-cli 
} | Select-Object TotalMinutes

# 出力例: TotalMinutes : 18.5
```

### 詳細計測（cargo-timings）

```powershell
# ビルド時間の詳細HTML生成
cargo build --release -p codex-cli --timings

# 出力: target/cargo-timings/cargo-timing-YYYYMMDDHHMMSS.html
```

**見るべきポイント**:
- 赤色バー（長い）= ボトルネッククレート → tree-sitter, ratatui, tokio
- 並列度グラフ = CPU使用効率 → 低い場合は `jobs` 増やす

---

## 🛠️ トラブルシューティング

### sccacheが効かない

```powershell
# 環境変数確認
echo $env:RUSTC_WRAPPER
# 出力: sccache

# sccache統計リセット
sccache --zero-stats

# 再ビルド後統計確認
cargo build -p codex-cli
sccache --show-stats
```

### ビルドが途中で止まる

```powershell
# メモリ不足の可能性 → jobs数を減らす
# .cargo/config.toml
[build]
jobs = 4                    # 8から4に削減
```

### リリースビルドが遅い

```powershell
# LTOを軽量版に変更（バイナリサイズは増えるが高速）
# Cargo.toml
[profile.release]
lto = "thin"                # "fat" → "thin"
codegen-units = 4           # 1 → 4
```

---

## 🎓 高度な最適化（上級者向け）

### クロスコンパイルキャッシュ（CI/CD用）

```yaml
# .github/workflows/build.yml
- name: Rust Build Cache
  uses: Swatinem/rust-cache@v2
  with:
    shared-key: "codex-release-v48"
    cache-targets: "release"

- name: sccache
  uses: mozilla-actions/sccache-action@v0.0.3
```

### Feature Flags導入（将来的）

```toml
# codex-cli/Cargo.toml
[features]
default = ["tui", "mcp"]
tui = ["codex-tui"]
mcp = ["codex-mcp-server", "codex-deep-research"]
minimal = []                # TUI/MCP無し軽量版

# ビルド例:
# cargo build -p codex-cli --no-default-features --features minimal
# → ビルド時間: 2〜5分
```

---

## 📚 参考資料

- [Fast Rust Builds (matklad)](https://matklad.github.io/2021/09/04/fast-rust-builds.html)
- [The Cargo Book - Build Cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [sccache GitHub](https://github.com/mozilla/sccache)
- [cargo-timings Documentation](https://doc.rust-lang.org/cargo/reference/timings.html)
- [OpenAI Codex Issue #1411](https://github.com/openai/codex/issues/1411) - codegen-units設定の根拠

---

## ✅ チェックリスト

開発環境セットアップ時に確認：

- [ ] sccacheインストール済み（`sccache --version`）
- [ ] 環境変数設定済み（`echo $env:RUSTC_WRAPPER` → `sccache`）
- [ ] `.cargo/config.toml` の `jobs` 設定確認（CPU論理コア数 - 2〜4）
- [ ] 開発時は `cargo build`（devプロファイル）使用
- [ ] リリース時のみ `cargo build --release` 使用
- [ ] ビルド時間計測（初回 vs 2回目で効果確認）

---

**作成者**: AI Assistant (CoT Mode)  
**バージョン**: codex v0.48.0  
**環境**: Windows 11, PowerShell 7.x, Rust 1.80+  
**関連ドキュメント**: `_docs/2025-10-15_Rustビルド時間分析レポート.md`

