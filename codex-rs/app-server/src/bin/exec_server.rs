//! Cargo entry point for the minimal exec-server integration-test fixture.
//!
//! This mirrors `//codex-rs/exec-server/testing:exec-server` so Cargo-backed
//! app-server integration tests can receive `CARGO_BIN_EXE_exec-server`.

use codex_exec_server::ExecServerRuntimePaths;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let current_exe = std::env::current_exe()?;
    let runtime_paths =
        ExecServerRuntimePaths::new(current_exe, /*codex_linux_sandbox_exe*/ None)?;
    codex_exec_server::run_main("ws://127.0.0.1:0", runtime_paths).await
}
