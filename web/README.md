# web (VSCode風 Read Only ワークスペース・ビューア)

この `web/` は、リモートPC上で起動する Read Only の「VSCodeっぽいファイルツリー + ビューア」です。

## 目的

- Tailscale 経由で自分だけがアクセスする前提
- サーバープロセスを起動したユーザーの `HOME` (`~/`) 配下のみを扱う
- Web UI から multi-root（複数パス）を追加/削除できる
- 編集はしない（Read Only）

## 起動

- 開発:
  - `pnpm -C web dev`
  - ブラウザ: `http://localhost:5173`
- 本番:
  - `pnpm -C web build`
  - `pnpm -C web start`
  - ブラウザ: `http://localhost:3000`

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
