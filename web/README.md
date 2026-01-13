# web（モバイル用Codex UI + Read Only Workspace）

この `web/` は、リモートPC上で起動する **Read Only の「VSCodeっぽいファイルツリー + ビューア」**と、モバイルからCodexを実行する **Codex UI** を提供します。

## 目的

- Tailscale 経由で自分だけがアクセスする前提
- サーバープロセスを起動したユーザーの `HOME` (`~/`) 配下のみを扱う
- Web UI から multi-root（複数パス）を追加/削除できる
- 編集はしない（Read Only）
- Codex実行は `app-server` を使う（承認フロー込み）

## 起動

- 開発:
  - `pnpm -C web dev`
  - ブラウザ: `http://localhost:5173`
- 本番:
  - `pnpm -C web build`
  - `pnpm -C web start`
  - ブラウザ: `http://localhost:3000`

## チャットUIについて（重要）

- チャットは React 自前UIではなく、VSCode拡張の `chat_view_client` を **iframe で埋め込み**して動かしています。
  - `GET /webview/chat?rootId=...`（HTMLを返す）
  - `WS /api/webview/ws?rootId=...`（webview⇄serverブリッジ）

現状、VSCode拡張の全機能同等化は未達です。進捗/差分は `.memo/web_vscode_extension_parity.md` を参照してください。

## Tailscale からアクセスする場合

デフォルトで `0.0.0.0` バインドです（tailnet から到達できます）。

- 開発（Vite）: `pnpm -C web dev`
- 本番（Express）: `pnpm -C web start`（`PORT` は必要なら変更）

localhost のみに絞りたい場合:
- 開発: `VITE_HOST=127.0.0.1 pnpm -C web dev`
- 本番: `HOST=127.0.0.1 pnpm -C web start`

## 制約・安全設計

- ルートとして登録できるパスは `HOME` 配下のみです（`..` や symlink 経由の逸脱は拒否）。
- API は `rootId` を必須とし、登録済み root 配下のみを参照できます。
- テキスト表示（`/api/file`）は UTF-8 としてデコードできるものだけを返します。
- `web/.data/workspace.json` に root 定義が保存されます。
- `web/.data/` はサーバー側ローカル保存で、git管理しません。
