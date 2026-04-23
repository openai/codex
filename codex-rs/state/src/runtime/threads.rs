use super::*;
use crate::SortDirection;
use codex_protocol::protocol::SessionSource;
use std::sync::atomic::Ordering;

impl StateRuntime {
    pub async fn get_thread(&self, id: ThreadId) -> anyhow::Result<Option<crate::ThreadMetadata>> {
        let row = sqlx::query(
            r#"
SELECT
    threads.id,
    threads.rollout_path,
    threads.created_at_ms AS created_at,
    threads.updated_at_ms AS updated_at,
    threads.source,
    threads.agent_nickname,
    threads.agent_role,
    threads.agent_path,
    threads.model_provider,
    threads.model,
    threads.reasoning_effort,
    threads.cwd,
    threads.cli_version,
    threads.title,
    threads.sandbox_policy,
    threads.approval_mode,
    threads.tokens_used,
    threads.first_user_message,
    threads.archived_at,
    threads.git_sha,
    threads.git_branch,
    threads.git_origin_url
FROM threads
WHERE threads.id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .transpose()
    }

    pub async fn get_thread_memory_mode(&self, id: ThreadId) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT memory_mode FROM threads WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(self.pool.as_ref())
            .await?;
        Ok(row.and_then(|row| row.try_get("memory_mode").ok()))
    }

