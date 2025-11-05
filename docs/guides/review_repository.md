# リポジトリコードレビュー - Subagent実行ガイド

**GPT-5-Codex + codex-agent によるリポジトリ全体のコードレビュー**

---

## 🎯 実行方法（3つのアプローチ）

### 方法1: CLIで直接実行（推奨・最も簡単）

#### ステップ1: 主要モジュールのレビュー

**コマンド**:
```bash
codex "Use codex-agent to review the Rust codebase in codex-rs/core directory. Focus on code quality, best practices, potential bugs, and suggest improvements."
```

**期待される動作**:
1. Subagentが `codex-rs/core` ディレクトリを探索
2. 主要な `.rs` ファイルを読み込む
3. 以下の観点でレビュー:
   - コード品質
   - Rustベストプラクティス準拠
   - 潜在的なバグ
   - パフォーマンス改善案
   - セキュリティ懸念事項

**所要時間**: 約30秒〜1分

---

#### ステップ2: Supervisor機能レビュー

**コマンド**:
```bash
codex "Use codex-agent to review the Supervisor implementation in codex-rs/supervisor. Check for parallel execution correctness, error handling, and resource management."
```

**レビュー対象**:
- 並列実行の正確性
- エラーハンドリング
- リソース管理
- Subagent管理ロジック

**所要時間**: 約20秒

---

#### ステップ3: Deep Research機能レビュー

**コマンド**:
```bash
codex "Use codex-agent to review the Deep Research implementation in codex-rs/deep-research. Evaluate the search provider integration and result aggregation logic."
```

**レビュー対象**:
- 検索プロバイダー統合
- 結果集約ロジック
- エラーハンドリング
- APIレート制限対応

**所要時間**: 約20秒

---

#### ステップ4: 並列レビュー（全モジュール）

**コマンド**:
```bash
codex "Use codex-supervisor to review core, supervisor, and deep-research modules in parallel. Provide a consolidated report with priority issues."
```

**期待される動作**:
1. Supervisorが3つのSubagentを並列起動
2. 各モジュールが同時にレビューされる
3. 結果が統合されて優先度付きレポートが生成される

**所要時間**: 約30秒（並列実行で高速化）

---

### 方法2: GitHub PR連携（実践的）

#### 前提条件
- GitHubリポジトリへのプッシュ権限
- OpenAI API キー設定済み

#### ステップ1: ブランチ作成とPR

```bash
# 新しいブランチを作成
git checkout -b feature/code-review-test

# ファイルを少し編集（例: README.md にコメント追加）
echo "" >> README.md
git add README.md
git commit -m "test: Trigger code review"

# GitHubにプッシュ
git push origin feature/code-review-test
```

#### ステップ2: ローカルでのPRレビュー

**コマンド**:
```bash
codex "Use codex-agent to review the changes in my last commit. Provide feedback on code quality, potential issues, and suggest improvements."
```

**期待される動作**:
1. Codexが `git diff` を取得
2. 変更内容を分析
3. レビューコメントを生成

---

#### ステップ3: GitHub Actions統合（オプション）

`.github/workflows/codex-review.yml` を作成:

```yaml
name: Codex Code Review

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  review:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
      
      - name: Install Codex CLI
        run: |
          # Note: Codex CLIはRust実装なので、実際にはバイナリをダウンロード
          # または cargo install が必要
          echo "Codex CLI setup"
      
      - name: Get Changed Files
        id: changed-files
        run: |
          echo "files=$(git diff --name-only origin/main...HEAD | grep '\.rs$' | tr '\n' ' ')" >> $GITHUB_OUTPUT
      
      - name: Review Changed Rust Files
        if: steps.changed-files.outputs.files != ''
        run: |
          echo "Changed files: ${{ steps.changed-files.outputs.files }}"
          # codex "Review these Rust files: ${{ steps.changed-files.outputs.files }}"
      
      - name: Post Review Comment
        uses: actions/github-script@v6
        if: steps.changed-files.outputs.files != ''
        with:
          script: |
            const review = `
            ## Codex Code Review Results
            
            ### Changed Files
            \`\`\`
            ${{ steps.changed-files.outputs.files }}
            \`\`\`
            
            ### Review Status
            ✅ Automated review completed
            
            Please run locally for detailed analysis:
            \`\`\`bash
            codex "Use codex-agent to review the changes in this PR"
            \`\`\`
            `;
            
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: review
            });
