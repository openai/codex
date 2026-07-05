use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionContextWindow;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::persisted_rollout_items;
use futures::StreamExt;
use futures::TryStreamExt;
use mongodb::Client;
use mongodb::Collection;
use mongodb::IndexModel;
use mongodb::bson::Bson;
use mongodb::bson::Document;
use mongodb::bson::doc;
use mongodb::bson::to_bson;
use mongodb::options::FindOneOptions;
use mongodb::options::FindOptions;
use mongodb::options::IndexOptions;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DeleteThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::SearchThreadsParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::StoredThreadSearchResult;
use crate::ThreadMetadataPatch;
use crate::ThreadPage;
use crate::ThreadRelationFilter;
use crate::ThreadSearchPage;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreFuture;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;
use crate::error::reject_paginated_history_mode;
use crate::mongodb_blob::ExternalItemField;
use crate::mongodb_blob::clear_blob_dir;
use crate::mongodb_blob::externalize_rollout_item;
use crate::mongodb_blob::hydrate_rollout_item;
use crate::mongodb_blob::remove_blob;
use crate::thread_metadata_sync::user_message_preview;
use crate::types::canonical_history_mode_from_rollout_items;

pub const DEFAULT_MONGODB_URI: &str = "mongodb://127.0.0.1:27017";

