use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use clap::Args;
use codex_cloud_tasks_client::TaskId;

use super::context::CloudContext;
use super::helpers::filter_variants;
use super::helpers::gather_variants;

#[derive(Debug, Args)]
pub struct ApplyArgs {
    /// Task identifier to apply.
    pub task_id: String,

    /// Variant index (1-based). Defaults to variant 1.
    #[arg(long)]
    pub variant: Option<usize>,

    /// Apply all variants sequentially instead of only the active attempt.
    #[arg(long)]
    pub all: bool,

    /// Run preflight only without applying changes.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(ctx: &mut CloudContext, args: &ApplyArgs) -> Result<()> {
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

    for variant in variants {
        let diff = variant
            .diff
            .clone()
            .ok_or_else(|| anyhow!("Variant {} has no diff", variant.variant_index))?;

        let preflight = backend
            .apply_task_preflight(task_id.clone(), Some(diff.clone()))
            .await
            .context("preflight failed")?;
        println!(
            "Preflight (variant {}): {}",
            variant.variant_index, preflight.message
        );
        if args.dry_run {
            continue;
        }

        let outcome = backend
            .apply_task(task_id.clone(), Some(diff))
            .await
            .context("apply failed")?;
        println!(
            "Apply (variant {}): {}",
            variant.variant_index, outcome.message
        );
        if !outcome.applied {
            println!(
                "Apply status for variant {}: {:?}",
                variant.variant_index, outcome.status
            );
        }
    }

    Ok(())
}
