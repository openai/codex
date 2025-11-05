# 並列実行 & カスタムエージェント クイックガイド

## 🚀 並列エージェント実行

### 基本コマンド

```bash
codex delegate-parallel <agent1>,<agent2>,<agent3> \
  --goals "Goal 1,Goal 2,Goal 3" \
  --scopes ./path1,./path2,./path3 \
  --budgets 40000,30000,20000 \
  --deadline 15
```

### 実践例

#### 例1: フルスタックレビュー

```bash
codex delegate-parallel code-reviewer,code-reviewer,test-gen \
  --goals "Review frontend,Review backend,Generate tests" \
  --scopes ./frontend,./backend,./tests \
  --budgets 50000,50000,40000
```

**実行時間**: 単一実行 18分 → 並列実行 6分（**66%短縮**）

#### 例2: マルチ言語プロジェクト

```bash
codex delegate-parallel ts-reviewer,python-reviewer,unity-reviewer \
  --goals "TypeScript review,Python review,Unity review" \
  --scopes ./web,./api,./Assets \
  --budgets 40000,35000,30000 \
  --deadline 20 \
  --out review-report.md
```

#### 例3: セキュリティ多層スキャン

```bash
codex delegate-parallel sec-audit,sec-audit,sec-audit \
  --goals "SQL injection,XSS scan,Dependency audit" \
  --scopes ./api,./web,./package.json \
  --budgets 30000,30000,20000
```

### オプション説明

| オプション | 説明 | 例 |
|-----------|------|---|
| `--goals` | 各エージェントのゴール（カンマ区切り） | `"Goal 1,Goal 2"` |
| `--scopes` | 各エージェントのスコープパス | `./src,./tests` |
| `--budgets` | 各エージェントのトークン予算 | `40000,30000` |
| `--deadline` | 全体の制限時間（分） | `15` |
| `--out` | 結果の出力先ファイル | `report.md` |

---

## 🤖 カスタムエージェント作成

### 基本コマンド

```bash
codex agent-create "<自然言語でタスクを記述>" \
  --budget 50000 \
  --save  # オプション: YAML として保存
```

### 実践例

#### 例1: コードメトリクス収集

```bash
codex agent-create "Count the number of TypeScript files and calculate total lines of code"
```

生成されるエージェント定義（例）:
```yaml
name: code-metrics-analyzer
goal: Count TypeScript files and calculate total LOC
tools:
  mcp: [codex_read_file, codex_grep, codex_codebase_search]
policies:
  context:
    max_tokens: 40000
success_criteria:
  - "TypeScript file count is accurate"
  - "LOC calculation includes all .ts and .tsx files"
  - "Report is formatted clearly"
```

#### 例2: TODO コメント集約

```bash
codex agent-create "Find all TODO comments in the codebase and create a summary report"
```

#### 例3: セキュリティチェック

```bash
codex agent-create "Review Python code for SQL injection vulnerabilities" \
  --budget 60000 \
  --save \
  --out security-report.md
```

#### 例4: リファクタリング計画

```bash
codex agent-create "Analyze the codebase and suggest refactoring opportunities to reduce cyclomatic complexity"
```

### オプション説明

| オプション | 説明 | デフォルト |
|-----------|------|----------|
| `--budget` | トークン予算 | 自動設定 |
| `--save` | YAML として保存 | `false` |
| `--out` | 結果の出力先 | 標準出力 |

---

## 🎯 組み合わせ活用

### パターン1: カスタムエージェント → 並列実行

```bash
# ステップ1: カスタムエージェントを作成・保存
codex agent-create "Analyze React components for performance issues" \
  --save

# ステップ2: 保存したエージェントを並列実行
codex delegate-parallel custom-agent,code-reviewer,test-gen \
  --goals "Performance analysis,Code review,Generate tests" \
  --scopes ./components,./src,./tests
```

### パターン2: マルチステージ並列実行

```bash
# フェーズ1: 分析（並列）
codex delegate-parallel custom-agent,custom-agent \
  --goals "Analyze frontend,Analyze backend" \
  --scopes ./frontend,./backend \
  --budgets 50000,50000

# フェーズ2: 修正（並列）
codex delegate-parallel code-reviewer,sec-audit \
  --goals "Review fixes,Security audit" \
  --scopes ./src,./
```

---