#[derive(Clone, Debug, Default, Serialize)]
pub struct MongoMigrationReport {
    pub scanned: usize,
    pub imported: usize,
    pub skipped: usize,
    pub items: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MongoMigrationProgress {
    pub completed: usize,
    pub total: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MongoMigrationOptions {
    pub include_active: bool,
    pub include_archived: bool,
    pub dry_run: bool,
    pub verify: bool,
    pub jobs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MongoThreadStoreConfig {
    pub codex_home: PathBuf,
    pub database: String,
    pub uri_env: String,
}

impl MongoThreadStoreConfig {
    pub fn resolved_uri(&self) -> ThreadStoreResult<String> {
        match std::env::var(&self.uri_env) {
            Ok(value) if value.trim().is_empty() => Err(ThreadStoreError::InvalidRequest {
                message: format!("{} is set but empty", self.uri_env),
            }),
            Ok(value) => Ok(value),
            Err(std::env::VarError::NotPresent) => Ok(DEFAULT_MONGODB_URI.to_string()),
            Err(err) => Err(ThreadStoreError::InvalidRequest {
                message: format!("failed to read {}: {err}", self.uri_env),
            }),
        }
    }

    fn namespace(&self) -> String {
        self.codex_home
            .canonicalize()
            .unwrap_or_else(|_| self.codex_home.clone())
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ThreadDocument {
    codex_home_namespace: String,
    thread_id: String,
    archived: bool,
    stored: StoredThread,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ItemDocument {
    codex_home_namespace: String,
    thread_id: String,
    sequence: i64,
    item: RolloutItem,
    #[serde(default)]
    external_fields: Vec<ExternalItemField>,
}

pub struct MongoThreadStore {
    config: MongoThreadStoreConfig,
    namespace: String,
    client: OnceCell<Client>,
    live_threads: Mutex<HashSet<ThreadId>>,
}

impl std::fmt::Debug for MongoThreadStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MongoThreadStore")
            .field("config", &self.config)
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl MongoThreadStore {
    pub fn new(config: MongoThreadStoreConfig) -> ThreadStoreResult<Self> {
        let _ = config.resolved_uri()?;
        Ok(Self {
            namespace: config.namespace(),
            config,
            client: OnceCell::new(),
            live_threads: Mutex::new(HashSet::new()),
        })
    }

    async fn client(&self) -> ThreadStoreResult<&Client> {
        self.client
            .get_or_try_init(|| async {
                let client = Client::with_uri_str(self.config.resolved_uri()?)
                    .await
                    .map_err(mongo_error)?;
                client
                    .database(&self.config.database)
                    .run_command(doc! { "ping": 1 }, None)
                    .await
                    .map_err(mongo_error)?;
                self.ensure_indexes(&client).await?;
                Ok(client)
            })
            .await
    }

    /// Eagerly validates the configured Mongo server and required indexes.
    pub async fn connect_strict(&self) -> ThreadStoreResult<()> {
        self.client().await.map(|_| ())
    }

    /// Returns whether this Codex-home namespace already contains Mongo session data.
    pub async fn namespace_has_data(&self) -> ThreadStoreResult<bool> {
        self.connect_strict().await?;
        let namespace_filter = doc! { "codex_home_namespace": &self.namespace };
        if self
            .threads()
            .await?
            .find_one(namespace_filter.clone(), None)
            .await
            .map_err(mongo_error)?
            .is_some()
        {
            return Ok(true);
        }
        Ok(self
            .items()
            .await?
            .find_one(namespace_filter, None)
            .await
            .map_err(mongo_error)?
            .is_some())
    }

    /// Removes every Mongo session document and sidecar blob in this namespace.
    pub async fn clear_namespace_data(&self) -> ThreadStoreResult<()> {
        self.connect_strict().await?;
        let namespace_filter = doc! { "codex_home_namespace": &self.namespace };
        let other_namespace_filter = doc! { "codex_home_namespace": { "$ne": &self.namespace } };
        let has_other_threads = self
            .threads()
            .await?
            .find_one(other_namespace_filter.clone(), None)
            .await
            .map_err(mongo_error)?
            .is_some();
        let has_other_items = self
            .items()
            .await?
            .find_one(other_namespace_filter, None)
            .await
            .map_err(mongo_error)?
            .is_some();
        if !has_other_threads && !has_other_items {
            self.items().await?.drop(None).await.map_err(mongo_error)?;
            self.threads()
                .await?
                .drop(None)
                .await
                .map_err(mongo_error)?;
            clear_blob_dir(&self.config.codex_home)?;
            let client = self.client().await?;
            self.ensure_indexes(client).await?;
            return Ok(());
        }
        self.items()
            .await?
            .delete_many(namespace_filter.clone(), None)
            .await
            .map_err(mongo_error)?;
        self.threads()
            .await?
            .delete_many(namespace_filter, None)
            .await
            .map_err(mongo_error)?;
        clear_blob_dir(&self.config.codex_home)?;
        Ok(())
    }

    /// Imports local JSONL and compressed JSONL rollouts into this Mongo namespace.
    /// This is an explicit migration path; normal Mongo operation never reads disk.
    pub async fn migrate_codex_home(
        &self,
        include_active: bool,
        include_archived: bool,
        dry_run: bool,
        verify: bool,
    ) -> ThreadStoreResult<MongoMigrationReport> {
        self.migrate_codex_home_with_progress(
            include_active,
            include_archived,
            dry_run,
            verify,
            |_| {},
        )
        .await
    }

    /// Imports local rollouts while reporting completed-rollout progress.
    pub async fn migrate_codex_home_with_progress(
        &self,
        include_active: bool,
        include_archived: bool,
        dry_run: bool,
        verify: bool,
        on_progress: impl FnMut(MongoMigrationProgress),
    ) -> ThreadStoreResult<MongoMigrationReport> {
        self.migrate_codex_home_with_progress_and_jobs(
            MongoMigrationOptions {
                include_active,
                include_archived,
                dry_run,
                verify,
                jobs: 1,
            },
            on_progress,
            |_| {},
        )
        .await
    }

    /// Imports local rollouts with bounded rollout-level concurrency.
    pub async fn migrate_codex_home_with_progress_and_jobs(
        &self,
        options: MongoMigrationOptions,
        mut on_progress: impl FnMut(MongoMigrationProgress),
        mut on_warning: impl FnMut(String),
    ) -> ThreadStoreResult<MongoMigrationReport> {
        let MongoMigrationOptions {
            include_active,
            include_archived,
            dry_run,
            verify,
            jobs,
        } = options;
        if jobs == 0 {
            return Err(ThreadStoreError::InvalidRequest {
                message: "Mongo migration jobs must be greater than zero".to_string(),
            });
        }
        self.connect_strict().await?;
        let mut report = MongoMigrationReport::default();
        let mut roots = Vec::new();
        if include_active {
            roots.push((
                self.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR),
                false,
            ));
        }
        if include_archived {
            roots.push((
                self.config
                    .codex_home
                    .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
                true,
            ));
        }
        let mut rollouts = Vec::new();
        for (root, archived) in roots {
            rollouts.extend(
                rollout_paths(&root)?
                    .into_iter()
                    .map(|path| (path, archived)),
            );
        }
        let total = rollouts.len();
        on_progress(MongoMigrationProgress {
            completed: 0,
            total,
        });
        let (warning_tx, mut warning_rx) = tokio::sync::mpsc::unbounded_channel();
        let worker_warning_tx = warning_tx.clone();
        let migrations =
            futures::stream::iter(rollouts.into_iter().map(move |(path, archived)| {
                let warning_tx = worker_warning_tx.clone();
                async move {
                    self.migrate_rollout_streaming(&path, archived, dry_run, verify, warning_tx)
                        .await
                }
            }))
            .buffer_unordered(jobs);
        drop(warning_tx);
        futures::pin_mut!(migrations);
        let mut migrations_done = false;
        let mut warnings_done = false;
        while !migrations_done || !warnings_done {
            tokio::select! {
                warning = warning_rx.recv(), if !warnings_done => match warning {
                    Some(warning) => on_warning(warning),
                    None => warnings_done = true,
                },
                partial = migrations.next(), if !migrations_done => match partial {
                    Some(partial) => {
                        let partial = partial?;
                        report.scanned += partial.scanned;
                        report.imported += partial.imported;
                        report.skipped += partial.skipped;
                        report.items += partial.items;
                        report.warnings.extend(partial.warnings);
                        on_progress(MongoMigrationProgress {
                            completed: report.scanned,
                            total,
                        });
                    }
                    None => {
                        migrations_done = true;
                        while let Ok(warning) = warning_rx.try_recv() {
                            on_warning(warning);
                        }
                        warnings_done = true;
                    }
                },
            }
        }
        Ok(report)
    }

    async fn migrate_rollout_streaming(
        &self,
        path: &std::path::Path,
        archived: bool,
        dry_run: bool,
        verify: bool,
        warning_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> ThreadStoreResult<MongoMigrationReport> {
        const INSERT_BATCH_SIZE: usize = 128;
        let mut partial = MongoMigrationReport {
            scanned: 1,
            ..MongoMigrationReport::default()
        };
        let mut reader = codex_rollout::open_rollout_line_reader(path)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to read rollout {}: {err}", path.display()),
            })?;
        let mut thread_id = None;
        let mut old_blob_file_names = None;
        let mut docs = Vec::with_capacity(INSERT_BATCH_SIZE);
        let mut sequence = 0i64;
        let mut line_number = 0usize;
        let mut first_user_message = None;
        while let Some(line) =
            reader
                .next_line()
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to read rollout {}: {err}", path.display()),
                })?
        {
            line_number += 1;
            if line.trim().is_empty() {
                continue;
            }
            let item = match reconstruct_rollout_line(&line) {
                Ok(rollout_line) => rollout_line.item,
                Err(reason) => {
                    record_migration_warning(
                        &mut partial,
                        &warning_tx,
                        format!(
                            "warning: skipping malformed rollout line {}:{}: {reason}",
                            path.display(),
                            line_number
                        ),
                    );
                    continue;
                }
            };
            if thread_id.is_none() {
                let RolloutItem::SessionMeta(meta_line) = &item else {
                    record_migration_warning(
                        &mut partial,
                        &warning_tx,
                        format!(
                            "warning: skipping rollout line {}:{} because SessionMeta is required before other rollout items",
                            path.display(),
                            line_number
                        ),
                    );
                    continue;
                };
                let id = meta_line.meta.id;
                thread_id = Some(id);
                if !dry_run {
                    old_blob_file_names =
                        Some(self.begin_rollout_import(id, meta_line, archived).await?);
                }
            }
            if first_user_message.is_none()
                && let RolloutItem::EventMsg(EventMsg::UserMessage(user_message)) = &item
            {
                first_user_message = user_message_preview(user_message);
            }
            partial.items += 1;
            if !dry_run {
                let id = thread_id.ok_or_else(|| ThreadStoreError::Internal {
                    message: "Mongo migration lost its initialized thread id".to_string(),
                })?;
                docs.push(self.item_document(id, sequence, item)?);
                if docs.len() >= INSERT_BATCH_SIZE {
                    self.insert_item_batch(std::mem::take(&mut docs)).await?;
                }
            }
            sequence += 1;
        }

        let Some(thread_id) = thread_id else {
            partial.skipped = 1;
            record_migration_warning(
                &mut partial,
                &warning_tx,
                format!(
                    "warning: skipping rollout {} because no usable SessionMeta could be reconstructed",
                    path.display()
                ),
            );
            return Ok(partial);
        };
        if !dry_run {
            if let Some(first_user_message) = first_user_message {
                let mut thread = self.thread_doc(thread_id, true).await?;
                thread.stored.preview = first_user_message.clone();
                thread.stored.first_user_message = Some(first_user_message);
                self.replace_thread(&thread).await?;
            }
            self.insert_item_batch(docs).await?;
            self.remove_unreferenced_blobs(old_blob_file_names.unwrap_or_default())
                .await?;
            if verify {
                let imported_items = self
                    .items()
                    .await?
                    .count_documents(self.key(thread_id), None)
                    .await
                    .map_err(mongo_error)? as usize;
                if imported_items != partial.items {
                    return Err(ThreadStoreError::Internal {
                        message: format!(
                            "Mongo verification failed for {thread_id}: expected {} items, found {imported_items}",
                            partial.items
                        ),
                    });
                }
            }
        }
        partial.imported = 1;
        Ok(partial)
    }

    async fn begin_rollout_import(
        &self,
        thread_id: ThreadId,
        meta_line: &SessionMetaLine,
        archived: bool,
    ) -> ThreadStoreResult<HashSet<String>> {
        let meta = &meta_line.meta;
        let created_at = chrono::DateTime::parse_from_rfc3339(&meta.timestamp)
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let stored = StoredThread {
            thread_id,
            extra_config: None,
            rollout_path: None,
            forked_from_id: meta.forked_from_id,
            parent_thread_id: meta.parent_thread_id,
            preview: String::new(),
            name: None,
            model_provider: meta
                .model_provider
                .clone()
                .unwrap_or_else(|| "openai".to_string()),
            model: None,
            reasoning_effort: None,
            created_at,
            updated_at: created_at,
            recency_at: created_at,
            archived_at: archived.then_some(Utc::now()),
            cwd: meta.cwd.clone(),
            cli_version: meta.cli_version.clone(),
            source: meta.source.clone(),
            history_mode: meta.history_mode,
            thread_source: meta.thread_source.clone(),
            agent_nickname: meta.agent_nickname.clone(),
            agent_role: meta.agent_role.clone(),
            agent_path: meta.agent_path.clone(),
            git_info: meta_line.git.clone(),
            approval_mode: Default::default(),
            permission_profile: Default::default(),
            token_usage: None,
            first_user_message: None,
            history: None,
        };
        let old_blob_file_names = self.blob_file_names(thread_id).await?;
        self.items()
            .await?
            .delete_many(self.key(thread_id), None)
            .await
            .map_err(mongo_error)?;
        self.threads()
            .await?
            .delete_one(self.key(thread_id), None)
            .await
            .map_err(mongo_error)?;
        self.threads()
            .await?
            .insert_one(
                ThreadDocument {
                    codex_home_namespace: self.namespace.clone(),
                    thread_id: thread_id.to_string(),
                    archived,
                    stored,
                },
                None,
            )
            .await
            .map_err(mongo_error)?;
        Ok(old_blob_file_names)
    }

    async fn insert_item_batch(&self, docs: Vec<ItemDocument>) -> ThreadStoreResult<()> {
        if docs.is_empty() {
            return Ok(());
        }
        self.items()
            .await?
            .insert_many(docs, None)
            .await
            .map_err(mongo_error)?;
        Ok(())
    }

    async fn ensure_indexes(&self, client: &Client) -> ThreadStoreResult<()> {
        let threads = client
            .database(&self.config.database)
            .collection::<ThreadDocument>("codex_threads");
        let items = client
            .database(&self.config.database)
            .collection::<ItemDocument>("codex_rollout_items");
        let unique = IndexOptions::builder().unique(true).build();
        threads.create_indexes(vec![
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "thread_id": 1 }).options(unique.clone()).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "archived": 1, "stored.recency_at": -1, "thread_id": 1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "archived": 1, "stored.cwd": 1, "stored.recency_at": -1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "archived": 1, "stored.source": 1, "stored.model_provider": 1, "stored.recency_at": -1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "archived": 1, "stored.created_at": -1, "thread_id": 1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "archived": 1, "stored.updated_at": -1, "thread_id": 1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "stored.parent_thread_id": 1 }).build(),
            IndexModel::builder().keys(doc! { "codex_home_namespace": 1, "stored.name": 1 }).build(),
        ], None).await.map_err(mongo_error)?;
        items
            .create_indexes(
                vec![
                    IndexModel::builder()
                        .keys(doc! { "codex_home_namespace": 1, "thread_id": 1, "sequence": 1 })
                        .options(unique)
                        .build(),
                ],
                None,
            )
            .await
            .map_err(mongo_error)?;
        Ok(())
    }

    async fn threads(&self) -> ThreadStoreResult<Collection<ThreadDocument>> {
        Ok(self
            .client()
            .await?
            .database(&self.config.database)
            .collection("codex_threads"))
    }

    async fn items(&self) -> ThreadStoreResult<Collection<ItemDocument>> {
        Ok(self
            .client()
            .await?
            .database(&self.config.database)
            .collection("codex_rollout_items"))
    }

    fn key(&self, thread_id: ThreadId) -> Document {
        doc! { "codex_home_namespace": &self.namespace, "thread_id": thread_id.to_string() }
    }

    async fn thread_doc(
        &self,
        thread_id: ThreadId,
        include_archived: bool,
    ) -> ThreadStoreResult<ThreadDocument> {
        let mut filter = self.key(thread_id);
        if !include_archived {
            filter.insert("archived", false);
        }
        self.threads()
            .await?
            .find_one(filter, None)
            .await
            .map_err(mongo_error)?
            .ok_or(ThreadStoreError::ThreadNotFound { thread_id })
    }

    async fn history(&self, thread_id: ThreadId) -> ThreadStoreResult<Vec<RolloutItem>> {
        let options = FindOptions::builder().sort(doc! { "sequence": 1 }).build();
        let cursor = self
            .items()
            .await?
            .find(self.key(thread_id), options)
            .await
            .map_err(mongo_error)?;
        let docs = cursor.try_collect::<Vec<_>>().await.map_err(mongo_error)?;
        let mut items = Vec::with_capacity(docs.len());
        for mut doc in docs {
            hydrate_rollout_item(&self.config.codex_home, &mut doc.item, &doc.external_fields)?;
            items.push(doc.item);
        }
        Ok(items)
    }

    async fn next_sequence(&self, thread_id: ThreadId) -> ThreadStoreResult<i64> {
        let options = FindOneOptions::builder()
            .sort(doc! { "sequence": -1 })
            .build();
        Ok(self
            .items()
            .await?
            .find_one(self.key(thread_id), options)
            .await
            .map_err(mongo_error)?
            .map_or(0, |doc| doc.sequence + 1))
    }

    async fn replace_thread(&self, doc: &ThreadDocument) -> ThreadStoreResult<()> {
        self.threads()
            .await?
            .replace_one(self.key(doc.stored.thread_id), doc, None)
            .await
            .map_err(mongo_error)?;
        Ok(())
    }

    fn item_document(
        &self,
        thread_id: ThreadId,
        sequence: i64,
        mut item: RolloutItem,
    ) -> ThreadStoreResult<ItemDocument> {
        let external_fields = externalize_rollout_item(&self.config.codex_home, &mut item)?;
        Ok(ItemDocument {
            codex_home_namespace: self.namespace.clone(),
            thread_id: thread_id.to_string(),
            sequence,
            item,
            external_fields,
        })
    }

    async fn blob_file_names(&self, thread_id: ThreadId) -> ThreadStoreResult<HashSet<String>> {
        let mut cursor = self
            .items()
            .await?
            .find(self.key(thread_id), None)
            .await
            .map_err(mongo_error)?;
        let mut file_names = HashSet::new();
        while let Some(doc) = cursor.try_next().await.map_err(mongo_error)? {
            file_names.extend(doc.external_fields.into_iter().map(|field| field.file_name));
        }
        Ok(file_names)
    }

    async fn remove_unreferenced_blobs(
        &self,
        file_names: HashSet<String>,
    ) -> ThreadStoreResult<()> {
        for file_name in file_names {
            let filter = doc! {
                "codex_home_namespace": &self.namespace,
                "external_fields.file_name": &file_name,
            };
            if self
                .items()
                .await?
                .find_one(filter, None)
                .await
                .map_err(mongo_error)?
                .is_none()
            {
                remove_blob(&self.config.codex_home, &file_name)?;
            }
        }
        Ok(())
    }

    async fn create_impl(&self, params: CreateThreadParams) -> ThreadStoreResult<()> {
        reject_paginated_history_mode(params.history_mode)?;
        let now = Utc::now();
        let session_meta = SessionMeta {
            session_id: params.session_id,
            id: params.thread_id,
            forked_from_id: params.forked_from_id,
            parent_thread_id: params.parent_thread_id,
            cwd: params.metadata.cwd.clone().unwrap_or_default(),
            originator: params.originator,
            source: params.source.clone(),
            thread_source: params.thread_source.clone(),
            model_provider: Some(params.metadata.model_provider.clone()),
            base_instructions: Some(params.base_instructions),
            dynamic_tools: (!params.dynamic_tools.is_empty()).then_some(params.dynamic_tools),
            selected_capability_roots: params.selected_capability_roots,
            memory_mode: matches!(params.metadata.memory_mode, ThreadMemoryMode::Disabled)
                .then_some("disabled".to_string()),
            history_mode: params.history_mode,
            multi_agent_version: params.multi_agent_version,
            context_window: Some(SessionContextWindow::new(params.initial_window_id)),
            ..SessionMeta::default()
        };
        let stored = StoredThread {
            thread_id: params.thread_id,
            extra_config: params.extra_config,
            rollout_path: None,
            forked_from_id: params.forked_from_id,
            parent_thread_id: params.parent_thread_id,
            preview: String::new(),
            name: None,
            model_provider: params.metadata.model_provider,
            model: None,
            reasoning_effort: None,
            created_at: now,
            updated_at: now,
            recency_at: now,
            archived_at: None,
            cwd: params.metadata.cwd.unwrap_or_default(),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            source: params.source,
            history_mode: params.history_mode,
            thread_source: params.thread_source,
            agent_nickname: session_meta.agent_nickname.clone(),
            agent_role: session_meta.agent_role.clone(),
            agent_path: session_meta.agent_path.clone(),
            git_info: None,
            approval_mode: Default::default(),
            permission_profile: Default::default(),
            token_usage: None,
            first_user_message: None,
            history: None,
        };
        self.threads()
            .await?
            .insert_one(
                ThreadDocument {
                    codex_home_namespace: self.namespace.clone(),
                    thread_id: params.thread_id.to_string(),
                    archived: false,
                    stored,
                },
                None,
            )
            .await
            .map_err(mongo_error)?;
        self.items()
            .await?
            .insert_one(
                ItemDocument {
                    codex_home_namespace: self.namespace.clone(),
                    thread_id: params.thread_id.to_string(),
                    sequence: 0,
                    item: RolloutItem::SessionMeta(SessionMetaLine {
                        meta: session_meta,
                        git: None,
                    }),
                    external_fields: Vec::new(),
                },
                None,
            )
            .await
            .map_err(mongo_error)?;
        self.live_threads.lock().await.insert(params.thread_id);
        Ok(())
    }

    async fn append_impl(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        let items = persisted_rollout_items(&params.items);
        if items.is_empty() {
            return Ok(());
        }
        if !self.live_threads.lock().await.contains(&params.thread_id) {
            return Err(ThreadStoreError::ThreadNotFound {
                thread_id: params.thread_id,
            });
        }
        let start = self.next_sequence(params.thread_id).await?;
        let docs = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| self.item_document(params.thread_id, start + index as i64, item))
            .collect::<ThreadStoreResult<Vec<_>>>()?;
        self.items()
            .await?
            .insert_many(docs, None)
            .await
            .map_err(mongo_error)?;
        Ok(())
    }

    async fn read_impl(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        let mut stored = self
            .thread_doc(params.thread_id, params.include_archived)
            .await?
            .stored;
        if params.include_history {
            reject_paginated_history_mode(stored.history_mode)?;
            stored.history = Some(StoredThreadHistory {
                thread_id: params.thread_id,
                items: self.history(params.thread_id).await?,
            });
        }
        Ok(stored)
    }

    async fn list_impl(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        if params.cwd_filters.as_ref().is_some_and(Vec::is_empty) {
            return Ok(ThreadPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        let mut filter =
            doc! { "codex_home_namespace": &self.namespace, "archived": params.archived };
        if !params.allowed_sources.is_empty() {
            filter.insert(
                "stored.source",
                doc! { "$in": bson_values(&params.allowed_sources)? },
            );
        }
        if let Some(providers) = &params.model_providers
            && !providers.is_empty()
        {
            filter.insert(
                "stored.model_provider",
                doc! { "$in": bson_values(providers)? },
            );
        }
        if let Some(cwds) = &params.cwd_filters {
            filter.insert("stored.cwd", doc! { "$in": bson_values(cwds)? });
        }
        if let Some(search) = &params.search_term {
            let regex = doc! { "$regex": search, "$options": "i" };
            filter.insert(
                "$or",
                vec![
                    Bson::Document(doc! { "stored.name": regex.clone() }),
                    Bson::Document(doc! { "stored.preview": regex.clone() }),
                    Bson::Document(doc! { "stored.first_user_message": regex }),
                ],
            );
        }
        if let Some(ThreadRelationFilter::DirectChildrenOf(parent)) = params.relation_filter {
            filter.insert(
                "stored.parent_thread_id",
                to_bson(&parent).map_err(mongo_serde_error)?,
            );
        }
        let field = match params.sort_key {
            crate::ThreadSortKey::CreatedAt => "stored.created_at",
            crate::ThreadSortKey::UpdatedAt => "stored.updated_at",
            crate::ThreadSortKey::RecencyAt => "stored.recency_at",
        };
        let direction = match params.sort_direction {
            crate::SortDirection::Asc => 1,
            crate::SortDirection::Desc => -1,
        };
        let offset = params
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("invalid Mongo thread cursor: {err}"),
            })?;
        let options = FindOptions::builder()
            .sort(doc! { field: direction, "thread_id": direction })
            .skip(offset)
            .limit(params.page_size as i64)
            .build();
        let cursor = self
            .threads()
            .await?
            .find(filter, options)
            .await
            .map_err(mongo_error)?;
        let mut items = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(mongo_error)?
            .into_iter()
            .map(|doc| doc.stored)
            .collect::<Vec<_>>();
        if let Some(ThreadRelationFilter::DescendantsOf(parent)) = params.relation_filter {
            let all = self
                .threads()
                .await?
                .find(
                    doc! { "codex_home_namespace": &self.namespace, "archived": params.archived },
                    None,
                )
                .await
                .map_err(mongo_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(mongo_error)?;
            let mut descendants = HashSet::from([parent]);
            loop {
                let before = descendants.len();
                for doc in &all {
                    if doc
                        .stored
                        .parent_thread_id
                        .is_some_and(|id| descendants.contains(&id))
                    {
                        descendants.insert(doc.stored.thread_id);
                    }
                }
                if descendants.len() == before {
                    break;
                }
            }
            items.retain(|thread| {
                thread.thread_id != parent && descendants.contains(&thread.thread_id)
            });
        }
        let next_cursor =
            (items.len() == params.page_size).then(|| (offset + items.len() as u64).to_string());
        Ok(ThreadPage { items, next_cursor })
    }
}

