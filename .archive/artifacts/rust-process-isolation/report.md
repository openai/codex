# Rustのプロセス分離 (2023-2025) 比較レポート

## 結論
- Linux カーネル沙場化の中心である Landlock は 2025 年に IPC 制御と監査ログ機能を追加し、Rust 製ランドロック用ライブラリや主要ディストリビューションへの展開が進んだことで、ユーザースペースから扱いやすいカーネルレベルサンドボックスへ成熟している。[1]
- Rust 製 OCI ランタイム Youki は 2024-2025 年に CPU アフィニティ制御や Wasm 実行時の終了処理など隔離強化に直結する修正を繰り返し、コンテナ境界のきめ細かな制御を提供している。[2]
- WebAssembly サンドボックス Wasmtime は 2024 年末の signals-based-traps 導入や 2025 年のスタック切替/ SIMD 完了などでプラットフォーム選択肢とスケジューリング性能を拡張し、Rust で構築する分離環境の選択肢を広げた。[3][4]
- マイクロ VM ソリューション Firecracker は 2024 年リリースで PCI 対応やスナップショット運用を改善し、軽量 VM を Rust で安全かつ柔軟に運用する基盤として進化している。[5]

## 主な変化 (2023 → 2025)
| レイヤ | 2023 の状況 | 2024-2025 の更新 | 影響 |
|-------|-------------|-----------------|------|
| カーネル LSM (Landlock) | 基本的なファイル/ソケット制限中心 | ABI6で信号・抽象UNIXソケットのスコープ制御、ABI7で監査ログとRust/Goライブラリ整備、主要ディストリへ展開[1] | IPC 越境やポリシーデバッグが容易になり自治体アプリでも導入が進む |
| コンテナ (Youki) | OCI v1 対応を着実に拡充 | 2025/07 の v0.5.4 で exec CPUアフィニティや cgroupsv2 ハンドリング改善、Wasm 実行終了処理修正[2] | コンテナ単位でのリソース固定化・Wasmワークロード隔離が強化 |
| WebAssembly (Wasmtime) | サンドボックス基盤として採用拡大 | 2024/12 signals-based-traps、ガード領域調整、asyncの no_std 化[3]。2025/03 スタック切替と Winch SIMD 完了で高速化[4] | 非POSIX環境への対応とマルチワークロード同居性能向上 |
| MicroVM (Firecracker) | Rust実装の軽量ハイパーバイザとして普及 | 2024 年版で PCI トランスポート対応、dirty tracking無しの差分スナップショット等を追加[5] | セキュリティ境界を保ったままハードウェア/ネットワーク構成の柔軟性が増す |

## 詳細分析
### Landlock (カーネル LSM)
- **IPC スコープ**: Linux 6.12 の `LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET` と `LANDLOCK_SCOPE_SIGNAL` により、抽象UNIXソケットとプロセス間 signal をドメイン境界で遮断できるようになり、多層サンドボックス構成が実現可能になった。[1]
- **監査ログ**: Linux 6.15 (ABI7) で拒否イベントを audit に残せるようになり、ランドロック導入時のポリシー調整や侵入兆候の観測が容易化。[1]
- **エコシステム**: Rust Landlock crate や Go ライブラリが ABI7 をサポートし、Nomad や GNOME tracker-extract などのユーザー空間プロジェクトがランドロックを利用。RHEL 9.6 など主要ディストリも標準有効化へ。[1]

### Youki (Rust OCI Runtime)
- **細粒度制御**: v0.5.4 で exec CPU アフィニティ設定、追加GID重複許可等を追加し、cgroupsv2 との連携や multi-tenant 環境のリソース固定が改善。[2]
- **安定性強化**: Wasm 実行後のプロセス終了処理、UID/GID マッピングテスト、capabilities ログなど隔離プラットフォームとして必要な多層チェックが更新された。[2]
- **Wasm との融合**: Wasmtime などのランタイムを内部 exec する際の挙動が改良され、コンテナと WebAssembly の混載が容易に。[2]

### Wasmtime (WebAssembly Sandbox)
- **トラップ制御の可搬性**: 28.0.0 の `signals-based-traps` フラグにより、ホストがシグナルハンドラを持たない環境でもトラップ処理が可能に。ガード領域サイズのデフォルト変更も含め、メモリ安全性と移植性のバランスが調整された。[3]
- **スケジューリング改善**: 31.0.0 では stack-switching が始まり async スケジューラ統合が前進、Winch JIT の SIMD 完了で高密度ワークロードでも性能と隔離を両立できる。[4]

### Firecracker (MicroVM)
- **機能性と隔離の両立**: 1.13.0 で VirtIO PCI トランスポートをオプション化しつつ、軽量VMの単純さを維持。diff スナップショットとカスタム CPU テンプレートにより、隔離されたテナント間で迅速に復旧・複製が可能。[5]

## 2025 年時点の評価
- **セキュリティ**: Landlock の監査ログと IPC 制御、Youki/Firecracker の cgroup・スナップショット整備で攻撃面が狭まり、Wasmtime のガード調整と stack-switching で wasm サンドボックスの信頼性も向上。
- **性能/運用性**: Wasmtime の新しいレジスタアロケータや SIMDe 完了、Youki の CPU アフィニティ、Firecracker の diff スナップショットは隔離を維持しつつ運用コストを下げる施策。
- **エコシステム成熟度**: Landlock と Rust crate、Nomad/Flatcar 等の実装事例が増え、Rust ベースの isolation スタックを end-to-end で構築できる環境が整ってきた。

## 残された課題
- Landlock の `LANDLOCK_SCOPE_SIGNAL` は修正済みだが、スレッドセーフなポリシー更新や ID 管理などの運用ガイド整備が進行中。[1]
- Youki はセキュリティ監査（Seccomp、eBPF）を継続的に強化する必要があり、Wasm 実行時のサンドボックス境界についてさらにベンチマークが求められる。[2]
- Wasmtime の stack-switching は実装途中であり、完全な async マルチテナントを実現するには追加の検証が必要。[4]
- Firecracker の PCI サポートは柔軟性を高める一方で攻撃面を広げるため、最小構成での利用指針が引き続き重要。[5]

## 参考文献
1. Landlock LSM, "Landlock news #5" (2025-05-19). https://github.com/landlock-lsm/landlock-lsm.github.io/blob/main/news/5.md
2. Youki Project, "CHANGELOG" (accessed 2025-10-10). https://raw.githubusercontent.com/youki-dev/youki/main/CHANGELOG.md
3. Bytecode Alliance, "Wasmtime 28.0.0 Release Notes" (2024-12-20). https://raw.githubusercontent.com/bytecodealliance/wasmtime/release-28.0.0/RELEASES.md
4. Bytecode Alliance, "Wasmtime 31.0.0 Release Notes" (2025-03-20). https://raw.githubusercontent.com/bytecodealliance/wasmtime/release-31.0.0/RELEASES.md
5. Firecracker MicroVM, "CHANGELOG" (2024). https://raw.githubusercontent.com/firecracker-microvm/firecracker/main/CHANGELOG.md
