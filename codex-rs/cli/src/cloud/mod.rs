use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use codex_common::CliConfigOverrides;

mod apply;
mod context;
mod diff;
mod export;
mod helpers;
mod list;
mod new;
mod show;
mod types;

#[derive(Debug, Parser)]
#[command(version, about = "Headless Codex Cloud commands", long_about = None)]
pub struct CloudCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub command: Option<CloudCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CloudCommand {
    /// Launch the interactive Codex Cloud TUI.
    #[command(name = "tui")]
    Tui,

    /// List Codex Cloud tasks. Shows up to 20 recent items. Reviews are hidden unless `--include-reviews`
    /// is supplied. `--env` accepts either an environment ID or the label shown in the TUI.
    List(list::ListArgs),

    /// Show task details and variants. Defaults to variant `1` (the active attempt). Use `--variant`
    /// for a specific attempt or `--all` to show every captured variant.
    Show(show::ShowArgs),

    /// Print the unified diff for a variant. Defaults to variant `1`.
    Diff(diff::DiffArgs),

    /// Export patches and reports for variants. Each variant is written to a folder like `var1/`
    /// containing both `patch.diff` and `report.json`. Defaults to variant `1`.
    Export(export::ExportArgs),

    /// Apply Cloud patches to the local workspace. Runs a preflight first; omit `--dry-run` to apply.
    /// Defaults to variant `1`, or use `--variant`/`--all` for other attempts.
    Apply(apply::ApplyArgs),

    /// Create a new Codex Cloud task. Defaults to `--base main` and `--best-of 1`. Accepts environment
    /// IDs or labels via `--env`.
    New(new::NewArgs),
}

pub async fn run(cli: CloudCli, sandbox: Option<PathBuf>) -> Result<()> {
    match cli.command {
        None | Some(CloudCommand::Tui) => {
            let tui_cli = codex_cloud_tasks::Cli {
                config_overrides: cli.config_overrides,
                command: None,
            };
            codex_cloud_tasks::run_main(tui_cli, sandbox).await
        }
        Some(command) => {
            let mut ctx = context::CloudContext::new(cli.config_overrides).await?;
            dispatch(&mut ctx, command).await
        }
    }
}

async fn dispatch(ctx: &mut context::CloudContext, command: CloudCommand) -> Result<()> {
    match command {
        CloudCommand::Tui => unreachable!("handled earlier"),
        CloudCommand::List(args) => list::run(ctx, &args).await,
        CloudCommand::Show(args) => show::run(ctx, &args).await,
        CloudCommand::Diff(args) => diff::run(ctx, &args).await,
        CloudCommand::Export(args) => export::run(ctx, &args).await,
        CloudCommand::Apply(args) => apply::run(ctx, &args).await,
        CloudCommand::New(args) => new::run(ctx, &args).await,
    }
}
