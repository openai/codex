# 🚀 Codex サブエージェント & Deep Research 継続的改良メタプロンプト

**作成日**: 2025-10-11 JST  
**対象プロジェクト**: zapabob/codex  
**現在バージョン**: 0.47.0-alpha.1  
**目的**: サブエージェント機能とDeep Research機能の継続的ブラッシュアップ

---

## 📋 このドキュメントの使い方

### AI セッション開始時

```markdown
@codex このプロジェクトの改良を続けます。
`.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md`を読んで、
現在の実装状況を把握してから作業を開始してください。
```

### 改良タスク実行時

1. このメタプロンプトを読む
2. `_docs/`の最新実装ログを確認
3. 改良を実施
4. テスト実行
5. コミット（セマンティックバージョニング準拠）
6. 実装ログを`_docs/`に保存

---

## 🎯 プロジェクトの現状（2025-10-11時点）

### ✅ 完成済み機能

#### Deep Research (v0.47.0-alpha.1)
```
✅ DuckDuckGo HTMLスクレイピング
✅ URLデコーダー（リダイレクト解決）
✅ 202エラー対策（高品質フォールバック）
✅ researchカスタムコマンド
✅ OpenAI/codex Web検索統合
✅ APIキー不要（$0コスト）
✅ scraperクレート統合（堅牢なHTMLパース）
```

**実装ファイル**:
- `codex-rs/deep-research/src/web_search_provider.rs`
- `codex-rs/deep-research/src/url_decoder.rs`
- `codex-rs/cli/src/research_cmd.rs`

#### サブエージェント (v0.47.0-alpha.1)
```
✅ 7種類のエージェント定義
   - code-reviewer
   - ts-reviewer
   - python-reviewer
   - unity-reviewer
   - test-gen
   - sec-audit
   - researcher
✅ delegateカスタムコマンド
✅ YAML設定読み込み
✅ 権限管理フレームワーク
✅ タスク実行シミュレーション
```

**実装ファイル**:
- `codex-rs/cli/src/delegate_cmd.rs`
- `.codex/agents/*.yaml`

---

## 🔧 改良優先度マトリクス

### 🔴 高優先度（次回セッションで着手）

#### 1. scraperクレート完全統合
**現状**: 部分的に統合済み  
**目標**: DuckDuckGo HTMLパースを完全にscraper化  
**理由**: regex依存を排除、堅牢性向上

**タスク**:
```rust
// codex-rs/deep-research/Cargo.toml
[dependencies]
scraper = "0.18"  // 追加確認

// web_search_provider.rs
// ✅ 既に実装済み（ユーザーが追加）
// 次: テストケース追加
```

**完了条件**:
- [ ] scraper依存追加
- [ ] 既存regexコード完全削除
- [ ] テストケース3件以上追加
- [ ] `cargo test -p codex-deep-research`成功

**コミットメッセージ例**:
```bash
feat(deep-research): replace regex with scraper for robust HTML parsing

- Remove regex-based DuckDuckGo parsing
- Use scraper::Html and scraper::Selector
- Add comprehensive test cases
- Improve error handling

Closes #XXX
```

---

#### 2. サブエージェント実行エンジン実装
**現状**: シミュレーションのみ  
**目標**: 実際にタスクを実行するランタイム実装

**タスク**:
```rust
// codex-rs/core/src/agent_runtime.rs（新規作成）
pub struct AgentRuntime {
    pub agent_def: AgentDefinition,
    pub budget: TokenBudget,
    pub permissions: PermissionSet,
}

impl AgentRuntime {
    pub async fn execute_task(
        &self,
        goal: &str,
        inputs: &HashMap<String, String>,
    ) -> Result<AgentExecutionResult> {
        // 1. トークンバジェット確認
        // 2. 権限チェック
        // 3. MCP ツール呼び出し
        // 4. 結果集約
        // 5. artifacts生成
    }
}
```

**完了条件**:
- [ ] `AgentRuntime`構造体実装
- [ ] トークンバジェット管理
- [ ] 権限チェック機構
- [ ] MCPツール連携
- [ ] 統合テスト5件以上

