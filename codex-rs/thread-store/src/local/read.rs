use std::path::Path;

use codex_protocol::ThreadId;
use codex_rollout::RolloutRecorder;
use codex_rollout::ThreadItem;
use codex_rollout::find_thread_name_by_id;
use codex_state::ThreadMetadata;

use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

use super::LocalThreadStore;
use super::helpers::display_error;
use super::helpers::io_error;
use super::helpers::memory_mode_from_items;
use super::helpers::metadata_from_items;
use super::helpers::stored_thread_from_metadata;

impl LocalThreadStore {
    pub(crate) async fn stored_thread_from_path(
        &self,
        path: &Path,
        archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let (items, thread_id, _) = RolloutRecorder::load_rollout_items(path)
            .await
            .map_err(io_error)?;
        let thread_id = thread_id.ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: format!("rollout {} is missing thread id", path.display()),
        })?;
        let mut metadata = metadata_from_items(
            path,
            &items,
            self.config.model_provider_id.as_str(),
            archived,
        )?;
        metadata.id = thread_id;

        let state_db = self.state_db().await;
        codex_rollout::state_db::read_repair_rollout_path(
            state_db.as_deref(),
            Some(thread_id),
            Some(archived),
            path,
        )
        .await;
        if let Some(db_metadata) = match state_db.as_deref() {
            Some(ctx) => ctx.get_thread(thread_id).await.map_err(display_error)?,
            None => None,
        } {
            metadata = db_metadata;
        }

        let memory_mode = match state_db.as_deref() {
            Some(ctx) => ctx
                .get_thread_memory_mode(thread_id)
                .await
                .map_err(display_error)?,
            None => memory_mode_from_items(&items),
        };
        let name = find_thread_name_by_id(&self.config.codex_home, &thread_id)
            .await
            .map_err(io_error)?;

        Ok(stored_thread_from_metadata(
            metadata,
            Some(path.to_path_buf()),
            name,
            memory_mode,
            include_history.then_some(StoredThreadHistory { thread_id, items }),
        ))
    }

    pub(crate) async fn stored_thread_from_state_metadata(
        &self,
        metadata: ThreadMetadata,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let thread_id = metadata.id;
        let name = find_thread_name_by_id(&self.config.codex_home, &thread_id)
            .await
            .map_err(io_error)?;
        let state_db = self.state_db().await;
        let memory_mode = match state_db.as_deref() {
            Some(ctx) => ctx
                .get_thread_memory_mode(thread_id)
                .await
                .map_err(display_error)?,
            None => None,
        };
        let history = if include_history {
            let params = LoadThreadHistoryParams {
                thread_id,
                owner: Default::default(),
                include_archived: metadata.archived_at.is_some(),
            };
            Some(self.load_history(params).await?)
        } else {
            None
        };
        Ok(stored_thread_from_metadata(
            metadata.clone(),
            Some(metadata.rollout_path.clone()),
            name,
            memory_mode,
            history,
        ))
    }

    pub(crate) async fn read_after_update(
        &self,
        thread_id: ThreadId,
    ) -> ThreadStoreResult<StoredThread> {
        self.read_thread(ReadThreadParams {
            thread_id,
            owner: Default::default(),
            include_archived: true,
            include_history: false,
        })
        .await
    }

    pub(crate) async fn stored_thread_from_thread_item(
        &self,
        item: ThreadItem,
        archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        if let Some(thread_id) = item.thread_id
            && let Some(ctx) = self.state_db().await.as_deref()
            && let Some(metadata) = ctx.get_thread(thread_id).await.map_err(display_error)?
        {
            return self
                .stored_thread_from_state_metadata(metadata, /*include_history*/ false)
                .await;
        }

        self.stored_thread_from_path(
            item.path.as_path(),
            archived,
            /*include_history*/ false,
        )
        .await
    }
}
