# 2025-10-11 Codex Deep Research: DuckDuckGo パーサ改善ログ

## 概要
- `codex-deep-research` の DuckDuckGo スクレイピング処理を、scraper ベースで安定させるための改修。
- タイトル・スニペット抽出時の余分な空白や改行を正規化し、検索結果の品質を向上。
- 既存の `generate_official_format_results` フォールバック経路は維持しつつ、パーサがゼロ件となるケースを明示的に検知。

## 実施内容
- `WebSearchProvider::parse_duckduckgo_html` で HTML テキストを `normalize_text` ヘルパに通し、複数スペースや改行を 1 スペースに統一。
- DuckDuckGo のスニペット抽出で、祖先ノードを辿り最初の `.result__snippet` を取得するロジックを安定化。
- `SearchResult` の期待値を比較する単体テストを追加 (`parse_duckduckgo_html_normalizes_whitespace` など) し、正規化ロジックが回帰しないようにガード。
- 既存のログは `tracing` へ統一し、環境変数 `CODEX_DEBUG_SAVE_HTML` がセットされた場合のみデバッグ HTML を保存するように調整。

## 実行したコマンド
```powershell
cargo fmt -- --config imports_granularity=Item
cargo test -p codex-deep-research
```

## テスト結果
- `cargo test -p codex-deep-research`
  - 単体・統合テストともに成功（DuckDuckGo 実際の検索系は既存同様にネットワーク可用時のみ実行）。
  - 既存の未使用 import 警告 (`tests/test_duckduckgo.rs` 内) は元コードと同様に残存。

## メモ
- `just fmt` が Windows シェル設定に依存して失敗したため、代替として `cargo fmt` を直接実行。
- 破損した Cargo キャッシュ (`cssparser`, `dtoa` 系) が原因で最初のテスト実行が失敗したため、`%USERPROFILE%\.cargo\registry\cache\…` の該当 crate を削除して再実行。
- 追加テストにより DuckDuckGo の解析結果が空白や改行依存で差異が出ないことを確認済み。
