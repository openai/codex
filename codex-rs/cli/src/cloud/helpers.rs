use std::collections::HashSet;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use chrono::DateTime;
use chrono::Utc;
use codex_cloud_tasks_client::AttemptStatus;
use codex_cloud_tasks_client::TaskId;
use codex_cloud_tasks_client::TaskStatus;
use codex_cloud_tasks_client::TaskText;
use codex_cloud_tasks_client::TurnAttempt;

use super::context::CloudContext;
use super::types::ShowOutput;
use super::types::TaskRow;
use super::types::VariantOutput;

pub fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Ready => "ready",
        TaskStatus::Applied => "applied",
        TaskStatus::Error => "error",
    }
}

fn attempt_status_label(status: AttemptStatus) -> String {
    match status {
        AttemptStatus::Pending => "pending".to_string(),
        AttemptStatus::InProgress => "in_progress".to_string(),
        AttemptStatus::Completed => "completed".to_string(),
        AttemptStatus::Failed => "failed".to_string(),
        AttemptStatus::Cancelled => "cancelled".to_string(),
        AttemptStatus::Unknown => "unknown".to_string(),
    }
}

pub fn format_datetime(ts: &DateTime<Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn display_status(row: &TaskRow) -> String {
    if row.is_review {
        format!("{} (review)", row.status)
    } else {
        row.status.clone()
    }
}

fn variants_label(row: &TaskRow) -> String {
    row.attempt_total
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub fn print_task_table(env: Option<&str>, include_reviews: bool, rows: &[TaskRow]) {
    if rows.is_empty() {
        match env {
            Some(e) => println!("No tasks found for environment {e}."),
            None => println!("No tasks found."),
        }
        return;
    }

    let id_width = rows
        .iter()
        .map(|row| row.id.0.len())
        .max()
        .unwrap_or(2)
        .max("ID".len());
    let status_width = rows
        .iter()
        .map(|row| display_status(row).len())
        .max()
        .unwrap_or("STATUS".len());
    let variants_width = rows
        .iter()
        .map(|row| variants_label(row).len())
        .max()
        .unwrap_or("VARIANTS".len())
        .max("VARIANTS".len());

    println!(
        "{:<id_width$}  {:<status_width$}  {:>variants_width$}  {:<24}  TITLE",
        "ID",
        "STATUS",
        "VARIANTS",
        "UPDATED",
        id_width = id_width,
        status_width = status_width,
        variants_width = variants_width,
    );
    println!(
        "{:-<width$}",
        "",
        width = id_width + status_width + variants_width + 24 + 8 + "TITLE".len()
    );

    for row in rows {
        let variants = variants_label(row);
        println!(
            "{:<id_width$}  {:<status_width$}  {:>variants_width$}  {:<24}  {}",
            row.id.0,
            display_status(row),
            variants,
            format_datetime(&row.updated_at),
            row.title,
            id_width = id_width,
            status_width = status_width,
            variants_width = variants_width,
        );
    }

    if let Some(label) = env {
        println!(
            "
Environment: {label}"
        );
    }
    if include_reviews {
        println!("Reviews included");
    }
    println!("Total tasks: {}", rows.len());
}

pub async fn gather_variants(
    ctx: &CloudContext,
    task_id: &TaskId,
    task_text: &TaskText,
    diff_opt: Option<String>,
) -> Result<Vec<VariantOutput>> {
    let mut variants = Vec::new();

    let base_messages = if task_text.messages.is_empty() {
        ctx.backend()
            .get_task_messages(task_id.clone())
            .await
            .unwrap_or_default()
    } else {
        task_text.messages.clone()
    };

    variants.push(VariantOutput {
        variant_index: 1,
        is_base: true,
        attempt_placement: task_text.attempt_placement,
        status: attempt_status_label(task_text.attempt_status),
        diff: diff_opt,
        messages: base_messages,
        prompt: task_text.prompt.clone(),
    });

    if let Some(turn_id) = &task_text.turn_id {
        let attempts = ctx
            .backend()
            .list_sibling_attempts(task_id.clone(), turn_id.clone())
            .await
            .context("failed to fetch sibling attempts")?;

        for (idx, attempt) in normalize_attempts(attempts).into_iter().enumerate() {
            variants.push(VariantOutput {
                variant_index: idx + 2,
                is_base: false,
                attempt_placement: attempt.attempt_placement,
                status: attempt_status_label(attempt.status),
                diff: attempt.diff.clone(),
                messages: attempt.messages.clone(),
                prompt: None,
            });
        }
    }

    Ok(variants)
}

fn normalize_attempts(mut attempts: Vec<TurnAttempt>) -> Vec<TurnAttempt> {
    let mut seen = HashSet::new();
    attempts.retain(|attempt| seen.insert(attempt.turn_id.clone()));
    attempts.sort_by(|a, b| {
        let left = a.attempt_placement.unwrap_or(i64::MAX);
        let right = b.attempt_placement.unwrap_or(i64::MAX);
        left.cmp(&right).then_with(|| a.turn_id.cmp(&b.turn_id))
    });
    attempts
}

pub fn filter_variants(
    mut variants: Vec<VariantOutput>,
    requested: Option<usize>,
    all: bool,
) -> Result<Vec<VariantOutput>> {
    if all {
        return Ok(variants);
    }
    let idx = requested.unwrap_or(1);
    variants.retain(|variant| variant.variant_index == idx);
    if variants.is_empty() {
        bail!("Variant {idx} not found");
    }
    Ok(variants)
}

pub fn print_show_output(output: &ShowOutput) {
    println!("Task {}", output.task_id);
    for variant in &output.variants {
        println!(
            "
Variant {}{} — status: {}",
            variant.variant_index,
            if variant.is_base { " (base)" } else { "" },
            variant.status
        );
        if let Some(placement) = variant.attempt_placement {
            println!("Attempt placement: {placement}");
        }
        if let Some(prompt) = &variant.prompt {
            println!(
                "
Prompt:
{prompt}
"
            );
        }
        if let Some(diff) = &variant.diff {
            println!(
                "Diff:
{diff}"
            );
        } else {
            println!("<no diff available>");
        }
        if !variant.messages.is_empty() {
            println!("Messages:");
            for (i, msg) in variant.messages.iter().enumerate() {
                println!("  [{}] {}", i + 1, msg);
            }
        }
    }
}
