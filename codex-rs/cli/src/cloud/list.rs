use anyhow::Context;
use anyhow::Result;
use clap::Args;
use codex_cloud_tasks_client::TaskSummary;

use super::context::CloudContext;
use super::helpers::print_task_table;
use super::helpers::status_label;
use super::new::resolve_env_id;
use super::types::TaskRow;

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Output JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Filter tasks to a specific environment (accepts either an ID like `env_…` or the label
    /// shown in the TUI).
    #[arg(long)]
    pub env: Option<String>,

    /// Include automated code-review tasks (hidden by default).
    #[arg(long)]
    pub include_reviews: bool,
}

pub async fn run(ctx: &mut CloudContext, args: &ListArgs) -> Result<()> {
    let backend = ctx.backend();
    let env_arg = args.env.as_deref();
    let resolved_env = if let Some(env) = env_arg {
        Some(resolve_env_id(ctx, env).await?)
    } else {
        None
    };
    let selected_env = resolved_env.as_deref().or(env_arg);

    let tasks = backend
        .list_tasks(selected_env)
        .await
        .context("failed to list Codex Cloud tasks")?;

    let rows: Vec<TaskRow> = tasks
        .into_iter()
        .filter(|task| args.include_reviews || !task.is_review)
        .map(TaskRow::from)
        .collect();

    let environment = selected_env.map(str::to_string);

    if args.json {
        let payload = serde_json::json!({
            "environment": environment,
            "include_reviews": args.include_reviews,
            "tasks": rows,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    print_task_table(selected_env, args.include_reviews, &rows);
    Ok(())
}

impl From<TaskSummary> for TaskRow {
    fn from(summary: TaskSummary) -> Self {
        Self {
            id: summary.id,
            title: summary.title,
            status: status_label(&summary.status).to_string(),
            updated_at: summary.updated_at,
            environment_label: summary.environment_label,
            files_changed: summary.summary.files_changed,
            lines_added: summary.summary.lines_added,
            lines_removed: summary.summary.lines_removed,
            is_review: summary.is_review,
        }
    }
}
