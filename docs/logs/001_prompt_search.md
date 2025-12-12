# 001 prompt_search

- [x] リポジトリの基本情報とドキュメント確認（AGENTS.md、README系、README.dev.mdの有無確認。README.dev.md は未確認=未存在）
- [x] プロンプト探索ルート統合関数の設計と実装（ホームとリポジトリを統合し優先順位を設定）
- [x] 既存呼び出しの置き換えと重複処理の実装（list_custom_prompts を新関数経由に変更）
- [x] TUI/CLI への反映確認と必要な修正（ListCustomPrompts イベント経由で拡張リストを供給）
- [x] テスト追加・実行と結果確認（custom_prompts モジュールの統合テスト追加、`cargo test -p codex-core custom_prompts` 実行）
- [x] 変更内容の最終確認と後処理（フォーマット実行、lint 修正、テスト、コミット、PR 作成準備）
