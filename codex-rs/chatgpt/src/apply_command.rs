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
    let mut cmd = tokio::process::Command::new("git");
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let toplevel_output = cmd
        .args(vec!["rev-parse", "--show-toplevel"])
        .output()
        .await?;

    if !toplevel_output.status.success() {
        anyhow::bail!("apply must be run from a git repository.");
    }

    let repo_root = String::from_utf8(toplevel_output.stdout)?
        .trim()
        .to_string();

    let mut git_apply_cmd = tokio::process::Command::new("git")
        .args(vec!["apply", "--3way"])
        .current_dir(&repo_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = git_apply_cmd.stdin.take() {
        tokio::io::AsyncWriteExt::write_all(&mut stdin, diff.as_bytes()).await?;
        drop(stdin);
    }

    let output = git_apply_cmd.wait_with_output().await?;

    if !output.status.success() {
        anyhow::bail!(
            "Git apply failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("Successfully applied diff");
    Ok(())
}
