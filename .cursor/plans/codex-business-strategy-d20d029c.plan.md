<!-- d20d029c-4632-426f-88e7-462d92371978 3a073ee0-428b-4d61-aeb0-c80681de96ee -->
# Codex v1.1.0 完全実装プラン

## Phase 1: Blueprint実行フロー (高優先度)

### 1.1 Rust CLI実行コマンド実装

- `codex-rs/cli/src/blueprint_commands.rs`に`Execute`サブコマンド追加
- `codex-rs/core/src/blueprint/executor.rs`（新規）を作成
  - BlueprintOrchestratorとの統合
  - 実行状態管理（Executing, Completed, Failed）
  - ロールバック機能
- 既存の`blueprint_orchestrator.rs`を活用して実行ロジック統合

### 1.2 進捗リアルタイム表示（WebSocket）

- `codex-rs/app-server/src/blueprint_ws.rs`（新規）作成
  - WebSocket endpoint: `/api/blueprint/ws/{blueprint_id}`
  - 進捗イベント: `ExecutionStarted`, `StepCompleted`, `FileChanged`, `TestPassed`, `Completed`
- `prism-web/lib/hooks/useBlueprintExecution.ts`（新規）作成
  - WebSocket接続管理
  - 進捗状態の購読

### 1.3 prism-web実行UI

- `prism-web/app/(dashboard)/blueprints/[id]/execute/page.tsx`（新規）
  - 実行ボタンとキャンセルボタン
  - リアルタイム進捗バー
  - ファイル変更リスト
  - ログストリーム表示
  - 完了/失敗ステータス

### 1.4 実行履歴とロールバック

- `codex-rs/core/src/blueprint/execution_log.rs`（新規）
- 実行ログのJSON永続化（`~/.codex/blueprints/executions/`）
- ロールバックコマンド: `codex blueprint rollback <execution-id>`

---

## Phase 2: prism-web完全統合

### 2.1 Supabase認証統合

- `prism-web/app/(auth)/login/page.tsx`を更新
  - Email/Password認証
  - GitHub OAuth統合
  - Google OAuth統合
  - Magic Link（パスワードレス）
- `prism-web/middleware.ts`（新規）作成
  - 認証ミドルウェア
  - セッション管理
- `prism-web/lib/auth/context.tsx`（新規）作成
  - AuthProvider実装
  - useAuthフック

### 2.2 APIキー暗号化と保存

- Supabase Edge Function強化
  - `supabase/functions/save-api-key/index.ts`を修正（Deno型エラー解消）
  - `supabase/functions/get-api-key/index.ts`（新規）
- AES-256-GCM暗号化
- ユーザーごとのキー分離

### 2.3 使用量トラッキングとビリング

- `prism-web/lib/hooks/useUsageTracking.ts`（新規）
- Supabase `usage_logs`テーブルへの記録
- `prism-web/app/(dashboard)/usage/page.tsx`（新規）
  - 使用量ダッシュボード
  - コスト計算と表示
  - 月次レポート

### 2.4 Next.jsビルド最適化

- `prism-web/next.config.js`更新
  - Static Export設定
  - Image Optimization
  - Webpack Bundle Analyzer統合
  - Code Splitting最適化
- `prism-web/.env.production`作成
- ビルドサイズ目標: < 500KB initial bundle

### 2.5 Vercelデプロイ設定

- `prism-web/vercel.json`更新
  - Environment Variables設定
  - Build Command最適化
  - Rewrites/Redirects設定
- GitHub Actionsワークフロー: `.github/workflows/deploy-web.yml`（新規）
  - 自動デプロイ（main branchマージ時）
  - Preview Deployments（PR時）

---

## Phase 3: Git可視化強化

### 3.1 インスタンシング実装（10K+コミット対応）

- `prism-web/components/visualizations/Scene3D.tsx`更新
  - `THREE.InstancedMesh`使用
  - コミットごとの個別Meshから統合Meshへ変更
  - Matrix変換でposition/rotation/scale制御
- メモリ使用量: 100Kコミット時 < 500MB目標

### 3.2 LOD (Level of Detail)システム

- `prism-web/lib/visualization/lod.ts`（新規）作成
  - カメラ距離ベースのLOD切り替え
  - 3段階: High (< 50 units), Medium (50-200 units), Low (> 200 units)
  - High: 詳細Mesh, Medium: 簡略Mesh, Low: Billboard Sprite
