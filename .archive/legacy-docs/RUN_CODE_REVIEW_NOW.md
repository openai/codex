# 🚀 今すぐコードレビューを実行！

**準備完了 ✅ 以下のコマンドを新しいターミナルで実行してください**

---

## ステップ1: サンプルファイルでテスト（30秒）

### コマンド（コピー&ペースト）:
```bash
codex "Use codex-agent to review examples/simple_add.rs for code quality, test coverage, and suggest improvements"
```

### 期待される動作:
1. Codex TUIが起動
2. モデル表示: `gpt-5-codex`
3. Subagent (`codex-agent`) が呼び出される
4. レビュー結果が表示される

### 確認ポイント:
- ✅ TUIが正常に起動したか
- ✅ `[MCP]` 表示が見えるか（Subagent呼び出し）
- ✅ レビュー結果が表示されたか

**終了方法**: `Ctrl + C`

---

## ステップ2: 実際のリポジトリコアファイルをレビュー（1分）

### コマンド:
```bash
codex "Use codex-agent to review the file codex-rs/core/src/codex.rs. Analyze:
1. Main execution flow
2. Error handling strategy
3. State management
4. Potential bugs or edge cases
5. Suggest specific code improvements with examples
"
```

### 期待される出力例:
```markdown
## Code Review: codex-rs/core/src/codex.rs

### Summary
✅ High quality code with good structure

### Strengths
- Clear separation of concerns
- Comprehensive error handling
- Well-documented APIs

### Issues
P1: Potential race condition at line 245
P2: Add more inline comments

### Improvements
[具体的なコード例付き]

### Score: 8.5/10
```

---

## ステップ3: 並列レビュー（全モジュール・2分）

### コマンド:
```bash
codex "Use codex-supervisor to review the following modules in parallel:
1. codex-rs/core - main logic and state management
2. codex-rs/supervisor - parallel execution
3. codex-rs/deep-research - search integration

Provide a consolidated report with priority issues and overall code quality scores."
```

### 期待される動作:
1. Supervisor起動
2. 3つのSubagentが並列実行
3. 結果が統合されて表示
4. 優先度付きのイシューリスト

### パフォーマンス:
- 単一実行: 約3分
- 並列実行: 約2分（**1.5倍高速化**）

---

## 実行手順（Windows PowerShell）

### 1. 新しいターミナルを開く
```powershell
# PowerShellを管理者権限で起動（推奨）
Start-Process powershell -Verb RunAs
```

### 2. プロジェクトディレクトリに移動
```powershell
cd C:\Users\downl\Desktop\codex-main\codex-main
```

### 3. 上記のコマンドを実行

---

## Cursor IDEでの実行（より簡単）

### 方法1: Composer使用

1. Cursor IDEを開く
2. `codex-rs/core/src/codex.rs` を開く
3. `Cmd/Ctrl + I` でComposerを起動
4. 以下を入力:
   ```
   @codex Review this file for code quality, potential bugs, and suggest improvements with code examples
   ```

### 方法2: Chat使用

1. Cursor Chat (`Cmd/Ctrl + L`) を開く
2. 以下を入力:
   ```
   Use codex-agent to review all the main Rust files in codex-rs/core and provide a summary report
   ```

---

## トラブルシューティング

### 問題: `stdout is not a terminal`

**解決策**: 新しいPowerShellウィンドウで直接実行
```powershell
Start-Process powershell
# 新しいウィンドウでコマンド実行
```

### 問題: Subagentが呼び出されない

**解決策**: プロンプトを明示的に
```bash
# ❌ 曖昧
codex "review the code"

# ✅ 明示的
codex "Use codex-agent MCP tool to review examples/simple_add.rs"
```

### 問題: モデルエラー

**解決策**: フォールバックモデル使用
```bash
codex --model gpt-4o "Use codex-agent to review examples/simple_add.rs"
```

---

## 📊 推奨実行順序

### 初めての方（15分）

1. **サンプルレビュー** (30秒):
   ```bash
   codex "Use codex-agent to review examples/simple_add.rs"
   ```

2. **単一ファイルレビュー** (1分):
   ```bash
   codex "Use codex-agent to review codex-rs/core/src/codex.rs"
   ```

3. **モジュールレビュー** (2分):
   ```bash
   codex "Use codex-agent to review codex-rs/supervisor"
   ```

### 実践的な使用（30分）