impl ThreadStore for MongoThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn create_thread(&self, p: CreateThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(self.create_impl(p))
    }
    fn resume_thread(&self, p: ResumeThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let doc = self.thread_doc(p.thread_id, p.include_archived).await?;
            let mode = p
                .history
                .as_deref()
                .map(|h| canonical_history_mode_from_rollout_items(h))
                .unwrap_or(doc.stored.history_mode);
            reject_paginated_history_mode(mode)?;
            self.live_threads.lock().await.insert(p.thread_id);
            Ok(())
        })
    }
    fn append_items(&self, p: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(self.append_impl(p))
    }
    fn persist_thread(&self, id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move { self.thread_doc(id, true).await.map(|_| ()) })
    }
    fn flush_thread(&self, id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move { self.thread_doc(id, true).await.map(|_| ()) })
    }
    fn shutdown_thread(&self, id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.thread_doc(id, true).await?;
            self.live_threads.lock().await.remove(&id);
            Ok(())
        })
    }
    fn discard_thread(&self, id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.live_threads.lock().await.remove(&id);
            Ok(())
        })
    }
    fn load_history(
        &self,
        p: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory> {
        Box::pin(async move {
            let thread = self.thread_doc(p.thread_id, p.include_archived).await?;
            reject_paginated_history_mode(thread.stored.history_mode)?;
            Ok(StoredThreadHistory {
                thread_id: p.thread_id,
                items: self.history(p.thread_id).await?,
            })
        })
    }
    fn read_thread(&self, p: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(self.read_impl(p))
    }
    fn read_thread_by_rollout_path(
        &self,
        _p: ReadThreadByRolloutPathParams,
    ) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async {
            Err(ThreadStoreError::Unsupported {
                operation: "read_thread_by_rollout_path",
            })
        })
    }
    fn list_threads(&self, p: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage> {
        Box::pin(self.list_impl(p))
    }
    fn search_threads(&self, p: SearchThreadsParams) -> ThreadStoreFuture<'_, ThreadSearchPage> {
        Box::pin(async move {
            let page = self
                .list_impl(ListThreadsParams {
                    page_size: p.page_size,
                    cursor: p.cursor,
                    sort_key: p.sort_key,
                    sort_direction: p.sort_direction,
                    allowed_sources: p.allowed_sources,
                    model_providers: None,
                    cwd_filters: None,
                    archived: p.archived,
                    search_term: Some(p.search_term),
                    relation_filter: None,
                    use_state_db_only: true,
                })
                .await?;
            Ok(ThreadSearchPage {
                items: page
                    .items
                    .into_iter()
                    .map(|thread| StoredThreadSearchResult {
                        snippet: thread
                            .first_user_message
                            .clone()
                            .unwrap_or_else(|| thread.preview.clone()),
                        thread,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
    }
    fn update_thread_metadata(
        &self,
        p: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let mut doc = self.thread_doc(p.thread_id, p.include_archived).await?;
            apply_patch(&mut doc.stored, p.patch);
            self.replace_thread(&doc).await?;
            Ok(doc.stored)
        })
    }
    fn archive_thread(&self, p: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut doc = self.thread_doc(p.thread_id, true).await?;
            doc.archived = true;
            doc.stored.archived_at = Some(Utc::now());
            self.replace_thread(&doc).await
        })
    }
    fn unarchive_thread(&self, p: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let mut doc = self.thread_doc(p.thread_id, true).await?;
            doc.archived = false;
            doc.stored.archived_at = None;
            self.replace_thread(&doc).await?;
            Ok(doc.stored)
        })
    }
    fn delete_thread(&self, p: DeleteThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let blob_file_names = self.blob_file_names(p.thread_id).await?;
            let result = self
                .threads()
                .await?
                .delete_one(self.key(p.thread_id), None)
                .await
                .map_err(mongo_error)?;
            if result.deleted_count == 0 {
                return Err(ThreadStoreError::ThreadNotFound {
                    thread_id: p.thread_id,
                });
            }
            self.items()
                .await?
                .delete_many(self.key(p.thread_id), None)
                .await
                .map_err(mongo_error)?;
            self.remove_unreferenced_blobs(blob_file_names).await?;
            self.live_threads.lock().await.remove(&p.thread_id);
            Ok(())
        })
    }
}

