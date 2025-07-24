use clap::Parser;
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
    codex_linux_sandbox::run_with_sandbox(|codex_linux_sandbox_exe| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let usage = run_main(inner, codex_linux_sandbox_exe)?;
        println!(
            "Token usage: total={} input={}{} output={}{}",
            usage.total_tokens,
            usage.input_tokens,
            usage
                .cached_input_tokens
                .map(|c| format!(" (cached {c})"))
                .unwrap_or_default(),
            usage.output_tokens,
            usage
                .reasoning_output_tokens
                .map(|r| format!(" (reasoning {r})"))
                .unwrap_or_default()
        );
        Ok(())
    })
}