- fps維持目標: 60fps @ 100Kコミット

### 3.3 アニメーション再生機能

- `prism-web/components/visualizations/Timeline.tsx`更新
  - 再生速度コントロール（0.5x, 1x, 2x, 4x, 8x）
  - シーク機能強化（ドラッグ可能なスライダー）
  - ループ再生オプション
- `prism-web/lib/visualization/animator.ts`（新規）
  - 時間ベースのアニメーション補間
  - カメラパス記録・再生

### 3.4 パフォーマンス最適化

- Web Worker分離: `prism-web/workers/git-parser.worker.ts`（新規）
  - Git JSON解析をメインスレッドから分離
  - 座標計算をWorkerで実行
- `prism-web/lib/visualization/octree.ts`（新規）
  - 空間分割によるカリング
  - Frustum Culling実装

---

## Phase 4: テスト追加

### 4.1 Rustユニットテスト

- `codex-rs/core/src/blueprint/executor_test.rs`（新規）
  - Blueprint実行フローテスト
  - ロールバックテスト
  - エラーハンドリングテスト
- `codex-rs/cli/src/blueprint_commands_test.rs`（新規）
  - CLIコマンドテスト
  - 状態遷移テスト
- `codex-rs/cli/src/git_commands_test.rs`（新規）
  - Git解析ロジックテスト
  - 3D座標計算テスト
- カバレッジ目標: 80%以上

### 4.2 E2Eテスト（Playwright）

- `prism-web/e2e/blueprint-flow.spec.ts`（新規）
  - Blueprint作成→承認→実行フロー
  - WebSocket進捗表示テスト
- `prism-web/e2e/auth-flow.spec.ts`（新規）
  - ログイン/ログアウト
  - OAuth統合
- `prism-web/e2e/visualization.spec.ts`（新規）
  - 3D可視化インタラクション
  - タイムライン操作
- `prism-web/playwright.config.ts`（新規）設定

### 4.3 パフォーマンステスト

- `codex-rs/benches/git_analysis.rs`（新規）
  - 大規模リポジトリ解析ベンチマーク
  - 10K, 50K, 100Kコミットでの性能測定
- `prism-web/tests/performance/rendering.test.ts`（新規）
  - 3D レンダリングfps測定
  - メモリ使用量監視
- CI統合: パフォーマンス退化検出

---

## 実装依存関係

### 新規依存関係（Rust）

```toml
# codex-rs/Cargo.toml
tokio-tungstenite = "0.21"  # WebSocket
futures-util = "0.3"
```

### 新規依存関係（prism-web）

```json
{
  "dependencies": {
    "@playwright/test": "^1.40.0",
    "vitest": "^1.0.4",
    "@vitest/ui": "^1.0.4",
    "three-stdlib": "^2.28.0",
    "stats.js": "^0.17.0"
  }
}
```

---

## マイルストーン

- **Week 1-2**: Phase 1（Blueprint実行フロー）
- **Week 3-4**: Phase 2（prism-web完全統合）
- **Week 5-6**: Phase 3（Git可視化強化）
- **Week 7-8**: Phase 4（テスト追加）

---

## 成果物

- Codex v1.1.0リリース
- prism-web本番環境（Vercel）
- 完全なテストスイート
- パフォーマンスベンチマーク結果
- デプロイメントドキュメント

### To-dos

- [ ] Rust CLIの名前をcodex→prismに変更（Cargo.toml、バイナリ名）
- [ ] 全コンポーネントのバージョンを1.0.0に統一
- [ ] Blueprint実行エンジン実装（executor.rs）
- [ ] WebSocket進捗配信サーバー実装
- [ ] 実行UIページ作成（リアルタイム進捗表示）
- [ ] ロールバック機能実装
- [ ] Supabase認証統合（Email/OAuth/MagicLink）
- [ ] APIキー暗号化・保存機能
- [ ] 使用量トラッキングとビリングUI
- [ ] Next.jsビルド最適化
- [ ] Vercelデプロイ設定とCI/CD
- [ ] THREE.InstancedMesh実装（10K+コミット対応）
- [ ] LODシステム実装（3段階）
- [ ] アニメーション再生機能
- [ ] Web Worker分離（パフォーマンス最適化）
- [ ] Rustユニットテスト（80%カバレッジ目標）
- [ ] Playwright E2Eテスト
- [ ] パフォーマンステストとベンチマーク