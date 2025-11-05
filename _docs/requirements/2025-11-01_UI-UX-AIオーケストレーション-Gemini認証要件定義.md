# 要件定義書: UI/UX強化・AIオーケストレーション・Gemini認証（PR #34 後続統合）

- 対象リポジトリ: zapabob/codex
- 参照PR: https://github.com/zapabob/codex/pull/34
- バージョン: v1.0
- 作成日: 2025-10-31
- 変更管理: 本書は PR #35, #36, #37 の要件を包含し、後方互換を前提に統合します

## 1. 概要

本要件定義書は、PR #34 で示された計画事項を具体化し、以下の3領域を完成させるための要件を定義します。

- UI/UX強化（ショートカット、視認性、アクセシビリティ）
- AIオーケストレーション（単一ライタ・競合制御・ステータス監視）
- Gemini認証（APIキー + OAuth 2.0/PKCE、geminicli 優先）

既存ユーザーの体験を壊さないこと（後方互換）を最優先とし、新機能は設定で有効化できる設計とします。

## 2. 目的・ゴール

- 同一リポジトリに対する複数CLI/GUI/サブエージェントの同時操作で状態破壊が起きないこと
- 主要操作のキーボードショートカットと視覚的ヒントを提供し、UXを改善すること
- Gemini接続をAPIキー/Googleアカウントの両モードで安全に提供し、既存APIキー運用を維持すること

## 3. スコープ

### インスコープ
- リポジトリレベルのロック、トークン予算管理、ステータスAPI/ダッシュボード
- ローカル限定のRPCプロトコル（UDS/Named Pipe/TCP）と単一ライタキュー
- TypeScriptクライアントSDKおよびGUI購読（イベント + ポーリングフォールバック）
- Gemini認証（APIキー + OAuth 2.0/PKCE）、geminicli 優先
- ドキュメント（EN/JA）、.env.sample 更新、テスト（Unit/Integration/E2E）

