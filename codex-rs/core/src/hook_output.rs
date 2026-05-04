use std::io::ErrorKind;
use std::time::Duration;
use std::time::SystemTime;

use crate::StateDbHandle;
use crate::rollout::list::find_thread_path_by_id_str;
use anyhow::Result;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::formatted_truncate_text;
use tokio::fs;
use tracing::warn;
use uuid::Uuid;

const HOOK_OUTPUTS_DIR: &str = "hook_outputs";
const HOOK_OUTPUT_TOKEN_LIMIT: usize = 2_500;
const HOOK_OUTPUT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 3);

/// Keeps hook text within the model-visible hook-output budget.
///
/// Oversized text is written in full under `$CODEX_HOME/hook_outputs/<thread_id>/`
/// and replaced with the same head/tail preview style used for other truncated
/// output, plus a path back to the preserved full text.
pub(crate) async fn cap_model_visible_hook_text(
    codex_home: &AbsolutePathBuf,
    thread_id: ThreadId,
    text: String,
    state_db: Option<StateDbHandle>,
) -> String {
    if approx_token_count(&text) <= HOOK_OUTPUT_TOKEN_LIMIT {
        return text;
    }

    let path = hook_output_path(codex_home, thread_id);
    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent.as_ref()).await
    {
        warn!(
            "failed to create hook output directory {}: {err}",
            parent.display()
        );
        return formatted_truncate_text(&text, TruncationPolicy::Tokens(HOOK_OUTPUT_TOKEN_LIMIT));
    }

    if let Err(err) = fs::write(path.as_ref(), &text).await {
        warn!("failed to write hook output {}: {err}", path.display());
        return formatted_truncate_text(&text, TruncationPolicy::Tokens(HOOK_OUTPUT_TOKEN_LIMIT));
    }

    let cleanup_codex_home = codex_home.clone();
    tokio::spawn(async move {
        if let Err(err) = cleanup_stale_hook_outputs(&cleanup_codex_home, thread_id, state_db).await
        {
            warn!("failed to clean up hook outputs: {err:?}");
        }
    });
    spilled_hook_output_preview(&text, &path)
}

fn hook_output_path(codex_home: &AbsolutePathBuf, thread_id: ThreadId) -> AbsolutePathBuf {
    codex_home
        .join(HOOK_OUTPUTS_DIR)
        .join(thread_id.to_string())
        .join(format!("{}.txt", Uuid::new_v4()))
}

/// Builds the model-visible replacement for a spilled hook output.
///
/// The path footer is budgeted before truncation so adding the recovery path
/// does not let the preview grow past the hook-output limit.
fn spilled_hook_output_preview(text: &str, path: &AbsolutePathBuf) -> String {
    let footer = format!("\n\nFull hook output saved to: {}", path.display());
    let preview_policy = TruncationPolicy::Tokens(
        HOOK_OUTPUT_TOKEN_LIMIT.saturating_sub(approx_token_count(&footer)),
    );
    format!("{}{footer}", formatted_truncate_text(text, preview_policy))
}

/// Removes hook-output directories whose threads are no longer live enough to retain.
///
/// A thread keeps its spilled outputs while it is active or while its rollout has
/// been updated within the retention window. Directories without a matching
/// rollout are treated as orphaned artifacts and removed.
pub(crate) async fn cleanup_stale_hook_outputs(
    codex_home: &AbsolutePathBuf,
    active_thread_id: ThreadId,
    state_db: Option<StateDbHandle>,
) -> Result<()> {
    let hook_outputs_dir = codex_home.join(HOOK_OUTPUTS_DIR);
    let mut entries = match fs::read_dir(hook_outputs_dir.as_ref()).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let now = SystemTime::now();
    let active_thread_id = active_thread_id.to_string();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !entry.file_type().await?.is_dir() {
            remove_hook_output_path(&path).await;
            continue;
        }

        let thread_id = entry.file_name();
        let thread_id = thread_id.to_string_lossy();
        if thread_id == active_thread_id {
            continue;
        }

        let rollout_path =
            find_thread_path_by_id_str(codex_home, &thread_id, state_db.as_deref()).await?;
        let Some(rollout_path) = rollout_path else {
            remove_hook_output_dir(&path).await;
            continue;
        };

        let modified = match fs::metadata(&rollout_path)
            .await
            .and_then(|metadata| metadata.modified())
        {
            Ok(modified) => modified,
            Err(err) => {
                warn!(
                    "failed to check rollout age for hook outputs {}: {err:?}",
                    path.display()
                );
                continue;
            }
        };

        if now
            .duration_since(modified)
            .ok()
            .is_some_and(|age| age >= HOOK_OUTPUT_RETENTION)
        {
            remove_hook_output_dir(&path).await;
        }
    }

    Ok(())
}

async fn remove_hook_output_dir(path: &std::path::Path) {
    if let Err(err) = fs::remove_dir_all(path).await {
        warn!(
            "failed to delete hook output directory {}: {err:?}",
            path.display()
        );
    }
}

async fn remove_hook_output_path(path: &std::path::Path) {
    if let Err(err) = fs::remove_file(path).await {
        warn!(
            "failed to delete hook output path {}: {err:?}",
            path.display()
        );
    }
}

#[cfg(test)]
#[path = "hook_output_tests.rs"]
mod tests;