    /// Get dynamic tools for a thread, if present.
    pub async fn get_dynamic_tools(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Vec<DynamicToolSpec>>> {
        let rows = sqlx::query(
            r#"
SELECT namespace, name, description, input_schema, defer_loading
FROM thread_dynamic_tools
WHERE thread_id = ?
ORDER BY position ASC
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let mut tools = Vec::with_capacity(rows.len());
        for row in rows {
            let input_schema: String = row.try_get("input_schema")?;
            let input_schema = serde_json::from_str::<Value>(input_schema.as_str())?;
            tools.push(DynamicToolSpec {
                namespace: row.try_get("namespace")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                input_schema,
                defer_loading: row.try_get("defer_loading")?,
            });
        }
        Ok(Some(tools))
    }

    /// Persist or replace the directional parent-child edge for a spawned thread.
    pub async fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: crate::DirectionalThreadSpawnEdgeStatus,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO thread_spawn_edges (
    parent_thread_id,
    child_thread_id,
    status
) VALUES (?, ?, ?)
ON CONFLICT(child_thread_id) DO UPDATE SET
    parent_thread_id = excluded.parent_thread_id,
    status = excluded.status
            "#,
        )
        .bind(parent_thread_id.to_string())
        .bind(child_thread_id.to_string())
        .bind(status.as_ref())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Update the persisted lifecycle status of a spawned thread's incoming edge.
    pub async fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: crate::DirectionalThreadSpawnEdgeStatus,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE thread_spawn_edges SET status = ? WHERE child_thread_id = ?")
            .bind(status.as_ref())
            .bind(child_thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    /// List direct spawned children of `parent_thread_id` whose edge matches `status`.
    pub async fn list_thread_spawn_children_with_status(
        &self,
        parent_thread_id: ThreadId,
        status: crate::DirectionalThreadSpawnEdgeStatus,
    ) -> anyhow::Result<Vec<ThreadId>> {
        self.list_thread_spawn_children_matching(parent_thread_id, Some(status))
            .await
    }

    /// List spawned descendants of `root_thread_id` whose edges match `status`.
    ///
    /// Descendants are returned breadth-first by depth, then by thread id for stable ordering.
    pub async fn list_thread_spawn_descendants_with_status(
        &self,
        root_thread_id: ThreadId,
        status: crate::DirectionalThreadSpawnEdgeStatus,
    ) -> anyhow::Result<Vec<ThreadId>> {
        self.list_thread_spawn_descendants_matching(root_thread_id, Some(status))
            .await
    }

    /// List all spawned descendants of `root_thread_id`.
    ///
    /// Descendants are returned breadth-first by depth, then by thread id for stable ordering.
    pub async fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
    ) -> anyhow::Result<Vec<ThreadId>> {
        self.list_thread_spawn_descendants_matching(root_thread_id, /*status*/ None)
            .await
    }

    /// Find a direct spawned child of `parent_thread_id` by canonical agent path.
    pub async fn find_thread_spawn_child_by_path(
        &self,
        parent_thread_id: ThreadId,
        agent_path: &str,
    ) -> anyhow::Result<Option<ThreadId>> {
        let rows = sqlx::query(
            r#"
SELECT threads.id
FROM thread_spawn_edges
JOIN threads ON threads.id = thread_spawn_edges.child_thread_id
WHERE thread_spawn_edges.parent_thread_id = ?
  AND threads.agent_path = ?
ORDER BY threads.id
LIMIT 2
            "#,
        )
        .bind(parent_thread_id.to_string())
        .bind(agent_path)
        .fetch_all(self.pool.as_ref())
        .await?;
        one_thread_id_from_rows(rows, agent_path)
    }

    /// Find a spawned descendant of `root_thread_id` by canonical agent path.
    pub async fn find_thread_spawn_descendant_by_path(
        &self,
        root_thread_id: ThreadId,
        agent_path: &str,
    ) -> anyhow::Result<Option<ThreadId>> {
        let rows = sqlx::query(
            r#"
WITH RECURSIVE subtree(child_thread_id) AS (
    SELECT child_thread_id
    FROM thread_spawn_edges
    WHERE parent_thread_id = ?
    UNION ALL
    SELECT edge.child_thread_id
    FROM thread_spawn_edges AS edge
    JOIN subtree ON edge.parent_thread_id = subtree.child_thread_id
)
SELECT threads.id
FROM subtree
JOIN threads ON threads.id = subtree.child_thread_id
WHERE threads.agent_path = ?
ORDER BY threads.id
LIMIT 2
            "#,
        )
        .bind(root_thread_id.to_string())
        .bind(agent_path)
        .fetch_all(self.pool.as_ref())
        .await?;
        one_thread_id_from_rows(rows, agent_path)
    }

    async fn list_thread_spawn_children_matching(
        &self,
        parent_thread_id: ThreadId,
        status: Option<crate::DirectionalThreadSpawnEdgeStatus>,
    ) -> anyhow::Result<Vec<ThreadId>> {
        let mut query = String::from(
            "SELECT child_thread_id FROM thread_spawn_edges WHERE parent_thread_id = ?",
        );
        if status.is_some() {
            query.push_str(" AND status = ?");
        }
        query.push_str(" ORDER BY child_thread_id");

        let mut sql = sqlx::query(query.as_str()).bind(parent_thread_id.to_string());
        if let Some(status) = status {
            sql = sql.bind(status.to_string());
        }

        let rows = sql.fetch_all(self.pool.as_ref()).await?;
        rows.into_iter()
            .map(|row| {
                ThreadId::try_from(row.try_get::<String, _>("child_thread_id")?).map_err(Into::into)
            })
            .collect()
    }

    async fn list_thread_spawn_descendants_matching(
        &self,
        root_thread_id: ThreadId,
        status: Option<crate::DirectionalThreadSpawnEdgeStatus>,
    ) -> anyhow::Result<Vec<ThreadId>> {
        let status_filter = if status.is_some() {
            " AND status = ?"
        } else {
            ""
        };
        let query = format!(
            r#"
WITH RECURSIVE subtree(child_thread_id, depth) AS (
    SELECT child_thread_id, 1
    FROM thread_spawn_edges
    WHERE parent_thread_id = ?{status_filter}
    UNION ALL
    SELECT edge.child_thread_id, subtree.depth + 1
    FROM thread_spawn_edges AS edge
    JOIN subtree ON edge.parent_thread_id = subtree.child_thread_id
    WHERE 1 = 1{status_filter}
)
SELECT child_thread_id
FROM subtree
ORDER BY depth ASC, child_thread_id ASC
            "#
        );

        let mut sql = sqlx::query(query.as_str()).bind(root_thread_id.to_string());
        if let Some(status) = status {
            let status = status.to_string();
            sql = sql.bind(status.clone()).bind(status);
        }

        let rows = sql.fetch_all(self.pool.as_ref()).await?;
        rows.into_iter()
            .map(|row| {
                ThreadId::try_from(row.try_get::<String, _>("child_thread_id")?).map_err(Into::into)
            })
            .collect()
    }

    async fn insert_thread_spawn_edge_if_absent(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO thread_spawn_edges (
    parent_thread_id,
    child_thread_id,
    status
) VALUES (?, ?, ?)
ON CONFLICT(child_thread_id) DO NOTHING
            "#,
        )
        .bind(parent_thread_id.to_string())
        .bind(child_thread_id.to_string())
        .bind(crate::DirectionalThreadSpawnEdgeStatus::Open.as_ref())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    async fn insert_thread_spawn_edge_from_source_if_absent(
        &self,
        child_thread_id: ThreadId,
        source: &str,
    ) -> anyhow::Result<()> {
        let Some(parent_thread_id) = thread_spawn_parent_thread_id_from_source_str(source) else {
            return Ok(());
        };
        self.insert_thread_spawn_edge_if_absent(parent_thread_id, child_thread_id)
            .await
    }

    /// Find a rollout path by thread id using the underlying database.
    pub async fn find_rollout_path_by_id(
        &self,
        id: ThreadId,
        archived_only: Option<bool>,
    ) -> anyhow::Result<Option<PathBuf>> {
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT rollout_path FROM threads WHERE id = ");
        builder.push_bind(id.to_string());
        match archived_only {
            Some(true) => {
                builder.push(" AND archived = 1");
            }
            Some(false) => {
                builder.push(" AND archived = 0");
            }
            None => {}
        }
        let row = builder.build().fetch_optional(self.pool.as_ref()).await?;
        Ok(row
            .and_then(|r| r.try_get::<String, _>("rollout_path").ok())
            .map(PathBuf::from))
    }

    /// Find the newest thread whose user-facing title exactly matches `title`.
    #[allow(clippy::too_many_arguments)]
    pub async fn find_thread_by_exact_title(
        &self,
        title: &str,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
        cwd: Option<&Path>,
    ) -> anyhow::Result<Option<crate::ThreadMetadata>> {
        let mut builder = QueryBuilder::<Sqlite>::new("");
        push_thread_select_columns(&mut builder);
        builder.push(" FROM threads");
        push_thread_filters(
            &mut builder,
            ThreadFilterOptions {
                archived_only,
                allowed_sources,
                model_providers,
                cwd_filters: None,
                anchor: None,
                sort_key: crate::SortKey::UpdatedAt,
                sort_direction: SortDirection::Desc,
                search_term: None,
            },
        );
        builder.push(" AND threads.title = ");
        builder.push_bind(title);
        if let Some(cwd) = cwd {
            builder.push(" AND threads.cwd = ");
            builder.push_bind(cwd.display().to_string());
        }
        push_thread_order_and_limit(
            &mut builder,
            crate::SortKey::UpdatedAt,
            SortDirection::Desc,
            /*limit*/ 1,
        );

        let row = builder.build().fetch_optional(self.pool.as_ref()).await?;
        row.map(|row| ThreadRow::try_from_row(&row).and_then(crate::ThreadMetadata::try_from))
            .transpose()
    }

    /// List threads using the underlying database.
    pub async fn list_threads(
        &self,
        page_size: usize,
        filters: ThreadFilterOptions<'_>,
    ) -> anyhow::Result<crate::ThreadsPage> {
        let limit = page_size.saturating_add(1);
        let sort_key = filters.sort_key;
        let sort_direction = filters.sort_direction;

        let mut builder = QueryBuilder::<Sqlite>::new("");
        push_thread_select_columns(&mut builder);
        builder.push(" FROM threads");
        push_thread_filters(&mut builder, filters);
        push_thread_order_and_limit(&mut builder, sort_key, sort_direction, limit);

        let rows = builder.build().fetch_all(self.pool.as_ref()).await?;
        let mut items = rows
            .into_iter()
            .map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .collect::<Result<Vec<_>, _>>()?;
        let num_scanned_rows = items.len();
        let next_anchor = if items.len() > page_size {
            items.pop();
            items
                .last()
                .and_then(|item| anchor_from_item(item, sort_key))
        } else {
            None
        };
        Ok(ThreadsPage {
            items,
            next_anchor,
            num_scanned_rows,
        })
    }

    /// List thread ids using the underlying database (no rollout scanning).
    pub async fn list_thread_ids(
        &self,
        limit: usize,
        anchor: Option<&crate::Anchor>,
        sort_key: crate::SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> anyhow::Result<Vec<ThreadId>> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT threads.id FROM threads");
        push_thread_filters(
            &mut builder,
            ThreadFilterOptions {
                archived_only,
                allowed_sources,
                model_providers,
                cwd_filters: None,
                anchor,
                sort_key,
                sort_direction: SortDirection::Desc,
                search_term: None,
            },
        );
        push_thread_order_and_limit(&mut builder, sort_key, SortDirection::Desc, limit);

        let rows = builder.build().fetch_all(self.pool.as_ref()).await?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                Ok(ThreadId::try_from(id)?)
            })
            .collect()
    }

    /// Insert or replace thread metadata directly.
    pub async fn upsert_thread(&self, metadata: &crate::ThreadMetadata) -> anyhow::Result<()> {
        self.upsert_thread_with_creation_memory_mode(metadata, /*creation_memory_mode*/ None)
            .await
    }

    pub async fn insert_thread_if_absent(
        &self,
        metadata: &crate::ThreadMetadata,
    ) -> anyhow::Result<bool> {
        let updated_at = self.allocate_thread_updated_at(metadata.updated_at)?;
        let result = sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    created_at_ms,
    updated_at_ms,
    source,
    agent_nickname,
    agent_role,
    agent_path,
    model_provider,
    model,
    reasoning_effort,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url,
    memory_mode
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(metadata.id.to_string())
        .bind(metadata.rollout_path.display().to_string())
        .bind(datetime_to_epoch_seconds(metadata.created_at))
        .bind(datetime_to_epoch_seconds(updated_at))
        .bind(datetime_to_epoch_millis(metadata.created_at))
        .bind(datetime_to_epoch_millis(updated_at))
        .bind(metadata.source.as_str())
        .bind(metadata.agent_nickname.as_deref())
        .bind(metadata.agent_role.as_deref())
        .bind(metadata.agent_path.as_deref())
        .bind(metadata.model_provider.as_str())
        .bind(metadata.model.as_deref())
        .bind(
            metadata
                .reasoning_effort
                .as_ref()
                .map(crate::extract::enum_to_string),
        )
        .bind(metadata.cwd.display().to_string())
        .bind(metadata.cli_version.as_str())
        .bind(metadata.title.as_str())
        .bind(metadata.sandbox_policy.as_str())
        .bind(metadata.approval_mode.as_str())
        .bind(metadata.tokens_used)
        .bind(metadata.first_user_message.as_deref().unwrap_or_default())
        .bind(metadata.archived_at.is_some())
        .bind(metadata.archived_at.map(datetime_to_epoch_seconds))
        .bind(metadata.git_sha.as_deref())
        .bind(metadata.git_branch.as_deref())
        .bind(metadata.git_origin_url.as_deref())
        .bind("enabled")
        .execute(self.pool.as_ref())
        .await?;
        self.insert_thread_spawn_edge_from_source_if_absent(metadata.id, metadata.source.as_str())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_thread_memory_mode(
        &self,
        thread_id: ThreadId,
        memory_mode: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE threads SET memory_mode = ? WHERE id = ?")
            .bind(memory_mode)
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_thread_title(
        &self,
        thread_id: ThreadId,
        title: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE threads SET title = ? WHERE id = ?")
            .bind(title)
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn touch_thread_updated_at(
        &self,
        thread_id: ThreadId,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        let updated_at = self.allocate_thread_updated_at(updated_at)?;
        let result =
            sqlx::query("UPDATE threads SET updated_at = ?, updated_at_ms = ? WHERE id = ?")
                .bind(datetime_to_epoch_seconds(updated_at))
                .bind(datetime_to_epoch_millis(updated_at))
                .bind(thread_id.to_string())
                .execute(self.pool.as_ref())
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Allocate a persisted `updated_at` value for thread-list cursor ordering.
    ///
    /// We keep a process-local high-water mark so hot rollout writes can get unique,
    /// monotonic millisecond timestamps without querying SQLite on every update. Older
    /// backfill/repair timestamps are allowed through unchanged so historical ordering
    /// remains tied to the rollout file mtimes.
    fn allocate_thread_updated_at(
        &self,
        updated_at: DateTime<Utc>,
    ) -> anyhow::Result<DateTime<Utc>> {
        let candidate = datetime_to_epoch_millis(updated_at);
        let allocated = loop {
            let current = self.thread_updated_at_millis.load(Ordering::Relaxed);

            // New wall-clock time: advance the process-local high-water mark and use it as-is.
            if candidate > current {
                if self
                    .thread_updated_at_millis
                    .compare_exchange(current, candidate, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    break candidate;
                }
                continue;
            }

            // Older timestamps come from backfill/repair paths that preserve rollout mtimes.
            // Do not drag historical rows forward just because this process has seen newer writes.
            if candidate.saturating_add(1000) <= current {
                break candidate;
            }

            // Same hot one-second bucket as the current high-water mark. Allocate the next
            // millisecond so updated_at remains unique and cursor-orderable inside the process.
            let bumped = current.saturating_add(1);
            if self
                .thread_updated_at_millis
                .compare_exchange(current, bumped, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break bumped;
            }
        };
        epoch_millis_to_datetime(allocated)
    }

    pub async fn update_thread_git_info(
        &self,
        thread_id: ThreadId,
        git_sha: Option<Option<&str>>,
        git_branch: Option<Option<&str>>,
        git_origin_url: Option<Option<&str>>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE threads
SET
    git_sha = CASE WHEN ? THEN ? ELSE git_sha END,
    git_branch = CASE WHEN ? THEN ? ELSE git_branch END,
    git_origin_url = CASE WHEN ? THEN ? ELSE git_origin_url END
WHERE id = ?
            "#,
        )
        .bind(git_sha.is_some())
        .bind(git_sha.flatten())
        .bind(git_branch.is_some())
        .bind(git_branch.flatten())
        .bind(git_origin_url.is_some())
        .bind(git_origin_url.flatten())
        .bind(thread_id.to_string())
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn upsert_thread_with_creation_memory_mode(
        &self,
        metadata: &crate::ThreadMetadata,
        creation_memory_mode: Option<&str>,
    ) -> anyhow::Result<()> {
        let updated_at = self.allocate_thread_updated_at(metadata.updated_at)?;
        sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    created_at_ms,
    updated_at_ms,
    source,
    agent_nickname,
    agent_role,
    agent_path,
    model_provider,
    model,
    reasoning_effort,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url,
    memory_mode
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    rollout_path = excluded.rollout_path,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at,
    created_at_ms = excluded.created_at_ms,
    updated_at_ms = excluded.updated_at_ms,
    source = excluded.source,
    agent_nickname = excluded.agent_nickname,
    agent_role = excluded.agent_role,
    agent_path = excluded.agent_path,
    model_provider = excluded.model_provider,
    model = excluded.model,
    reasoning_effort = excluded.reasoning_effort,
    cwd = excluded.cwd,
    cli_version = excluded.cli_version,
    title = excluded.title,
    sandbox_policy = excluded.sandbox_policy,
    approval_mode = excluded.approval_mode,
    tokens_used = excluded.tokens_used,
    first_user_message = excluded.first_user_message,
    archived = excluded.archived,
    archived_at = excluded.archived_at,
    git_sha = excluded.git_sha,
    git_branch = excluded.git_branch,
    git_origin_url = excluded.git_origin_url
            "#,
        )
        .bind(metadata.id.to_string())
        .bind(metadata.rollout_path.display().to_string())
        .bind(datetime_to_epoch_seconds(metadata.created_at))
        .bind(datetime_to_epoch_seconds(updated_at))
        .bind(datetime_to_epoch_millis(metadata.created_at))
        .bind(datetime_to_epoch_millis(updated_at))
        .bind(metadata.source.as_str())
        .bind(metadata.agent_nickname.as_deref())
        .bind(metadata.agent_role.as_deref())
        .bind(metadata.agent_path.as_deref())
        .bind(metadata.model_provider.as_str())
        .bind(metadata.model.as_deref())
        .bind(
            metadata
                .reasoning_effort
                .as_ref()
                .map(crate::extract::enum_to_string),
        )
        .bind(metadata.cwd.display().to_string())
        .bind(metadata.cli_version.as_str())
        .bind(metadata.title.as_str())
        .bind(metadata.sandbox_policy.as_str())
        .bind(metadata.approval_mode.as_str())
        .bind(metadata.tokens_used)
        .bind(metadata.first_user_message.as_deref().unwrap_or_default())
        .bind(metadata.archived_at.is_some())
        .bind(metadata.archived_at.map(datetime_to_epoch_seconds))
        .bind(metadata.git_sha.as_deref())
        .bind(metadata.git_branch.as_deref())
        .bind(metadata.git_origin_url.as_deref())
        .bind(creation_memory_mode.unwrap_or("enabled"))
        .execute(self.pool.as_ref())
        .await?;
        self.insert_thread_spawn_edge_from_source_if_absent(metadata.id, metadata.source.as_str())
            .await?;
        Ok(())
    }

    /// Persist dynamic tools for a thread if none have been stored yet.
    ///
    /// Dynamic tools are defined at thread start and should not change afterward.
    /// This only writes the first time we see tools for a given thread.
    pub async fn persist_dynamic_tools(
        &self,
        thread_id: ThreadId,
        tools: Option<&[DynamicToolSpec]>,
    ) -> anyhow::Result<()> {
        let Some(tools) = tools else {
            return Ok(());
        };
        if tools.is_empty() {
            return Ok(());
        }
        let thread_id = thread_id.to_string();
        let mut tx = self.pool.begin().await?;
        for (idx, tool) in tools.iter().enumerate() {
            let position = i64::try_from(idx).unwrap_or(i64::MAX);
            let input_schema = serde_json::to_string(&tool.input_schema)?;
            sqlx::query(
                r#"
INSERT INTO thread_dynamic_tools (
    thread_id,
    position,
    namespace,
    name,
    description,
    input_schema,
    defer_loading
) VALUES (?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(thread_id, position) DO NOTHING
                "#,
            )
            .bind(thread_id.as_str())
            .bind(position)
            .bind(tool.namespace.as_deref())
            .bind(tool.name.as_str())
            .bind(tool.description.as_str())
            .bind(input_schema)
            .bind(tool.defer_loading)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Apply rollout items incrementally using the underlying database.
    pub async fn apply_rollout_items(
        &self,
        builder: &ThreadMetadataBuilder,
        items: &[RolloutItem],
        new_thread_memory_mode: Option<&str>,
        updated_at_override: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let existing_metadata = self.get_thread(builder.id).await?;
        let mut metadata = existing_metadata
            .clone()
            .unwrap_or_else(|| builder.build(&self.default_provider));
        metadata.rollout_path = builder.rollout_path.clone();
        for item in items {
            apply_rollout_item(&mut metadata, item, &self.default_provider);
        }
        if let Some(existing_metadata) = existing_metadata.as_ref() {
            metadata.prefer_existing_git_info(existing_metadata);
        }
        let updated_at = match updated_at_override {
            Some(updated_at) => Some(updated_at),
            None => file_modified_time_utc(builder.rollout_path.as_path()).await,
        };
        if let Some(updated_at) = updated_at {
            metadata.updated_at = updated_at;
        }
        // Keep the thread upsert before dynamic tools to satisfy the foreign key constraint:
        // thread_dynamic_tools.thread_id -> threads.id.
        let upsert_result = if existing_metadata.is_none() {
            self.upsert_thread_with_creation_memory_mode(&metadata, new_thread_memory_mode)
                .await
        } else {
            self.upsert_thread(&metadata).await
        };
        upsert_result?;
        if let Some(memory_mode) = extract_memory_mode(items)
            && let Err(err) = self
                .set_thread_memory_mode(builder.id, memory_mode.as_str())
                .await
        {
            return Err(err);
        }
        let dynamic_tools = extract_dynamic_tools(items);
        if let Some(dynamic_tools) = dynamic_tools
            && let Err(err) = self
                .persist_dynamic_tools(builder.id, dynamic_tools.as_deref())
                .await
        {
            return Err(err);
        }
        Ok(())
    }

    /// Mark a thread as archived using the underlying database.
    pub async fn mark_archived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        archived_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let Some(mut metadata) = self.get_thread(thread_id).await? else {
            return Ok(());
        };
        metadata.archived_at = Some(archived_at);
        metadata.rollout_path = rollout_path.to_path_buf();
        if let Some(updated_at) = file_modified_time_utc(rollout_path).await {
            metadata.updated_at = updated_at;
        }
        if metadata.id != thread_id {
            warn!(
                "thread id mismatch during archive: expected {thread_id}, got {}",
                metadata.id
            );
        }
        self.upsert_thread(&metadata).await
    }

    /// Mark a thread as unarchived using the underlying database.
    pub async fn mark_unarchived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
    ) -> anyhow::Result<()> {
        let Some(mut metadata) = self.get_thread(thread_id).await? else {
            return Ok(());
        };
        metadata.archived_at = None;
        metadata.rollout_path = rollout_path.to_path_buf();
        if let Some(updated_at) = file_modified_time_utc(rollout_path).await {
            metadata.updated_at = updated_at;
        }
        if metadata.id != thread_id {
            warn!(
                "thread id mismatch during unarchive: expected {thread_id}, got {}",
                metadata.id
            );
        }
        self.upsert_thread(&metadata).await
    }

    /// Delete a thread metadata row by id.
    pub async fn delete_thread(&self, thread_id: ThreadId) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM threads WHERE id = ?")
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected())
    }
}

fn one_thread_id_from_rows(
    rows: Vec<sqlx::sqlite::SqliteRow>,
    agent_path: &str,
) -> anyhow::Result<Option<ThreadId>> {
    let mut ids = rows
        .into_iter()
        .map(|row| {
            let id: String = row.try_get("id")?;
            ThreadId::try_from(id).map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    match ids.len() {
        0 => Ok(None),
        1 => Ok(ids.pop()),
        _ => Err(anyhow::anyhow!(
            "multiple agents found for canonical path `{agent_path}`"
        )),
    }
}

pub(super) fn push_thread_select_columns(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push(
        r#"
SELECT
    threads.id,
    threads.rollout_path,
    threads.created_at_ms AS created_at,
    threads.updated_at_ms AS updated_at,
    threads.source,
    threads.agent_nickname,
    threads.agent_role,
    threads.agent_path,
    threads.model_provider,
    threads.model,
    threads.reasoning_effort,
    threads.cwd,
    threads.cli_version,
    threads.title,
    threads.sandbox_policy,
    threads.approval_mode,
    threads.tokens_used,
    threads.first_user_message,
    threads.archived_at,
    threads.git_sha,
    threads.git_branch,
    threads.git_origin_url
"#,
    );
}

pub(super) fn extract_dynamic_tools(items: &[RolloutItem]) -> Option<Option<Vec<DynamicToolSpec>>> {
    items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) => Some(meta_line.meta.dynamic_tools.clone()),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

pub(super) fn extract_memory_mode(items: &[RolloutItem]) -> Option<String> {
    items.iter().rev().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) => meta_line.meta.memory_mode.clone(),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

fn thread_spawn_parent_thread_id_from_source_str(source: &str) -> Option<ThreadId> {
    let parsed_source = serde_json::from_str(source)
        .or_else(|_| serde_json::from_value::<SessionSource>(Value::String(source.to_string())));
    match parsed_source.ok() {
        Some(SessionSource::SubAgent(codex_protocol::protocol::SubAgentSource::ThreadSpawn {
            parent_thread_id,
            ..
        })) => Some(parent_thread_id),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub struct ThreadFilterOptions<'a> {
    pub archived_only: bool,
    pub allowed_sources: &'a [String],
    pub model_providers: Option<&'a [String]>,
    pub cwd_filters: Option<&'a [PathBuf]>,
    pub anchor: Option<&'a crate::Anchor>,
    pub sort_key: SortKey,
    pub sort_direction: SortDirection,
    pub search_term: Option<&'a str>,
}

pub(super) fn push_thread_filters<'a>(
    builder: &mut QueryBuilder<'a, Sqlite>,
    options: ThreadFilterOptions<'a>,
) {
    let ThreadFilterOptions {
        archived_only,
        allowed_sources,
        model_providers,
        cwd_filters,
        anchor,
        sort_key,
        sort_direction,
        search_term,
    } = options;
    builder.push(" WHERE 1 = 1");
    if archived_only {
        builder.push(" AND threads.archived = 1");
    } else {
        builder.push(" AND threads.archived = 0");
    }
    builder.push(" AND threads.first_user_message <> ''");
    if !allowed_sources.is_empty() {
        builder.push(" AND threads.source IN (");
        let mut separated = builder.separated(", ");
        for source in allowed_sources {
            separated.push_bind(source);
        }
        separated.push_unseparated(")");
    }
    if let Some(model_providers) = model_providers
        && !model_providers.is_empty()
    {
        builder.push(" AND threads.model_provider IN (");
        let mut separated = builder.separated(", ");
        for provider in model_providers {
            separated.push_bind(provider);
        }
        separated.push_unseparated(")");
    }
    match cwd_filters {
        Some([]) => {
            builder.push(" AND 1 = 0");
        }
        Some(cwd_filters) => {
            builder.push(" AND threads.cwd IN (");
            let mut separated = builder.separated(", ");
            for cwd in cwd_filters {
                separated.push_bind(cwd.display().to_string());
            }
            separated.push_unseparated(")");
        }
        None => {}
    }
    if let Some(search_term) = search_term {
        builder.push(" AND instr(threads.title, ");
        builder.push_bind(search_term);
        builder.push(") > 0");
    }
    if let Some(anchor) = anchor {
        let anchor_ts = datetime_to_epoch_millis(anchor.ts);
        let column = match sort_key {
            SortKey::CreatedAt => "threads.created_at_ms",
            SortKey::UpdatedAt => "threads.updated_at_ms",
        };
        let operator = match sort_direction {
            SortDirection::Asc => ">",
            SortDirection::Desc => "<",
        };
        builder.push(" AND (");
        builder.push(column);
        builder.push(" ");
        builder.push(operator);
        builder.push(" ");
        builder.push_bind(anchor_ts);
        builder.push(")");
    }
}

pub(super) fn push_thread_order_and_limit(
    builder: &mut QueryBuilder<'_, Sqlite>,
    sort_key: SortKey,
    sort_direction: SortDirection,
    limit: usize,
) {
    let order_column = match sort_key {
        SortKey::CreatedAt => "threads.created_at_ms",
        SortKey::UpdatedAt => "threads.updated_at_ms",
    };
    let order_direction = match sort_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    builder.push(" ORDER BY ");
    builder.push(order_column);
    builder.push(" ");
    builder.push(order_direction);
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Anchor;
    use crate::DirectionalThreadSpawnEdgeStatus;
    use crate::runtime::test_support::test_thread_metadata;
    use crate::runtime::test_support::unique_temp_dir;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::GitInfo;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[tokio::test]
    async fn upsert_thread_keeps_creation_memory_mode_for_existing_rows() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id");
        let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());

        runtime
            .upsert_thread_with_creation_memory_mode(&metadata, Some("disabled"))
            .await
            .expect("initial insert should succeed");

        let memory_mode: String =
            sqlx::query_scalar("SELECT memory_mode FROM threads WHERE id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("memory mode should be readable");
        assert_eq!(memory_mode, "disabled");

        metadata.title = "updated title".to_string();
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("upsert should succeed");

        let memory_mode: String =
            sqlx::query_scalar("SELECT memory_mode FROM threads WHERE id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("memory mode should remain readable");
        assert_eq!(memory_mode, "disabled");
    }

    #[tokio::test]
    async fn list_threads_updated_after_returns_oldest_changes_first() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let older_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread id");
        let middle_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000002").expect("valid thread id");
        let newer_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000003").expect("valid thread id");
        let older_updated_at =
            DateTime::<Utc>::from_timestamp(1_700_000_100, 0).expect("valid older timestamp");
        let newer_updated_at =
            DateTime::<Utc>::from_timestamp(1_700_000_200, 0).expect("valid newer timestamp");

        for (thread_id, updated_at) in [
            (older_id, older_updated_at),
            (newer_id, newer_updated_at),
            (middle_id, newer_updated_at),
        ] {
            let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
            metadata.updated_at = updated_at;
            metadata.first_user_message = Some("hello".to_string());
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("thread insert should succeed");
        }

        let anchor = Anchor {
            ts: older_updated_at,
        };
        let model_providers = ["test-provider".to_string()];
        let page = runtime
            .list_threads(
                /*page_size*/ 1,
                ThreadFilterOptions {
                    archived_only: false,
                    allowed_sources: &[],
                    model_providers: Some(&model_providers),
                    cwd_filters: None,
                    anchor: Some(&anchor),
                    sort_key: SortKey::UpdatedAt,
                    sort_direction: SortDirection::Asc,
                    search_term: None,
                },
            )
            .await
            .expect("list should succeed");

        let ids = page.items.iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(ids, vec![newer_id]);
        assert_eq!(
            page.next_anchor,
            Some(Anchor {
                ts: DateTime::<Utc>::from_timestamp_millis(1_700_000_200_000)
                    .expect("valid timestamp"),
            })
        );

        let page = runtime
            .list_threads(
                /*page_size*/ 1,
                ThreadFilterOptions {
                    archived_only: false,
                    allowed_sources: &[],
                    model_providers: Some(&model_providers),
                    cwd_filters: None,
                    anchor: page.next_anchor.as_ref(),
                    sort_key: SortKey::UpdatedAt,
                    sort_direction: SortDirection::Asc,
                    search_term: None,
                },
            )
            .await
            .expect("second page should succeed");

        let ids = page.items.iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(ids, vec![middle_id]);
        assert_eq!(page.next_anchor, None);
    }

    #[tokio::test]
    async fn list_threads_filters_by_cwd() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let first_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000101").expect("valid thread id");
        let second_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000102").expect("valid thread id");
        let other_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000103").expect("valid thread id");
        let first_cwd = codex_home.join("first");
        let second_cwd = codex_home.join("second");
        let other_cwd = codex_home.join("other");

        for (thread_id, cwd, updated_at) in [
            (first_id, first_cwd.clone(), 1_700_000_100),
            (second_id, second_cwd.clone(), 1_700_000_300),
            (other_id, other_cwd, 1_700_000_500),
        ] {
            let mut metadata = test_thread_metadata(&codex_home, thread_id, cwd);
            metadata.updated_at =
                DateTime::<Utc>::from_timestamp(updated_at, 0).expect("valid timestamp");
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("thread insert should succeed");
        }

        let cwd_filters = vec![first_cwd, second_cwd];
        let page = runtime
            .list_threads(
                /*page_size*/ 10,
                ThreadFilterOptions {
                    archived_only: false,
                    allowed_sources: &[],
                    model_providers: None,
                    cwd_filters: Some(cwd_filters.as_slice()),
                    anchor: None,
                    sort_key: SortKey::UpdatedAt,
                    sort_direction: SortDirection::Desc,
                    search_term: None,
                },
            )
            .await
            .expect("list should succeed");

        let ids = page.items.iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(ids, vec![second_id, first_id]);

        let page = runtime
            .list_threads(
                /*page_size*/ 10,
                ThreadFilterOptions {
                    archived_only: false,
                    allowed_sources: &[],
                    model_providers: None,
                    cwd_filters: Some(&[]),
                    anchor: None,
                    sort_key: SortKey::UpdatedAt,
                    sort_direction: SortDirection::Desc,
                    search_term: None,
                },
            )
            .await
            .expect("list with empty cwd filters should succeed");

        assert_eq!(page.items, Vec::new());
    }

    #[tokio::test]
    async fn apply_rollout_items_restores_memory_mode_from_session_meta() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000456").expect("valid thread id");
        let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let builder = ThreadMetadataBuilder::new(
            thread_id,
            metadata.rollout_path.clone(),
            metadata.created_at,
            SessionSource::Cli,
        );
        let items = vec![RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                forked_from_id: None,
                timestamp: metadata.created_at.to_rfc3339(),
                cwd: PathBuf::new(),
                originator: String::new(),
                cli_version: String::new(),
                source: SessionSource::Cli,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
                model_provider: None,
                base_instructions: None,
                dynamic_tools: None,
                memory_mode: Some("polluted".to_string()),
            },
            git: None,
        })];

        runtime
            .apply_rollout_items(
                &builder, &items, /*new_thread_memory_mode*/ None,
                /*updated_at_override*/ None,
            )
            .await
            .expect("apply_rollout_items should succeed");

        let memory_mode = runtime
            .get_thread_memory_mode(thread_id)
            .await
            .expect("memory mode should load");
        assert_eq!(memory_mode.as_deref(), Some("polluted"));
    }

    #[tokio::test]
    async fn apply_rollout_items_preserves_existing_git_branch_and_fills_missing_git_fields() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000457").expect("valid thread id");
        let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
        metadata.git_branch = Some("sqlite-branch".to_string());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let created_at = metadata.created_at.to_rfc3339();
        let builder = ThreadMetadataBuilder::new(
            thread_id,
            metadata.rollout_path.clone(),
            metadata.created_at,
            SessionSource::Cli,
        );
        let items = vec![RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                forked_from_id: None,
                timestamp: created_at,
                cwd: PathBuf::new(),
                originator: String::new(),
                cli_version: String::new(),
                source: SessionSource::Cli,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
                model_provider: None,
                base_instructions: None,
                dynamic_tools: None,
                memory_mode: None,
            },
            git: Some(GitInfo {
                commit_hash: Some(codex_git_utils::GitSha::new("rollout-sha")),
                branch: Some("rollout-branch".to_string()),
                repository_url: Some("git@example.com:openai/codex.git".to_string()),
            }),
        })];

        runtime
            .apply_rollout_items(
                &builder, &items, /*new_thread_memory_mode*/ None,
                /*updated_at_override*/ None,
            )
            .await
            .expect("apply_rollout_items should succeed");

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.git_sha.as_deref(), Some("rollout-sha"));
        assert_eq!(persisted.git_branch.as_deref(), Some("sqlite-branch"));
        assert_eq!(
            persisted.git_origin_url.as_deref(),
            Some("git@example.com:openai/codex.git")
        );
    }

    #[tokio::test]
    async fn update_thread_git_info_preserves_newer_non_git_metadata() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000789").expect("valid thread id");
        let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let updated_at = datetime_to_epoch_millis(
            DateTime::<Utc>::from_timestamp(1_700_000_100, 0).expect("timestamp"),
        );
        sqlx::query(
            "UPDATE threads SET updated_at = ?, updated_at_ms = ?, tokens_used = ?, first_user_message = ? WHERE id = ?",
        )
        .bind(updated_at / 1000)
        .bind(updated_at)
        .bind(123_i64)
        .bind("newer preview")
        .bind(thread_id.to_string())
        .execute(runtime.pool.as_ref())
        .await
        .expect("concurrent metadata write should succeed");

        let updated = runtime
            .update_thread_git_info(
                thread_id,
                Some(Some("abc123")),
                Some(Some("feature/branch")),
                Some(Some("git@example.com:openai/codex.git")),
            )
            .await
            .expect("git info update should succeed");
        assert!(updated, "git info update should touch the thread row");

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.tokens_used, 123);
        assert_eq!(
            persisted.first_user_message.as_deref(),
            Some("newer preview")
        );
        assert_eq!(datetime_to_epoch_millis(persisted.updated_at), updated_at);
        assert_eq!(persisted.git_sha.as_deref(), Some("abc123"));
        assert_eq!(persisted.git_branch.as_deref(), Some("feature/branch"));
        assert_eq!(
            persisted.git_origin_url.as_deref(),
            Some("git@example.com:openai/codex.git")
        );
    }

    #[tokio::test]
    async fn insert_thread_if_absent_preserves_existing_metadata() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000791").expect("valid thread id");

        let mut existing = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
        existing.tokens_used = 123;
        existing.first_user_message = Some("newer preview".to_string());
        existing.updated_at = DateTime::<Utc>::from_timestamp(1_700_000_100, 0).expect("timestamp");
        runtime
            .upsert_thread(&existing)
            .await
            .expect("initial upsert should succeed");

        let mut fallback = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
        fallback.tokens_used = 0;
        fallback.first_user_message = None;
        fallback.updated_at = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");

        let inserted = runtime
            .insert_thread_if_absent(&fallback)
            .await
            .expect("insert should succeed");
        assert!(!inserted, "existing rows should not be overwritten");

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.tokens_used, 123);
        assert_eq!(
            persisted.first_user_message.as_deref(),
            Some("newer preview")
        );
        assert_eq!(
            datetime_to_epoch_millis(persisted.updated_at),
            datetime_to_epoch_millis(existing.updated_at)
        );
    }

    #[tokio::test]
    async fn update_thread_git_info_can_clear_fields() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000790").expect("valid thread id");
        let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
        metadata.git_sha = Some("abc123".to_string());
        metadata.git_branch = Some("feature/branch".to_string());
        metadata.git_origin_url = Some("git@example.com:openai/codex.git".to_string());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let updated = runtime
            .update_thread_git_info(thread_id, Some(None), Some(None), Some(None))
            .await
            .expect("git info clear should succeed");
        assert!(updated, "git info clear should touch the thread row");

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.git_sha, None);
        assert_eq!(persisted.git_branch, None);
        assert_eq!(persisted.git_origin_url, None);
    }

    #[tokio::test]
    async fn touch_thread_updated_at_updates_only_updated_at() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000791").expect("valid thread id");
        let mut metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());
        metadata.title = "original title".to_string();
        metadata.first_user_message = Some("first-user-message".to_string());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let touched_at = DateTime::<Utc>::from_timestamp(1_700_001_111, 0).expect("timestamp");
        let touched = runtime
            .touch_thread_updated_at(thread_id, touched_at)
            .await
            .expect("touch should succeed");
        assert!(touched);

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.updated_at, touched_at);
        assert_eq!(persisted.title, "original title");
        assert_eq!(
            persisted.first_user_message.as_deref(),
            Some("first-user-message")
        );
    }

    #[tokio::test]
    async fn thread_updated_at_uses_unique_epoch_millis_and_reads_legacy_seconds() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let first_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000901").expect("valid thread id");
        let second_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000902").expect("valid thread id");
        let older_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000903").expect("valid thread id");
        let updated_at =
            DateTime::<Utc>::from_timestamp_millis(1_700_001_111_123).expect("timestamp millis");
        let mut first = test_thread_metadata(&codex_home, first_id, codex_home.clone());
        first.updated_at = updated_at;
        let mut second = test_thread_metadata(&codex_home, second_id, codex_home.clone());
        second.updated_at = updated_at;

        runtime
            .upsert_thread(&first)
            .await
            .expect("first upsert should succeed");
        runtime
            .upsert_thread(&second)
            .await
            .expect("second upsert should succeed");

        let first = runtime
            .get_thread(first_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        let second = runtime
            .get_thread(second_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(
            datetime_to_epoch_millis(first.updated_at),
            1_700_001_111_123
        );
        assert_eq!(
            datetime_to_epoch_millis(second.updated_at),
            1_700_001_111_124
        );
        let second_row: (i64, i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT created_at, updated_at, created_at_ms, updated_at_ms FROM threads WHERE id = ?",
        )
        .bind(second_id.to_string())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("thread timestamp row should load");
        assert_eq!(
            second_row,
            (
                datetime_to_epoch_seconds(second.created_at),
                1_700_001_111,
                Some(datetime_to_epoch_millis(second.created_at)),
                Some(1_700_001_111_124)
            )
        );

        let older_updated_at =
            DateTime::<Utc>::from_timestamp_millis(1_700_001_100_123).expect("timestamp millis");
        let mut older = test_thread_metadata(&codex_home, older_id, codex_home.clone());
        older.updated_at = older_updated_at;
        runtime
            .upsert_thread(&older)
            .await
            .expect("older upsert should succeed");
        let older = runtime
            .get_thread(older_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(
            datetime_to_epoch_millis(older.updated_at),
            1_700_001_100_123
        );

        sqlx::query("UPDATE threads SET updated_at = ? WHERE id = ?")
            .bind(1_700_001_112_i64)
            .bind(first_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .expect("legacy timestamp write should succeed");
        let legacy = runtime
            .get_thread(first_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(
            datetime_to_epoch_millis(legacy.updated_at),
            1_700_001_112_000
        );
    }

    #[tokio::test]
    async fn apply_rollout_items_uses_override_updated_at_when_provided() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000792").expect("valid thread id");
        let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.clone());

        runtime
            .upsert_thread(&metadata)
            .await
            .expect("initial upsert should succeed");

        let builder = ThreadMetadataBuilder::new(
            thread_id,
            metadata.rollout_path.clone(),
            metadata.created_at,
            SessionSource::Cli,
        );
        let items = vec![RolloutItem::EventMsg(EventMsg::TokenCount(
            codex_protocol::protocol::TokenCountEvent {
                info: Some(codex_protocol::protocol::TokenUsageInfo {
                    total_token_usage: codex_protocol::protocol::TokenUsage {
                        input_tokens: 0,
                        cached_input_tokens: 0,
                        output_tokens: 0,
                        reasoning_output_tokens: 0,
                        total_tokens: 321,
                    },
                    last_token_usage: codex_protocol::protocol::TokenUsage::default(),
                    model_context_window: None,
                }),
                rate_limits: None,
            },
        ))];
        let override_updated_at =
            DateTime::<Utc>::from_timestamp(1_700_001_234, 0).expect("timestamp");

        runtime
            .apply_rollout_items(
                &builder,
                &items,
                /*new_thread_memory_mode*/ None,
                Some(override_updated_at),
            )
            .await
            .expect("apply_rollout_items should succeed");

        let persisted = runtime
            .get_thread(thread_id)
            .await
            .expect("thread should load")
            .expect("thread should exist");
        assert_eq!(persisted.tokens_used, 321);
        assert_eq!(persisted.updated_at, override_updated_at);
    }

    #[tokio::test]
    async fn thread_spawn_edges_track_directional_status() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home, "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let parent_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000900").expect("valid thread id");
        let child_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000901").expect("valid thread id");
        let grandchild_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000902").expect("valid thread id");

        runtime
            .upsert_thread_spawn_edge(
                parent_thread_id,
                child_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("child edge insert should succeed");
        runtime
            .upsert_thread_spawn_edge(
                child_thread_id,
                grandchild_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("grandchild edge insert should succeed");

        let children = runtime
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("open child list should load");
        assert_eq!(children, vec![child_thread_id]);

        let descendants = runtime
            .list_thread_spawn_descendants_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("open descendants should load");
        assert_eq!(descendants, vec![child_thread_id, grandchild_thread_id]);

        runtime
            .set_thread_spawn_edge_status(child_thread_id, DirectionalThreadSpawnEdgeStatus::Closed)
            .await
            .expect("edge close should succeed");

        let open_children = runtime
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("open child list should load");
        assert_eq!(open_children, Vec::<ThreadId>::new());

        let closed_children = runtime
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Closed,
            )
            .await
            .expect("closed child list should load");
        assert_eq!(closed_children, vec![child_thread_id]);

        let closed_descendants = runtime
            .list_thread_spawn_descendants_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Closed,
            )
            .await
            .expect("closed descendants should load");
        assert_eq!(closed_descendants, vec![child_thread_id]);

        let open_descendants_from_child = runtime
            .list_thread_spawn_descendants_with_status(
                child_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("open descendants from child should load");
        assert_eq!(open_descendants_from_child, vec![grandchild_thread_id]);

        let all_descendants = runtime
            .list_thread_spawn_descendants(parent_thread_id)
            .await
            .expect("all descendants should load");
        assert_eq!(all_descendants, vec![child_thread_id, grandchild_thread_id]);
    }
}