fn apply_patch(thread: &mut StoredThread, patch: ThreadMetadataPatch) {
    if let Some(v) = patch.name {
        thread.name = v;
    }
    if let Some(v) = patch.preview {
        thread.preview = v;
    }
    if let Some(v) = patch.title {
        thread.name = Some(v);
    }
    if let Some(v) = patch.model_provider {
        thread.model_provider = v;
    }
    if let Some(v) = patch.model {
        thread.model = Some(v);
    }
    if let Some(v) = patch.reasoning_effort {
        thread.reasoning_effort = Some(v);
    }
    if let Some(v) = patch.created_at {
        thread.created_at = v;
    }
    if let Some(v) = patch.updated_at {
        thread.updated_at = v;
    }
    if let Some(v) = patch.advance_recency_at
        && v > thread.recency_at
    {
        thread.recency_at = v;
    }
    if let Some(v) = patch.source {
        thread.source = v;
    }
    if let Some(v) = patch.thread_source {
        thread.thread_source = v;
    }
    if let Some(v) = patch.agent_nickname {
        thread.agent_nickname = v;
    }
    if let Some(v) = patch.agent_role {
        thread.agent_role = v;
    }
    if let Some(v) = patch.agent_path {
        thread.agent_path = v;
    }
    if let Some(v) = patch.cwd {
        thread.cwd = v;
    }
    if let Some(v) = patch.cli_version {
        thread.cli_version = v;
    }
    if let Some(v) = patch.approval_mode {
        thread.approval_mode = v;
    }
    if let Some(v) = patch.permission_profile {
        thread.permission_profile = v;
    }
    if let Some(v) = patch.token_usage {
        thread.token_usage = Some(v);
    }
    if let Some(v) = patch.first_user_message {
        thread.first_user_message = Some(v);
    }
    if let Some(git) = patch.git_info {
        let current = thread.git_info.get_or_insert(GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: None,
        });
        if let Some(v) = git.sha {
            current.commit_hash = v.as_deref().map(codex_git_utils::GitSha::new);
        }
        if let Some(v) = git.branch {
            current.branch = v;
        }
        if let Some(v) = git.origin_url {
            current.repository_url = v;
        }
    }
}

