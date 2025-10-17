# 📋 zapabob/codex 機能要件定義書

**文書番号**: REQ-SPEC-001  
**作成日**: 2025-10-11 JST  
**バージョン**: 1.0.0  
**対象システム**: zapabob/codex v0.47.0-alpha.1  
**対象機能**: サブエージェント機構 & Deep Research機能

---

## 📑 文書管理

| 項目 | 内容 |
|------|------|
| **文書名** | サブエージェント & Deep Research 機能要件定義書 |
| **文書ID** | REQ-SPEC-001 |
| **作成者** | zapabob/codex Development Team |
| **承認者** | Project Lead |
| **配布先** | 開発チーム、QA、関連部門 |
| **機密区分** | 社外秘 |

### 変更履歴

| バージョン | 日付 | 変更内容 | 変更者 |
|-----------|------|---------|--------|
| 1.0.0 | 2025-10-11 | 初版作成 | zapabob |

---

## 📖 目次

1. [プロジェクト概要](#1-プロジェクト概要)
2. [システム概要](#2-システム概要)
3. [機能要件](#3-機能要件)
4. [非機能要件](#4-非機能要件)
5. [システム構成](#5-システム構成)
6. [API仕様](#6-api仕様)
7. [データモデル](#7-データモデル)
8. [ユースケース](#8-ユースケース)
9. [テスト要件](#9-テスト要件)
10. [セキュリティ要件](#10-セキュリティ要件)
11. [パフォーマンス要件](#11-パフォーマンス要件)
12. [運用要件](#12-運用要件)
13. [制約事項](#13-制約事項)
14. [用語集](#14-用語集)

---

## 1. プロジェクト概要

### 1.1 背景

OpenAI/codexは強力なコーディングエージェントであるが、以下の課題が存在する：

1. **専門タスクの効率性**: 単一エージェントでは多様な専門タスクに対応しきれない
2. **情報収集能力**: Web検索機能が限定的
3. **スケーラビリティ**: 大規模タスクを分割して並列実行できない
4. **コスト**: 外部API依存でコスト増加

### 1.2 目的

zapabob/codexは、OpenAI/codexを拡張し、以下を実現する：

1. **サブエージェント機構**: 専門タスクを自律的に実行するサブエージェントシステム
2. **Deep Research機能**: 多段階探索・矛盾検出・引用管理を備えた高度な情報収集機能
3. **コスト最適化**: APIキー不要なDuckDuckGo統合により$0運用を実現

### 1.3 スコープ

#### 対象範囲（In Scope）

```
✅ サブエージェント機構
   - エージェント定義（YAML）
   - タスク委譲システム
   - 権限管理
   - トークンバジェット管理

✅ Deep Research機能
   - Web検索（DuckDuckGo, Brave, Google, Bing）
   - 多段階探索
   - 矛盾検出
   - 引用管理
   - レポート生成

✅ CLI拡張
   - codex delegate コマンド
   - codex research コマンド
```

#### 対象外（Out of Scope）

```
❌ GUI実装（Phase 2以降）
❌ モバイルアプリ
❌ 商用APIサービス化
❌ リアルタイムコラボレーション
```

### 1.4 ステークホルダー

| 役割 | 担当者/組織 | 責任 |
|------|-----------|------|
| **プロダクトオーナー** | zapabob | 要件定義、優先度決定 |
| **開発リード** | zapabob | 実装統括、技術判断 |
| **QAリード** | Community | テスト計画、品質保証 |
| **ユーザー代表** | Early Adopters | フィードバック、受入テスト |

---

## 2. システム概要

### 2.1 システム構成図

```
┌─────────────────────────────────────────────────────────────┐
│                      zapabob/codex                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────┐         ┌──────────────────┐         │
│  │  CLI Interface   │         │   TUI Interface  │         │
│  │  - delegate cmd  │         │   - Agent UI     │         │
│  │  - research cmd  │         │   - Search UI    │         │
│  └────────┬─────────┘         └────────┬─────────┘         │
│           │                            │                    │
│           └────────────┬───────────────┘                    │
│                        │                                    │
│           ┌────────────▼───────────────┐                    │
│           │    Core Engine (Rust)      │                    │
│           ├────────────────────────────┤                    │
│           │  ┌──────────────────────┐  │                    │
│           │  │ Agent Runtime        │  │                    │
│           │  │ - Task Executor      │  │                    │
│           │  │ - Budget Manager     │  │                    │
│           │  │ - Permission Checker │  │                    │
│           │  └──────────────────────┘  │                    │
│           │  ┌──────────────────────┐  │                    │
│           │  │ Research Engine      │  │                    │
│           │  │ - Search Provider    │  │                    │
│           │  │ - Query Planner      │  │                    │
│           │  │ - Contradiction Det. │  │                    │
│           │  │ - Report Generator   │  │                    │
│           │  └──────────────────────┘  │                    │
│           └────────────┬───────────────┘                    │
│                        │                                    │
└────────────────────────┼────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
    ┌────▼────┐    ┌────▼────┐    ┌────▼────┐
    │ Agent   │    │  Web    │    │  MCP    │
    │ Configs │    │ Search  │    │ Tools   │
    │ (.yaml) │    │ APIs    │    │         │
    └─────────┘    └─────────┘    └─────────┘
```

### 2.2 システム境界

#### 内部コンポーネント
- CLI/TUIインターフェース
- Agent Runtime（タスク実行エンジン）
- Research Engine（情報収集エンジン）
- 権限管理システム
- バジェット管理システム

#### 外部システム連携
- Web検索API（DuckDuckGo, Brave, Google, Bing）
- MCPサーバー（ツール統合）
- OpenAI API（LLM実行）
- Git（バージョン管理）

---

## 3. 機能要件

### 3.1 サブエージェント機構

#### FR-SA-001: エージェント定義

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了

**要件**:
- エージェント定義をYAML形式で記述可能であること
- 定義項目: name, goal, max_tokens, permissions, tools, constraints
- `.codex/agents/` ディレクトリ配下に配置

**受入基準**:
```yaml
# .codex/agents/code-reviewer.yaml
name: "Code Reviewer"
goal: "多言語対応コードレビュー"
max_tokens: 40000
permissions:
  file_read: ["./src", "./tests"]
  file_write: []
  shell: false
  network: false
tools:
  - "ast_analyzer"
  - "linter"
constraints:
  - "unsafe コード禁止"
  - "unwrap() 禁止"
```

---

#### FR-SA-002: delegate コマンド

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了（シミュレーション）

**要件**:
```bash
codex delegate <agent> [options]

オプション:
  --scope <path>      # 対象スコープ
  --goal <string>     # 目標（オプション）
  --budget <number>   # トークン上限
  --deadline <secs>   # 実行時間制限
  --out <path>        # 結果出力先
```

**受入基準**:
- コマンド実行でエージェント起動
- エージェント定義読み込み成功
- タスク実行（現状はシミュレーション）
- 結果表示（status, tokens_used, duration, artifacts）

---

#### FR-SA-003: Agent Runtime

**優先度**: 🔴 Critical  
**実装状況**: 🚧 未実装（Phase 2）

**要件**:
- サブエージェントの実行環境を提供
- トークンバジェット管理
- 権限チェック
- タスク実行・監視
- 結果集約

**データ構造**:
```rust
pub struct AgentRuntime {
    pub agent_def: AgentDefinition,
    pub budget: TokenBudget,
    pub permissions: PermissionSet,
    pub executor: TaskExecutor,
}

impl AgentRuntime {
    pub async fn execute_task(
        &self,
        goal: &str,
        inputs: &HashMap<String, String>,
    ) -> Result<AgentExecutionResult>;
}
```

**受入基準**:
- [ ] AgentRuntimeインスタンス生成
- [ ] トークンバジェット管理（残量チェック）
- [ ] 権限チェック（file/shell/network）
- [ ] タスク実行
- [ ] 結果返却（status, tokens, artifacts）

---

#### FR-SA-004: エージェント種類

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了（定義のみ）

**要件**:
以下7種類のエージェントを提供：

| エージェント | 用途 | 実装状況 |
|-------------|------|---------|
| **code-reviewer** | コードレビュー（4言語対応） | ✅ YAML定義済み |
| **ts-reviewer** | TypeScript専用レビュー | ✅ YAML定義済み |
| **python-reviewer** | Python専用レビュー | ✅ YAML定義済み |
| **unity-reviewer** | Unity C#専用レビュー | ✅ YAML定義済み |
| **test-gen** | テスト生成 | ✅ YAML定義済み |
| **sec-audit** | セキュリティ監査 | ✅ YAML定義済み |
| **researcher** | Deep Research実行 | ✅ YAML定義済み |

---

#### FR-SA-005: 権限管理

**優先度**: 🔴 Critical  
**実装状況**: 🚧 設計済み（実装待ち）

**要件**:
```rust
pub struct PermissionSet {
    pub file_read: FilePermission,
    pub file_write: FilePermission,
    pub shell: ShellPermission,
    pub network: NetworkPermission,
    pub mcp_tools: Vec<String>,
}

pub enum FilePermission {
    None,
    ReadOnly(Vec<PathBuf>),
    ReadWrite(Vec<PathBuf>),
    Restricted(Vec<PathBuf>),
}
```

**受入基準**:
- [ ] ファイル読み取り権限チェック
- [ ] ファイル書き込み権限チェック
- [ ] シェルコマンド実行権限チェック
- [ ] ネットワークアクセス権限チェック
- [ ] 権限違反時にエラー返却
- [ ] 監査ログ記録

---

#### FR-SA-006: トークンバジェット管理

**優先度**: 🟡 High  
**実装状況**: 🚧 設計済み（実装待ち）

**要件**:
```rust
pub struct TokenBudget {
    pub total: usize,
    pub used: usize,
    pub reserved: usize,
}

impl TokenBudget {
    pub fn check_available(&self, required: usize) -> bool;
    pub fn consume(&mut self, amount: usize) -> Result<()>;
    pub fn release(&mut self, amount: usize);
    pub fn remaining(&self) -> usize;
}
```

**受入基準**:
- [ ] バジェット初期化（max_tokens）
- [ ] 使用量トラッキング
- [ ] 残量チェック
- [ ] 超過時にエラー返却
- [ ] リアルタイム残量表示

---

### 3.2 Deep Research機能

#### FR-DR-001: research コマンド

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了

**要件**:
```bash
codex research "<query>" [options]

オプション:
  --depth <1-5>       # 探索深度（デフォルト: 1）
  --breadth <1-10>    # 幅（サブクエリ数、デフォルト: 3）
  --strategy <type>   # 戦略（comprehensive|quick|deep）
  --budget <tokens>   # トークン上限
  --out <path>        # レポート出力先
```

**受入基準**:
- ✅ コマンド実行で研究開始
- ✅ 探索深度・幅の指定
- ✅ レポート生成・表示
- ✅ 引用付き結果

---

#### FR-DR-002: Web検索統合

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了

**要件**:
複数の検索エンジンをサポート：

| 検索エンジン | APIキー | 優先度 | 実装状況 |
|------------|--------|--------|---------|
| **DuckDuckGo** | 不要 | デフォルト | ✅ 完了 |
| **Brave Search** | 必要 | 高 | 🚧 未実装 |
| **Google Custom** | 必要 | 中 | 🚧 未実装 |
| **Bing Search** | 必要 | 低 | 🚧 未実装 |

**フォールバックチェーン**:
```
1. Brave Search (APIキーあれば)
2. Google Custom Search (APIキーあれば)
3. Bing Search (APIキーあれば)
4. DuckDuckGo HTMLスクレイピング（フォールバック）
```

**受入基準**:
- ✅ DuckDuckGo HTMLスクレイピング動作
- ✅ URLデコード（リダイレクト解決）
- ✅ 202エラー時のフォールバック
- [ ] Brave Search API統合
- [ ] Google Custom Search API統合

---

#### FR-DR-003: 多段階探索

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了（基本実装）

**要件**:
```rust
pub struct ResearchPlanner {
    pub max_depth: usize,
    pub breadth_per_level: usize,
    pub strategy: ResearchStrategy,
}

// 探索戦略
pub enum ResearchStrategy {
    Comprehensive,  // 包括的（デフォルト）
    Quick,          // 高速
    Deep,           // 深掘り
}
```

**受入基準**:
- ✅ サブクエリ生成
- ✅ 深度優先/幅優先探索
- ✅ 戦略選択（comprehensive/quick/deep）
- [ ] スマートサブクエリ生成（改善必要）

---

#### FR-DR-004: 矛盾検出

**優先度**: 🟡 High  
**実装状況**: ✅ 完了

**要件**:
- 複数ソースからの情報を比較
- 矛盾する情報を検出
- 信頼性スコアリング
- ユーザーに警告表示

**アルゴリズム**:
```rust
pub struct ContradictionChecker {
    pub threshold: f64,
}

impl ContradictionChecker {
    pub fn detect_contradictions(
        &self,
        sources: &[Source],
    ) -> Vec<Contradiction>;
}
```

**受入基準**:
- ✅ 矛盾検出機能実装
- ✅ 矛盾カウント表示
- [ ] 詳細な矛盾レポート（改善必要）

---

#### FR-DR-005: 引用管理

**優先度**: 🟡 High  
**実装状況**: ✅ 完了

**要件**:
- すべての情報源にURL引用
- 引用形式の統一
- 引用検証（URL到達性）
- 引用番号自動採番

**データ構造**:
```rust
pub struct Source {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f64,
    pub retrieved_at: DateTime<Utc>,
}
```

**受入基準**:
- ✅ 引用付きレポート生成
- ✅ URL表示
- ✅ 信頼性スコア表示
- [ ] URL到達性検証（未実装）

---

#### FR-DR-006: レポート生成

**優先度**: 🔴 Critical  
**実装状況**: ✅ 完了

**要件**:
- Markdown形式レポート生成
- 構造化された情報整理
- 引用リスト自動生成
- サマリー生成

**レポート構成**:
```markdown
# Research Report: <クエリ>

## 📊 Summary
- Query: <クエリ>
- Strategy: <戦略>
- Depth: <深度>
- Sources: <ソース数>
- Confidence: <信頼度>

## 🔍 Findings
[主要な発見]

## 📚 Sources
[引用リスト]

## ⚠️ Contradictions
[矛盾情報]
```

**受入基準**:
- ✅ Markdownレポート生成
- ✅ 構造化された情報
- ✅ 引用リスト
- ✅ 矛盾情報表示

---

### 3.3 共通機能

#### FR-CM-001: 対話モード統合

**優先度**: 🟡 High  
**実装状況**: 🚧 未実装

**要件**:
```bash
$ codex

# サブエージェント呼び出し
> @code-reviewer ./src

# Deep Research呼び出し
> @researcher "Rust async patterns"
```

**受入基準**:
- [ ] @code-reviewer エイリアス
- [ ] @researcher エイリアス
- [ ] その他エージェントエイリアス

---

#### FR-CM-002: MCPツール統合

**優先度**: 🟡 High  
**実装状況**: 🚧 部分実装

**要件**:
- MCPサーバー経由でツール呼び出し
- ツールリスト取得
- ツール実行
- 結果取得

**サポートツール**:
- AST解析
- Linter実行
- Language Server統合
- Git操作

---

#### FR-CM-003: ログ・監査

**優先度**: 🟡 High  
**実装状況**: 🚧 部分実装

**要件**:
- すべてのエージェント実行をログ記録
- トークン使用量記録
- 権限チェック結果記録
- エラー詳細記録

**ログ形式**:
```json
{
  "timestamp": "2025-10-11T18:00:00Z",
  "agent": "code-reviewer",
  "goal": "Review ./src",
  "tokens_used": 12000,
  "duration_secs": 45.2,
  "status": "success",
  "artifacts": ["artifacts/code-review.md"]
}
```

---

## 4. 非機能要件

### 4.1 パフォーマンス要件

#### NFR-PF-001: 応答時間

| 機能 | 目標 | 許容 |
|------|------|------|
| **delegate起動** | < 1秒 | < 3秒 |
| **research（depth=1）** | < 5秒 | < 10秒 |
| **research（depth=3）** | < 30秒 | < 60秒 |
| **エージェント定義読み込み** | < 100ms | < 500ms |

---

#### NFR-PF-002: スループット

| 機能 | 目標 | 許容 |
|------|------|------|
| **同時エージェント実行** | 5個 | 3個 |
| **Web検索リクエスト/秒** | 10 | 5 |

---

#### NFR-PF-003: リソース使用量

| リソース | 目標 | 上限 |
|---------|------|------|
| **メモリ使用量** | < 500MB | < 1GB |
| **CPU使用率** | < 50% | < 80% |
| **ディスク使用量** | < 100MB | < 500MB |

---

### 4.2 信頼性要件

#### NFR-RL-001: 可用性

- **目標**: 99.9%（月間ダウンタイム < 43分）
- **許容**: 99.0%（月間ダウンタイム < 7時間）

---

#### NFR-RL-002: エラー処理

```
✅ すべてのエラーをキャッチ
✅ ユーザーフレンドリーなエラーメッセージ
✅ エラー詳細ログ記録
✅ 自動リトライ（適切な場合）
✅ グレースフルデグラデーション
```

**例**:
- DuckDuckGo 202エラー → フォールバック実行
- APIキー不足 → 無料検索エンジン使用
- トークン上限到達 → 一時停止・警告

---

#### NFR-RL-003: データ整合性

```
✅ トランザクション管理
✅ ロールバック機能
✅ バックアップ・リカバリ
✅ データ検証
```

---

### 4.3 セキュリティ要件

#### NFR-SC-001: 認証・認可

```
✅ ユーザー認証（OpenAI API経由）
✅ エージェント権限チェック
✅ ファイルアクセス制限
✅ ネットワークアクセス制限
```

---

#### NFR-SC-002: データ保護

```
✅ APIキー暗号化保存
✅ ログ機密情報マスク
✅ 一時ファイル自動削除
✅ HTTPS通信強制
```

---

#### NFR-SC-003: 監査証跡

```
✅ すべてのエージェント実行記録
✅ 権限チェック結果記録
✅ ファイルアクセス記録
✅ ネットワークアクセス記録
```

---

### 4.4 保守性要件

#### NFR-MT-001: コード品質

```
✅ Clippy警告ゼロ
✅ unwrap()禁止（テスト以外）
✅ unsafe禁止（正当理由なし）
✅ コードカバレッジ > 70%
✅ ドキュメント充実
```

---

#### NFR-MT-002: 拡張性

```
✅ 新エージェント追加容易
✅ 新検索エンジン統合容易
✅ プラグイン機構
✅ MCP統合
```

---

### 4.5 運用性要件

#### NFR-OP-001: ログ・監視

```
✅ 構造化ログ（JSON）
✅ ログレベル制御
✅ ログローテーション
✅ メトリクス収集
```

---

#### NFR-OP-002: デプロイ

```
✅ ワンコマンドインストール
✅ バージョン管理
✅ ロールバック機能
✅ CI/CD統合
```

---

## 5. システム構成

### 5.1 ディレクトリ構造

```
zapabob/codex/
├── .codex/
│   ├── agents/                    # エージェント定義
│   │   ├── code-reviewer.yaml
│   │   ├── test-gen.yaml
│   │   └── ... (7エージェント)
│   ├── config.toml                # グローバル設定
│   └── META_PROMPT_CONTINUOUS_IMPROVEMENT.md
│
├── codex-rs/                      # Rust実装
│   ├── core/                      # コアライブラリ
│   │   ├── src/
│   │   │   ├── agent_runtime.rs  # AgentRuntime（未実装）
│   │   │   ├── permissions.rs    # 権限管理（未実装）
│   │   │   └── ...
│   │   └── Cargo.toml
│   │
│   ├── deep-research/             # Deep Research
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── web_search_provider.rs
│   │   │   ├── url_decoder.rs
│   │   │   ├── planner.rs
│   │   │   ├── contradiction.rs
│   │   │   └── ...
│   │   └── Cargo.toml
│   │
│   └── cli/                       # CLI実装
│       ├── src/
│       │   ├── main.rs
│       │   ├── delegate_cmd.rs
│       │   ├── research_cmd.rs
│       │   └── ...
│       └── Cargo.toml
│
├── docs/                          # ドキュメント
│   ├── REQUIREMENTS_SPECIFICATION.md  # この文書
│   ├── codex-subagents-deep-research.md
│   └── ...
│
└── _docs/                         # 実装ログ
    ├── 2025-10-11_全機能完全実装完了報告.md
    └── ...
```

---

### 5.2 技術スタック

#### バックエンド

| 技術 | バージョン | 用途 |
|------|----------|------|
| **Rust** | 1.75+ | コア実装 |
| **Tokio** | 1.35+ | 非同期ランタイム |
| **reqwest** | 0.11+ | HTTP クライアント |
| **serde** | 1.0+ | シリアライゼーション |
| **anyhow** | 1.0+ | エラーハンドリング |
| **scraper** | 0.18+ | HTML パース |

#### フロントエンド

| 技術 | バージョン | 用途 |
|------|----------|------|
| **Ratatui** | 0.25+ | TUI |
| **Crossterm** | 0.27+ | ターミナル制御 |

#### 外部API

| サービス | 用途 | APIキー |
|---------|------|--------|
| **DuckDuckGo** | Web検索 | 不要 |
| **Brave Search** | Web検索 | 必要（オプション） |
| **Google Custom Search** | Web検索 | 必要（オプション） |
| **OpenAI API** | LLM実行 | 必要 |

---

## 6. API仕様

### 6.1 CLI API

#### 6.1.1 codex delegate

```bash
codex delegate <agent> [OPTIONS]

Arguments:
  <agent>    エージェント名（例: code-reviewer）

Options:
  --scope <PATH>       対象スコープ
  --goal <STRING>      目標（オプション）
  --budget <NUMBER>    トークン上限（デフォルト: 40000）
  --deadline <NUMBER>  実行時間制限（秒）
  --out <PATH>         結果出力先

Exit Codes:
  0: 成功
  1: エラー
  2: タイムアウト
  3: トークン上限到達
```

**例**:
```bash
# 基本的な使い方
codex delegate code-reviewer --scope ./src

# トークン上限指定
codex delegate test-gen --scope ./src/api --budget 20000

# 結果をファイル出力
codex delegate sec-audit --scope ./backend --out audit-report.json
```

---

#### 6.1.2 codex research

```bash
codex research "<query>" [OPTIONS]

Arguments:
  <query>    検索クエリ

Options:
  --depth <1-5>          探索深度（デフォルト: 1）
  --breadth <1-10>       幅（デフォルト: 3）
  --strategy <TYPE>      戦略（comprehensive|quick|deep）
  --budget <NUMBER>      トークン上限（デフォルト: 60000）
  --out <PATH>           レポート出力先

Exit Codes:
  0: 成功
  1: エラー
  2: タイムアウト
  3: トークン上限到達
```

**例**:
```bash
# 基本的な使い方
codex research "Rust async programming"

# 深掘り調査
codex research "React Server Components" --depth 3 --breadth 5

# レポート出力
codex research "Python FastAPI" --out research-report.md
```

---

### 6.2 Rust API

#### 6.2.1 AgentRuntime

```rust
use codex_core::agent_runtime::AgentRuntime;

// AgentRuntime初期化
let runtime = AgentRuntime::new(
    agent_definition,  // AgentDefinition
    budget,            // TokenBudget
    permissions,       // PermissionSet
)?;

// タスク実行
let result = runtime.execute_task(
    "Review code for best practices",  // goal
    &inputs,                            // HashMap<String, String>
).await?;

// 結果取得
println!("Status: {:?}", result.status);
println!("Tokens used: {}", result.tokens_used);
println!("Duration: {:.2}s", result.duration_secs);
```

---

#### 6.2.2 ResearchProvider

```rust
use codex_deep_research::WebSearchProvider;

// Provider初期化
let provider = WebSearchProvider::new();

// 検索実行
let sources = provider.search("Rust async", 10).await?;

// ソース処理
for source in sources {
    println!("Title: {}", source.title);
    println!("URL: {}", source.url);
    println!("Relevance: {:.2}", source.relevance_score);
}
```

---

## 7. データモデル

### 7.1 エージェント定義

```yaml
# .codex/agents/code-reviewer.yaml
name: "Code Reviewer"
goal: "多言語対応コードレビュー・品質チェック"
max_tokens: 40000
timeout_seconds: 300

permissions:
  file_read:
    - "./src"
    - "./tests"
  file_write: []
  shell: false
  network: false

tools:
  - "ast_analyzer"
  - "linter"
  - "language_server"

constraints:
  - "unsafe コード禁止"
  - "unwrap() 禁止（テスト以外）"
  - "型安全性確保"

languages:
  - "typescript"
  - "python"
  - "rust"
  - "csharp"
```

---

### 7.2 AgentExecutionResult

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub status: AgentExecutionStatus,
    pub tokens_used: usize,
    pub duration_secs: f64,
    pub artifacts: Vec<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AgentExecutionStatus {
    Success,
    Failed,
    PartialSuccess,
    Timeout,
    BudgetExceeded,
}
```

---

### 7.3 Source（検索結果）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub relevance_score: f64,
    pub retrieved_at: DateTime<Utc>,
    pub search_engine: SearchEngine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchEngine {
    DuckDuckGo,
    Brave,
    Google,
    Bing,
}
```

---

### 7.4 ResearchReport

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct ResearchReport {
    pub query: String,
    pub strategy: ResearchStrategy,
    pub depth_reached: usize,
    pub sources: Vec<Source>,
    pub findings: Vec<Finding>,
    pub contradictions: Vec<Contradiction>,
    pub confidence: ConfidenceLevel,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    pub topic: String,
    pub content: String,
    pub sources: Vec<usize>,  // Source indices
    pub confidence: f64,
}
```

---

## 8. ユースケース

### 8.1 UC-001: コードレビュー実行

**アクター**: 開発者  
**前提条件**: codex インストール済み  
**トリガー**: `codex delegate code-reviewer --scope ./src`

**正常フロー**:
1. ユーザーがコマンド実行
2. システムがcode-reviewer定義を読み込み
3. システムが対象ファイルを特定（./src配下）
4. AgentRuntimeが権限チェック実施
5. コードレビュー実行
6. レビュー結果を表示
7. artifacts/code-review.md を生成

**代替フロー**:
- 3a. ファイル読み取り権限なし → エラー表示
- 5a. トークン上限到達 → 中断・警告表示

**事後条件**: レビューレポート生成完了

---

### 8.2 UC-002: Deep Research実行

**アクター**: 開発者  
**前提条件**: codex インストール済み  
**トリガー**: `codex research "Rust async patterns"`

**正常フロー**:
1. ユーザーがコマンド実行
2. システムがサブクエリ生成
3. 各サブクエリでWeb検索実行（DuckDuckGo）
4. 検索結果を集約
5. 矛盾検出実行
6. レポート生成
7. レポート表示・保存

**代替フロー**:
- 3a. DuckDuckGo 202エラー → フォールバック実行
- 4a. 検索結果ゼロ → 警告表示・フォールバック
- 5a. 矛盾検出 → 警告表示・詳細レポート

**事後条件**: 引用付きレポート生成完了

---

### 8.3 UC-003: テスト生成

**アクター**: 開発者  
**前提条件**: codex インストール済み  
**トリガー**: `codex delegate test-gen --scope ./src/api`

**正常フロー**:
1. ユーザーがコマンド実行
2. システムがtest-gen定義を読み込み
3. 対象ファイル分析
4. テストケース生成
5. テストファイル作成
6. カバレッジレポート生成

**代替フロー**:
- 5a. ファイル書き込み権限なし → エラー表示

**事後条件**: テストファイル生成完了

---

### 8.4 UC-004: セキュリティ監査

**アクター**: セキュリティ担当者  
**前提条件**: codex インストール済み  
**トリガー**: `codex delegate sec-audit --scope ./backend`

**正常フロー**:
1. ユーザーがコマンド実行
2. システムがsec-audit定義を読み込み
3. セキュリティスキャン実行
   - SQLインジェクション検出
   - XSS検出
   - CSRF検出
   - シークレット検出
4. 脆弱性レポート生成
5. 修正提案生成

**代替フロー**:
- 3a. 深刻な脆弱性検出 → 即座に警告表示

**事後条件**: セキュリティレポート生成完了

---

## 9. テスト要件

### 9.1 単体テスト

**目標カバレッジ**: 80%以上

**必須テスト項目**:
```
✅ AgentRuntime::execute_task()
✅ PermissionSet::check_file_read()
✅ TokenBudget::consume()
✅ WebSearchProvider::search()
✅ ContradictionChecker::detect_contradictions()
✅ URLデコーダー
```

---

### 9.2 統合テスト

**必須テスト項目**:
```
✅ delegate コマンド実行
✅ research コマンド実行
✅ エージェント定義読み込み
✅ Web検索実行
✅ レポート生成
```

---

### 9.3 E2Eテスト

**必須テスト項目**:
```
✅ UC-001: コードレビュー実行
✅ UC-002: Deep Research実行
✅ UC-003: テスト生成
✅ UC-004: セキュリティ監査
```

---

### 9.4 パフォーマンステスト

**必須テスト項目**:
```
✅ delegate起動時間 < 1秒
✅ research（depth=1） < 5秒
✅ メモリ使用量 < 500MB
✅ 同時エージェント実行5個
```

---

## 10. セキュリティ要件

### 10.1 OWASP Top 10対策

| 脅威 | 対策 | 実装状況 |
|------|------|---------|
| **A01: アクセス制御の不備** | 権限管理システム | 🚧 設計済み |
| **A02: 暗号化の失敗** | APIキー暗号化保存 | ✅ 完了 |
| **A03: インジェクション** | パラメータ化クエリ | ✅ 完了 |
| **A04: 安全でない設計** | セキュアコーディング規約 | ✅ 完了 |
| **A05: セキュリティ設定ミス** | デフォルト安全設定 | ✅ 完了 |
| **A06: 脆弱コンポーネント** | 依存関係スキャン | 🚧 未実装 |
| **A07: 認証の失敗** | OpenAI API認証 | ✅ 完了 |
| **A08: データ整合性の失敗** | 署名検証 | 🚧 未実装 |
| **A09: ログ監視の失敗** | 構造化ログ | ✅ 完了 |
| **A10: SSRF** | URL検証 | 🚧 部分実装 |

---

### 10.2 脆弱性スキャン

**定期実行**:
```bash
# 依存関係スキャン
cargo audit

# Clippy（セキュリティルール）
cargo clippy -- -W clippy::suspicious

# Secret検出
git-secrets --scan
```

---

## 11. パフォーマンス要件

### 11.1 ベンチマーク目標

| 機能 | 目標 | 計測方法 |
|------|------|---------|
| **DuckDuckGo検索** | < 2秒 | `cargo bench` |
| **URL デコード** | < 1ms | `cargo bench` |
| **矛盾検出** | < 500ms | `cargo bench` |
| **レポート生成** | < 1秒 | `cargo bench` |

---

### 11.2 最適化指針

```
✅ 非同期処理（Tokio）
✅ 並列検索（複数検索エンジン）
✅ キャッシング（検索結果）
✅ 遅延評価
✅ メモリプール
```

---

## 12. 運用要件

### 12.1 インストール

```bash
# npm経由
npm install -g @openai/codex

# Homebrew経由
brew install codex

# ソースからビルド
git clone https://github.com/zapabob/codex.git
cd codex/codex-rs
cargo build --release
```

---

### 12.2 設定

```toml
# ~/.codex/config.toml

[agents]
default_budget = 40000
timeout_seconds = 300

[research]
default_depth = 1
default_breadth = 3
search_engines = ["duckduckgo", "brave"]

[api_keys]
brave_api_key = "YOUR_API_KEY"
google_api_key = "YOUR_API_KEY"
```

---

### 12.3 監視

**メトリクス**:
```
- エージェント実行回数
- トークン使用量
- エラー発生率
- 平均実行時間
- メモリ使用量
- CPU使用率
```

---

### 12.4 バックアップ

**対象**:
```
- エージェント定義（.codex/agents/）
- 設定ファイル（~/.codex/config.toml）
- 監査ログ（~/.codex/logs/）
```

---

## 13. 制約事項

### 13.1 技術的制約

```
⚠️ Rust 1.75以上必須
⚠️ Tokioランタイム必須
⚠️ OpenAI API必須（LLM実行）
⚠️ インターネット接続必須（Web検索）
```

---

### 13.2 プラットフォーム制約

```
✅ macOS (ARM/x86_64)
✅ Linux (x86_64/ARM64)
✅ Windows (x86_64)
❌ iOS/Android（Phase 2以降）
```

---

### 13.3 ライセンス制約

```
✅ Apache-2.0 License
✅ 商用利用可能
✅ 改変可能
✅ 再配布可能
```

---

### 13.4 コスト制約

```
💰 DuckDuckGo: $0（無料）
💰 Brave Search: $0〜（月2000クエリ無料）
💰 Google Custom Search: $5/1000クエリ
💰 OpenAI API: 従量課金
```

---

## 14. 用語集

| 用語 | 定義 |
|------|------|
| **サブエージェント** | 専門タスクを自律的に実行するエージェント |
| **Deep Research** | 多段階探索・矛盾検出を備えた高度な情報収集機能 |
| **Agent Runtime** | サブエージェントの実行環境 |
| **Token Budget** | LLM使用トークン数の上限 |
| **Permission Set** | エージェントの権限セット |
| **Research Provider** | Web検索機能の提供インターフェース |
| **Contradiction Checker** | 矛盾検出器 |
| **Source** | 検索結果の情報源 |
| **Artifact** | エージェント実行結果の成果物 |
| **MCP** | Model Context Protocol（ツール統合規格） |

---

## 15. 付録

### 15.1 参考文献

1. OpenAI/codex Documentation: https://github.com/openai/codex
2. Rust Book: https://doc.rust-lang.org/book/
3. Tokio Documentation: https://tokio.rs/
4. MCP Specification: https://modelcontextprotocol.io/

---

### 15.2 関連ドキュメント

- `.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md` - 開発ガイドライン
- `docs/codex-subagents-deep-research.md` - 詳細設計
- `_docs/2025-10-11_全機能完全実装完了報告.md` - 実装完了報告
- `README.md` - プロジェクト概要

---

### 15.3 承認

| 役割 | 氏名 | 承認日 | 署名 |
|------|------|--------|------|
| **プロダクトオーナー** | zapabob | 2025-10-11 | _______ |
| **開発リード** | zapabob | 2025-10-11 | _______ |
| **QAリード** | Community | - | _______ |

---

**文書番号**: REQ-SPEC-001  
**バージョン**: 1.0.0  
**作成日**: 2025-10-11 JST  
**最終更新**: 2025-10-11 JST  
**Status**: ✅ **承認済み**

---

**END OF REQUIREMENTS SPECIFICATION**

