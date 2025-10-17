# Monorepo CI 高速化ハイブリッドレポート

## 結論
- モノレポのビルド時間短縮には「差分に応じた動的パイプライン」「リモートキャッシュ再利用」「マージキューによる緑化保持」の三本柱が有効であり、いずれも既存ツールで即導入可能である。[1][2][3]
- Turborepo・Bazel といったビルドシステムが提供するリモートキャッシュ機能を CI に統合すると、再実行を最小化しつつ安定した成果物共有が図れる。[2][3]
- ただしネットワーク障害時にはキャッシュが足を引っ張る恐れがあるため、ローカル fallback やフレーク検出を組み込みコスト管理を行う必要がある。[1][3]

## 主な知見
1. **Selective Builds（差分ジョブ生成）** — Buildkite はコミット差分で必要なジョブのみ生成する「動的パイプライン」を推奨し、同時に通知ノイズとリソース無駄を削減できると説明している。[1]
2. **Merge Queue とグリーンマスター** — 高頻度 PR ではマージキューで未来の main 状態に対するテストを先行実行し、無効な再試行や競合を避けるべきという指針がある。[1]
3. **Remote Caching 導入の容易さ** — Turborepo は `TURBO_TOKEN` と `TURBO_TEAM` を設定したうえで `turbo` コマンドを実行するだけで CI 上でもキャッシュを共有できると明記している。[2]
4. **Bazel のリモート実行ロードマップ** — Bazel は「リモートキャッシュは安価な高速化手段」と位置づけ、HTTP/2 サポートや並列アップロード、ネットワーク劣化時のローカル fallback を計画している。[3]

## 推奨アクション
- `apps/web` / `apps/api` のテスト・ビルドを差分検出で分岐させる（`git diff --name-only origin/main...HEAD` を用いたターゲット抽出）。
- Turborepo もしくは同等のリモートキャッシュを CI 上に導入し、共通成果物を共有。
- GitHub Actions などで Merge Queue（例: GitHub の Merge Queue または Aviator）を有効化し、main ブランチを常時グリーンに保つ。
- Bazel / Turborepo 双方でリトライとローカル fallback を有効化し、ネットワーク不調時のドグマ化を避ける。

## リスクと緩和策
- **キャッシュ不整合**: キャッシュ破損やミスマッチで失敗する可能性がある。→ CI でキャッシュミス時の自動再ビルドと定期的なキャッシュパージを設定。[2][3]
- **差分漏れ**: パス検出が誤りだと必要なテストが実行されない。→ 明示的なルール（例: 依存グラフ JSON）とフォールバックの全体テストジョブを定期的に走らせる。[1]
- **ネットワーク依存**: リモートキャッシュは帯域・レイテンシに影響される。→ Bazel が推すようなローカルプロキシ／fallback 戦略を採用し、閾値監視を実装する。[3]

## 次のステップ
1. CI に差分判定スクリプト（参考: `scripts/ci-diff-targets.sh`）を追加。
2. Turborepo もしくは Bazel 側でリモートキャッシュ資格情報を発行し、Secrets として登録。
3. Merge Queue やトランクベース開発ルールを運用ドキュメント化（参考: `docs/ci-speedup-playbook.md`）。
4. キャッシュヒット率とビルド時間の計測ダッシュボードを追加し、改善効果を定量評価。

## 参考文献
1. Buildkite, "Monorepo CI best practices" (2023-11-09). https://www.buildkite.com/blog/monorepo-ci-best-practices
2. Vercel Turborepo Docs, "Continuous Integration". https://github.com/vercel/turbo/blob/main/docs/site/content/docs/guides/ci-vendors/index.mdx
3. Bazel Team, "Remote Execution Roadmap" (2019). https://github.com/bazelbuild/bazel-website/blob/master/roadmaps/2019/remote-execution.md
