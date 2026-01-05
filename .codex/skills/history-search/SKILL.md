---
name: history-search
description: メインエージェントが codex exec をサブエージェントとして起動し、指定した期間（shard）のセッション履歴（rollout JSONL）からナレッジ台帳/RAG向けの知識候補を証拠付きで収集・統合する
metadata:
  short-description: セッション履歴RAG用のshard探索ワーカー（インデックス不要、JSON出力固定）
---

## このスキルがやること

このスキルは **メインエージェント（オーケストレータ）向け**の手順です。メインが `codex exec` をサブエージェントとして複数回起動し、セッション履歴（`$CODEX_HOME/sessions/**/rollout-*.jsonl` と `$CODEX_HOME/archived_sessions/**/rollout-*.jsonl`）から「ナレッジ台帳に載せるべき知識候補」を **証拠（rollout_path + line_no）付きで収集**して、メイン側で統合します。

重要: 探索範囲は scope の shard（since/until など）で明確に絞る。ワーカーは shard 外を勝手に広げない。

## 絶対条件（オーケストレータ）

- メインエージェントの最終応答を JSON にする必要はない（通常は Markdown や文章で可）
- 推測で埋めない。候補は evidence（rollout_path + line_no + snippet）で裏付ける
- `session_meta.payload.instructions`（多くの場合 line 1）だけを根拠に候補を埋めない（固定文の再掲になりやすい）。原則として **会話（user/assistant）や tool 出力（function_call_output）**の行から evidence を取る
- セッション冒頭に注入されがちな「手順/規約/スキル一覧」ブロック（例: `<INSTRUCTIONS>...`, `## Skills`, `<environment_context>...`）だけで候補を埋めない。これは“過去にやったこと”ではなく固定文の再掲になりやすい（必要なら「静的ポリシー」として別枠に落とす）

## 絶対条件（codex exec ワーカー）

- **ワーカーの最終応答は JSON のみ**（説明文、コードフェンス、前置き、ログは禁止）
- 変更は禁止（ファイル編集・削除・git操作・ネットワークアクセスをしない）
 - セッション冒頭の注入ブロック（`session_meta`、`<INSTRUCTIONS>`、`## Skills`、`<environment_context>` 等）からの引用だけで `top_hits` を埋めない。原則として `event_msg.user_message` / `response_item.message`（会話本文）/ `response_item.function_call_output` を根拠にする

## 入力

メイン（オーケストレータ）が、各 shard ごとにワーカーへ渡す。

- query: 人間の依頼文
- scope (JSON): shard 指定と探索制約（キーは固定）

scope のキー（固定、すべて必須）:

- shard_id: string（任意の識別子）
- since / until: "YYYY-MM-DD"（この shard の範囲）
- recent_files: number（新しい rollout を優先する上限）
- include_archived: boolean（archived_sessions も見るか）
- project_root: string（通常 `"."`。cwd がこの配下のセッションを優先）
- kinds: ["command","file","error","message","tool"]（優先観測）
- max_hits: number（候補行の上限）
- expand_window: number（近傍確認の行数）

## 推奨する探索のしかた（自由度は残す）

ワーカーは shard 範囲内で、次の観点をバランスよく探す（優先順位は query に従う）。

1. **反復コマンド/手順**: 何度も使っているコマンド、手順、確認観点
2. **運用ルール/禁止事項**: “〜しない”“〜のみ”“必ず”のような規範、承認条件
3. **再現・切り分けの型**: 詰まったときの切り分け手順（観点→確認→次の一手）
4. **結果の裏取り**: 成功/失敗を述べるなら、必ず `function_call_output`（exit code 等）に紐づける
5. **意思決定ログ**: 何を捨て、何を採用したか（理由と制約）
6. **作業の前提条件**: どのディレクトリで実行したか、承認/権限/サンドボックスの前提

## 出力（JSON 契約）

ワーカーの最終応答は `references/WORKER_OUTPUT_SCHEMA.json` に適合する JSON のみ。

- `scope` は入力をそのまま返す（キー追加は禁止）
- `top_hits` は最大 5 件
- `rank` は 1..N の連番（欠番なし）
- `utility` の根拠に、可能なら **別日の evidence** を最低2件入れる（無ければその旨を rationale に書いて減点する）
