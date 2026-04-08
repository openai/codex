use clap::Parser;

#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;

#[derive(Debug, Parser)]
struct ExecServerArgs {
    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default).
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = codex_exec_server::DEFAULT_LISTEN_URL
    )]
    listen: String,
}

fn main() -> anyhow::Result<()> {
    dispatch_linux_sandbox_arg0();

    let linux_sandbox_alias = LinuxSandboxAlias::create();
    let codex_linux_sandbox_exe = linux_sandbox_alias.as_ref().map(|alias| alias.path.clone());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let args = ExecServerArgs::parse();
        let runtime = codex_exec_server::ExecServerRuntimeConfig::new(codex_linux_sandbox_exe);
        codex_exec_server::run_main_with_runtime(&args.listen, runtime)
            .await
            .map_err(|err| anyhow::Error::msg(err.to_string()))?;
        Ok(())
    })
}

#[cfg(target_os = "linux")]
struct LinuxSandboxAlias {
    _temp_dir: tempfile::TempDir,
    path: PathBuf,
}

#[cfg(not(target_os = "linux"))]
struct LinuxSandboxAlias;

#[cfg(target_os = "linux")]
impl LinuxSandboxAlias {
    fn create() -> Option<Self> {
        let current_exe = std::env::current_exe().ok()?;
        let temp_dir = tempfile::Builder::new()
            .prefix("codex-exec-server-")
            .tempdir()
            .ok()?;
        let path = temp_dir.path().join(CODEX_LINUX_SANDBOX_ARG0);
        std::os::unix::fs::symlink(current_exe, &path).ok()?;
        Some(Self {
            _temp_dir: temp_dir,
            path,
        })
    }
}

#[cfg(not(target_os = "linux"))]
impl LinuxSandboxAlias {
    fn create() -> Option<Self> {
        None
    }
}

#[cfg(target_os = "linux")]
fn dispatch_linux_sandbox_arg0() {
    let argv0 = std::env::args_os().next().unwrap_or_default();
    let exe_name = std::path::Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if exe_name == CODEX_LINUX_SANDBOX_ARG0 {
        codex_linux_sandbox::run_main();
    }
}

#[cfg(not(target_os = "linux"))]
fn dispatch_linux_sandbox_arg0() {}