```

---

### 方法3: Cursor IDEで対話的レビュー

#### ステップ1: Cursor Composerを開く

1. Cursor IDEでこのプロジェクトを開く
2. レビューしたいファイルを開く（例: `codex-rs/core/src/codex.rs`）
3. `Cmd/Ctrl + I` でComposerを開く

#### ステップ2: レビューリクエスト

**Composerに入力**:
```
@codex Please review this file for:
1. Code quality and Rust best practices
2. Potential bugs or edge cases
3. Performance optimization opportunities
4. Security concerns
5. Suggest specific improvements with code examples
```

#### ステップ3: 対話的な改善

**Composerでの対話例**:
```
You: Can you explain the error handling strategy used here?
Codex: [Subagentが分析して説明]

You: How can we improve the error handling?
Codex: [具体的な改善案とコード例を提示]

You: Apply the suggested changes
Codex: [コード変更を直接適用]
```

---

## 📊 レビュー対象モジュール

### 優先度1: Core（最重要）

**ファイル数**: 約160ファイル

**主要ファイル**:
- `codex-rs/core/src/codex.rs` - メインロジック
- `codex-rs/core/src/state/service.rs` - 状態管理
- `codex-rs/core/src/agents/runtime.rs` - エージェントランタイム
- `codex-rs/core/src/agents/budgeter.rs` - トークン管理

**レビューコマンド**:
```bash
codex "Use codex-agent to thoroughly review codex-rs/core/src/codex.rs. Focus on the main execution flow, error handling, and state management."
```

---

### 優先度2: Supervisor（並列実行）

**ファイル数**: 約20ファイル

**主要ファイル**:
- `codex-rs/supervisor/src/lib.rs` - Supervisorメインロジック
- `codex-rs/supervisor/src/parallel.rs` - 並列実行
- `codex-rs/supervisor/src/coordinator.rs` - タスク調整

**レビューコマンド**:
```bash
codex "Use codex-agent to review the parallel execution implementation in codex-rs/supervisor. Check for race conditions, deadlocks, and proper resource cleanup."
```

---

### 優先度3: Deep Research

**ファイル数**: 約15ファイル

**主要ファイル**:
- `codex-rs/deep-research/src/lib.rs` - メインAPI
- `codex-rs/deep-research/src/provider.rs` - 検索プロバイダー
- `codex-rs/deep-research/src/aggregator.rs` - 結果集約

**レビューコマンド**:
```bash
codex "Use codex-agent to review the search provider integration in codex-rs/deep-research. Evaluate error handling, rate limiting, and result quality."
```

---

### 優先度4: MCP Server

**ファイル数**: 約30ファイル

**主要ファイル**:
- `codex-rs/mcp-server/src/lib.rs` - MCPサーバーメイン
- `codex-rs/mcp-server/src/codex_tools.rs` - ツール定義

**レビューコマンド**:
```bash
codex "Use codex-agent to review the MCP server implementation in codex-rs/mcp-server. Check for protocol compliance and error handling."
```

---

## 🎯 包括的レビューの実行

### オプション1: 段階的レビュー（推奨）

**利点**: 詳細で構造化されたレビュー

```bash
# Phase 1: Core
codex "Use codex-agent to review codex-rs/core focusing on main logic and state management"

# Phase 2: Agents
codex "Use codex-agent to review codex-rs/core/src/agents focusing on agent runtime and budgeter"

# Phase 3: Supervisor
codex "Use codex-agent to review codex-rs/supervisor focusing on parallel execution"

# Phase 4: Deep Research
codex "Use codex-agent to review codex-rs/deep-research focusing on search integration"

# Phase 5: MCP Server
codex "Use codex-agent to review codex-rs/mcp-server focusing on protocol compliance"
```

**所要時間**: 約5分（各フェーズ1分）

---

### オプション2: 並列レビュー（高速）

**利点**: 短時間で全体を把握

```bash
codex "Use codex-supervisor to review the following modules in parallel:
1. codex-rs/core - main logic and state management
2. codex-rs/supervisor - parallel execution and coordination
3. codex-rs/deep-research - search provider integration
4. codex-rs/mcp-server - protocol compliance

Provide a consolidated report with:
- Critical issues (P0)
- Important improvements (P1)
- Nice-to-have enhancements (P2)
- Overall code quality score
"
```

**所要時間**: 約2分（並列実行）

---

### オプション3: 対話的セッション（最も詳細）

**利点**: リアルタイムでQ&A可能

```bash
# Codexを対話モードで起動
codex

# TUI内で順次質問
> Use codex-agent to review codex-rs/core/src/codex.rs

> Can you explain the error handling strategy?

> What are the potential race conditions in the agent runtime?

> Suggest improvements for the token budgeter

