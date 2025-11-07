<!-- 8053b5ba-2749-440b-ae9c-c135c31d43b4 9b3fdfa1-b757-4312-b7c1-b2744a7292b8 -->
# Windows AI Module - Best Practices Implementation

## Phase 1: Feature Flags修正

### 1.1 Cargo.toml完全なfeature list

`codex-rs/windows-ai/Cargo.toml`

必要なWindows features:

- `Win32_Storage_FileSystem` - CreateFileW
- `Win32_System_IO` - DeviceIoControl
- `Win32_Graphics_Direct3D12` - DirectML
- `Win32_AI_MachineLearning_DirectML` - DirectML統合

### 1.2 正しいimport構文

`codex-rs/windows-ai/src/lib.rs`

```rust
use windows::Win32::Storage::FileSystem::{
    CreateFileW, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_NONE
};
```

## Phase 2: エラーハンドリング強化

### 2.1 Windows Result型の正しい使用

- `windows::core::Result`を使用
- `?`演算子で伝播
- anyhow::Contextでコンテキスト追加

### 2.2 HRESULT処理

```rust
match hr {
    Ok(value) => Ok(value),
    Err(e) => Err(anyhow::anyhow!("Windows AI error: {:?}", e)),
}
```

## Phase 3: DirectML統合

### 3.1 DirectML Device作成

`codex-rs/windows-ai/src/ml.rs`

- GPU選択（RTX 3080優先）
- Command queue作成
- Operator初期化

### 3.2 Tensor処理

- Input tensor準備
- Model inference
- Output tensor取得

## Phase 4: カーネルドライバー通信

### 4.1 IOCTL定義

`codex-rs/windows-ai/src/lib.rs`

```rust
const IOCTL_AI_GPU_STATUS: u32 = CTL_CODE(
    FILE_DEVICE_UNKNOWN,
    0x800,
    METHOD_BUFFERED,
    FILE_ANY_ACCESS
);
```

### 4.2 DeviceIoControl正しい使用

```rust
use windows::Win32::System::IO::DeviceIoControl;

unsafe {
    DeviceIoControl(
        handle,
        IOCTL_AI_GPU_STATUS,
        Some(&input as *const _ as *const c_void),
        size_of_val(&input) as u32,
        Some(&mut output as *mut _ as *mut c_void),
        size_of_val(&mut output) as u32,
        Some(&mut bytes_returned),
        None,
    )?;
}
```

## Phase 5: Safe Rust Wrapper

### 5.1 RAII Handle管理

`codex-rs/windows-ai/src/handle.rs` (新規)

```rust
pub struct SafeHandle(HANDLE);

impl Drop for SafeHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0); }
    }
}
```

### 5.2 型安全なAPI

公開APIは全てSafe Rust:

- unsafeは内部実装のみ
- Publicメソッドは全てSafe

## Phase 6: テスト実装

### 6.1 Unit tests

`codex-rs/windows-ai/src/lib.rs`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_availability_check() {
        // Windows 11以上でのみテスト実行
    }
}
```

### 6.2 Integration tests

- カーネルドライバー通信テスト
- DirectML inference test
- GPU stats取得テスト

## Phase 7: ドキュメント追加

`codex-rs/windows-ai/README.md`

- API使用例
- セットアップ手順
- トラブルシューティング
- ベストプラクティス

---

## 実装ファイル

1. `codex-rs/windows-ai/Cargo.toml` - feature flags完全化
2. `codex-rs/windows-ai/src/lib.rs` - import修正、IOCTL定義
3. `codex-rs/windows-ai/src/ml.rs` - DirectML実装
4. `codex-rs/windows-ai/src/handle.rs` - RAII wrapper (新規)
5. `codex-rs/windows-ai/src/windows_impl.rs` - 実装修正
6. `codex-rs/windows-ai/README.md` - ドキュメント (新規)
7. `codex-rs/core/src/windows_ai_integration.rs` - 条件付きコンパイル
8. `codex-rs/core/Cargo.toml` - optional dependency化

---

## ベストプラクティス

- Safe Rust公開API
- RAII pattern for handles
- 正しいfeature gating
- 詳細なエラーメッセージ
- 包括的テスト
- ドキュメント完備