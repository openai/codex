use clap::Parser;
use codex_tui::Cli;
use codex_tui::run_main;

fn main() -> anyhow::Result<()> {
    codex_linux_sandbox::run_with_sandbox(|codex_linux_sandbox_exe| async move {
        let cli = Cli::parse();
        run_main(cli, codex_linux_sandbox_exe)?;
        Ok(())
    })
}
