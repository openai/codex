# 🛠️ 開発支援スクリプト

このディレクトリには、zapabob/codex開発を効率化するスクリプトが含まれています。

---

## 📝 実装ログ自動生成

新しい実装ログファイルをテンプレートから自動生成します。

### 使い方

```powershell
# Windows PowerShell
.\scripts\new-implementation-log.ps1 "機能名"

# 例
.\scripts\new-implementation-log.ps1 "scraperクレート完全統合"
```

### 生成されるファイル

```
_docs/2025-10-11_scraperクレート完全統合.md
```

### テンプレート内容

- 実装日時（自動）
- バージョン（VERSIONファイルから自動取得）
- 実装内容セクション
- 完了条件チェックリスト
- テスト結果セクション
- コミット情報セクション
- 今後の課題セクション

---

## 🔢 バージョン更新

VERSIONファイルをセマンティックバージョニングに従って更新します。

### 使い方

```powershell
# Patch更新（バグ修正）
.\scripts\bump-version.ps1 patch
# 0.47.0-alpha.1 → 0.47.1-alpha.1

# Minor更新（新機能追加）
.\scripts\bump-version.ps1 minor
# 0.47.0-alpha.1 → 0.48.0-alpha.1

# Major更新（Breaking Change）
.\scripts\bump-version.ps1 major
# 0.47.0-alpha.1 → 1.0.0-alpha.1
```

### 更新後の作業

スクリプト実行後、以下のファイルも手動で更新してください：

1. **CHANGELOG.md**
   ```markdown
   ## [0.48.0-alpha.1] - 2025-10-12
   
   ### Added
   - scraperクレート完全統合
   ```

2. **codex-rs/Cargo.toml**
   ```toml
   [workspace.package]
   version = "0.48.0-alpha.1"
   ```

3. **codex-cli/package.json**
   ```json
   {
     "version": "0.48.0-alpha.1"
   }
   ```

4. **コミット**
   ```bash
   git add VERSION CHANGELOG.md codex-rs/Cargo.toml codex-cli/package.json
   git commit -m "chore: bump version to 0.48.0-alpha.1"
   ```

---

## 📊 スクリプト一覧

| スクリプト | 用途 | 使用頻度 |
|-----------|------|---------|
| **new-implementation-log.ps1** | 実装ログ生成 | 高（実装後毎回） |
| **bump-version.ps1** | バージョン更新 | 中（リリース時） |

---

## 🚀 今後の追加予定

### pre-commit フック（優先度: 🟡 中）

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "Running pre-commit checks..."

# Format
cargo fmt --all --check || {
    echo "❌ Format check failed. Run: cargo fmt --all"
    exit 1
}

# Clippy
cargo clippy --all-targets --all-features -- -D warnings || {
    echo "❌ Clippy check failed."
    exit 1
}

# Tests
cargo test --all-features || {
    echo "❌ Tests failed."
    exit 1
}

echo "✅ All checks passed!"
```

**インストール方法**:
```bash
cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

---

## 💡 スクリプト開発ガイドライン

新しいスクリプトを追加する際は、以下を遵守してください：

### 1. ファイル名規約

```
動詞-対象.ps1   # PowerShell（Windows）
動詞-対象.sh    # Bash（macOS/Linux）

例:
- new-implementation-log.ps1
- bump-version.ps1
- install-hooks.sh
```

### 2. 必須要素

```powershell
# 1. ヘッダーコメント
# 🛠️ スクリプトの説明
# Usage: .\scripts\スクリプト名.ps1 <引数>

# 2. パラメータ定義
param(
    [Parameter(Mandatory=$true)]
    [string]$RequiredParam
)

# 3. エラーハンドリング
try {
    # 処理
} catch {
    Write-Host "❌ Error: $_" -ForegroundColor Red
    exit 1
}

# 4. 成功メッセージ
Write-Host "✅ Success!" -ForegroundColor Green
```

### 3. ドキュメント

新しいスクリプトを追加したら、このREADMEを更新してください。

---

## 🔗 関連ドキュメント

- [.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md](../.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md) - 開発フロー全体
- [docs/contributing.md](../docs/contributing.md) - コントリビューションガイド
- [docs/install.md](../docs/install.md) - インストール手順

---

## 📝 変更履歴

| 日付 | 変更内容 | 追加者 |
|------|---------|--------|
| 2025-10-11 | 初版作成（new-implementation-log.ps1, bump-version.ps1） | AI Assistant |

---

**🎉 ええスクリプトを！完璧や！！！ 🎉**

