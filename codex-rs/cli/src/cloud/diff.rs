use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::Args;
use codex_cloud_tasks_client::TaskId;

use super::context::CloudContext;
use super::helpers::filter_variants;
use super::helpers::gather_variants;

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Task identifier to diff.
    pub task_id: String,

    /// Variant index (1-based). Defaults to 1 (the active variant).
    #[arg(long)]
    pub variant: Option<usize>,
}

pub async fn run(ctx: &mut CloudContext, args: &DiffArgs) -> Result<()> {
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
    let mut variants = filter_variants(variants, args.variant, false)?;
    let variant = variants
        .pop()
        .ok_or_else(|| anyhow!("Variant not available"))?;

    match &variant.diff {
        Some(diff) => {
            print!("{diff}");
            if !diff.ends_with('\n') {
                println!();
            }
        }
        None => bail!("Variant {} has no diff", variant.variant_index),
    }
    Ok(())
}