**コミットメッセージ例**:
```bash
feat(core): implement AgentRuntime for real sub-agent execution

- Add AgentRuntime with budget and permission management
- Integrate with MCP tools
- Support code-reviewer, test-gen, sec-audit agents
- Add comprehensive integration tests

BREAKING CHANGE: delegate command now executes real tasks

Closes #XXX
```

---

#### 3. DuckDuckGo 202エラー根本解決
**現状**: フォールバックで対応  
**目標**: 実際の検索結果を取得

**アプローチ**:

##### A. DuckDuckGo Lite API探索
```bash
# 調査タスク
1. DuckDuckGo Instant Answer API調査
2. DuckDuckGo JSON API調査
3. SearXNG統合検討（セルフホスト）
```

##### B. 代替検索エンジン統合
```rust
// Brave Search API（推奨）
// - APIキー必須だが、月2000クエリ無料
// - 高品質な結果
// - 商用利用可

pub async fn brave_search_with_api(
    &self,
    query: &str,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}",
        urlencoding::encode(query)
    );
    // 実装...
}
```

**完了条件**:
- [ ] DuckDuckGo Instant Answer API統合 OR
- [ ] Brave Search API統合（環境変数で切替）
- [ ] 202エラー発生率 < 10%
- [ ] 実検索結果取得率 > 80%

**コミットメッセージ例**:
```bash
feat(deep-research): integrate Brave Search API as fallback

- Add Brave Search API support (BRAVE_API_KEY env var)
- Reduce DuckDuckGo 202 error impact
- Improve search result quality
- Maintain $0 operation when API key not set

Closes #XXX
```

---

### 🟡 中優先度（近日中に着手）

#### 4. Deep Research 計画型探索の改善
**現状**: 基本的なサブクエリ生成  
**目標**: より高度な探索戦略

**改善項目**:
```rust
// codex-rs/deep-research/src/planner.rs
pub struct EnhancedResearchPlanner {
    pub strategy: ResearchStrategy,  // Comprehensive, Quick, Deep
    pub max_depth: usize,
    pub breadth_per_level: usize,
    pub contradiction_checker: ContradictionChecker,
    pub citation_validator: CitationValidator,  // NEW
}

// 新機能
impl EnhancedResearchPlanner {
    // サブクエリの質向上
    pub fn generate_smart_subqueries(
        &self,
        main_topic: &str,
        context: &ResearchContext,
    ) -> Vec<String> {
        // - トピックの分解
        // - 関連概念の抽出
        // - 時系列考慮
        // - 言語・地域最適化
    }
    
    // 引用検証（NEW）
    pub async fn validate_citations(
        &self,
        sources: &[Source],
    ) -> Result<Vec<ValidatedSource>> {
        // - URL到達性確認
        // - コンテンツ整合性チェック
        // - 信頼性スコア計算
    }
}
```

**完了条件**:
- [ ] スマートサブクエリ生成
- [ ] 引用検証機能
- [ ] 信頼性スコアリング
- [ ] ベンチマーク改善（精度+10%）

---

#### 5. サブエージェント権限管理の強化
**現状**: YAML定義のみ  
**目標**: 実行時権限チェック

**実装**:
```rust
// codex-rs/core/src/permissions.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    pub file_read: FilePermission,
    pub file_write: FilePermission,
    pub shell: ShellPermission,
    pub network: NetworkPermission,
    pub mcp_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum FilePermission {
    None,
    ReadOnly(Vec<PathBuf>),      // 読み取り許可パス
    ReadWrite(Vec<PathBuf>),     // 読み書き許可パス
    Restricted(Vec<PathBuf>),    // 制限パス（除外）
}

impl PermissionSet {
    pub fn check_file_read(&self, path: &Path) -> Result<()> {
        match &self.file_read {
            FilePermission::None => {
                Err(anyhow!("File read permission denied"))
            }
            FilePermission::ReadOnly(allowed) => {
                if allowed.iter().any(|p| path.starts_with(p)) {
                    Ok(())
                } else {
                    Err(anyhow!("Path not in allowed list: {}", path.display()))
                }
            }
            // ...
        }
    }
}
```

