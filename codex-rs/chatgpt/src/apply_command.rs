use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;

use crate::chatgpt_client::OutputItem;
use crate::chatgpt_client::PrOutputItem;
use crate::chatgpt_client::get_task_response;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

#[derive(Debug, Parser)]
pub struct ApplyCommand {
    pub task_id: String,

    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,
}
pub async fn run_apply_command(apply_cli: ApplyCommand) -> anyhow::Result<()> {
    let config = Config::load_with_cli_overrides(
        apply_cli
            .config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        ConfigOverrides::default(),
    )?;

    init_chatgpt_token_from_auth(&config.codex_home).await?;

    let task_response = get_task_response(&config, apply_cli.task_id).await?;
    let diff_turn = match task_response.current_assistant_turn {
        Some(turn) => turn,
        None => anyhow::bail!("No diff turn found"),
    };
    let output_diff = diff_turn.output_items.iter().find_map(|item| match item {
        OutputItem::Pr(PrOutputItem { output_diff }) => Some(output_diff),
        _ => None,
    });
    match output_diff {
        Some(output_diff) => apply_diff(&output_diff.diff).await?,
        None => anyhow::bail!("No PR output item found"),
    }

    Ok(())
}

async fn apply_diff(diff: &str) -> anyhow::Result<()> {
    let toplevel_output = tokio::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()
        .await?;

    if !toplevel_output.status.success() {
        anyhow::bail!(
            "Git rev-parse failed with status {}: {}",
            toplevel_output.status,
            String::from_utf8_lossy(&toplevel_output.stderr)
        );
    }

    let repo_root = String::from_utf8(toplevel_output.stdout)?
        .trim()
        .to_string();

    let mut git_apply_cmd = tokio::process::Command::new("git")
        .args(&["apply", "--3way"])
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

    Ok(())
}
