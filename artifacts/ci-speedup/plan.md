# Hybrid Plan: Monorepo CI 高速化

## 1. Scope
- **Goal**: Monorepo（apps/web, apps/api 想定）での CI 実行時間短縮策を調査し、具体的なパイプライン最適化案と導入手順（コーディング観点を含む）を提案する。
- **Evaluation Axes**: ビルド/テストの並列化、差分ベース実行、キャッシュ・リモートエグゼキュータ、モジュラー化、開発者体験・ロールバック性。
- **Success Criteria**:
  - 2023-2025 の事例やベストプラクティスを複数ドメインから引用。
  - 改善案が定量指標（時間短縮、コスト削減、安定性維持）を伴う。
  - ハイブリッド成果として、研究レポート＋実装計画（YAML/スクリプト雛形）を提供。
- **Constraints**: 予算 80,000 tokens 想定、SLA 120 分。軽量フォールバック条件（レート制限・情報不足）を監視。
- **Excluded**: モノレポ移行そのものや CI SaaS 選定は範囲外。Infra-as-code の詳細実装は別途。

## 2. Sub-Queries & Priorities
1. モノレポ CI のボトルネック（キャッシュ、差分検出、テスト戦略）の最新事例。
2. Bazel、Nx、Gradle Enterprise などのリモートキャッシュ＆分散ビルド活用例。
3. GitHub Actions / Buildkite / CircleCI 等でのパイプライン最適化パターン。
4. テスト選択（Test Impact Analysis）、モジュールごとのワークフロー分割事例。
5. 成功指標（成功率、時間削減率、開発者体験）とリスク（キャッシュポイズニング、フレーク増加）。

## 3. Refutation Queries
- キャッシュ導入が失敗した事例やリグレッションの報告。
- 差分実行で抜け漏れが発生したケース。
- リモートエグゼキュータのコストに関する批判。

## 4. Implementation Stub (Hybrid Output)
- `docs/ci-speedup-playbook.md` に導入手順とリスク管理をまとめる案。
- `scripts/ci-diff-targets.sh` 雛形で差分ベース実行の疑似コード化。

## 5. Stop Conditions
- 3+ ドメインで成功例とリスク例が揃う。
- 提案アクションが CI 改善ロードマップとして機能する。
- 追加探索で新情報が得られず飽和したと判断。

