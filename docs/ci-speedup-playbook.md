# Monorepo CI スピードアップ実装プレイブック

このドキュメントは `apps/web` と `apps/api` を含むモノレポ CI の高速化施策を段階的に導入するための手順書です。Buildkite のベストプラクティスと Turborepo/Bazel の公式ドキュメントをベースにしています。[1][2][3]

## フェーズ 0: 現状診断
- CI 実行時間・失敗率・再試行回数・キャッシュヒット率を取得し、SLO を設定する。
- main ブランチの緑化時間、PR 同時投入数、キュー待ちの有無を棚卸。

## フェーズ 1: 差分ベース実行（Selective Builds）
- `scripts/ci-diff-targets.sh` を CI の最初のジョブに追加し、変更ファイルからターゲットを抽出。
- `apps/web`, `apps/api` の各ワークフローは抽出されたパスに応じてジョブをスキップまたは実行。
- 週次でフルビルドを残し、差分漏れを検出するガードレールを設定。[1]

## フェーズ 2: Merge Queue 導入
- GitHub Merge Queue もしくは Aviator 等を利用し、PR をキューに登録後 CI を実行。
- main ブランチへマージされる前に未来の HEAD を想定したテストを回し、常にグリーンを維持。[1]
- 通知ノイズ削減のため、Slack/メールはキューバッチごとにまとめる。

## フェーズ 3: リモートキャッシュ
- Turborepo を利用している場合:
  1. `TURBO_TOKEN` と `TURBO_TEAM` を CI シークレットに登録。
  2. ジョブ内で `turbo login --token $TURBO_TOKEN`（必要な場合）を実行。
  3. `turbo run build --filter=apps/...` などのタスクを実行してキャッシュを共有。[2]
- Bazel を利用している場合:
  1. `--remote_cache` でキャッシュエンドポイントを指定し、HTTP/2 サポートや並列アップロードを有効化。
  2. ネットワーク劣化時には自動でローカルビルドへフォールバックする設定を追加。[3]

## フェーズ 4: キャッシュ衛生と可観測性
- キャッシュヒット率、CI 実行時間、再試行率をダッシュボード化。
- キャッシュ破損時の Purge コマンド、手動再実行 Runbook を整備。
- キャッシュ利用時の成果物ハッシュを保存し、ロールバック手順に組み込む。

## フェーズ 5: 開発者エクスペリエンス強化
- トランクベース開発（TBD）＋フィーチャーフラグ運用を公式化し、短サイクルで main にマージする文化を醸成。[1]
- 新規サービス追加時の CI ドキュメントテンプレートを整備し、標準化する。

---

## 参考文献
1. Buildkite, "Monorepo CI best practices". https://www.buildkite.com/blog/monorepo-ci-best-practices
2. Vercel Turborepo Docs, "Continuous Integration". https://github.com/vercel/turbo/blob/main/docs/site/content/docs/guides/ci-vendors/index.mdx
3. Bazel Team, "Remote Execution Roadmap". https://github.com/bazelbuild/bazel-website/blob/master/roadmaps/2019/remote-execution.md
