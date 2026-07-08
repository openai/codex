//! Cargo entry point for the minimal exec-server integration-test fixture.
//!
//! This mirrors `//codex-rs/exec-server/testing:exec-server` so Cargo-backed
//! app-server integration tests can receive `CARGO_BIN_EXE_exec-server`. It
//! also handles the filesystem-helper argv mode because exec-server re-execs
//! `codex_self_exe` for sandboxed filesystem requests.

use codex_exec_server::ExecServerRuntimePaths;
use std::ffi::OsStr;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args_os();
    let _ = args.next();
    if args.next().as_deref() == Some(OsStr::new(codex_exec_server::CODEX_FS_HELPER_ARG1)) {
        codex_exec_server::run_fs_helper_main();
    }

    let current_exe = std::env::current_exe()?;
    let runtime_paths =
        ExecServerRuntimePaths::new(current_exe, /*codex_linux_sandbox_exe*/ None)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(codex_exec_server::run_main(
            "ws://127.0.0.1:0",
            runtime_paths,
        ))
}