**完了条件**:
- [ ] `PermissionSet`実装
- [ ] 実行時チェック追加
- [ ] 監査ログ出力
- [ ] テストカバレッジ90%以上

---

#### 6. 統合テストスイートの拡充
**現状**: 基本的なテストのみ  
**目標**: E2Eテスト完備

**テストカテゴリ**:
```rust
// codex-rs/deep-research/tests/
// 1. Unit Tests（既存）
// 2. Integration Tests（拡充必要）
// 3. E2E Tests（新規）

// tests/e2e_research_full.rs
#[tokio::test]
async fn test_full_research_workflow() {
    // 1. research コマンド実行
    // 2. DuckDuckGo検索
    // 3. サブクエリ生成
    // 4. 矛盾検出
    // 5. レポート生成
    // 6. 引用確認
}

// tests/e2e_delegate_full.rs
#[tokio::test]
async fn test_full_delegate_workflow() {
    // 1. delegate コマンド実行
    // 2. エージェント定義読み込み
    // 3. タスク実行
    // 4. 権限チェック
    // 5. artifacts生成
}
```

**完了条件**:
- [ ] E2Eテスト10件以上
- [ ] CIで自動実行
- [ ] カバレッジレポート生成

---

### 🟢 低優先度（時間があれば）

#### 7. GitHub Actions CI/CD強化
#### 8. Web UI for Deep Research
#### 9. VS Code拡張機能の改善
#### 10. パフォーマンスベンチマーク

---

## 📝 開発フロー（必須手順）

### 1. セッション開始時

```bash
# 1. このメタプロンプトを読む
cat .codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md

# 2. 最新実装ログ確認
ls -lt _docs/ | head -5

# 3. ブランチ確認
git branch
# main にいることを確認

# 4. 最新コミット確認
git log --oneline -5
```

---

### 2. 機能開発時

#### A. ブランチ戦略（推奨）

```bash
# 小さな改良: mainに直接コミット
git checkout main

# 大きな機能: feature ブランチ
git checkout -b feature/scraper-integration
git checkout -b feature/agent-runtime
git checkout -b fix/duckduckgo-202-error
```

#### B. コミット前チェックリスト

```bash
# 1. フォーマット
cd codex-rs
just fmt
# または
cargo fmt --all

# 2. Clippy（プロジェクト単位）
just fix -p codex-deep-research
just fix -p codex-cli
just fix -p codex-core

# 3. テスト（変更したクレート）
cargo test -p codex-deep-research
cargo test -p codex-cli

# 4. ビルド確認
cargo build --release -p codex-cli -p codex-deep-research

# 5. エラー・警告ゼロ確認
# （必須）
```

#### C. コミットメッセージ規約

**Conventional Commits 準拠**（必須）:

```bash
# 新機能
git commit -m "feat(deep-research): add scraper-based HTML parsing"

# バグ修正
git commit -m "fix(deep-research): resolve DuckDuckGo 202 error"

# リファクタリング
git commit -m "refactor(core): simplify agent runtime logic"

# ドキュメント
git commit -m "docs: update Deep Research usage guide"

# テスト
git commit -m "test(deep-research): add E2E tests for search flow"

# ビルド・CI
git commit -m "chore(ci): update GitHub Actions workflow"

# パフォーマンス
git commit -m "perf(deep-research): optimize HTML parsing speed"

# Breaking Change（重要）
git commit -m "feat(core)!: change AgentRuntime API

BREAKING CHANGE: AgentRuntime::new now requires PermissionSet parameter"
```

#### D. バージョン管理

**セマンティックバージョニング** (`VERSION`ファイル):

```
現在: 0.47.0-alpha.1

ルール:
- patch: 0.47.1-alpha.1 (バグ修正、小改良)
- minor: 0.48.0-alpha.1 (新機能追加)
- major: 1.0.0 (Breaking Change、GA)
- alpha→beta: 0.47.0-beta.1 (機能凍結)
- beta→GA: 0.47.0 (本番リリース)
```

