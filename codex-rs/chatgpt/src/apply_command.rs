use std::path::PathBuf;

use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;

use crate::chatgpt_token::init_chatgpt_token_from_auth;
use crate::get_task::GetTaskResponse;
use crate::get_task::OutputItem;
use crate::get_task::PrOutputItem;
use crate::get_task::get_task;

/// Applies the latest diff from a Codex agent task.
#[derive(Debug, Parser)]
pub struct ApplyCommand {
    pub task_id: String,

    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,
}
pub async fn run_apply_command(
    apply_cli: ApplyCommand,
    cwd: Option<PathBuf>,
) -> anyhow::Result<()> {
    let config = Config::load_with_cli_overrides(
        apply_cli
            .config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        ConfigOverrides::default(),
    )?;

    init_chatgpt_token_from_auth(&config.codex_home).await?;

    let task_response = get_task(&config, apply_cli.task_id).await?;
    apply_diff_from_task(task_response, cwd).await
}

pub async fn apply_diff_from_task(
    task_response: GetTaskResponse,
    cwd: Option<PathBuf>,
) -> anyhow::Result<()> {
    let diff_turn = match task_response.current_diff_task_turn {
        Some(turn) => turn,
        None => anyhow::bail!("No diff turn found"),
    };
    let output_diff = diff_turn.output_items.iter().find_map(|item| match item {
        OutputItem::Pr(PrOutputItem { output_diff }) => Some(output_diff),
        _ => None,
    });
    match output_diff {
        Some(output_diff) => apply_diff(&output_diff.diff, cwd).await,
        None => anyhow::bail!("No PR output item found"),
    }
}

async fn apply_diff(diff: &str, cwd: Option<PathBuf>) -> anyhow::Result<()> {
    let cwd = cwd.unwrap_or(std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()));
    let req = codex_git_apply::ApplyGitRequest {
        cwd,
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    };
    let res = codex_git_apply::apply_git_patch(&req)?;
    if res.exit_code != 0 {
        anyhow::bail!(
            "Git apply failed (applied={}, skipped={}, conflicts={})\nstdout:\n{}\nstderr:\n{}",
            res.applied_paths.len(),
            res.skipped_paths.len(),
            res.conflicted_paths.len(),
            res.stdout,
            res.stderr
        );
    }
    println!("Successfully applied diff");
    Ok(())
}
