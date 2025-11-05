# 2025-10-23 リポジトリ整理整頓計画

## 目的

ファイルを削除せず、公式OpenAI/codexリポジトリとの整合性を保ちながら、zapabob/codexリポジトリを整理整頓する。

## 現状分析

### 公式との差分
- **変更ファイル数**: 644ファイル
- **追加行数**: 473,323行
- **削除行数**: 1,361行

### 整理が必要なファイル

#### 1. 一時ファイル・ビルドログ（ルートディレクトリ）
```
build-clean-release.log
build-final.log
build-foreground.log
build-incremental.log
build-progress-20251019-180428.log
build-progress-20251021-185401.log
clean-build-install.log
```

#### 2. テストファイル（ルートディレクトリ）
```
test-codex-v048.ps1
test1_python_async.md
test2_ml_japanese.md
test3_react_rsc.md
test_results_rust_async.md
_temp_improvement_plan.md
```

#### 3. 実行ファイル（examples/）
```
simple_add_test.exe
simple_add_test.pdb
simple_multiply_test.exe
simple_multiply_test.pdb
```

#### 4. その他
```
Cargo.toml.just-backup
CUsersdownl.codexbin
```

## 整理方針

### 原則
1. **ファイル削除なし**: 全て移動または整理
2. **公式整合性**: 公式リポジトリと同じ構造を維持
3. **独自機能**: `zapabob/` 配下に統一
4. **一時ファイル**: `_temp/` または `.gitignore` 追加

### ディレクトリ構造（整理後）

```
codex/
├── codex-rs/              # 公式Rust実装（変更最小化）
├── codex-cli/             # 公式CLI
├── docs/                  # 公式ドキュメント
├── scripts/               # 公式スクリプト
├── examples/              # 公式サンプル
│
├── zapabob/               # 独自機能（統一）
│   ├── docs/              # 独自ドキュメント
│   │   ├── implementation-logs/  # _docs/から移動
│   │   ├── guides/
│   │   └── architecture/
│   ├── scripts/           # 独自スクリプト
│   ├── extensions/        # IDE拡張
│   ├── sdk/               # TypeScript SDK
│   └── reports/           # レビューレポート
│
├── _temp/                 # 一時ファイル（.gitignore）
│   ├── build-logs/        # ビルドログ移動先
│   ├── test-outputs/      # テスト出力移動先
│   └── artifacts/         # 一時成果物
│
├── .cursor/               # Cursor設定（独自）
├── .codex/                # Codex設定（独自）
│
└── archive/               # アーカイブ（既存）
```

## 実行計画

### Step 1: 一時ファイルの移動

#### ビルドログ → `_temp/build-logs/`
```powershell
New-Item -ItemType Directory -Path "_temp/build-logs" -Force
Move-Item "build-*.log" "_temp/build-logs/"
Move-Item "clean-build-install.log" "_temp/build-logs/"
```

#### テストファイル → `_temp/test-outputs/`
```powershell
New-Item -ItemType Directory -Path "_temp/test-outputs" -Force
Move-Item "test*.md" "_temp/test-outputs/"
Move-Item "test*.ps1" "_temp/test-outputs/"
Move-Item "_temp_improvement_plan.md" "_temp/test-outputs/"
```

#### 実行ファイル → `_temp/build-artifacts/`
```powershell
New-Item -ItemType Directory -Path "_temp/build-artifacts" -Force
Move-Item "examples/*.exe" "_temp/build-artifacts/"
Move-Item "examples/*.pdb" "_temp/build-artifacts/"
```

### Step 2: 独自ドキュメントの整理

#### _docs/ → zapabob/docs/implementation-logs/
```powershell
New-Item -ItemType Directory -Path "zapabob/docs/implementation-logs" -Force
Move-Item "_docs/*" "zapabob/docs/implementation-logs/"
```

#### artifacts/ → zapabob/reports/
```powershell
Move-Item "artifacts/*" "zapabob/reports/"
```

### Step 3: .gitignoreの更新

```gitignore
# 一時ファイル
_temp/
*.log
*.exe
*.pdb
build-progress-*.log

# IDE設定（既存）
.vscode/
.idea/

# ビルド成果物
target/
node_modules/
dist/

# OS固有
.DS_Store
Thumbs.db

# バックアップ
*.backup
*.bak
*.just-backup
```

### Step 4: 不要ファイルの除去