### アウトオブスコープ
- リモートネットワーク公開（ローカル限定）
- .codex/* を超える長期永続化

## 4. 用語

- **単一ライタ（single-writer）**: fs/vcsの変更系操作を直列化する実行キュー
- **preimage/base mismatch**: 競合検出のための事前ハッシュ/ベースコミット不一致
- **backpressure**: キュー満杯時に 429 を返しリトライを促す制御

## 5. 詳細要件

### 5.1 Rust Core / CLI

#### 1) ロック機構
- `.codex/lock.json` の原子的生成（O_EXCL）
- 保持情報: `{version, pid, ppid, uid, hostname, repo_path, started_at, expires_at?}`
- ステール判定: PID/PPID 生存、TTL
- CLI:
  - `codex lock status`
  - `codex lock remove [--force]`

#### 2) トークン予算・利用追跡
- 全体予算、警告閾値、エージェント毎の上限
- 閾値到達の警告発火
- ステータスAPIとGUIに集計を露出

#### 3) オーケストレータサーバ
- **トランスポート優先**: UDS（0700）/ Named Pipe / TCP(127.0.0.1, エフェメラル、.codex/orchestrator.port)
- **セキュリティ**: ローカル限定、HMAC-SHA256（.codex/secret）、±5分時刻スキュー
- **単一ライタキュー**: Tokio mpsc、容量設定、429 リトライガイダンス
- **Idempotency**: 10分TTLの応答キャッシュ（idem_key）
- **RPC v1.0**:
  - lock: status, acquire, release
  - status: get
  - fs: read(path), write(path, content, preimage_sha), patch(unified_diff, base_commit)
  - vcs: diff, commit(message), push(remote, branch)
  - agent: register, heartbeat, list
  - task: submit, cancel
  - tokens: reportUsage, getBudget
  - session: start, end
  - pubsub: subscribe, unsubscribe
- **エラーセマンティクス**: 409（競合）, 429（バックプレッシャ）
- **CLI動作**: サーバ不在時に自動起動。書き込みは既定でオーケストレータ経由（必要時のみエスケープ可能）

### 5.2 TypeScript SDK / GUI

#### TypeScript SDK
- **新規パッケージ**: `packages/codex-protocol-client`
  - トランスポート自動検出
  - ジッタ付き再接続、タイムアウト
  - イベント購読
  - 型付きラッパー
  - Reactフック（`useProtocol`, `useProtocolEvent`）

#### GUI（Next.js + MUI）
- **ショートカット**:
  - `Cmd/Ctrl+Enter` → Run
  - `Cmd/Ctrl+S` → Commit
  - `Cmd/Ctrl+Shift+S` → Push
  - `Cmd/Ctrl+D` → Diff
  - `Cmd/Ctrl+Z` → Revert
  - `?` → Help
- **実装詳細**:
  - 入力中は除外
  - `aria-keyshortcuts` とツールチップでヒント表示
- **OrchestratorStatusDashboard**:
  - `lock.changed` / `tokens.updated` を購読
  - 5秒ポーリングをフォールバックで維持

### 5.3 Gemini 認証

#### プロバイダ設計
- **GeminiAuthProvider**（`CredentialSource = ApiKey | OAuth`）
- **解決優先**: 環境変数（`GEMINI_API_KEY` | `GOOGLE_AI_STUDIO_API_KEY`）> 設定 > セキュアストレージ

#### APIキーモード
- `x-goog-api-key` を自動付与

#### OAuth 2.0モード（PKCE, ループバック127.0.0.1:0）
- **主にVertex想定**（最低スコープ: `https://www.googleapis.com/auth/cloud-platform`）
- **CLI**:
  - `codex auth gemini login`
  - `codex auth gemini status`
  - `codex auth gemini logout`
  - ブラウザ起動、コード交換、キーリング保存（不可時 `.codex/credentials.json` 0600 + 警告）
  - ステータス出力はマスク
- **GUI**:
  - Sign in / Sign out / Status
  - RPC: `auth.status`, `auth.login.start`, `auth.logout`
  - イベント: `auth.changed`

#### geminicli 優先
- `prefer_cli = true` を既定
- 失敗/未検出時は内蔵PKCEへフォールバック

#### 後方互換
- 既存APIキー運用は既定を維持

### 5.4 設定・デフォルト

#### .codex/config.toml
```toml
[auth.gemini]
mode = "api-key"      # "api-key" | "oauth"
provider = "ai_studio" # "ai_studio" | "vertex"
prefer_cli = true
project = ""           # provider=vertex の場合に使用
region  = ""

[orchestrator]
queue_capacity = 1024
transport_preference = "auto" # "uds" | "pipe" | "tcp" | "auto"
tcp_port = 0 # 0 でエフェメラル

[tokens]
total_budget = 100000
warning_threshold = 80000
per_agent_limit = 20000
```

#### 環境変数（上書き可）
- `GEMINI_API_KEY`
- `GOOGLE_AI_STUDIO_API_KEY`
- `GOOGLE_OAUTH_CLIENT_ID`
- `GCP_PROJECT_ID`
- `VERTEX_REGION`

### 5.5 ドキュメント（EN/JA）

- **README**: アーキ図、クイックスタート、ショートカット一覧、オーケストレータ概要
- **docs/protocol.md**: メッセージ封筒、トランスポート、RPC/イベント、エラー、セキュリティ
- **docs/orchestration.md**: 単一ライタ、役割とワークフロー、監視
- **docs/auth-gemini.md**: APIキー/OAuth手順（Vertex中心）、スコープ、セキュリティ
- **docs/troubleshooting-locks.md**, **docs/tokens.md**, **docs/security.md**
- **.env.sample** 更新

### 5.6 テスト・QA

#### Unit Tests
- ロックライフサイクル
- トークン算術
- HMAC
- idempotency
- PKCE
- トークン更新スキュー

#### Integration Tests
- マルチインスタンスロック
- fs.patch 競合（1つ成功・他は409）
- キュー満杯（429 + retry_after）
- auth login/status/logout
- geminicli 検出/フォールバック

#### E2E Tests
- GUIショートカット→オーケストレータ経由の動作
- イベント駆動更新（フォールバック動作含む）

#### 互換試験
- `GEMINI_API_KEY` 設定時は現行動作を維持

### 5.7 受け入れ基準

- ✅ 書き込み操作がオーケストレータ経由で直列化され競合が発生しない
- ✅ 409/429 セマンティクスが明確で、重複リトライはidempotencyで吸収される
- ✅ GUIショートカットが動作し、ヒント/アクセシビリティが満たされる
- ✅ ステータスダッシュボードがロック/トークンをリアルタイム反映（イベント + ポーリング）
- ✅ Gemini認証がAPIキー/OAuthの両モードで機能し、秘密情報が安全に保存/表示される
- ✅ 既存ユーザーの挙動を破壊しない（デフォルトはAPIキー運用）
- ✅ ドキュメント/サンプル/テストが整備される

## 6. セキュリティ要件

- ✅ ローカルバインドのみ（UDS/Named Pipe/127.0.0.1）
- ✅ HMAC秘密鍵（`.codex/secret`）生成・更新、時刻スキュー±5分
- ✅ OAuthトークンはキーリング保存を優先、フォールバック時は 0600 で保存・明示警告
- ✅ ログに秘密情報を出力しない（マスク）
- ✅ GUI/CLIの表示には最小限の秘匿情報のみ

## 7. 互換性・移行

- ✅ 既存の `GEMINI_API_KEY` / `GOOGLE_AI_STUDIO_API_KEY` があれば、そのまま利用（既定）
- ✅ 新機能は設定で有効化。既定は互換モード
- ✅ geminicli が存在すればログインフローを優先

## 8. 運用・マージ方針

- **マージ方式**: Squash and merge
- **推奨順序**: #35 → #36 → #37（または統合PRで一括）
- **管理者バイパス**: ありの手動マージを許容（チェック未完了時は手動）

## 9. マイルストーン

- **M1**: Lock/Token/Status + GUIショートカット（テスト・Docs）
- **M2**: Orchestrator RPC + TS SDK + GUI購読
- **M3**: Gemini認証（APIキー/OAuth, geminicli 優先）+ Docs
- **M4**: 統合E2E・最終ドキュメント・リリースノート

## 付録

### A. ショートカット一覧
- `Cmd/Ctrl+Enter`: Run
- `Cmd/Ctrl+S`: Commit
- `Cmd/Ctrl+Shift+S`: Push
- `Cmd/Ctrl+D`: Diff
- `Cmd/Ctrl+Z`: Revert
- `?`: Help
- **備考**: 入力欄フォーカス中は抑止、aria-keyshortcuts 付与、ツールチップ表示

### B. 主要環境変数
- `GEMINI_API_KEY`, `GOOGLE_AI_STUDIO_API_KEY`
- `GOOGLE_OAUTH_CLIENT_ID`
- `GCP_PROJECT_ID`, `VERTEX_REGION`

### C. RPCエラーコード
- **409**: preimage/base mismatch
- **429**: queue full（retry_after 秒）

---

## 実装チェックリスト

### Phase 1: Lock & Token Infrastructure (M1)
- [ ] `.codex/lock.json` 原子的生成（O_EXCL）
- [ ] ステール判定ロジック（PID/PPID生存、TTL）
- [ ] `codex lock status/remove` CLI実装
- [ ] トークン予算管理（全体/警告/エージェント毎）
- [ ] Unit tests（ロックライフサイクル、トークン算術）

### Phase 2: Orchestrator Server (M2)
- [ ] UDS/Named Pipe/TCP トランスポート実装
- [ ] HMAC-SHA256認証（.codex/secret）
- [ ] 単一ライタキュー（Tokio mpsc）
- [ ] Idempotency（10分TTL応答キャッシュ）
- [ ] RPC v1.0 全API実装
- [ ] 409/429 エラーセマンティクス
- [ ] TypeScript SDK（codex-protocol-client）
- [ ] Integration tests（競合、キュー満杯）

### Phase 3: GUI Enhancements (M1/M2)
- [ ] キーボードショートカット（6種類）
- [ ] aria-keyshortcuts属性
- [ ] ツールチップ表示
- [ ] OrchestratorStatusDashboard
- [ ] イベント購読 + 5秒ポーリングフォールバック
- [ ] E2E tests

### Phase 4: Gemini Authentication (M3)
- [ ] GeminiAuthProvider実装
- [ ] APIキーモード（x-goog-api-key）
- [ ] OAuth 2.0/PKCEモード
- [ ] geminicli検出・優先ロジック
- [ ] キーリング保存（フォールバック: .codex/credentials.json 0600）
- [ ] CLI: `codex auth gemini login/status/logout`
- [ ] GUI: Sign in/out/Status
- [ ] ステータスマスク処理
- [ ] Integration tests

### Phase 5: Documentation & Release (M4)
- [ ] README更新（アーキ図、ショートカット一覧）
- [ ] docs/protocol.md
- [ ] docs/orchestration.md
- [ ] docs/auth-gemini.md
- [ ] docs/troubleshooting-locks.md
- [ ] docs/tokens.md
- [ ] docs/security.md
- [ ] .env.sample更新
- [ ] 統合E2Eテスト
- [ ] リリースノート作成

---

**作成者**: zapabob  
**ステータス**: 🟡 実装待ち  
**関連PR**: #34, #35, #36, #37（予定）

