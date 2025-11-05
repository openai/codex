<!-- 4faf255b-c10d-4457-8fc6-18e7e40c9992 736eec3e-ff87-464f-86c1-e41027c4ce03 -->
# CUDA完全統合 - CLI AI推論 × MCP × Git可視化GPU高速化

## 概要

CUDA Runtime APIを完全統合し、以下を実現：

1. CLI AI推論のCUDA高速化（MCP経由）
2. git解析のCUDA並列化（analyze_commits等）
3. 3D/4D可視化のGPUレンダリング高速化
4. Windows AI統合との共存
5. 警告0・型エラー0保証

## アーキテクチャ

```
Codex CLI
  ├→ Windows AI API (既存統合)
  └→ CUDA Runtime (新規)
       ├→ MCP Tool (cuda_execute)
       ├→ Git Analysis (並列化)
       └→ Kernel Driver (Pinned Memory)
           ↓
         GPU (RTX 3080)
```

## 実装フェーズ

### Phase 1: CUDA Runtimeクレート作成

**新規クレート**: `codex-rs/cuda-runtime/`

**依存**: `cuda-sys`, `cudarc` または自作FFI

```rust
// Cargo.toml
[dependencies]
cudarc = { version = "0.11", features = ["driver", "nvrtc"] }
anyhow = { workspace = true }
tracing = { workspace = true }

// src/lib.rs
pub struct CudaRuntime {
    device: CudaDevice,
    stream: CudaStream,
}

impl CudaRuntime {
    pub fn new() -> Result<Self>;
    pub fn allocate_device_memory(&self, size: usize) -> Result<DevicePtr>;
    pub fn copy_to_device(&self, src: &[u8], dst: DevicePtr) -> Result<()>;
    pub fn launch_kernel(&self, kernel: &str, grid: Dim3, block: Dim3) -> Result<()>;
}
```

**ファイル**:

- `codex-rs/cuda-runtime/Cargo.toml`
- `codex-rs/cuda-runtime/src/lib.rs`
- `codex-rs/cuda-runtime/src/device.rs`
- `codex-rs/cuda-runtime/src/memory.rs`
- `codex-rs/cuda-runtime/src/kernel.rs`

### Phase 2: Git解析CUDA並列化

**変更ファイル**: `codex-rs/cli/src/git_commands.rs`

現在の実装（CPU順次処理）:

```rust
for (i, oid) in revwalk.enumerate() {
    let commit = repo.find_commit(oid)?;
    // ... 3D座標計算（遅い）
}
```

CUDA並列化:

```rust
// コミット情報をGPUに転送
let commit_data = prepare_commit_data(&commits);
cuda.copy_to_device(&commit_data, gpu_buffer)?;

// CUDA kernelで並列計算
cuda.launch_kernel("calculate_3d_positions", grid, block)?;

// 結果を取得
cuda.copy_from_device(gpu_buffer, &mut results)?;
```

**新規ファイル**:

- `codex-rs/cli/src/git_cuda.rs` - CUDA並列化実装
- `codex-rs/cuda-runtime/kernels/git_analysis.cu` - CUDAカーネル

### Phase 3: MCP経由CUDA公開

**変更ファイル**: `codex-rs/mcp-server/src/`

新しいMCP Tool追加:

```rust
// tools/cuda.rs
pub async fn cuda_execute(
    code: &str,
    input_data: Vec<f32>,
) -> Result<Vec<f32>> {
    let cuda = CudaRuntime::new()?;
    
    // JITコンパイル
    let kernel = cuda.compile_kernel(code)?;
    
    // 実行
    let output = cuda.execute_kernel(&kernel, &input_data)?;
    
    Ok(output)
}
```

**ファイル**:

- `codex-rs/mcp-server/src/tools/cuda.rs`
- `codex-rs/mcp-server/src/tools/mod.rs` (更新)

### Phase 4: CLI統合

**変更ファイル**: `codex-rs/cli/src/main.rs`

CLI引数追加:

