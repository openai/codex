# 003 merge upstream rust-v0.85.0 競合メモ（判断材料）

目的: `origin/chore/merge-upstream-rust-v0.85.0` を `origin/main` に取り込む際に発生した競合の内容・論点を残す。

## 前提（観測した状況）

- 対象ブランチ: `origin/chore/merge-upstream-rust-v0.85.0`
- マージ先: `origin/main`
- merge-base: `1c8ad10980476652f182ab4bd918d41d7c84fe2b`（`vscode-extension-v0.1.16` 付近）
- 試験マージ（`git merge --no-commit --no-ff origin/chore/merge-upstream-rust-v0.85.0`）では、少なくとも以下2ファイルで競合を確認:
  - `codex-rs/core/src/project_doc.rs`
  - `vscode-extension/src/ui/chat_view.ts`

（※ 試験マージは `git merge --abort` で取り消し済み。）

## 競合 1: `codex-rs/core/src/project_doc.rs`

### main 側で入っている変更（小）

- tests の `ConfigBuilder` で `harness_overrides(ConfigOverrides { cwd: ... })` を設定するように変更されている。
  - 目的: テスト内 `cwd` をより「実運用に近い経路」で注入する（推測ではなく差分から読み取れる範囲）。

### 0.85取り込み側で入っている変更（大）

- `get_user_instructions()` の組み立てロジックが刷新されている:
  - `config.user_instructions` → `read_project_docs()` → skills section →（feature有効時）child agents message の順で結合。
  - `Feature::ChildAgentsMd` を見て `HIERARCHICAL_AGENTS_MESSAGE`（`include_str!("../hierarchical_agents_message.md")`）を末尾に追加。
- skills section の文言・フォーマットが変わり、テストもそれに合わせて大幅に書き換えられている。
  - 旧: `load_skills(&cfg)` を呼んで skills を生成していた。
  - 新: tests 内で `SkillMetadata` を直に作り、`SkillScope` を含めて `get_user_instructions()` に渡す形に変更されている。

### 論点（判断ポイント）

- main の `harness_overrides` を、0.85側の新しいテストセットアップ（`make_config()`）にも引き継ぐべきか。
  - 0.85側は `config.cwd = ...` を直にセットしているが、main 側は `ConfigBuilder` の overrides 経由で `cwd` を入れている。
  - どちらが「テストとして正しい前提」かを決める必要がある（副作用として、Config の他フィールド解決に影響し得る）。
- `Feature::ChildAgentsMd` の有効/無効により user instructions の末尾メッセージが変わるため、codex-mine 側の期待値（snapshots/テスト文言）もそれに合わせる必要がある。

## 競合 2: `vscode-extension/src/ui/chat_view.ts`

### main 側で入っている変更（小）

- 長いURL/コード等の折り返し改善（CSS）:
  - `.md a, .md code { overflow-wrap: anywhere; }`
  - `.fileLink, .autoFileLink, .autoUrlLink { overflow-wrap: anywhere; }`

### 0.85取り込み側で入っている変更（大）

- Settings UI / アカウント・CLI variant 周りの追加:
  - `settingsRequest/settingsResponse` の message ハンドリングを追加し、`load/accountSwitch/accountLogout/setCliVariant` を処理する。
  - それに対応する provider 側 callback（`onAccountList/onAccountRead/onAccountSwitch/onAccountLogout/onSetCliVariant`）がコンストラクタ引数として増える。
- HTML/CSS 側にも Settings overlay（パネル）を追加。
- CSS の折り返しも追加されており、main 側の小変更と同じ目的の行が別形で存在する（重複/差分調整が必要）。
  - 例: 0.85側は `.fileLink, .autoFileLink, .autoUrlLink { overflow-wrap: anywhere; word-break: break-word; }` のように `word-break` を追加している。

### 論点（判断ポイント）

- main の折り返し改善（`.md a, .md code ...` 等）を、0.85側の変更に統合して「意図を欠かさない」形で残す必要がある。
- 0.85側の Settings UI 追加を採用するか（multi-account/variant 切り替えを必要とするか）で、競合解消の方針が決まる。
  - 採用する場合: 0.85側の構造を基本にし、main のCSS改善を取り込む。
  - 採用しない場合: 0.85側の関連変更を落とす必要があるが、0.85ブランチ全体との整合を崩しやすい（推奨しづらい）。

## 再現用コマンド（参考）

```sh
git fetch --all --prune
base=$(git merge-base origin/main origin/chore/merge-upstream-rust-v0.85.0)
echo "$base"

# それぞれの差分
git diff --stat "$base"..origin/main -- codex-rs/core/src/project_doc.rs vscode-extension/src/ui/chat_view.ts
git diff --stat "$base"..origin/chore/merge-upstream-rust-v0.85.0 -- codex-rs/core/src/project_doc.rs vscode-extension/src/ui/chat_view.ts

# 試験マージ（※ 失敗したら競合が出る）
git switch -c tmp/merge-test origin/main
git merge --no-commit --no-ff origin/chore/merge-upstream-rust-v0.85.0
# 解消せずに戻す
git merge --abort
```

