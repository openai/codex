<!-- 9334bde7-e2a1-4129-bdd0-3bb2d644ccb7 1198d1d5-67ea-421a-a386-f41fbca52eba -->
# Windows 11 25H2 MCP統合とGPU最適化実装計画

## 目標

- Windows 11 25H2のMCP統合
- カーネルドライバーからCodexへのGPU最適化接続
- Rust 2025年最新ベストプラクティス適用
- RustCuda（cust）の最新実装パターン適用
- 型定義の改善と警告0達成

## Phase 1: コンパイルエラー修正（最優先）

### 1.1 TUIのrender_refエラー修正

- [ ] `codex-rs/tui/src/bottom_pane/mod.rs`の`render_ref`呼び出しを修正
- [ ] `StatusIndicatorWidget`と`ChatComposer`の`WidgetRef`実装確認
- [ ] `GpuStatsWidget`の`WidgetRef`実装確認

### 1.2 コンパイル確認

- [ ] `cargo build --release -p codex-cli`でエラー0確認
- [ ] `cargo clippy`で警告確認

## Phase 2: RustCuda（cust）ベストプラクティス適用

### 2.1 CUDA Runtime実装の改善

- [ ] `codex-rs/cuda-runtime/src/cuda_impl.rs`の型定義を改善
- [ ] `DeviceCopy`トレイトの適切な使用
- [ ] エラーハンドリングの改善（`anyhow::Result` + `map_err`）
- [ ] Rust 2024 Editionの`unsafe_op_in_unsafe_fn` lint対応

### 2.2 メモリ管理の最適化

- [ ] `DeviceBuffer`のライフタイム管理
- [ ] Pinned Memoryの実装（カーネルドライバー連携）
- [ ] ゼロコピー転送の実装

## Phase 3: Windows 11 25H2 MCP統合

### 3.1 Windows AI MCP API調査

- [ ] `windows.ai.agents.mcp.h`の存在確認
- [ ] Windows 11 25H2 SDKのMCP関連API調査
- [ ] MCP ProtocolのWindows実装確認

### 3.2 MCP統合レイヤー実装

- [ ] `codex-rs/windows-ai/src/mcp.rs`新規作成
- [ ] Windows AI MCP Runtimeラッパー実装
- [ ] Codex MCP Serverとの統合

### 3.3 MCP通信プロトコル実装

- [ ] JSON-RPC 2.0 over Windows AI API
- [ ] 非同期通信（`tokio`）
- [ ] エラーハンドリングとリトライロジック

## Phase 4: カーネルドライバー→Codex GPU最適化接続

### 4.1 カーネルドライバー拡張

- [ ] `codex-rs/windows-ai/src/kernel_driver.rs`の拡張
- [ ] GPU統計のリアルタイム取得
- [ ] Pinned Memory Poolの管理
- [ ] GPU-awareスケジューリング

### 4.2 CUDA Runtime統合

- [ ] `codex-rs/cuda-runtime`とカーネルドライバーの接続
- [ ] GPUメモリの最適化割り当て
- [ ] カーネルドライバー経由のGPU操作

### 4.3 パフォーマンス最適化

- [ ] レイテンシ削減（目標: 10ms → 4ms）
- [ ] スループット向上（目標: 100 req/s → 300 req/s）
- [ ] GPU利用率向上（目標: 60% → 85%）

## Phase 5: 型定義とベストプラクティス

### 5.1 型定義の改善

- [ ] ジェネリクス型の適切な制約
- [ ] トレイト境界の最適化
- [ ] ライフタイムの明示

### 5.2 Rust 2025ベストプラクティス

- [ ] Rust 2024 Editionの新機能活用
- [ ] `unsafe`ブロックの適切な使用
- [ ] エラーハンドリングの統一（`anyhow::Result`）
- [ ] 非同期処理の最適化（`tokio`）

### 5.3 警告0達成

- [ ] `cargo clippy --all-targets -- -W clippy::all`で警告0確認
- [ ] 未使用変数・フィールドの削除または`_`プレフィックス
- [ ] 型安全性の向上

## Phase 6: テストと検証

### 6.1 単体テスト

- [ ] CUDA Runtimeのテスト
- [ ] Windows AI APIのテスト
- [ ] カーネルドライバー統合のテスト

### 6.2 統合テスト

- [ ] MCP統合のE2Eテスト
- [ ] GPU最適化のパフォーマンステスト
- [ ] カーネルドライバー接続のテスト

### 6.3 最終検証

- [ ] `cargo check`でエラー0確認
- [ ] `cargo clippy`で警告0確認
- [ ] `cargo test`で全テスト通過確認

## 実装ファイル

### 修正ファイル

- `codex-rs/tui/src/bottom_pane/mod.rs` - render_refエラー修正
- `codex-rs/cuda-runtime/src/cuda_impl.rs` - RustCudaベストプラクティス適用
- `codex-rs/cuda-runtime/src/lib.rs` - 型定義改善

### 新規ファイル

- `codex-rs/windows-ai/src/mcp.rs` - Windows 11 25H2 MCP統合
- `codex-rs/windows-ai/src/kernel_cuda_bridge.rs` - カーネルドライバー↔CUDA接続

### 拡張ファイル

- `codex-rs/windows-ai/src/kernel_driver.rs` - GPU最適化機能追加
- `codex-rs/core/src/windows_ai_integration.rs` - MCP統合追加

## 技術スタック

- **Rust**: 2024 Edition
- **CUDA**: RustCuda (`cust` 0.3)
- **Windows AI**: Windows 11 25H2 SDK
- **MCP**: JSON-RPC 2.0
- **非同期**: `tokio`
- **エラーハンドリング**: `anyhow` + `thiserror`

## 期待される成果

- ✅ コンパイルエラー0
- ✅ Clippy警告0
- ✅ Windows 11 25H2 MCP統合完了
- ✅ カーネルドライバー↔Codex GPU最適化接続完了
- ✅ レイテンシ: 10ms → 4ms（-60%）
- ✅ スループット: 100 req/s → 300 req/s（+200%）
- ✅ GPU利用率: 60% → 85%（+25%）

### To-dos

- [ ] TUIのrender_refエラー修正とコンパイルエラー0達成
- [ ] RustCuda（cust）のベストプラクティス適用と型定義改善
- [ ] Windows 11 25H2 MCP統合実装（windows.ai.agents.mcp.h）
- [ ] カーネルドライバーからCodexへのGPU最適化接続実装
- [ ] 型定義の改善とRust 2025ベストプラクティス適用
- [ ] Clippy警告0達成と型安全性向上
- [ ] MCP統合とGPU最適化の統合テスト実装
- [ ] 最終検証（cargo check/clippy/test）でエラー0・警告0確認