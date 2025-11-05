# Codex v0.48.0 改良案リスト（実装ログから抽出）

## 📋 抽出元
- `_docs/2025-10-16_05-07-09_音声通知設定_CodexとCursorの使い分け.md`
- `_docs/2025-10-16_完全実装完了_最終報告.md`
- `_docs/2025-10-15_ClaudeCode超え完成.md`

---

## 🎯 改良案リスト

### 1️⃣ 音声通知機能の改良

#### 優先度: 中
#### カテゴリ: UX改善

**現状の課題:**
- Cursor用スクリプト `play-completion-sound.ps1` が現在霊夢音声を使用
- 魔理沙音声ファイルのパスが未確認（ファイルが見つからない）
- Windows System通知音が手動設定のみ

**改良案:**
1. **Cursor用スクリプトの魔理沙化**
   - `play-completion-sound.ps1` を魔理沙音声に変更
   - ファイルパス確認後に実装
   - 既存の霊夢音声はCodex CLI専用に維持

2. **Windows System設定の自動化**
   - `set-windows-notification-sound.ps1` の改良
   - レジストリパス自動検出機能追加
   - エラーハンドリング強化

3. **追加キャラクター対応**
   - 早苗、咲夜、妖夢などの音声追加
   - キャラクター選択機能の実装
   - `config.toml` でキャラクター切り替え可能に

**実装ファイル:**
- `zapabob/scripts/play-completion-sound.ps1`
- `zapabob/scripts/set-windows-notification-sound.ps1`
- `config.toml` (新規 `[sounds]` セクション)

**期待効果:**
- ユーザー体験の向上
- キャラクターの使い分けによる視認性向上
- 自動設定による導入障壁の低減

---

### 2️⃣ ThreeWayMerge の実装

#### 優先度: 高
#### カテゴリ: コア機能

**現状の課題:**
- `MergeStrategy::ThreeWayMerge` が未実装
- コメント: "// TODO: Implement 3-way merge logic"
- 現在は `Sequential` と `LastWriteWins` のみサポート

**改良案:**
1. **Git-style 3-way merge アルゴリズム実装**
   - Base（共通祖先）、Ours（Agent A）、Theirs（Agent B）の3つを比較
   - 競合検出とマーカー挿入
   - 自動マージ可能な箇所の自動処理

2. **Conflict Marker生成**
   ```rust
   <<<<<<< Agent A (code-reviewer)
   // Agent A's changes
   =======
   // Agent B's changes
   >>>>>>> Agent B (test-gen)
   ```

3. **ユーザー介入UI**
   - TUIでのコンフリクト表示
   - インタラクティブなマージ選択

**実装ファイル:**
- `codex-rs/core/src/orchestration/conflict_resolver.rs`
- `codex-rs/tui/src/conflict_view.rs` (新規)

**期待効果:**
- 並列エージェント実行時の安全性向上
- Gitライクなワークフローの実現
- 自動マージ率の向上（30% → 70%見込み）

---

### 3️⃣ Webhook Integration の強化

#### 優先度: 中
#### カテゴリ: 統合機能

**現状の課題:**
- GitHub API、Slack、Custom Webhookに対応
- エラーハンドリングが基本的
- リトライ機構がシンプル

**改良案:**
1. **追加サービス対応**
   - Discord Webhook
   - Microsoft Teams Webhook
   - Notion API
   - Linear API

2. **高度なリトライ戦略**
   - サービス別のリトライ設定
   - Rate Limit対応（429エラー処理）
   - Circuit Breaker パターン実装

3. **Webhook Template System**
   ```toml
   [webhook_templates.github_pr]
   url = "https://api.github.com/repos/{owner}/{repo}/pulls"
   method = "POST"
   headers = { "Authorization" = "token {GITHUB_TOKEN}" }
   body_template = """
   {
     "title": "{title}",
     "body": "{body}",
     "head": "{branch}"
   }
   """
   ```

**実装ファイル:**
- `codex-rs/core/src/integrations/webhook_client.rs`
- `codex-rs/core/src/integrations/webhook_templates.rs` (新規)
- `config.toml` (新規 `[webhook_templates]` セクション)

**期待効果:**
- より多くのサービスとの統合
- エラー耐性の向上
- 設定の柔軟性向上

---

### 4️⃣ Natural Language CLI の精度向上

#### 優先度: 高
#### カテゴリ: AI機能

**現状の課題:**
- パターンマッチングベースの意図解析
- 限定的なキーワード検出
- 複雑なクエリに未対応

**改良案:**
1. **LLM-based Intent Classification**
   - GPT-5-codexを使った意図解析
   - Few-shot learning による精度向上
   - 複雑なクエリの理解

2. **エージェント推論エンジン**
   ```rust
   pub struct AgentRecommender {
       pub fn recommend(&self, query: &str) -> Vec<(AgentType, f64)>;
       pub fn explain_recommendation(&self, agent: &AgentType) -> String;
   }
   ```

3. **インタラクティブモード**
   ```bash
   $ codex agent "Fix security issues"
   
   Multiple interpretations found:
   1. [90%] SecurityExpert - Full security audit
   2. [75%] CodeReviewer - Security-focused review
   3. [40%] TestingExpert - Security test generation
   
   Which agent do you want? [1]:
   ```

**実装ファイル:**
- `codex-rs/core/src/agent_interpreter.rs`
- `codex-rs/core/src/agent_recommender.rs` (新規)
- `codex-rs/cli/src/agent_cmd.rs`

