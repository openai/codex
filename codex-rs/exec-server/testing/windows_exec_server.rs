//! Minimal Windows exec-server fixture for cross-platform tests.
//!
//! Keeping this wrapper separate avoids depending on the full Codex binary's
//! Windows cross-build, which is not yet supported by the Bazel graph. Linking
//! only the exec-server also makes the Wine test substantially faster to
//! iterate on.

use codex_exec_server::ExecServerRuntimePaths;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::args().nth(1).as_deref() == Some(codex_exec_server::CODEX_FS_HELPER_ARG1) {
        codex_exec_server::run_fs_helper_main();
    }

    let current_exe = std::env::current_exe()?;
    // This fixture is always a Windows executable, so it neither invokes nor
    // needs the separate Linux sandbox binary.
    let runtime_paths =
        ExecServerRuntimePaths::new(current_exe, /*codex_linux_sandbox_exe*/ None)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(codex_exec_server::run_main(
        "ws://127.0.0.1:0",
        runtime_paths,
    ))
}
