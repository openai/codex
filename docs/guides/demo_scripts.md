# 🚀 Codex MCP デモスクリプト集

**バージョン**: codex-cli 0.47.0-alpha.1  
**作成日**: 2025年10月12日  
**目的**: 実装したMCP機能の実践デモ

---

## 📝 使い方

各デモは、新しいPowerShellまたはターミナルで実行してください。

```powershell
# ディレクトリに移動
cd C:\Users\downl\Desktop\codex-main\codex-main

# デモを実行（下記のコマンドをコピー＆ペースト）
```

---

## 🎯 デモ1: Deep Research（高度な調査）

### 目的
Deep Research機能を使って、Rustの非同期ランタイム（tokio vs async-std）を比較調査

### コマンド
```bash
codex "Use the codex-deep-research tool to research 'Rust tokio vs async-std performance comparison' with depth 2 and comprehensive strategy. Provide a detailed comparison report with citations."
```

### 期待される結果
- 複数のソースから情報収集
- パフォーマンス比較データ
- 引用付きレポート
- 実装推奨事項

### 実行時間の目安
**2-5分**（検索深度とソース数による）

---

## 🎯 デモ2: Supervisor（並列エージェント実行）

### 目的
Supervisor機能を使って、セキュリティレビューとテスト生成を並列実行

### コマンド
```bash
codex "Use the codex-supervisor tool to coordinate SecurityExpert and TestingExpert agents in parallel. Goal: Review the authentication module for security vulnerabilities and generate comprehensive unit tests. Strategy: parallel. Merge strategy: concatenate."
```

### 期待される結果
- 2つのエージェントが並列実行（2.5x高速）
- セキュリティレビューレポート
- ユニットテストコード生成
- 統合された最終レポート

### 実行時間の目安
**3-7分**（並列実行により高速）

---

## 🎯 デモ3: Subagent（カスタムエージェント作成）

### 目的
動的にカスタムエージェントを生成し、特定のタスクを実行

### コマンド
```bash
codex "Use the codex-subagent tool with action 'start_task' and agent_type 'PerformanceExpert'. Task: Analyze the core/src/agents/runtime.rs file and provide performance optimization recommendations, focusing on async operations and memory allocation."
```

### 期待される結果
- パフォーマンス専門家エージェント起動
- runtime.rs の詳細分析
- 最適化推奨事項
- コード例付き改善提案

### 実行時間の目安
**2-4分**

---

## 🎯 デモ4: Custom Command（定義済みコマンド実行）

### 目的
事前定義されたカスタムコマンドを使って、セキュリティレビューを実行

### コマンド
```bash
codex "Use the codex-custom-command tool with action 'execute' and command_name 'security_review'. Context: 'codex-rs/core/src/agents/'"
```

### 期待される結果
- セキュリティ脆弱性スキャン
- 潜在的な問題の特定
- 修正推奨事項
- CVE参照（該当する場合）

### 実行時間の目安
**1-3分**

---

## 🎯 デモ5: Hook（ライフサイクルイベント）

### 目的
タスク完了時のフックを実行し、自動化されたアクションをトリガー

### コマンド
```bash
codex "Use the codex-hook tool to trigger 'on_task_complete' event with context: 'Security review and test generation completed successfully. Generate a summary report and commit changes to a new branch.'"
```

### 期待される結果
- タスク完了イベントの処理
- サマリーレポート生成
- 自動ブランチ作成
- コミット実行（承認後）

### 実行時間の目安
**30秒-2分**

---

## 🎯 デモ6: 自己参照型オーケストレーション（codex-agent）

### 目的
Codex が自分自身をツールとして使用し、再帰的にタスクを実行

### コマンド
```bash
codex "Use the codex tool (self-referential) to create a new Codex instance that specializes in documentation generation. Then, have that instance generate comprehensive API documentation for the codex-rs/mcp-server module."
```

### 期待される結果
- 新しいCodexインスタンス起動
- ドキュメント専門エージェント生成
- API ドキュメント生成
- Markdown形式で出力

### 実行時間の目安
**4-8分**（再帰的実行のため）

---

## 🎯 デモ7: Playwright（ブラウザ自動化）

### 目的
Playwright MCPを使って、Webページのスクリーンショットを取得

### コマンド
```bash
codex "Use the playwright MCP server to navigate to https://www.rust-lang.org/ and take a screenshot. Save it as 'rust-lang-screenshot.png' in the current directory."
```

### 期待される結果
- ブラウザ自動起動
- 指定URLに移動
- スクリーンショット取得
- ファイル保存確認

### 実行時間の目安
**1-2分**

---

## 🎯 デモ8: Web Search（リアルタイム検索）

### 目的
Brave Search APIを使って、最新情報を検索

### コマンド
```bash
codex "Use the web-search MCP server to find the latest news about Rust 1.75 release. Summarize the top 5 new features with links to official documentation."
```

### 期待される結果
- リアルタイムWeb検索
- 最新のRustニュース
- Top 5機能のサマリー
- 公式ドキュメントへのリンク

### 実行時間の目安
**30秒-1分**

