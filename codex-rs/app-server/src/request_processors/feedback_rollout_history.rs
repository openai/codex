use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_feedback::FeedbackAttachment;
use codex_feedback::FeedbackAttachmentPath;
use codex_feedback::MAX_ATTACHMENT_BYTES;
use codex_feedback::MAX_ATTACHMENTS_BYTES;
use codex_rollout::find_rollout_path_by_rollout_id;
use codex_rollout::read_session_meta_line;
use codex_rollout::rollout_id_from_path;

const MAX_HISTORY_ROLLOUTS: usize = 64;
const HISTORY_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn history_base_attachments(
    codex_home: &Path,
    attachments: &[FeedbackAttachmentPath],
) -> Vec<FeedbackAttachment> {
    let selected = attachments
        .iter()
        .filter_map(|attachment| rollout_id_from_path(&attachment.path))
        .collect::<HashSet<_>>();
    let mut pending = attachments
        .iter()
        .filter(|attachment| rollout_id_from_path(&attachment.path).is_some())
        .map(|attachment| attachment.path.clone())
        .collect::<Vec<_>>();
    let mut positions = HashMap::new();
    let mut ancestors: Vec<(PathBuf, usize)> = Vec::new();
    // Preserve discovered prefixes if the bounded filesystem walk times out.
    let discovery = async {
        let mut next = 0;
        while next < pending.len() {
            let path = &pending[next];
            next += 1;
            let meta = match read_session_meta_line(path).await {
                Ok(meta) => meta,
                Err(err) => {
                    tracing::error!(
                        ?err,
                        "failed to read feedback history metadata at {}",
                        path.display()
                    );
                    continue;
                }
            };
            let Some(base) = meta.meta.history_base else {
                continue;
            };
            if selected.contains(&base.thread_id) {
                continue;
            }
            let Ok(end_byte_offset) = usize::try_from(base.end_byte_offset) else {
                continue;
            };
            if end_byte_offset > MAX_ATTACHMENT_BYTES {
                continue;
            }
            if let Some(&index) = positions.get(&base.thread_id) {
                let (_, previous_end_byte_offset): &mut (PathBuf, usize) = &mut ancestors[index];
                *previous_end_byte_offset = (*previous_end_byte_offset).max(end_byte_offset);
                continue;
            }
            if positions.len() == MAX_HISTORY_ROLLOUTS {
                tracing::error!("feedback history rollout limit reached");
                break;
            }
            // The wire field names an immutable rollout, not the owner's current thread.
            let path = match find_rollout_path_by_rollout_id(codex_home, base.thread_id).await {
                Ok(Some(path)) => path,
                Ok(None) => continue,
                Err(err) => {
                    tracing::error!(
                        ?err,
                        "failed to locate feedback history rollout {}",
                        base.thread_id
                    );
                    continue;
                }
            };
            positions.insert(base.thread_id, ancestors.len());
            pending.push(path.clone());
            ancestors.push((path, end_byte_offset));
        }
    };
    if tokio::time::timeout(HISTORY_LOOKUP_TIMEOUT, discovery)
        .await
        .is_err()
    {
        tracing::error!("feedback history lookup timed out");
    }
    // Freeze exactly the inherited bytes before upload. Unreadable or oversized
    // ancestors are best effort and cannot turn an accepted report into an error.
    tokio::task::spawn_blocking(move || {
        let mut remaining = MAX_ATTACHMENTS_BYTES;
        let mut result = Vec::new();
        for (path, end_byte_offset) in ancestors {
            if end_byte_offset > remaining {
                continue;
            }
            let plain_path = codex_rollout::plain_rollout_path(&path);
            let Some(filename) = plain_path.file_name() else {
                continue;
            };
            match codex_rollout::read_rollout_prefix(&path, end_byte_offset) {
                Ok(Some(buffer)) if buffer.len() == end_byte_offset => {
                    remaining -= buffer.len();
                    result.push(FeedbackAttachment {
                        buffer,
                        filename: filename.to_string_lossy().into_owned(),
                        content_type: Some("application/jsonl".to_string()),
                    });
                }
                Ok(_) => tracing::error!(
                    "couldn't attach history: incomplete feedback history prefix at {}",
                    path.display()
                ),
                Err(err) => tracing::error!(
                    ?err,
                    "couldn't read feedback history prefix at {}",
                    path.display()
                ),
            }
        }
        result
    })
    .await
    .unwrap_or_default() // Keep uploading the selected rollouts if this task fails.
}

#[cfg(test)]
#[path = "feedback_rollout_history_tests.rs"]
mod tests;
