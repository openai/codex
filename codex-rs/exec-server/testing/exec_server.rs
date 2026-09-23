//! Minimal exec-server fixture for Bazel-only integration tests.
//!
//! Linking only exec-server avoids depending on the full Codex CLI binary
//! when a test only needs a WebSocket executor endpoint. It handles the arg0
//! helper mode because sandboxed process requests re-exec this binary.

use codex_exec_server::ExecServerRuntimePaths;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use std::ffi::OsStr;

const CODEX_LINUX_SANDBOX_EXE_ENV_VAR: &str = "CODEX_TEST_LINUX_SANDBOX_EXE";

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(target_os = "linux")]
    codex_utils_pty::init_spawn_helper(std::env::args_os());
    let mut args = std::env::args_os();
    let _ = args.next();
    let argv1 = args.next();
    #[cfg(unix)]
    if argv1.as_deref() == Some(OsStr::new(codex_exec_server::CODEX_ARG0_EXEC_HELPER_ARG1)) {
        codex_exec_server::run_arg0_exec_helper_main();
    }
    if argv1.as_deref() == Some(OsStr::new(codex_exec_server::CODEX_FS_HELPER_ARG1)) {
        codex_exec_server::run_fs_helper_main();
    }

    let current_exe = std::env::current_exe()?;
    let codex_linux_sandbox_exe =
        std::env::var_os(CODEX_LINUX_SANDBOX_EXE_ENV_VAR).map(std::path::PathBuf::from);
    let runtime_paths = ExecServerRuntimePaths::new(current_exe, codex_linux_sandbox_exe)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(codex_exec_server::run_main(
            "ws://127.0.0.1:0",
            runtime_paths,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ))
}
