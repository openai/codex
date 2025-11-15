# npmパッケージ化 @zapabob/codex

**日時**: 2025-11-15 14:03:00  
**タスク**: GitHubでの配布版としてnpmを`@zapabob/codex`としてパッケージ化  
**バージョン**: 2.1.0

---

## 🎯 実装概要

Githubでの配布版としてnpmを`@zapabob/codex`としてパッケージ化を完了。ルートの`package.json`と`codex-cli/package.json`を統一し、GitHub Actionsでの自動公開設定も更新しました。

---

## 📋 変更内容

### 1. ルートの`package.json`更新 ✅

**変更前**:
```json
{
  "name": "@zapabob/codex-cli",
  "version": "2.1.0",
  ...
}
```

**変更後**:
```json
{
  "name": "@zapabob/codex",
  "version": "2.1.0",
  ...
}
```

**ファイル**: `package.json`

---

### 2. `codex-cli/package.json`更新 ✅

**変更内容**:
- バージョンを`1.2.0`から`2.1.0`に統一
- `description`、`keywords`、`author`、`bugs`、`homepage`を追加
- `engines`を`node >=18.0.0`、`npm >=9.0.0`に更新
- `cpu`フィールドを追加（x64, arm64）
- `files`に`README.md`と`LICENSE`を追加
- `publishConfig`をnpmjs.orgに変更（`access: public`）

**ファイル**: `codex-cli/package.json`

---

### 3. `.npmignore`ファイル作成 ✅

npmパッケージに含めないファイルを指定する`.npmignore`を作成。

**除外対象**:
- 開発ファイル（node_modules、.git、.vscodeなど）
- ビルド成果物（target、dist、buildなど）
- ドキュメント（README.md以外）
- テストファイル
- CI/CD設定
- Rust関連ファイル（Cargo.toml、Cargo.lockなど）
- Python関連ファイル

**ファイル**: `.npmignore`

---

### 4. GitHub Actionsワークフロー更新 ✅

**変更内容**:
- `rust-release.yml`のnpm公開設定を`@openai`から`@zapabob`に変更

**変更箇所**:
```yaml
# 変更前
scope: "@openai"

# 変更後
scope: "@zapabob"
```

**ファイル**: `.github/workflows/rust-release.yml`

---

### 5. README.md更新 ✅

**変更内容**:
- インストール手順を`@zapabob/codex-cli`から`@zapabob/codex`に変更
- npmバッジのURLを更新

**変更箇所**:
- 英語版: `npm install -g @zapabob/codex-cli` → `npm install -g @zapabob/codex`
- 日本語版: 同様に変更
- バッジ: `@zapabob/codex-cli` → `@zapabob/codex`

**ファイル**: `README.md`

---

## 📦 パッケージ情報

### パッケージ名
- **正式名称**: `@zapabob/codex`
- **バージョン**: `2.1.0`
- **スコープ**: `@zapabob`

### 公開設定
- **レジストリ**: `https://registry.npmjs.org/`
- **アクセス**: `public`
- **公開方法**: GitHub Actions自動公開（OIDC認証）

### サポートプラットフォーム
- **OS**: Windows (win32), macOS (darwin), Linux
- **CPU**: x64, arm64
- **Node.js**: >=18.0.0
- **npm**: >=9.0.0

### パッケージ内容
```
@zapabob/codex/
├── bin/
│   └── codex.js          # エントリーポイント
├── vendor/               # プラットフォーム別バイナリ
│   ├── x86_64-pc-windows-msvc/
│   ├── x86_64-apple-darwin/
│   ├── aarch64-apple-darwin/
│   ├── x86_64-unknown-linux-gnu/
│   ├── x86_64-unknown-linux-musl/
│   ├── aarch64-unknown-linux-gnu/
│   ├── aarch64-unknown-linux-musl/
│   └── aarch64-pc-windows-msvc/
├── README.md
├── LICENSE
└── package.json
```

---

## 🚀 公開手順

### 自動公開（GitHub Actions）

1. **タグ作成**:
   ```bash
   git tag -a rust-v2.1.0 -m "Release 2.1.0"
   git push origin rust-v2.1.0
   ```

2. **ワークフロー実行**:
   - `rust-release.yml`が自動実行
   - ビルド → パッケージング → npm公開

### 手動公開（ローカル）

1. **ビルド**:
   ```bash
   cd codex-cli
   python3 scripts/build_npm_package.py \
     --package codex \
     --release-version 2.1.0 \
     --vendor-src ../vendor
   ```

2. **パッケージ作成**:
   ```bash
   npm pack
   ```

3. **公開**:
   ```bash
   npm publish --access public
   ```

---

## 🔍 検証コマンド

### インストールテスト
```bash
# グローバルインストール
npm install -g @zapabob/codex

# バージョン確認
codex --version
# 出力: codex-cli 2.1.0

# ヘルプ確認
codex --help
```

### パッケージ情報確認
```bash
# npmレジストリから情報取得
npm view @zapabob/codex

# バージョン一覧
npm view @zapabob/codex versions

# 最新バージョン
npm view @zapabob/codex version
```

---

## 📊 影響範囲

### 既存ユーザーへの影響
- **破壊的変更**: パッケージ名が`@zapabob/codex-cli`から`@zapabob/codex`に変更
- **移行手順**:
  ```bash
  # 旧パッケージをアンインストール
  npm uninstall -g @zapabob/codex-cli
  
  # 新パッケージをインストール
  npm install -g @zapabob/codex
  ```

### ドキュメント更新
- ✅ README.md（英語版・日本語版）
- ✅ GitHub Actionsワークフロー
- ✅ package.json（ルート・codex-cli）

---

## ✅ 完了項目

- [x] ルートの`package.json`を`@zapabob/codex`に変更
- [x] `codex-cli/package.json`を更新（バージョン統一、メタデータ追加）
- [x] `.npmignore`ファイルを作成
- [x] GitHub Actionsワークフローを`@zapabob`スコープに更新
- [x] README.mdのインストール手順を更新
- [x] npmバッジのURLを更新

---

## 🔗 関連リンク

- **npmパッケージ**: https://www.npmjs.com/package/@zapabob/codex
- **GitHubリポジトリ**: https://github.com/zapabob/codex
- **GitHub Actions**: `.github/workflows/rust-release.yml`

---

## 📝 備考

- npm公開にはOIDC認証を使用（GitHub Actions経由）
- 手動公開の場合は`npm login`が必要
- パッケージサイズは約133.5MB（8プラットフォーム対応）
- クロスプラットフォーム対応バイナリを含む

---

**実装完了**: 2025-11-15 14:03:00  
**実行者**: zapabob  
**ステータス**: ✅ 完了

