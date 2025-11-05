# 2025-10-23 main branch commit log

## Summary
コミット完了！zapabob/codexのメインブランチに変更をプッシュしたで！

## Commit Details
- **Commit Hash**: `271d7718`
- **Branch**: `main`
- **Message**: `feat: integrate rMCP subagent and deep research, optimize semver sync and build speed`
- **Files Changed**: 28 files
- **Insertions**: 20,119 lines
- **Deletions**: 27,664 lines

## Key Changes
1. **rMCP Subagent & Deep Research Integration**
   - `AgentRuntime::filter_codex_mcp_tools`を更新して`codex-subagent`、`codex-deep-research`、完全修飾MCPツール名を認識
   - `build_codex_mcp_tools_description`を拡張してサブエージェント、deep research、supervisor、custom command、hook、auto-orchestrateツールのガイダンスを追加
   - ハイフン付きCodex MCPツール名がフィルタリングを通過するように修正

2. **SemVer Sync & Build Optimization**
   - ワークスペース/パッケージメタデータをOpenAI/codexのセマンティックバージョン（`0.48.0`）に同期
   - `resolve_runtime_budget`ヘルパーを追加してCLIコマンドのランタイムトークン予算を統一
   - `codex-stdio-to-uds`依存関係の参照を追加
   - DeepResearch web searchのuser-agent文字列を正規化

3. **New Files**
   - `.gitattributes`を作成
   - `.specstory/history/2025-10-22_20-51Z-クリーナーリリースビルドとグローバルインストール.md`
   - `.specstory/history/2025-10-22_20-54Z-clean-release-build-and-global-install.md`
   - `_docs/2025-10-23_rmcp_subagent_deepresearch_log.md`
   - `_docs/2025-10-23_semver_sync_and_fast_build_log.md`

## Git Operations
```bash
git add -A
git commit -m "feat: integrate rMCP subagent and deep research, optimize semver sync and build speed"
git push origin main
```

## Validation
- ✅ コミット成功
- ✅ プッシュ成功 (f6d32a3f..271d7718)
- ✅ 28ファイルの変更をコミット
- ✅ メインブランチに反映

## Next Steps
- メインブランチの変更がリモートにプッシュ済み
- CI/CDパイプラインでビルドとテストが実行される予定
- 他のコントリビューターが変更をpullできる状態

## Notes
- PowerShellの日本語コミットメッセージでエスケープ問題が発生したため、英語メッセージに変更
- Conventional Commits形式に従ってコミット
- 実装ログファイルもコミットに含まれた

