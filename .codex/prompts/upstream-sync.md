上流(main)取り込み指示のテンプレート（Codex向け）

- 目的: forkブランチに `upstream/main` を取り込む手順を明示し、無断フォールバックや破壊的操作を避ける。
- 前提確認: `git status --short --branch` で作業ツリーの汚れと現在ブランチを報告する。未コミット変更がある場合はユーザーに退避方法（stash/commit/一時ブランチ）を選んでもらう。
- 取得: 常に `git fetch upstream` を先に実行し、fetchログを共有する。
- 取り込み方針はユーザーに確認（例: `rebase upstream/main` か `merge upstream/main`）。デフォルトで勝手に選ばない。
- 推奨の判断基準:
  - ✅ rebase を優先: 自分専用のトピックブランチで履歴を直線に保ちたいとき。`git pull --rebase` ではなく、明示的に `git rebase upstream/main` を使う。
  - ✅ merge を選択: ブランチを複数人で共有している場合、既にCIリンクが貼られている場合、過去に `merge` を使っており履歴を書き換えたくない場合、大規模コンフリクトが予想される場合。
  - 決め手がなければ「コミットをまとめ直したいなら rebase、履歴を書き換えたくないなら merge」と伝える。
- 実行ステップ例（rebaseの場合）
  1) 退避方針に従い作業ツリーをクリーンにする（stash pop/ワークツリー復元はユーザー許可後）。
  2) `git rebase upstream/main` を実行し、コンフリクトがあればその場で明示。解決方針が不明なら中断してユーザー判断を仰ぐ。
  3) rebase完了後 `git status -sb` と `git log --oneline -5 --decorate` で結果を共有。
- マージを選ぶ場合も同様に手順を明示し、コンフリクトは隠さない。`git merge --no-ff upstream/main` を基本とするが、オプション変更はユーザー了承後。
- 未コミット変更を含む場合の基本方針:
  - まず必要な変更をブランチにコミットする（コミットメッセージは短く理由を書く）。残りは `git stash push -m "<reason>"` で明示的に退避。
  - 退避したものは rebase/merge 完了後に `git stash pop` で戻す。失敗時は必ずログを共有して指示待ち。
- 典型コマンド（rebase案）
  ```
  git status -sb
  git fetch upstream
  git stash push -m "pre-upstream-sync"   # 必要なら
  git rebase upstream/main
  git stash pop                            # 必要なら
  git status -sb
  git log --oneline -5 --decorate
  ```
- 典型コマンド（merge案）
  ```
  git status -sb
  git fetch upstream
  git stash push -m "pre-upstream-sync"   # 必要なら
  git merge --no-ff upstream/main
  git stash pop                            # 必要なら
  git status -sb
  git log --oneline -5 --decorate
  ```
- 破壊的操作禁止: `git reset --hard`, `git push -f` は明示的指示がない限り使用しない。失敗時に黙ってロールバックしない。
- 付随タスク: 変更がRust領域に及ぶ場合は `just fmt` を必ず実行し、`just fix -p <project>` やテスト実行はユーザーに要確認。
- ログと未解決事項は必ず報告し、推測でデフォルト値やフォールバックを入れない。