fn bson_values<T: Serialize>(values: &[T]) -> ThreadStoreResult<Vec<Bson>> {
    values
        .iter()
        .map(|v| to_bson(v).map_err(mongo_serde_error))
        .collect()
}
fn mongo_error(err: mongodb::error::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("Mongo thread store failure: {err}"),
    }
}
fn mongo_serde_error(err: mongodb::bson::ser::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("Mongo thread store serialization failure: {err}"),
    }
}

fn record_migration_warning(
    report: &mut MongoMigrationReport,
    warning_tx: &tokio::sync::mpsc::UnboundedSender<String>,
    warning: String,
) {
    let _ = warning_tx.send(warning.clone());
    report.warnings.push(warning);
}

fn reconstruct_rollout_line(line: &str) -> Result<RolloutLine, String> {
    if let Ok(rollout_line) = serde_json::from_str::<RolloutLine>(line) {
        return Ok(rollout_line);
    }

    let trimmed = line.trim();
    let candidate = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(start), Some(end)) if start <= end => &trimmed[start..=end],
        (Some(start), None) => &trimmed[start..],
        _ => trimmed,
    };
    let mut value = parse_reconstructed_json(candidate)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "reconstructed value is not an object".to_string())?;
    if !object.contains_key("type") || !object.contains_key("payload") {
        return Err("missing required type or payload fields".to_string());
    }
    object
        .entry("timestamp")
        .or_insert_with(|| Value::String("1970-01-01T00:00:00.000Z".to_string()));
    normalize_legacy_path_uri_fields(&mut value);
    serde_json::from_value(value)
        .map_err(|err| format!("reconstructed object is missing required rollout fields: {err}"))
}