1. **コアモジュール詳細レビュー** (5分):
   ```bash
   codex "Use codex-agent to thoroughly review codex-rs/core focusing on:
   - Main execution flow in src/codex.rs
   - Agent runtime in src/agents/runtime.rs
   - Token budgeter in src/agents/budgeter.rs
   - State management in src/state/service.rs
   "
   ```

2. **並列実行機能レビュー** (3分):
   ```bash
   codex "Use codex-agent to review codex-rs/supervisor checking for:
   - Race conditions
   - Deadlocks
   - Resource leaks
   - Proper error propagation
   "
   ```

3. **Deep Research機能レビュー** (3分):
   ```bash
   codex "Use codex-agent to review codex-rs/deep-research evaluating:
   - API rate limit handling
   - Error recovery
   - Result aggregation logic
   - Search provider integration
   "
   ```

4. **包括的並列レビュー** (2分):
   ```bash
   codex "Use codex-supervisor to review core, supervisor, and deep-research in parallel"
   ```

---

## 🎯 期待される成果

### レビュー後に得られる情報:

1. **コード品質スコア**:
   - 各モジュールの評価（0-10点）
   - 全体の品質指標

2. **優先度付きイシューリスト**:
   - P0: Critical（すぐに修正が必要）
   - P1: Important（重要な改善）
   - P2: Nice-to-have（あれば良い改善）

3. **具体的な改善提案**:
   - Before/Afterコード例
   - ベストプラクティスへの言及
   - パフォーマンス最適化案

4. **技術的負債の特定**:
   - 複雑度が高い箇所
   - テストカバレッジが低い箇所
   - ドキュメント不足の箇所

---

## 📝 レビュー結果の活用

### 1. イシュー作成

レビュー結果から GitHub Issue を作成:
```bash
# 例: P1イシューをGitHub Issueに
git checkout -b fix/improve-error-handling
# 修正を実施
git commit -m "fix: Improve error handling based on code review"
```

### 2. ドキュメント更新

コメント不足を指摘された場合:
```rust
// Before
pub fn process(&self) -> Result<()> { ... }

// After
/// Processes the current operation with state validation.
///
/// # Errors
/// Returns an error if:
/// - State is invalid
/// - Resource is unavailable
pub fn process(&self) -> Result<()> { ... }
```

### 3. テスト追加

エッジケースを指摘された場合:
```rust
#[test]
fn test_edge_case_empty_input() {
    // レビューで指摘されたエッジケースのテスト
}
```

---

## 🔥 今すぐ実行！

**最も簡単な開始方法**（30秒）:

1. 新しいPowerShellウィンドウを開く
2. 以下をコピー&ペースト:

```bash
cd C:\Users\downl\Desktop\codex-main\codex-main ; codex "Use codex-agent to review examples/simple_add.rs"
```

3. Enterキーを押す
4. レビュー結果を確認

**次のステップ**（1分）:

```bash
codex "Use codex-agent to review codex-rs/core/src/codex.rs focusing on error handling and state management"
```

**包括的レビュー**（2分）:

```bash
codex "Use codex-supervisor to review core, supervisor, and deep-research modules in parallel with priority issues"
```

---

## 📊 ベンチマーク（参考）

| レビュー対象 | 実行時間 | トークン消費 | Subagent数 |
|-------------|---------|-------------|-----------|
| サンプルファイル | 30秒 | ~500 tokens | 1 |
| 単一モジュールファイル | 1分 | ~1,500 tokens | 1 |
| モジュール全体 | 2分 | ~3,000 tokens | 1 |
| 並列レビュー（3モジュール） | 2分 | ~5,000 tokens | 3 |

**並列実行の効果**:
- 単一実行: 3分
- 並列実行: 2分（**33%削減**）

---

## ✅ チェックリスト

実行前の確認:
- [ ] Codex CLI インストール済み（`codex --version`）
- [ ] デフォルトモデル: gpt-5-codex
- [ ] MCP Server: codex-agent 有効（`codex mcp list`）
- [ ] OpenAI API キー設定済み

実行後の確認:
- [ ] TUIが正常に起動した
- [ ] Subagentが呼び出された（`[MCP]` 表示）
- [ ] レビュー結果が表示された
- [ ] 改善提案が具体的だった

---

**Status**: ✅ すぐに実行可能！  
**推奨**: まずはサンプルファイル（30秒）から試してください 🚀

---

**作成日**: 2025-10-13  
**実装**: zapabob/codex + codex-agent MCP  
**参考**: Web検索結果（GitHubPR連携、Codex設定）