```rust
struct MultitoolCli {
    // 既存フラグ
    #[cfg(target_os = "windows")]
    #[clap(long, global = true)]
    pub use_windows_ai: bool,
    
    // 新規フラグ
    #[clap(long, global = true)]
    pub use_cuda: bool,
    
    #[clap(long, global = true)]
    pub cuda_device: Option<i32>,
}
```

### Phase 5: Windows AI × CUDA統合

**新規ファイル**: `codex-rs/core/src/hybrid_acceleration.rs`

両方のAPIを協調動作:

```rust
pub enum AccelerationMode {
    WindowsAI,
    CUDA,
    Hybrid,  // 最速パスを自動選択
}

pub async fn execute_with_acceleration(
    prompt: &str,
    mode: AccelerationMode,
) -> Result<String> {
    match mode {
        AccelerationMode::Hybrid => {
            // Windows AI + CUDA両方使用
            // タスクに応じて最適な方を選択
        }
    }
}
```

### Phase 6: Kamui4D超えの最適化

**変更ファイル**:

- `codex-rs/cli/src/git_commands.rs` - CUDA並列化使用
- `codex-rs/tauri-gui/src/components/git/SceneVR.tsx` - GPUレンダリング最適化

**最適化ポイント**:

1. コミット座標計算: CPU → CUDA（100-1000倍高速化）
2. ヒートマップ生成: 順次 → 並列（50-100倍）
3. 3Dレンダリング: InstancedMesh最適化

### Phase 7: テスト・ベンチマーク

**新規ファイル**:

- `codex-rs/cuda-runtime/benches/git_analysis_bench.rs`
- `codex-rs/cuda-runtime/tests/integration_test.rs`

**ベンチマーク**:

```rust
#[bench]
fn bench_git_analysis_cpu(b: &mut Bencher) {
    // CPU実装: 10,000コミット解析
}

#[bench]
fn bench_git_analysis_cuda(b: &mut Bencher) {
    // CUDA実装: 同じデータ
    // 期待: 100-1000倍高速
}
```

### Phase 8: 型エラー・警告ゼロ保証

**チェックポイント**:

1. `cargo check --all-features`
2. `cargo clippy --all-features -- -D warnings`
3. `cargo build --release`

**修正対象**:

- 未使用変数削除
- 型変換の明示化
- ライフタイム注釈
- エラーハンドリング徹底

## 重要なファイル

既存（活用）:

- `codex-rs/cli/src/git_commands.rs` (320行) - git解析
- `codex-rs/mcp-server/` (2000行) - MCP実装
- `codex-rs/windows-ai/` (655行) - Windows AI統合
- `kernel-extensions/windows/ai_driver/` (2088行) - カーネルドライバー

新規作成:

- `codex-rs/cuda-runtime/` - CUDA統合（推定800行）
- `codex-rs/cli/src/git_cuda.rs` - git CUDA並列化（推定400行）
- `codex-rs/mcp-server/src/tools/cuda.rs` - MCP CUDA tool（推定200行）
- `codex-rs/core/src/hybrid_acceleration.rs` - ハイブリッド加速（推定300行）

## 期待される効果

- CLI AI推論: 10ms → 2-3ms (-70-80%)
- git解析（10,000コミット）: 5秒 → 0.05秒 (100倍)
- 3D可視化FPS: 30fps → 120fps (4倍)
- Kamui4Dとの比較: 同等以上のパフォーマンス

## 警告・エラーゼロの保証

- Rust型システムによる静的保証
- cudarc crateによる安全なFFI
- エラーハンドリング100%
- Clippy lint準拠

### To-dos

- [ ] CUDA Runtimeクレート作成（cudarc使用）
- [ ] CUDA device初期化・メモリ管理実装
- [ ] git解析CUDA並列化実装（git_cuda.rs）
- [ ] CUDAカーネル実装（git_analysis.cu相当をRustで）
- [ ] MCP CUDA tool実装（mcp-server統合）
- [ ] ハイブリッド加速レイヤー実装（Windows AI × CUDA）
- [ ] CLI統合（--use-cuda, --cuda-device引数）
- [ ] 3D/4D可視化GPU最適化
- [ ] テスト・ベンチマーク作成
- [ ] 型エラー・警告ゼロ確認（clippy, check）
- [ ] ドキュメント・実装ログ作成