fn parse_reconstructed_json(candidate: &str) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_str(candidate) {
        return Ok(value);
    }
    if let Some(Ok(value)) = serde_json::Deserializer::from_str(candidate)
        .into_iter::<Value>()
        .next()
    {
        return Ok(value);
    }

    let mut repaired = candidate.to_string();
    for closing in missing_json_closers(candidate) {
        repaired.push(closing);
    }
    serde_json::from_str(&repaired)
        .map_err(|err| format!("could not reconstruct JSON object: {err}"))
}

fn missing_json_closers(candidate: &str) -> Vec<char> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in candidate.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' if stack.last() == Some(&character) => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.into_iter().rev().collect()
}

fn normalize_legacy_path_uri_fields(value: &mut Value) {
    let Some(payload) = value.get_mut("payload").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(payload_type) = payload.get("type").and_then(Value::as_str) else {
        return;
    };
    match payload_type {
        "exec_command_begin" | "exec_command_end" => {
            normalize_legacy_path_uri(payload.get_mut("cwd"));
        }
        "view_image_tool_call" => {
            normalize_legacy_path_uri(payload.get_mut("path"));
        }
        "item_completed" => {
            if let Some(item) = payload.get_mut("item").and_then(Value::as_object_mut)
                && item.get("type").and_then(Value::as_str) == Some("command_execution")
            {
                normalize_legacy_path_uri(item.get_mut("cwd"));
            }
        }
        _ => {}
    }
}

fn normalize_legacy_path_uri(value: Option<&mut Value>) {
    let Some(Value::String(path)) = value else {
        return;
    };
    if path.starts_with("file:") || !std::path::Path::new(path).is_absolute() {
        return;
    }
    if let Ok(uri) = url::Url::from_file_path(path.as_str()) {
        *path = uri.to_string();
    }
}

fn rollout_paths(root: &std::path::Path) -> ThreadStoreResult<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to scan {}: {err}", dir.display()),
        })? {
            let entry = entry.map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to scan {}: {err}", dir.display()),
            })?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("rollout-")
                        && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
                })
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
#[path = "mongodb_tests.rs"]
mod tests;