## 💡 ベストプラクティス

### 1. 並列実行

✅ **推奨**:
- スコープを明確に分離（フォルダ単位）
- 各エージェントに適切な予算を設定
- デッドラインを余裕を持って設定

❌ **非推奨**:
- 同じファイルを複数エージェントで編集
- トークン予算が極端に少ない（< 10000）
- デッドラインなしで長時間実行

### 2. カスタムエージェント

✅ **推奨**:
- 具体的で明確なタスク記述
- "Find X and do Y" の形式
- 期待する出力形式を明記

❌ **非推奨**:
- 曖昧なタスク（"いい感じに直して"）
- 複数の無関係なタスクを混在
- セキュリティリスクの高い操作を無制限に許可

### 3. トークン予算管理

| タスク種別 | 推奨予算 |
|-----------|---------|
| コードレビュー（小） | 20,000 |
| コードレビュー（中） | 40,000 |
| コードレビュー（大） | 60,000 |
| テスト生成 | 30,000 |
| セキュリティ監査 | 30,000 |
| カスタムエージェント | 40,000-60,000 |

---

## 🔒 セキュリティガイドライン

### デフォルトで安全

カスタムエージェントは以下のツールのみ使用（安全）:
- ✅ `codex_read_file` - ファイル読み込み
- ✅ `codex_grep` - 検索
- ✅ `codex_codebase_search` - セマンティック検索
- ✅ `codex_apply_patch` - パッチ適用

### 危険なツールは明示的に指定

```bash
# シェル実行が必要な場合は明示的に
codex agent-create "Run npm audit and fix vulnerabilities (use shell)" \
  --budget 50000
```

### 監査ログ

すべてのエージェント実行は自動的に監査ログに記録されます:
- エージェント名
- 実行時刻
- トークン使用量
- 実行結果（成功/失敗）

ログ場所: `.codex/audit/`

---

## 📊 パフォーマンスチューニング

### 並列実行の最適数

| システムリソース | 推奨並列数 |
|----------------|----------|
| CPU 4コア | 2-3 |
| CPU 8コア | 4-6 |
| CPU 16コア | 8-12 |

### トークン効率

```bash
# 低効率: 1つのエージェントで全て
codex delegate code-reviewer --scope ./entire-project --budget 100000

# 高効率: 並列化して分散
codex delegate-parallel code-reviewer,code-reviewer,code-reviewer \
  --scopes ./frontend,./backend,./tests \
  --budgets 35000,35000,30000
```

---

## 🐛 トラブルシューティング

### エラー: "Agent not found"

```bash
# 利用可能なエージェントを確認
ls .codex/agents/

# または、カスタムエージェントを作成
codex agent-create "..."
```

### エラー: "Budget exceeded"

```bash
# 予算を増やす
codex agent-create "..." --budget 80000

# または、タスクを分割して並列実行
codex delegate-parallel agent1,agent2 --budgets 40000,40000
```

### 並列実行が遅い

```bash
# 並列数を減らす（システムリソース不足）
# 3つ → 2つに減らす
codex delegate-parallel agent1,agent2 ...

# または、デッドラインを延長
--deadline 30  # 15分 → 30分
```

---

## 🎓 学習パス

### レベル1: 基本

```bash
# 単一エージェント実行
codex delegate code-reviewer --scope ./src --budget 40000
```

### レベル2: 並列実行

```bash
# 2つのエージェントを並列実行
codex delegate-parallel code-reviewer,test-gen \
  --scopes ./src,./tests
```

### レベル3: カスタムエージェント

```bash
# プロンプトからエージェント作成
codex agent-create "Find all console.log statements"
```

### レベル4: 高度な組み合わせ

```bash
# カスタムエージェント + 並列実行 + 保存
codex agent-create "Custom task" --save
codex delegate-parallel custom-agent,code-reviewer,test-gen
```

---

## 📚 さらなる情報

- **実装ログ**: `_docs/2025-10-11_並列実行カスタムエージェント実装完了.md`
- **サブエージェント全般**: `SUBAGENTS_QUICKSTART.md`
- **Deep Research**: `docs/zdr.md`
- **設定ガイド**: `docs/config.md`

---

**最終更新**: 2025-10-11  
**バージョン**: 0.47.0-alpha.1  
**プロジェクト**: zapabob/codex