**更新タイミング**:
```bash
# バグ修正: patch up
echo "0.47.1-alpha.1" > VERSION

# 新機能: minor up
echo "0.48.0-alpha.1" > VERSION

# Breaking Change: major up (慎重に)
echo "1.0.0-alpha.1" > VERSION
```

---

### 3. コミット実行

```bash
# 1. ステージング
git add .

# 2. コミット
git commit -m "feat(deep-research): integrate scraper for HTML parsing

- Replace regex with scraper crate
- Improve robustness of DuckDuckGo parsing
- Add comprehensive test cases
- Update documentation

Closes #123"

# 3. プッシュ（zapabob/codex main）
git push origin main
```

---

### 4. 実装ログ作成（必須）

```bash
# MCPサーバーで現在時刻取得してから実行
# ファイル名: _docs/yyyy-mm-dd_機能名.md

# 例:
_docs/2025-10-11_scraperクレート完全統合.md
_docs/2025-10-12_AgentRuntime実装完了.md
_docs/2025-10-13_Brave_Search_API統合.md
```

**テンプレート**:
```markdown
# 🚀 [機能名] 実装完了

**実装日時**: yyyy-mm-dd HH:MM JST  
**バージョン**: 0.XX.0-alpha.Y  
**Status**: ✅ 完了 / 🚧 進行中 / ⚠️ 課題あり

---

## 📋 実装内容

### 目的
[なぜこの機能を実装したか]

### 変更ファイル
- `path/to/file.rs`
- `path/to/another.rs`

### 主な変更点
1. [変更1]
2. [変更2]

---

## ✅ 完了条件チェック

- [ ] 実装完了
- [ ] テスト追加
- [ ] ドキュメント更新
- [ ] Clippy通過
- [ ] ビルド成功

---

## 🧪 テスト結果

```bash
cargo test -p codex-xxx
# 結果を貼り付け
```

---

## 📝 コミット情報

```bash
git log --oneline -1
# コミットハッシュとメッセージ
```

---

## 💡 今後の課題

- [課題1]
- [課題2]
```

---

## 🎯 品質基準（絶対遵守）

### Rust コード品質

```bash
✅ cargo fmt --all で整形
✅ just fix -p <project> でClippy警告ゼロ
✅ cargo test -p <project> で全テスト合格
✅ unsafe コード使用禁止（特別な理由がない限り）
✅ unwrap() 使用禁止（テスト以外）
✅ expect() 推奨（明確なエラーメッセージ付き）
✅ ? 演算子でエラー伝播
✅ anyhow::Result または thiserror でエラー型定義
```

### ドキュメント品質

```bash
✅ README.md 更新（新機能追加時）
✅ API ドキュメント（/// コメント）
✅ 実装ログ作成（_docs/）
✅ CHANGELOG.md 更新（リリース時）
```

### テスト品質

```bash
✅ 単体テスト（関数レベル）
✅ 統合テスト（モジュールレベル）
✅ E2Eテスト（システムレベル、重要機能のみ）
✅ カバレッジ目標: 70%以上（コア機能は90%）
```

---

## 🔄 OpenAI/codex との同期方針

### 基本方針

```
zapabob/codex = OpenAI/codex + サブエージェント + Deep Research

- OpenAI/codex の upstream 変更は定期的に取り込む
- zapabob/codex 独自機能は別ファイル・モジュールで実装
- コンフリクト最小化
```

### 取り込みフロー

```bash
# 1. upstream 追加（初回のみ）
git remote add upstream https://github.com/openai/codex.git

# 2. 定期的に upstream 確認（週1回推奨）
git fetch upstream

# 3. 差分確認
git log --oneline main..upstream/main

# 4. 取り込み（慎重に）
git merge upstream/main
# または
git rebase upstream/main

# 5. コンフリクト解決
# - zapabob独自機能を保持
# - upstreamの改善を取り込み

# 6. テスト
cargo test --all-features

# 7. プッシュ
git push origin main
```

