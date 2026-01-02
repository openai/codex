mod startup_update;

use clap::Parser;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_tui::Cli;
use codex_tui::run_main;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        startup_update::maybe_emit_update_status();

        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let exit_info = run_main(inner, codex_linux_sandbox_exe).await?;
        let token_usage = exit_info.token_usage;
        let total_duration_ms = exit_info.total_duration_ms;
        if !token_usage.is_zero() {
            println!("{}", codex_core::protocol::FinalOutput::from(token_usage));
        }
        if total_duration_ms > 0 {
            println!(
                "Session duration: {}",
                format_duration_ms(total_duration_ms)
            );
        }
        Ok(())
    })
}

fn format_duration_ms(millis: i64) -> String {
    if millis < 1000 {
        return format!("{millis}ms");
    }
    if millis < 60_000 {
        return format!("{:.2}s", millis as f64 / 1000.0);
    }
    let minutes = millis / 60_000;
    let seconds = (millis % 60_000) / 1000;
    format!("{minutes}m {seconds:02}s")
}
