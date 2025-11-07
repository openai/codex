<!-- 4faf255b-c10d-4457-8fc6-18e7e40c9992 0e51ce52-5400-46c0-af61-22cf970b5dc2 -->
# C/C++ to Rust Migration Plan

## 方針

1. **カーネルドライバー**: カーネルモジュール自体はCのまま（カーネル空間はC必須）、ユーザー空間のFFIラッパーを完全にRust化
2. **OpenXR SDK**: bindgenのまま、型安全なRustラッパーで安全性向上
3. **Metal API**: objc crateのまま、型安全なRustラッパーで安全性向上

## Phase 1: カーネルドライバーFFIラッパーのRust化

### 1.1 Windows AI Driver FFIラッパー

**現状**: Cコード (`kernel-extensions/windows/ai_driver/ai_driver.c`) を直接呼び出し

**実装**:

- **新ファイル**: `codex-rs/windows-ai/src/kernel_driver_ffi.rs`
- **内容**:
- `#[repr(C)]`構造体でC構造体を定義
- `extern "C"`関数でIOCTL呼び出しをラップ
- エラーハンドリングをRustの`Result`型に変換
- メモリ安全性を保証（`unsafe`ブロックを最小化）
- リソース管理をRAIIパターンで実装

**変更ファイル**:

- `codex-rs/windows-ai/src/kernel_driver.rs` - FFIラッパーを使用するように変更

### 1.2 Linux AI Scheduler FFIラッパー

**現状**: Cコード (`kernel-extensions/linux/ai_scheduler/ai_scheduler.c`) を直接呼び出し

**実装**:

- **新ファイル**: `codex-rs/linux-ai-scheduler/src/lib.rs`
- **内容**:
- `/proc/ai_scheduler`インターフェースをRustで安全に読み書き
- `libc`クレートを使用してシステムコールをラップ
- エラーハンドリングをRustの`Result`型に変換
- ファイルI/Oを`std::fs`で安全に実装

### 1.3 Linux AI Memory FFIラッパー

**現状**: Cコード (`kernel-extensions/linux/ai_mem/ai_mem.c`) を直接呼び出し

**実装**:

- **新ファイル**: `codex-rs/linux-ai-memory/src/lib.rs`
- **内容**:
- `/proc/ai_memory`インターフェースをRustで安全に読み書き
- メモリ統計を型安全な構造体で取得
- リソースリークを防ぐRAIIパターン

### 1.4 Linux AI GPU FFIラッパー

**現状**: Cコード (`kernel-extensions/linux/ai_gpu/ai_gpu.c`) を直接呼び出し

**実装**:

- **新ファイル**: `codex-rs/linux-ai-gpu/src/lib.rs`
- **内容**:
- `/proc/ai_gpu`インターフェースをRustで安全に読み書き
- GPU統計を型安全な構造体で取得
- DMA操作を安全にラップ

## Phase 2: OpenXR SDK Rustラッパー

### 2.1 型安全なOpenXRラッパー

**現状**: bindgenで生成された生のC bindingsを直接使用

**実装**:

- **新ファイル**: `codex-rs/vr-runtime/src/openxr_safe.rs`
- **内容**:
- `XrInstance`を`OpenXrInstance`構造体でラップ（`Drop`トレイトで自動クリーンアップ）
- `XrSession`を`OpenXrSession`構造体でラップ
- エラーコードを`OpenXrError` enumに変換
- リソース管理をRAIIパターンで実装
- `unsafe`ブロックを最小化し、公開APIはすべて`unsafe`なし

**変更ファイル**:

- `codex-rs/vr-runtime/src/openxr_impl.rs` - 安全なラッパーを使用するように変更

### 2.2 OpenXR拡張機能の型安全ラッパー

**実装**:

- **新ファイル**: `codex-rs/vr-runtime/src/openxr_extensions.rs`
- **内容**:
- `XR_EXT_hand_tracking`を`HandTrackingExtension`でラップ
- `XR_KHR_vulkan_enable2`を`VulkanExtension`でラップ
- 拡張機能の有効性を型システムで保証

## Phase 3: Metal API Rustラッパー

### 3.1 型安全なMetalラッパー

**現状**: objc crateでObjective-C APIを直接呼び出し

**実装**:

- **新ファイル**: `codex-rs/metal-runtime/src/metal_safe.rs`
- **内容**:
- `MTLDevice`を`MetalDevice`構造体でラップ
- `MTLCommandQueue`を`MetalCommandQueue`構造体でラップ
- メモリ管理をRustの所有権システムで管理
- エラーハンドリングを`Result`型に変換
- `unsafe`ブロックを最小化

**変更ファイル**:

- `codex-rs/metal-runtime/src/metal_impl.rs` - 安全なラッパーを使用するように変更

### 3.2 Metal Performance Shadersラッパー

**実装**:

- **新ファイル**: `codex-rs/metal-runtime/src/mps_safe.rs`
- **内容**:
- MPS操作を型安全なAPIでラップ
- 行列演算、ニューラルネットワーク推論を安全に実装

## Phase 4: 統合とテスト

### 4.1 FFIラッパーの統合テスト

**新ファイル**: `codex-rs/windows-ai/tests/kernel_driver_ffi_test.rs`
**新ファイル**: `codex-rs/linux-ai-scheduler/tests/scheduler_ffi_test.rs`

### 4.2 ドキュメント更新

**変更ファイル**: `docs/rust-migration.md` - Rust化の進捗とベストプラクティスを文書化

## 技術的考慮事項

- **unsafe使用**: 最小限に抑え、すべての`unsafe`ブロックにコメントで理由を記載
- **エラーハンドリング**: CのエラーコードをRustの`Result`型に変換
- **メモリ安全性**: RAIIパターンでリソース管理、`Drop`トレイトで自動クリーンアップ
- **型安全性**: Cの`void*`をRustの型システムで表現
- **パフォーマンス**: ゼロコスト抽象化を維持

## 完了条件

- [ ] Windows AI Driver FFIラッパー実装完了
- [ ] Linux AI Scheduler FFIラッパー実装完了
- [ ] Linux AI Memory FFIラッパー実装完了
- [ ] Linux AI GPU FFIラッパー実装完了
- [ ] OpenXR SDK Rustラッパー実装完了
- [ ] Metal API Rustラッパー実装完了
- [ ] すべての`unsafe`ブロックにドキュメント追加
- [ ] 統合テスト通過
- [ ] ドキュメント更新完了

### To-dos

- [ ] Windows AI Driver FFIラッパー実装（IOCTL呼び出し、エラーハンドリング、RAII）
- [ ] Linux AI Scheduler FFIラッパー実装（/proc/ai_scheduler、型安全なI/O）
- [ ] Linux AI Memory FFIラッパー実装（/proc/ai_memory、メモリ統計）
- [ ] Linux AI GPU FFIラッパー実装（/proc/ai_gpu、GPU統計、DMA操作）
- [ ] OpenXR SDK型安全ラッパー実装（XrInstance/Session、エラーハンドリング、RAII）
- [ ] OpenXR拡張機能型安全ラッパー実装（HandTracking、Vulkan）
- [ ] Metal API型安全ラッパー実装（MTLDevice/CommandQueue、メモリ管理）
- [ ] Metal Performance Shaders型安全ラッパー実装
- [ ] FFIラッパーの統合テスト作成
- [ ] Rust化の進捗とベストプラクティスを文書化