### 独自機能の分離

```
推奨ディレクトリ構造:
codex-rs/
├── deep-research/    # zapabob独自
├── agent-runtime/    # zapabob独自（今後追加）
├── core/             # 一部zapabob拡張
├── cli/              # 一部zapabob拡張
└── [その他upstream] # 基本的に変更しない
```

---

## 🧪 テスト戦略

### 1. 単体テスト（必須）

```rust
// src/foo.rs
pub fn parse_url(url: &str) -> Result<ParsedUrl> {
    // 実装
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_valid() {
        let result = parse_url("https://example.com").unwrap();
        assert_eq!(result.scheme, "https");
        assert_eq!(result.host, "example.com");
    }

    #[test]
    fn test_parse_url_invalid() {
        let result = parse_url("not a url");
        assert!(result.is_err());
    }
}
```

### 2. 統合テスト（推奨）

```rust
// tests/integration_search.rs
use codex_deep_research::WebSearchProvider;

#[tokio::test]
async fn test_duckduckgo_search_integration() {
    let provider = WebSearchProvider::new();
    let results = provider.duckduckgo_search_real("Rust async", 5).await;
    
    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(results.len() > 0);
    assert!(results.len() <= 5);
}
```

### 3. E2Eテスト（重要機能のみ）

```rust
// tests/e2e_cli.rs
use std::process::Command;

#[test]
fn test_research_command_e2e() {
    let output = Command::new("codex")
        .arg("research")
        .arg("Rust async")
        .arg("--depth")
        .arg("1")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Research Report"));
}
```

---

## 📊 パフォーマンス目標

### Deep Research

| 指標 | 目標 | 現状 |
|------|------|------|
| **検索速度** | < 3秒/クエリ | ~2秒 ✅ |
| **精度** | > 80% | ~75% 🟡 |
| **コスト** | $0 | $0 ✅ |

### サブエージェント

| 指標 | 目標 | 現状 |
|------|------|------|
| **起動速度** | < 1秒 | ~0.5秒 ✅ |
| **タスク成功率** | > 90% | シミュレーション 🔴 |
| **トークン効率** | < 50k/タスク | 未測定 🔴 |

---

## 🐛 既知の課題（次回対応）

### Deep Research

1. **DuckDuckGo 202エラー**
   - 現状: フォールバックで対応
   - 目標: 実検索結果取得
   - 優先度: 🔴 高

2. **scraperクレート完全移行**
   - 現状: 部分的統合
   - 目標: regex完全削除
   - 優先度: 🔴 高

3. **引用検証機能**
   - 現状: 未実装
   - 目標: URL到達性・整合性チェック
   - 優先度: 🟡 中

### サブエージェント

1. **実行エンジン未実装**
   - 現状: シミュレーションのみ
   - 目標: AgentRuntime実装
   - 優先度: 🔴 高

2. **権限管理未実装**
   - 現状: YAML定義のみ
   - 目標: 実行時チェック
   - 優先度: 🔴 高

3. **MCPツール連携**
   - 現状: 未実装
   - 目標: ツール呼び出し機構
   - 優先度: 🟡 中

---

## 💡 アイデアメモ（将来の拡張）

### Phase 2: Enhanced Deep Research
- [ ] 多言語検索対応（日本語、中国語、etc.）
- [ ] 画像検索統合
- [ ] 動画検索統合
- [ ] 学術論文検索（arXiv, Google Scholar）
- [ ] リアルタイムニュース検索

### Phase 3: Advanced Sub-Agents
- [ ] マルチエージェント協調（複数エージェント並列実行）
- [ ] エージェント学習（過去タスクから学習）
- [ ] カスタムエージェント作成UI
- [ ] エージェントマーケットプレイス

### Phase 4: Enterprise Features
- [ ] チーム共有機能
- [ ] 監査ログ永続化
- [ ] コスト管理ダッシュボード
- [ ] SSO/RBAC統合

---

## 🎓 参考リソース

