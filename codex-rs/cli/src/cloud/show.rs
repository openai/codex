use anyhow::Context;
use anyhow::Result;
use clap::Args;
use codex_cloud_tasks_client::TaskId;

use super::context::CloudContext;
use super::helpers::filter_variants;
use super::helpers::gather_variants;
use super::helpers::print_show_output;
use super::types::ShowOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Task identifier to show.
    pub task_id: String,

    /// Variant index (1-based) to focus on. Defaults to 1 (the active attempt).
    #[arg(long)]
    pub variant: Option<usize>,

    /// Show all captured variants instead of just the active attempt.
    #[arg(long)]
    pub all: bool,

    /// Print JSON instead of a textual summary.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(ctx: &mut CloudContext, args: &ShowArgs) -> Result<()> {
    let backend = ctx.backend();
    let task_id = TaskId(args.task_id.clone());
    let task_text = backend
        .get_task_text(task_id.clone())
        .await
        .context("failed to fetch task text")?;

    let diff_opt = backend
        .get_task_diff(task_id.clone())
        .await
        .context("failed to fetch task diff")?;

    let variants = gather_variants(ctx, &task_id, &task_text, diff_opt).await?;
    let variants = filter_variants(variants, args.variant, args.all)?;
    let output = ShowOutput {
        task_id: args.task_id.clone(),
        variants,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print_show_output(&output);
    }
    Ok(())
}