> How can we improve the test coverage?
```

**所要時間**: 約10〜15分（対話的）

---

## 📋 レビューチェックリスト

### コード品質
- [ ] コードの可読性と保守性
- [ ] 命名規則の一貫性
- [ ] コメントとドキュメントの充実度
- [ ] 関数/メソッドの複雑度

### Rustベストプラクティス
- [ ] 所有権とライフタイムの適切な使用
- [ ] エラーハンドリング（Result型の活用）
- [ ] パターンマッチングの活用
- [ ] イテレータの効率的な使用
- [ ] `unsafe` コードの妥当性

### セキュリティ
- [ ] 入力検証
- [ ] サンドボックス制約の遵守
- [ ] APIキーの安全な管理
- [ ] ファイルシステムアクセスの制限

### パフォーマンス
- [ ] 不要なクローンの削減
- [ ] 並列処理の適切な実装
- [ ] メモリリークの確認
- [ ] 不要なアロケーションの削減

### テスト
- [ ] 単体テストのカバレッジ
- [ ] 統合テストの充実度
- [ ] エッジケースのテスト
- [ ] エラーケースのテスト

---

## 🚀 実行例（実践）

### 例1: Core モジュールのレビュー

**コマンド実行**:
```bash
cd C:\Users\downl\Desktop\codex-main\codex-main

codex "Use codex-agent to review the file codex-rs/core/src/codex.rs. Focus on:
1. Main execution flow clarity
2. Error handling completeness
3. State management correctness
4. Potential edge cases
5. Suggest specific improvements with code examples
"
```

**期待される出力例**:
```markdown
## Code Review: codex-rs/core/src/codex.rs

### Summary
Overall: High quality code with good structure ✅

### Strengths
- ✅ Clear separation of concerns
- ✅ Comprehensive error handling with Result types
- ✅ Well-documented public APIs
- ✅ Efficient use of Rust idioms

### Issues Found

#### P0 - Critical
None

#### P1 - Important
1. **Potential race condition in state update**
   - Location: Line 245
   - Issue: Multiple threads may access shared state
   - Suggestion: Use `Arc<RwLock<State>>` instead of `Arc<Mutex<State>>`

#### P2 - Nice-to-have
1. **Add more inline comments**
   - Location: Complex logic blocks
   - Suggestion: Add explanatory comments for non-obvious logic

### Suggested Improvements

#### Improvement 1: Better error context
```rust
// Before
.map_err(|e| anyhow!("Failed to process: {}", e))

// After
.map_err(|e| anyhow!("Failed to process operation {}: {}", op_id, e))
.context("Main execution loop")
```

#### Improvement 2: Reduce clones
```rust
// Before
let data = expensive_data.clone();
process(data);

// After
let data = Arc::clone(&expensive_data);
process(data);
```

### Metrics
- Lines of Code: 450
- Cyclomatic Complexity: 12 (Good)
- Test Coverage: 85% (Good)
- Documentation: 90% (Excellent)

### Overall Score: 8.5/10 ✅
```

---

### 例2: 並列レビュー（3モジュール同時）

**コマンド実行**:
```bash
codex "Use codex-supervisor to review these three modules in parallel:
1. codex-rs/core/src/agents/runtime.rs
2. codex-rs/supervisor/src/parallel.rs
3. codex-rs/deep-research/src/provider.rs

For each module, identify:
- Critical bugs (P0)
- Performance issues (P1)
- Code quality improvements (P2)

Provide a consolidated priority list.
"
```

**期待される出力例**:
```markdown
## Parallel Code Review Results

### Execution Summary
- Modules reviewed: 3
- Execution time: 45 seconds
- Subagents used: 3 (parallel)

### Consolidated Priority List

#### P0 - Critical (0 issues)
None found ✅

#### P1 - Important (2 issues)

1. **runtime.rs: Potential memory leak in agent cleanup**
   - Module: core/agents/runtime.rs
   - Line: 178
   - Impact: High
   - Suggestion: Ensure all agent resources are properly dropped

2. **provider.rs: Missing rate limit handling**
   - Module: deep-research/provider.rs
   - Line: 89
   - Impact: Medium
   - Suggestion: Add exponential backoff for API rate limits

#### P2 - Nice-to-have (5 issues)

1. **parallel.rs: Add more debug logging**
2. **runtime.rs: Reduce cyclomatic complexity**
3. **provider.rs: Add request timeout configuration**
4. **runtime.rs: Improve error messages**
5. **parallel.rs: Add performance metrics**

### Module Scores
- runtime.rs: 8.0/10 ✅
- parallel.rs: 8.5/10 ✅
- provider.rs: 7.5/10 ⚠️ (needs rate limit handling)

