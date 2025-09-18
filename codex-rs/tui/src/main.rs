use clap::Parser;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_tui::Cli;
use codex_tui::run_main;
use owo_colors::OwoColorize;
use supports_color::Stream;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let exit_info = run_main(inner, codex_linux_sandbox_exe).await?;
        let token_usage = exit_info.token_usage;
        let conversation_id = exit_info.conversation_id;
        if !token_usage.is_zero() {
            println!("{}", codex_core::protocol::FinalOutput::from(token_usage),);
            if let Some(session_id) = conversation_id {
                let command = format!("codex resume {session_id}");
                let prefix = "To continue this session, run ";
                let suffix = ".";
                if supports_color::on(Stream::Stdout).is_some() {
                    println!("{}{}{}", prefix, command.cyan(), suffix);
                } else {
                    println!("{prefix}{command}{suffix}");
                }
            }
        }
        Ok(())
    })
}
