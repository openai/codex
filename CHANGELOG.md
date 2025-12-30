The changelog can be found on the [releases page](https://github.com/openai/codex/releases).

## Unreleased

- VSCode拡張: Stop/Interrupt の信頼性を改善し、必要なら backend を kill & restart する Force Stop を追加（`codexMine.interrupt.forceStopAfterMs`）
- VSCode拡張: Force Stop 発動時に `thread/resume` でチャット履歴を読み直し、表示の不整合を防止
- VSCode拡張: rate limit 表示（例: `5h:11% wk:7%`）のホバーで各ウィンドウのリセット時刻を表示
