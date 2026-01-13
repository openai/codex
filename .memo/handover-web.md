---
title: web（モバイル用Codex UI + Read Only Workspace）引き継ぎメモ
date: 2026-01-13
status: handover
---

## TL;DR

- `web/` は **Read Only ワークスペース（Explorer/Viewer） + Codex実行UI** を、モバイル（Tailscale経由）で使うためのWebアプリ。
- チャットUIは React の自前実装ではなく、**VSCode拡張の `chat_view_client` を iframe で埋め込み**して「機能同等UX」を目指している。
- **重大な未解決**: 「New（新規セッション作成）後に *チャット上部のタブ一覧（複数セッションのタブ）に反映されない*」が継続。機能同等化は未達。

---

## 起動（開発/本番）

- 開発（ホットリロード）:
  - `pnpm -C web dev`
  - フロント: `http://<host>:5173`
  - API/WS: `http://<host>:3000`（Vite proxy）
- 本番:
  - `pnpm -C web build`
  - `pnpm -C web start`
  - `http://<host>:3000`

Tailscale で使う前提なので、`0.0.0.0` bind（デフォルト）でOK。

---

## 主要要件（ユーザー要求）

- 1ユーザー前提（Tailscale経由で本人のみ利用）
- workspace roots は **サーバープロセス起動ユーザーの `HOME` 配下のみ**追加可能
- Read Only（閲覧のみ）
- Codex実行は **`app-server`** を使う
- `sandbox=workspace-write` + `approvalPolicy=on-request` を使い、承認フロー込みで実行
- `codex` / `codex-mine` は **全体設定で切替**（root個別ではない）

---

## アーキテクチャ概要

### サーバー

- `web/server/index.mjs`（Express + WS）
  - workspace root 管理: `web/.data/workspace.json`
  - chat state: `web/.data/chat/<rootId>.json`
  - webview state（`vscode.setState` 相当）: `web/.data/webview_state/<rootId>.json`
  - VSCode拡張のUIアセットを配信:
    - `/_ext/ui/*` → `vscode-extension/dist/ui/*`
    - `/_ext/vendor/*` → `vscode-extension/resources/vendor/*`
  - Webview HTML:
    - `GET /webview/chat?rootId=...`（`web/server/vscode_chat_html.mjs`）
  - Webview WS:
    - `WS /api/webview/ws?rootId=...`（chat_view_client と同じ message 型に寄せるブリッジ）

### クライアント

- `web/src/ui/App.tsx`
  - モバイル: 左上☰のdrawer（Explorer/Viewer）、右上⚙の設定メニュー
  - チャット: `web/src/ui/VscodeChatPane.tsx`（iframeで `/webview/chat` を表示）
  - root 選択モーダル（New時/切替時）
- `web/src/ui/Explorer.tsx` / `EditorPane.tsx`
  - Read Only ファイルツリー + Monaco viewer

---

## データ保存（重要）

- `web/.data/` は `.gitignore` 対象（サーバー側ローカル保存）
- `workspace.json` の `settings.cliCommand` が全体CLI切替（`codex` or `codex-mine`）

---

## 既知の未解決（優先順）

### 1) New後に「タブ一覧」が更新されない（致命）

症状:
- Newでセッション作成は走るが、chat_view_client の上部タブに追加されない/変化しないことがある（ユーザー報告）。

現状の実装:
- root選択で session を作成したら、同一rootの場合に親→iframeへ `postMessage` で `codexMine.refreshState` を送り、server側が `state` を再送する。
- それでも反映されないケースが残っている。

次にやること（調査手順）:
- serverログに「session作成→store反映→sendFullState」が走っているかを確認できるように **明示的なログ**を追加（サイレントに握りつぶさない）。
- `web/.data/chat/<rootId>.json` を直接確認して、sessions配列に新規が入っているか、`activeSessionId` が更新されているかを確認。
- Webview側（iframe）で `ready` → `state` を受けているか、WSが切れていないかを確認（モバイルはリモートデバッグ推奨）。

### 2) VSCode拡張と同等の全機能は未達

チェックリストは `.memo/web_vscode_extension_parity.md` を参照。

---

## 実装の入口（ファイル案内）

- Web app:
  - `web/src/ui/App.tsx`（全体UI）
  - `web/src/ui/VscodeChatPane.tsx`（iframe + ブリッジ）
  - `web/src/ui/AddRootModal.tsx`（root追加）
  - `web/src/ui/Explorer.tsx` / `web/src/ui/EditorPane.tsx`
- Server:
  - `web/server/index.mjs`（API + WS + app-server起動）
  - `web/server/app_server.mjs`（`codex(-mine) app-server` のJSON-RPCラッパ）
  - `web/server/vscode_chat_html.mjs`（webview HTML）
- VSCode拡張（参照元）:
  - `vscode-extension/docs/spec.md`
  - `vscode-extension/src/ui/chat_view_client.ts`

---

## リポジトリ状態の注意

- 現時点で `vscode-extension/` に未コミット変更が残っている場合がある（`git status` を必ず確認）。
- ユーザー指示なしに `reset/checkout` で消さない（AGENTS.md）。
