use super::LocalThreadStore;
use crate::MoveThreadToSectionParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn move_thread_to_section(
    store: &LocalThreadStore,
    params: MoveThreadToSectionParams,
) -> ThreadStoreResult<()> {
    if params
        .section
        .as_deref()
        .is_some_and(|section| section.trim().is_empty())
    {
        return Err(ThreadStoreError::InvalidRequest {
            message: "section must not be empty".to_owned(),
        });
    }
    if params.section.is_none() && params.before_thread_id.is_some() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "before thread cannot be specified without a section".to_owned(),
        });
    }

    let Some(state_db) = store.state_db().await else {
        return Err(ThreadStoreError::Unsupported {
            operation: "thread/section/move",
        });
    };

    let updated = state_db
        .move_thread_to_section(
            params.thread_id,
            params.section.as_deref(),
            params.before_thread_id,
        )
        .await
        .map_err(|err| {
            let message = err.to_string();
            if message.starts_with("before thread ")
                || message.starts_with("thread ")
                || message.starts_with("section ")
            {
                ThreadStoreError::InvalidRequest { message }
            } else {
                ThreadStoreError::Internal {
                    message: format!("failed to move thread {}: {message}", params.thread_id),
                }
            }
        })?;
    if !updated {
        return Err(ThreadStoreError::ThreadNotFound {
            thread_id: params.thread_id,
        });
    }

    Ok(())
}