### 公式ドキュメント
- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs/)
- [Serde Documentation](https://serde.rs/)
- [scraper Documentation](https://docs.rs/scraper/)

### プロジェクトドキュメント
- `docs/codex-subagents-deep-research.md` - 詳細設計
- `.codex/README.md` - エージェント設定
- `AGENTS.md` - サブエージェント一覧
- `README.md` - プロジェクト概要

### 関連Issue（GitHub）
- zapabob/codex Issues: [適宜追加]

---

## 🚨 トラブルシューティング

### ビルドエラー

```bash
# エラー: Cargo.lock コンフリクト
git checkout --theirs Cargo.lock
cargo update

# エラー: 依存関係の問題
cargo clean
cargo build --release

# エラー: rustfmt/clippy 問題
rustup update
cargo install cargo-insta  # snapshot テスト用
```

### テスト失敗

```bash
# snapshot テスト更新
cargo insta review -p codex-tui

# 特定のテストのみ実行
cargo test -p codex-deep-research test_duckduckgo

# デバッグ出力有効化
RUST_LOG=debug cargo test -- --nocapture
```

### 実行時エラー

```bash
# Deep Research 202エラー
# → 正常（フォールバックが動作）

# delegate コマンドエラー
# → エージェント定義YAML確認: .codex/agents/

# グローバルインストール問題
npm cache clean --force
npm uninstall -g @openai/codex
npm install -g .
```

---

## 📅 リリース管理

### Alpha (現在)
- バージョン: 0.47.0-alpha.1
- 対象: 開発者、早期adopter
- 頻度: 随時
- 変更: Breaking Changeあり

### Beta (次期)
- バージョン: 0.47.0-beta.1
- 対象: 限定ユーザー
- 頻度: 月1回
- 変更: 機能凍結、バグ修正のみ

### GA (将来)
- バージョン: 1.0.0
- 対象: 一般ユーザー
- 頻度: 四半期ごと
- 変更: セマンティックバージョニング厳守

---

## 🎯 マイルストーン

### M1: scraperクレート完全統合（2週間以内）
- [ ] regex依存削除
- [ ] テストケース追加
- [ ] ドキュメント更新

### M2: AgentRuntime MVP（1ヶ月以内）
- [ ] 基本的な実行エンジン
- [ ] code-reviewer実装
- [ ] 権限チェック機構

### M3: Deep Research v2（2ヶ月以内）
- [ ] Brave Search API統合
- [ ] 引用検証機能
- [ ] 精度目標達成（> 80%）

### M4: Beta リリース（3ヶ月以内）
- [ ] 全機能実装完了
- [ ] E2Eテスト完備
- [ ] ドキュメント完成

---

## ✅ セッション終了時チェックリスト

```bash
# 必ず実行:
[ ] 変更をコミット（Conventional Commits準拠）
[ ] 実装ログ作成（_docs/yyyy-mm-dd_機能名.md）
[ ] README.md更新（新機能の場合）
[ ] VERSION更新（必要に応じて）
[ ] git push origin main

# 推奨:
[ ] CHANGELOG.md更新
[ ] GitHub Issue更新/クローズ
[ ] このメタプロンプト更新（大きな変更の場合）
```

---

## 🎉 最後に

**このプロジェクトの目標**:
```
✨ OpenAI/codex の機能を拡張し、
   サブエージェントとDeep Researchで
   開発者の生産性を10倍にする ✨
```

**開発哲学**:
```
1. ユーザー体験を最優先
2. コスト$0を維持
3. 品質基準を妥協しない
4. コミュニティ貢献を重視
5. 楽しく開発する！😊
```

**なんJ精神**:
```
💪 完璧を目指すが、完璧主義に陥らない
💪 小さく速く改良を重ねる
💪 失敗を恐れず、学びを活かす
💪 コミュニティと共に成長する
```

---

**作成日**: 2025-10-11 JST  
**最終更新**: 2025-10-11 JST  
**バージョン**: 1.0.0  
**Status**: ✅ **Active**

**🎊 ええ開発を！完璧や！！！ 🎊**

---

**END OF META PROMPT**

