use super::StateRuntime;
use codex_protocol::ThreadId;
use std::collections::HashSet;

const MAX_THREAD_SPAWN_LINEAGE_DEPTH: usize = 64;

impl StateRuntime {
    /// Find the direct spawned parent of `child_thread_id`, if present.
    pub async fn get_thread_spawn_parent(
        &self,
        child_thread_id: ThreadId,
    ) -> anyhow::Result<Option<ThreadId>> {
        sqlx::query_scalar::<_, String>(
            "SELECT parent_thread_id FROM thread_spawn_edges WHERE child_thread_id = ?",
        )
        .bind(child_thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?
        .map(|value| ThreadId::try_from(value).map_err(Into::into))
        .transpose()
    }

    /// Resolve the automation owner for `thread_id`, following persisted thread-spawn lineage.
    pub async fn automation_owner_thread_id_for_lineage(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<ThreadId>> {
        let mut current_thread_id = thread_id;
        let mut visited_thread_ids = HashSet::from([thread_id]);

        for _ in 0..MAX_THREAD_SPAWN_LINEAGE_DEPTH {
            let mut parent_thread_id_from_source = None;
            if let Some(metadata) = self.get_thread(current_thread_id).await? {
                if let Some(owner_thread_id) = metadata.automation_owner_thread_id {
                    return Ok(Some(owner_thread_id));
                }
                parent_thread_id_from_source =
                    thread_spawn_parent_thread_id_from_source_str(metadata.source.as_str());
            }

            let Some(parent_thread_id) = self
                .get_thread_spawn_parent(current_thread_id)
                .await?
                .or(parent_thread_id_from_source)
            else {
                return Ok(None);
            };
            if !visited_thread_ids.insert(parent_thread_id) {
                return Ok(None);
            }
            current_thread_id = parent_thread_id;
        }

        Ok(None)
    }

    pub(super) async fn upsert_thread_spawn_edge_from_metadata(
        &self,
        metadata: &crate::ThreadMetadata,
        previous_metadata: Option<&crate::ThreadMetadata>,
    ) -> anyhow::Result<()> {
        if let Some(parent_thread_id) =
            thread_spawn_parent_thread_id_from_source_str(metadata.source.as_str())
        {
            return self
                .upsert_thread_spawn_edge(
                    parent_thread_id,
                    metadata.id,
                    crate::DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await;
        }

        if crate::parse_persisted_session_source(metadata.source.as_str())
            .is_some_and(|session_source| session_source.is_automation())
            && let Some(owner_thread_id) = metadata.automation_owner_thread_id
        {
            return self
                .upsert_thread_spawn_edge(
                    owner_thread_id,
                    metadata.id,
                    crate::DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await;
        }

        if metadata.automation_owner_thread_id.is_some() {
            return Ok(());
        }

        if previous_metadata.is_some_and(thread_metadata_had_spawn_lineage) {
            sqlx::query("DELETE FROM thread_spawn_edges WHERE child_thread_id = ?")
                .bind(metadata.id.to_string())
                .execute(self.pool.as_ref())
                .await?;
        }
        Ok(())
    }
}

fn thread_metadata_had_spawn_lineage(metadata: &crate::ThreadMetadata) -> bool {
    metadata.automation_owner_thread_id.is_some()
        || thread_spawn_parent_thread_id_from_source_str(metadata.source.as_str()).is_some()
        || crate::parse_persisted_session_source(metadata.source.as_str())
            .is_some_and(|session_source| session_source.is_automation())
}

pub(super) fn thread_spawn_parent_thread_id_from_source_str(source: &str) -> Option<ThreadId> {
    crate::persisted_session_source_parent_thread_id(source)
}
