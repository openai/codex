use super::*;

impl StateRuntime {
    pub async fn get_thread(&self, id: ThreadId) -> anyhow::Result<Option<crate::ThreadMetadata>> {
        let row = sqlx::query(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    agent_nickname,
    agent_role,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .transpose()
    }

    /// Get dynamic tools for a thread, if present.
    pub async fn get_dynamic_tools(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Vec<DynamicToolSpec>>> {
        let rows = sqlx::query(
            r#"
SELECT name, description, input_schema
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
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                input_schema,
            });
        }
        Ok(Some(tools))
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

    /// List threads using the underlying database.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_threads(
        &self,
        page_size: usize,
        anchor: Option<&crate::Anchor>,
        sort_key: crate::SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
        search_term: Option<&str>,
    ) -> anyhow::Result<crate::ThreadsPage> {
        let limit = page_size.saturating_add(1);

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    agent_nickname,
    agent_role,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
            "#,
        );
        push_thread_filters(
            &mut builder,
            archived_only,
            allowed_sources,
            model_providers,
            anchor,
            sort_key,
            search_term,
        );
        push_thread_order_and_limit(&mut builder, sort_key, limit);

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
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id FROM threads");
        push_thread_filters(
            &mut builder,
            archived_only,
            allowed_sources,
            model_providers,
            anchor,
            sort_key,
            None,
        );
        push_thread_order_and_limit(&mut builder, sort_key, limit);

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
        sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    agent_nickname,
    agent_role,
    model_provider,
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
    git_origin_url
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    rollout_path = excluded.rollout_path,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at,
    source = excluded.source,
    agent_nickname = excluded.agent_nickname,
    agent_role = excluded.agent_role,
    model_provider = excluded.model_provider,
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
        .bind(datetime_to_epoch_seconds(metadata.updated_at))
        .bind(metadata.source.as_str())
        .bind(metadata.agent_nickname.as_deref())
        .bind(metadata.agent_role.as_deref())
        .bind(metadata.model_provider.as_str())
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
        .execute(self.pool.as_ref())
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
    name,
    description,
    input_schema
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT(thread_id, position) DO NOTHING
                "#,
            )
            .bind(thread_id.as_str())
            .bind(position)
            .bind(tool.name.as_str())
            .bind(tool.description.as_str())
            .bind(input_schema)
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
        otel: Option<&OtelManager>,
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut metadata = self
            .get_thread(builder.id)
            .await?
            .unwrap_or_else(|| builder.build(&self.default_provider));
        metadata.rollout_path = builder.rollout_path.clone();
        for item in items {
            apply_rollout_item(&mut metadata, item, &self.default_provider);
        }
        if let Some(updated_at) = file_modified_time_utc(builder.rollout_path.as_path()).await {
            metadata.updated_at = updated_at;
        }
        // Keep the thread upsert before dynamic tools to satisfy the foreign key constraint:
        // thread_dynamic_tools.thread_id -> threads.id.
        if let Err(err) = self.upsert_thread(&metadata).await {
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "apply_rollout_items")]);
            }
            return Err(err);
        }
        let dynamic_tools = extract_dynamic_tools(items);
        if let Some(dynamic_tools) = dynamic_tools
            && let Err(err) = self
                .persist_dynamic_tools(builder.id, dynamic_tools.as_deref())
                .await
        {
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "persist_dynamic_tools")]);
            }
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

pub(super) fn extract_dynamic_tools(items: &[RolloutItem]) -> Option<Option<Vec<DynamicToolSpec>>> {
    items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) => Some(meta_line.meta.dynamic_tools.clone()),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

pub(super) fn push_thread_filters<'a>(
    builder: &mut QueryBuilder<'a, Sqlite>,
    archived_only: bool,
    allowed_sources: &'a [String],
    model_providers: Option<&'a [String]>,
    anchor: Option<&crate::Anchor>,
    sort_key: SortKey,
    search_term: Option<&'a str>,
) {
    builder.push(" WHERE 1 = 1");
    if archived_only {
        builder.push(" AND archived = 1");
    } else {
        builder.push(" AND archived = 0");
    }
    builder.push(" AND first_user_message <> ''");
    if !allowed_sources.is_empty() {
        builder.push(" AND source IN (");
        let mut separated = builder.separated(", ");
        for source in allowed_sources {
            separated.push_bind(source);
        }
        separated.push_unseparated(")");
    }
    if let Some(model_providers) = model_providers
        && !model_providers.is_empty()
    {
        builder.push(" AND model_provider IN (");
        let mut separated = builder.separated(", ");
        for provider in model_providers {
            separated.push_bind(provider);
        }
        separated.push_unseparated(")");
    }
    if let Some(search_term) = search_term {
        builder.push(" AND instr(title, ");
        builder.push_bind(search_term);
        builder.push(") > 0");
    }
    if let Some(anchor) = anchor {
        let anchor_ts = datetime_to_epoch_seconds(anchor.ts);
        let column = match sort_key {
            SortKey::CreatedAt => "created_at",
            SortKey::UpdatedAt => "updated_at",
        };
        builder.push(" AND (");
        builder.push(column);
        builder.push(" < ");
        builder.push_bind(anchor_ts);
        builder.push(" OR (");
        builder.push(column);
        builder.push(" = ");
        builder.push_bind(anchor_ts);
        builder.push(" AND id < ");
        builder.push_bind(anchor.id.to_string());
        builder.push("))");
    }
}

pub(super) fn push_thread_order_and_limit(
    builder: &mut QueryBuilder<'_, Sqlite>,
    sort_key: SortKey,
    limit: usize,
) {
    let order_column = match sort_key {
        SortKey::CreatedAt => "created_at",
        SortKey::UpdatedAt => "updated_at",
    };
    builder.push(" ORDER BY ");
    builder.push(order_column);
    builder.push(" DESC, id DESC");
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);
}