#### バックアップファイル → archive/backups/
```powershell
New-Item -ItemType Directory -Path "archive/backups" -Force
Move-Item "Cargo.toml.just-backup" "archive/backups/"
Move-Item "CUsersdownl.codexbin" "archive/backups/" -ErrorAction SilentlyContinue
```

### Step 5: ドキュメント構造の整理

#### zapabob/docs/ の構造化
```
zapabob/docs/
├── implementation-logs/   # _docs/から移動
│   ├── 2025-10-23_*.md
│   └── ...
├── guides/                # 既存CURSOR_*.mdなど
├── architecture/          # .mmdファイル
├── reviews/               # CODE_REVIEW_*.md
└── releases/              # リリースノート
```

### Step 6: READMEの更新

#### ルートREADME.md
公式リポジトリとの関係を明記:
```markdown
# Codex (zapabob fork)

OpenAI/codex の拡張版。rmcp統合、AIオーケストレーション、DeepResearch機能を追加。

## 公式リポジトリとの関係
- **上流**: https://github.com/openai/codex
- **フォーク**: https://github.com/zapabob/codex
- **差分**: AIオーケストレーション、サブエージェント、DeepResearch機能

## 独自機能
- rmcp 0.8.3+ベストプラクティス準拠
- 8種類の特化エージェント
- Deep Research with caching
- Cursor IDE統合

詳細は `zapabob/README.md` を参照。
```

#### zapabob/README.md
独自機能の詳細を記載:
```markdown
# zapabob拡張機能

## ディレクトリ構造
- `docs/` - 実装ログ、ガイド
- `scripts/` - 独自スクリプト
- `extensions/` - IDE拡張
- `sdk/` - TypeScript SDK
```

## 実装手順

### Phase 1: 一時ファイルの整理
1. `_temp/` ディレクトリ作成
2. ビルドログ移動
3. テストファイル移動
4. 実行ファイル移動

### Phase 2: ドキュメントの整理
1. `_docs/` → `zapabob/docs/implementation-logs/`
2. `artifacts/` → `zapabob/reports/`
3. ドキュメント分類

### Phase 3: 設定ファイルの整理
1. `.gitignore` 更新
2. バックアップファイル移動
3. 不要ファイル確認

### Phase 4: ドキュメント構造化
1. `zapabob/docs/` のサブディレクトリ作成
2. ドキュメント分類
3. README更新

### Phase 5: 検証
1. ビルドテスト
2. Git状態確認
3. 公式リポジトリとの差分確認

## 期待される結果

### Before（現状）
```
codex/
├── build-*.log (散在)
├── test*.md (散在)
├── _docs/ (236ファイル)
├── artifacts/
├── archive/
└── zapabob/
```

### After（整理後）
```
codex/
├── codex-rs/ (公式)
├── docs/ (公式)
├── .cursor/ (Cursor設定)
├── .codex/ (Codex設定)
│
├── zapabob/ (独自機能 - 統一)
│   ├── docs/
│   │   ├── implementation-logs/ (231 .md)
│   │   ├── guides/
│   │   ├── architecture/
│   │   └── reviews/
│   ├── scripts/
│   ├── extensions/
│   └── sdk/
│
├── _temp/ (.gitignore) (一時ファイル)
│   ├── build-logs/
│   ├── test-outputs/
│   └── build-artifacts/
│
└── archive/ (アーカイブ)
```

## メリット

### 1. 見通しの改善
- ルートディレクトリがクリーン
- 独自機能が`zapabob/`に統一
- 一時ファイルが分離

### 2. 公式との整合性
- 公式ディレクトリ構造を維持
- 差分が明確
- マージが容易

### 3. 保守性向上
- ドキュメントが分類
- 検索が容易
- 新規開発者が理解しやすい

### 4. Git管理の改善
- `.gitignore`で一時ファイル除外
- リポジトリサイズ削減
- クリーンな履歴

## 注意事項

### 実行前の確認
- [ ] すべての変更をコミット済み
- [ ] ビルドが成功している
- [ ] テストが通っている

### 実行中の注意
- ファイル移動後、ビルドテストを実施
- Gitで追跡されているファイルは`git mv`使用
- リンク切れがないか確認

### 実行後の検証
- [ ] ビルドが通る
- [ ] テストが通る
- [ ] ドキュメントリンクが有効
- [ ] 公式との差分が明確

## 次のアクション

実行しますか？ (y/n)

実行する場合、以下の順序で進めます：
1. 一時ファイル移動（_temp/作成）
2. ドキュメント整理（zapabob/docs/実装ログ移動）
3. .gitignore更新
4. README更新
5. コミット&プッシュ