**注意**: Brave Search API キーが必要です。`~/.codex/config.toml` に設定してください。

---

## 📊 パフォーマンス比較デモ

### 目的
並列実行と逐次実行のパフォーマンスを比較

### デモ9A: 逐次実行（Sequential）
```bash
$startTime = Get-Date
codex "Use codex-supervisor with strategy 'sequential' to run SecurityExpert and TestingExpert"
$endTime = Get-Date
$sequentialTime = ($endTime - $startTime).TotalSeconds
Write-Host "Sequential execution time: $sequentialTime seconds"
```

### デモ9B: 並列実行（Parallel）
```bash
$startTime = Get-Date
codex "Use codex-supervisor with strategy 'parallel' to run SecurityExpert and TestingExpert"
$endTime = Get-Date
$parallelTime = ($endTime - $startTime).TotalSeconds
Write-Host "Parallel execution time: $parallelTime seconds"
```

### デモ9C: 比較計算
```powershell
$speedup = $sequentialTime / $parallelTime
Write-Host "Speedup: ${speedup}x faster with parallel execution"
```

### 期待される結果
- 逐次実行: 6-10分
- 並列実行: 3-5分
- スピードアップ: **2.0-2.5x**

---

## 🎯 デモ10: トークン予算管理

### 目的
トークン予算機能をテストし、制限を超えた場合の動作を確認

### コマンド
```bash
codex "Use codex-subagent with action 'start_task', agent_type 'CodeExpert', and task 'Implement a complete REST API with authentication, database integration, and comprehensive error handling.' Note: This task is intentionally large to test token budget limits."
```

### 期待される結果
- トークン予算の自動チェック
- 制限超過時の警告
- タスクの適切な分割
- または、予算内での最大限の実装

### 実行時間の目安
**3-6分**

---

## 📋 デモ実行チェックリスト

実行前に以下を確認してください：

### 環境確認
- [ ] Codex CLI インストール済み（`codex --version`）
- [ ] MCP サーバー設定済み（`codex mcp list`）
- [ ] 必要なAPI キー設定済み（Brave Search等）

### 実行前準備
- [ ] 適切なディレクトリに移動
- [ ] サンドボックス設定確認（`~/.codex/config.toml`）
- [ ] ターミナルを対話モードで起動

### 実行中の注意点
- デモ中に承認プロンプトが表示される場合があります
- ネットワーク接続が必要です
- 一部のデモは数分かかる場合があります

### 実行後の確認
- [ ] 生成されたファイルを確認
- [ ] 監査ログを確認（`~/.codex/audit-logs/`）
- [ ] トークン使用量を確認

---

## 🎥 デモ録画推奨設定

デモ動画を作成する場合、以下の設定を推奨します：

### 録画ツール
- **Windows**: OBS Studio, Xbox Game Bar
- **macOS**: QuickTime, ScreenFlow
- **Linux**: SimpleScreenRecorder, Kazam

### 推奨設定
- **解像度**: 1920x1080 (Full HD)
- **フレームレート**: 30 FPS
- **音声**: マイクON（説明付き）
- **形式**: MP4 (H.264)

### デモ構成
1. イントロ（15秒）: 機能の概要説明
2. コマンド実行（2-5分）: 実際のデモ
3. 結果確認（30秒）: 生成されたファイル/レポート
4. まとめ（15秒）: キーポイントの強調

---

## 📊 期待される成果物

全デモ実行後、以下の成果物が得られます：

### 生成ファイル
- レビューレポート（Markdown）
- テストコード（Rust）
- API ドキュメント（Markdown）
- スクリーンショット（PNG）

### パフォーマンスデータ
- 実行時間ログ
- トークン使用量
- メモリ使用量

### 監査ログ
- エージェント実行履歴
- ツール呼び出し記録
- エラーログ

---

## 🎉 デモ完了後のアクション

### 1. レポート作成
```bash
# デモ実行結果をまとめる
codex "Create a comprehensive demo report based on the execution logs in ~/.codex/audit-logs/"
```

### 2. パフォーマンスグラフ作成
```bash
# Python スクリプトでグラフ生成
python scripts/generate_performance_graphs.py
```

### 3. スクリーンショット整理
```bash
# デモで生成したファイルを整理
mkdir demo_outputs
mv *.png *.md demo_outputs/
```

### 4. GitHubへプッシュ
```bash
git add demo_outputs/
git commit -m "docs: Add demo execution results and performance benchmarks"
git push origin main
```

---

## 🚀 次のステップ

デモ実行完了後：

1. **デモレポート作成**
   - 実行結果のサマリー
   - パフォーマンスベンチマーク
   - スクリーンショット集

2. **PR送信**
   - OpenAI/codex への PR
   - デモ動画リンク追加
   - パフォーマンスデータ添付

3. **コミュニティ共有**
   - Twitter/LinkedIn で告知
   - Reddit/HackerNews に投稿
   - ブログ記事執筆

---

**作成者**: zapabob  
**作成日**: 2025-10-12  
**Codex Version**: 0.47.0-alpha.1  
**Status**: Ready for execution 🚀

