# zapabob/codex - 独自拡張ディレクトリ

このディレクトリには、[OpenAI/codex](https://github.com/openai/codex)の公式リポジトリに対するzapabob独自の拡張機能とドキュメントが含まれています。

## 📁 ディレクトリ構造

```
zapabob/
├── docs/           # 独自ドキュメント（PR手順、SNS投稿文等）
├── scripts/        # PowerShellビルド・管理スクリプト
├── extensions/     # VSCode/Windsurf拡張
├── sdk/            # TypeScript SDK
└── reports/        # コードレビューレポート
```

## 🎯 主な独自機能

### 1. **サブエージェントシステム** 
- `.codex/agents/*.yaml` でエージェント定義
- `codex delegate` コマンドで専門タスクを委譲
- 並列実行による高速化

### 2. **Deep Research**
- 多段階探索による深い調査
- 引用必須レポート生成
- `codex research` コマンド

### 3. **自律オーケストレーション** (ClaudeCode風)
- TaskAnalyzerによる自動タスク分析
- AutoOrchestratorによる自律的サブエージェント実行
- 閾値ベース（0.7）の自動判定

## 📝 ドキュメント

### 導入ガイド
- [自律オーケストレーション クイックスタート](../docs/quickstart-auto-orchestration.md)
- [サブエージェント クイックスタート](../docs/quickstart-subagents.md)
- [Deep Research クイックスタート](../docs/quickstart-deepresearch.md)

### 詳細仕様
- [サブエージェント & Deep Research 要件定義](../docs/codex-subagents-deep-research.md)
- [自律オーケストレーション 設計書](../docs/auto-orchestration.md)
- [実装ロードマップ v2](../docs/implementation-roadmap-v2.md)

### zapabob独自ドキュメント
- [SNS宣伝文](docs/SNS宣伝文_自律オーケストレーション.md)
- [PR作成手順](docs/PR作成手順_OpenAI.md)
- [Cursor MCP クイックガイド](docs/CURSOR_MCP_QUICK_GUIDE.md)

## 🛠️ ビルド・スクリプト

zapabob/scripts/配下のPowerShellスクリプト:
- `build-with-progress.ps1` - プログレス表示付きビルド
- `monitor-build.ps1` - ビルド監視
- `fast-build.ps1` - 高速ビルド
- `update-version.ps1` - バージョン更新

## 🔧 開発環境

- **OS**: Windows 11
- **Shell**: PowerShell
- **Rust**: 1.85+
- **Node.js**: 18+
- **PNPM**: 9+

## 📊 公式リポジトリとの差分管理

zapabobフォークは、OpenAI公式リポジトリと定期的に同期しつつ、独自機能を維持します:

1. 公式の変更を `upstream/main` から取得
2. zapabob独自機能は `zapabob/` 配下に集約
3. コアコード (`codex-rs/`) は可能な限り公式と同期

## 🔗 リンク

- [OpenAI/codex 公式](https://github.com/openai/codex)
- [zapabob/codex フォーク](https://github.com/zapabob/codex)
- [実装ログ](_docs/)

## 📄 ライセンス

公式と同じライセンスに従います。詳細は [LICENSE](../LICENSE) を参照。

---

**バージョン**: 0.48.0 
**最終更新**: 2025-10-15

