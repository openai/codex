use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use clap::Args;
use codex_cloud_tasks_client::TaskId;
use serde_json::json;

use super::context::CloudContext;
use super::helpers::filter_variants;
use super::helpers::gather_variants;

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Task identifier to export.
    pub task_id: String,

    /// Destination directory for exports. Defaults to the current working directory.
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Export a specific variant (1-based). Defaults to variant 1.
    #[arg(long)]
    pub variant: Option<usize>,

    /// Export all variants instead of just the active attempt.
    #[arg(long)]
    pub all: bool,
}

pub async fn run(ctx: &mut CloudContext, args: &ExportArgs) -> Result<()> {
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

    let base_dir = args
        .dir
        .clone()
        .unwrap_or(std::env::current_dir().context("failed to determine current directory")?);

    for variant in variants {
        let diff = variant
            .diff
            .as_ref()
            .ok_or_else(|| anyhow!("Variant {} has no diff", variant.variant_index))?;

        let dir = base_dir.join(format!("var{}", variant.variant_index));
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {dir:?}"))?;

        let patch_path = dir.join("patch.diff");
        let mut patch_file = fs::File::create(&patch_path)
            .with_context(|| format!("failed to create {patch_path:?}"))?;
        patch_file.write_all(diff.as_bytes())?;

        let report = json!({
            "task_id": args.task_id,
            "variant_index": variant.variant_index,
            "status": variant.status,
            "attempt_placement": variant.attempt_placement,
            "prompt": variant.prompt,
            "messages": variant.messages,
        });
        let report_path = dir.join("report.json");
        fs::write(&report_path, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("failed to write {report_path:?}"))?;

        println!(
            "Exported variant {} to {}",
            variant.variant_index,
            dir.display()
        );
    }

    Ok(())
}