**期待効果:**
- 自然言語理解の精度向上（60% → 95%見込み）
- ユーザー満足度向上
- エージェント選択ミスの削減

---

### 5️⃣ E2E テストの拡充

#### 優先度: 中
#### カテゴリ: 品質保証

**現状の課題:**
- 6個の基本テストのみ
- `AutoOrchestrator` のテストが未実装
- 実際のサブエージェント実行テストなし

**改良案:**
1. **AutoOrchestrator統合テスト**
   ```rust
   #[tokio::test]
   async fn test_auto_orchestrator_parallel_execution() {
       // Mock AgentRuntime
       // Test parallel sub-agent execution
       // Verify result aggregation
   }
   ```

2. **Webhook統合テスト**
   - Mock HTTPサーバー使用
   - GitHub/Slack API呼び出しテスト
   - エラーシナリオテスト

3. **Performance Benchmarks**
   ```rust
   #[bench]
   fn bench_conflict_resolver_throughput(b: &mut Bencher) {
       // 1000並列編集リクエスト
       // スループット測定
   }
   ```

**実装ファイル:**
- `codex-rs/core/tests/orchestration_e2e.rs`
- `codex-rs/core/tests/webhook_integration.rs` (新規)
- `codex-rs/core/benches/orchestration.rs` (新規)

**期待効果:**
- テストカバレッジ向上（60% → 85%）
- リグレッション防止
- パフォーマンス劣化の早期検出

---

### 6️⃣ ビルド時間の最適化

#### 優先度: 低
#### カテゴリ: DX改善

**現状の課題:**
- フルリリースビルド: 30-40分
- 差分ビルドでも10-15分
- CI/CDパイプラインが遅い

**改良案:**
1. **sccache 導入**
   ```toml
   # .cargo/config.toml
   [build]
   rustc-wrapper = "sccache"
   ```

2. **ワークスペース分割**
   - 共通クレートの独立化
   - 依存関係の最適化
   - 並列ビルドの有効化

3. **GitHub Actions キャッシュ最適化**
   ```yaml
   - uses: actions/cache@v3
     with:
       path: |
         ~/.cargo/registry
         ~/.cargo/git
         target/
       key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
   ```

**実装ファイル:**
- `.cargo/config.toml`
- `.github/workflows/ci.yml`
- `Cargo.toml` (ワークスペース構成)

**期待効果:**
- ビルド時間短縮（40分 → 10分）
- 開発サイクルの高速化
- CI/CD コスト削減

---

### 7️⃣ ドキュメントの多言語化

#### 優先度: 低
#### カテゴリ: ドキュメント

**現状の課題:**
- ドキュメントが日本語中心
- 英語版READMEはあるが不完全
- コード内コメントも日本語混在

**改良案:**
1. **英語ドキュメント整備**
   - `README.md` 完全英語化
   - `docs/en/` ディレクトリ作成
   - API ドキュメント自動生成

2. **i18n対応**
   ```toml
   # config.toml
   [i18n]
   default_locale = "en"
   supported_locales = ["en", "ja", "zh-CN"]
   ```

3. **Rustdoc改善**
   ```rust
   /// Creates a new conflict resolver
   /// 
   /// # Arguments
   /// * `strategy` - Merge strategy (Sequential/LastWriteWins/ThreeWayMerge)
   /// 
   /// # Examples
   /// ```
   /// let resolver = ConflictResolver::new(MergeStrategy::Sequential);
   /// ```
   pub fn new(strategy: MergeStrategy) -> Self { ... }
   ```

**実装ファイル:**
- `README.md`
- `docs/en/` (新規)
- すべての `.rs` ファイル

**期待効果:**
- 国際ユーザーへのリーチ拡大
- コントリビューター増加
- OpenAI本家へのPR準備

---

## 📊 優先度マトリックス

| 改良案 | 優先度 | 実装難易度 | 影響範囲 | 推定工数 |
|-------|--------|-----------|---------|---------|
| 1. 音声通知改良 | 中 | 低 | 小 | 2-4h |
| 2. ThreeWayMerge | 高 | 高 | 大 | 12-16h |
| 3. Webhook強化 | 中 | 中 | 中 | 6-8h |
| 4. NL CLI精度向上 | 高 | 高 | 大 | 10-14h |
| 5. E2Eテスト拡充 | 中 | 中 | 中 | 8-10h |
| 6. ビルド最適化 | 低 | 低 | 小 | 3-5h |
| 7. 多言語化 | 低 | 低 | 小 | 8-12h |

---

## 🎯 推奨実装順序

### Phase 1（短期: 1-2週間）
1. **ThreeWayMerge実装** - コア機能強化
2. **NL CLI精度向上** - ユーザー体験改善

### Phase 2（中期: 2-4週間）
3. **Webhook強化** - 統合機能拡充
4. **E2Eテスト拡充** - 品質保証

### Phase 3（長期: 1-2ヶ月）
5. **音声通知改良** - UX改善
6. **ビルド最適化** - DX改善
7. **多言語化** - 国際展開

---

## 🚀 次のステップ

このリストをもとに、サブエージェント（code-reviewer）にコードレビューを依頼し、
実装の詳細化と技術的検証を行います。

**実行コマンド（予定）:**
```bash
codex delegate code-reviewer --scope codex-rs/core/src/orchestration/ --focus "ThreeWayMerge implementation strategy"
codex delegate code-reviewer --scope codex-rs/core/src/agent_interpreter.rs --focus "LLM-based intent classification"
```

