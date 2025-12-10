#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]

use std::path::PathBuf;

pub use codex_tui::AppExitInfo;
pub use codex_tui::Cli;
pub use codex_tui::update_action;

/// Entry point for the experimental TUI v2 crate.
///
/// Currently this is a thin shim that delegates to the existing `codex-tui`
/// implementation so behavior and rendering remain identical while the new
/// viewport is developed behind a feature toggle.
pub async fn run_main(
    cli: Cli,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<AppExitInfo> {
    #[allow(clippy::print_stdout)] // for now
    {
        println!("Note: You are running the experimental TUI v2 implementation.");
    }
    codex_tui::run_main(cli, codex_linux_sandbox_exe).await
}