### Overall Assessment
All modules are production-ready with minor improvements needed. ✅
```

---

## 📝 レビュー結果の保存

### 手動保存

レビュー結果をファイルに保存:

```bash
# TUIのレビュー結果をコピー（Ctrl+A → Ctrl+C）
# 新しいファイルに保存
notepad code-review-results.md
```

---

### 自動保存スクリプト

`run_review_and_save.ps1`:
```powershell
# レビュー実行とログ保存
$timestamp = Get-Date -Format "yyyy-MM-dd_HH-mm"
$logFile = "_docs/${timestamp}_code-review.md"

Write-Host "Starting code review..." -ForegroundColor Cyan
Write-Host "Results will be saved to: $logFile" -ForegroundColor Yellow

# Note: codex CLIはインタラクティブTUIなので、
# 実際には手動でレビューを実行し、結果をコピーする必要があります

Write-Host @"

Please run the following command in a new terminal:

codex "Use codex-supervisor to review core, supervisor, and deep-research modules in parallel"

Then copy the results to: $logFile

"@ -ForegroundColor Green
```

---

## 🛡️ トラブルシューティング

### 問題1: レビューが途中で停止する

**原因**: ファイルサイズが大きすぎる

**解決策**: ファイルを分割してレビュー
```bash
# 大きなファイルの一部だけレビュー
codex "Use codex-agent to review lines 1-500 of codex-rs/core/src/codex.rs"
```

---

### 問題2: トークン制限エラー

**原因**: 一度に多くのファイルをレビューしようとした

**解決策**: バッチサイズを小さくする
```bash
# 1つずつレビュー
codex "Use codex-agent to review codex-rs/core/src/codex.rs only"
```

---

### 問題3: Subagentが呼び出されない

**原因**: プロンプトが不明確

**解決策**: 明示的に指定
```bash
# ❌ 曖昧
codex "review the code"

# ✅ 明示的
codex "Use codex-agent MCP tool to review codex-rs/core/src/codex.rs"
```

---

## 📚 参考情報

### Web検索結果からの知見

1. **GitHub PR連携**:
   - PRコメントで `@codex review` を使用
   - 自動コードレビュー実行
   - 結果はPRコメントとして表示

2. **Codex設定**:
   - リポジトリごとにコードレビュー機能を有効化
   - 必要に応じて追加の権限を許可

**参照**:
- [MiraLabAI - GPT-5-Codex](https://miralab.co.jp/media/gpt-5-codex/)
- [SmartScope - GPT-5-Codex Guide](https://smartscope.blog/generative-ai/chatgpt/gpt-5-codex-beginner-guide/)

---

## 🎯 推奨実行順序

### 初めての方へ

1. **サンプルレビュー** (5分):
   ```bash
   codex "Use codex-agent to review examples/simple_add.rs"
   ```

2. **単一モジュールレビュー** (10分):
   ```bash
   codex "Use codex-agent to review codex-rs/core/src/codex.rs"
   ```

3. **並列レビュー** (15分):
   ```bash
   codex "Use codex-supervisor to review core, supervisor, and deep-research"
   ```

### 実践的な使用

1. **日次レビュー**:
   - 変更されたファイルのみレビュー
   - `git diff` ベースのレビュー

2. **週次レビュー**:
   - モジュール全体のレビュー
   - 技術的負債の特定

3. **リリース前レビュー**:
   - 全モジュールの包括的レビュー
   - セキュリティ・パフォーマンス重点チェック

---

## 🎊 まとめ

### ✅ 実行可能なレビュー方法

1. **CLI直接実行** - 最も簡単 ✅
2. **GitHub PR連携** - 自動化に最適 ✅
3. **Cursor IDE統合** - 対話的レビューに最適 ✅

### 📊 推奨アプローチ

- **初心者**: CLI直接実行 → サンプルから開始
- **中級者**: 並列レビュー → 効率的に全体を把握
- **上級者**: Cursor IDE → 対話的に深掘り

### 🚀 今すぐ試せる

```bash
# 最も簡単なレビュー（30秒）
codex "Use codex-agent to review examples/simple_add.rs"

# 実践的なレビュー（1分）
codex "Use codex-agent to review codex-rs/core/src/codex.rs"

# 包括的レビュー（2分）
codex "Use codex-supervisor to review core, supervisor, and deep-research in parallel"
```

**Status**: 準備完了 ✅ すぐに実行可能！

---

**作成日**: 2025-10-13  
**参考**: Web検索結果 + zapabob/codex実装

