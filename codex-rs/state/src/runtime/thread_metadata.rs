//! Batched metadata reads for best-effort thread enrichment. Missing or invalid rows are skipped
//! independently so one unreadable thread does not hide metadata for the rest of a page.

use std::collections::HashMap;

use codex_protocol::ThreadId;
use sqlx::QueryBuilder;
use sqlx::Sqlite;

use super::StateRuntime;
use super::threads::push_thread_select_columns;
use crate::ThreadMetadata;
use crate::model::ThreadRow;

const THREAD_METADATA_BATCH_SIZE: usize = 999;

impl StateRuntime {
    /// Fetch metadata for multiple threads, omitting missing or invalid records.
    pub async fn get_threads(
        &self,
        thread_ids: &[ThreadId],
    ) -> anyhow::Result<HashMap<ThreadId, ThreadMetadata>> {
        let mut threads = HashMap::with_capacity(thread_ids.len());
        // Keep each query within SQLite's bind limit, including for large search result sets.
        for thread_ids in thread_ids.chunks(THREAD_METADATA_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Sqlite>::new("");
            push_thread_select_columns(&mut builder);
            builder.push(" FROM threads WHERE threads.id IN (");
            let mut separated = builder.separated(", ");
            for thread_id in thread_ids {
                separated.push_bind(thread_id.to_string());
            }
            separated.push_unseparated(")");

            let rows = builder.build().fetch_all(self.pool.as_ref()).await?;
            for row in rows {
                if let Ok(metadata) =
                    ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from)
                {
                    threads.insert(metadata.id, metadata);
                }
            }
        }
        Ok(threads)
    }
}

#[cfg(test)]
#[path = "thread_metadata_tests.rs"]
mod tests;
