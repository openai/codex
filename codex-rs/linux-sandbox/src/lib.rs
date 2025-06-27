#[cfg(target_os = "linux")]
mod landlock;
#[cfg(target_os = "linux")]
mod linux_run_main;

#[cfg(target_os = "linux")]
pub use linux_run_main::run_main;

use std::future::Future;
use std::path::PathBuf;

/// Helper that consolidates the common boilerplate found in several Codex
/// binaries (`codex`, `codex-exec`, `codex-tui`) around dispatching to the
/// `codex-linux-sandbox` sub-command.
///
/// When the current executable is invoked through the hard-link or alias
/// named `codex-linux-sandbox` we *directly* execute [`run_main`](crate::run_main)
/// (which never returns). Otherwise we:
/// 1.  Construct a Tokio multi-thread runtime.
/// 2.  Derive the path to the current executable (so children can re-invoke
///     the sandbox) when running on Linux.
/// 3.  Execute the provided async `main_fn` inside that runtime, forwarding
///     any error.
///
/// This function eliminates duplicated code across the various `main.rs`
/// entry-points.
pub fn run_with_sandbox<F, Fut>(main_fn: F) -> anyhow::Result<()>
where
    F: FnOnce(Option<PathBuf>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    use std::path::Path;

    // Determine if we were invoked via the special alias.
    let argv0 = std::env::args().next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if exe_name == "codex-linux-sandbox" {
        // Safety: [`run_main`] never returns.
        crate::run_main();
    }

    // Regular invocation – create a Tokio runtime and execute the provided
    // async entry-point.
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let codex_linux_sandbox_exe: Option<PathBuf> = if cfg!(target_os = "linux") {
            std::env::current_exe().ok()
        } else {
            None
        };

        main_fn(codex_linux_sandbox_exe).await
    })
}

#[cfg(not(target_os = "linux"))]
pub fn run_main() -> ! {
    panic!("codex-linux-sandbox is only supported on Linux");
}
