use anyhow::Context;
use codex_core::config::Config;
use futures::TryStreamExt;
use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;
use ignore::WalkBuilder;
use reqwest::StatusCode;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::process::Command;

const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 200;
const DEFAULT_ALPHA: f32 = 0.6;
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const INDEX_DIR: &str = ".codex/repo_hybrid_index";
const DB_FILE_NAME: &str = "index.sqlite";
const CHUNK_LINE_COUNT: usize = 40;
const CHUNK_LINE_OVERLAP: usize = 8;
const SNIPPET_LINE_COUNT: usize = 6;
const MAX_FILE_SIZE_BYTES: u64 = 1_500_000;
const EMBED_BATCH_SIZE: usize = 64;
const VECTOR_CANDIDATE_MULTIPLIER: usize = 8;
const LEXICAL_CANDIDATE_MULTIPLIER: usize = 8;
const FALLBACK_RG_LIMIT: usize = 2_000;
const SQLITE_BIND_CHUNK_SIZE: usize = 900;
const METADATA_EMBEDDING_MODEL: &str = "embedding_model";
const METADATA_EMBEDDING_READY: &str = "embedding_ready";
const EMBEDDING_REASON_MISSING_API_KEY: &str = "missing_api_key";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepoHybridSearchParams {
    /// Required natural-language query describing what to find in the repository.
    pub query: String,
    /// Maximum number of results to return. Must be > 0; capped at 200.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional repository root path. Defaults to the current working directory.
    #[serde(default)]
    pub repo_root: Option<String>,
    /// Optional glob filters (for example: ["src/**/*.rs", "docs/**"]).
    /// When omitted, all indexable files are considered.
    #[serde(default)]
    pub file_globs: Option<Vec<String>>,
    /// Blend weight between lexical and embedding scores.
    /// `0.0` = lexical-only, `1.0` = embedding-only.
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    /// Optional embedding model override. Defaults to `text-embedding-3-small`.
    #[serde(default)]
    pub embedding_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepoIndexRefreshParams {
    #[serde(default)]
    pub repo_root: Option<String>,
    #[serde(default)]
    pub file_globs: Option<Vec<String>>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub force_full: bool,
    #[serde(default)]
    pub require_embeddings: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoHybridSearchResultItem {
    pub path: String,
    pub line_range: LineRange,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoIndexRefreshStats {
    pub scanned_files: usize,
    pub updated_files: usize,
    pub removed_files: usize,
    pub indexed_chunks: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RepoIndexWarmOutcome {
    pub repo_root: PathBuf,
    pub stats: RepoIndexRefreshStats,
    pub embedding_status: RepoEmbeddingStatus,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EmbeddingMode {
    Required,
    Skip,
}

impl EmbeddingMode {
    fn ready(self) -> bool {
        matches!(self, Self::Required)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RepoEmbeddingStatus {
    pub mode_used: EmbeddingMode,
    pub ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct SelectedEmbeddingMode {
    mode: EmbeddingMode,
    reason: Option<&'static str>,
}

impl SelectedEmbeddingMode {
    fn status(&self) -> RepoEmbeddingStatus {
        RepoEmbeddingStatus {
            mode_used: self.mode,
            ready: self.mode.ready(),
            reason: self.reason.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone)]
struct ChunkDraft {
    start_line: usize,
    end_line: usize,
    content: String,
    snippet: String,
}

#[derive(Debug, Clone)]
struct ChunkRecord {
    path: String,
    start_line: usize,
    end_line: usize,
    snippet: String,
}

#[derive(Debug, Clone)]
struct ScannedFile {
    absolute_path: PathBuf,
    modified_sec: i64,
    modified_nsec: i64,
    size_bytes: i64,
}

#[derive(Debug, Clone, Copy)]
struct ExistingFile {
    modified_sec: i64,
    modified_nsec: i64,
    size_bytes: i64,
}

pub(crate) fn create_tool_for_query_project() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<RepoHybridSearchParams>();
    let input_schema = create_tool_input_schema(schema);
    Tool {
        name: "query_project".into(),
        title: Some("Query Project".to_string()),
        input_schema,
        output_schema: None,
        description: Some(
            "Search the current repository for relevant code snippets.\n\
             Call this before directly reading files so you start from ranked, relevant locations.\n\
             Use `query` for what you want to find, and optionally narrow with `file_globs` or `repo_root`.\n\
             Returns ranked matches with file path, line range, snippet, and score.\n\
             Automatically performs an incremental index refresh before searching."
                .into(),
        ),
        annotations: None,
        icons: None,
        meta: None,
    }
}

pub(crate) fn create_tool_for_repo_index_refresh() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<RepoIndexRefreshParams>();
    let input_schema = create_tool_input_schema(schema);
    Tool {
        name: "repo_index_refresh".into(),
        title: Some("Repo Index Refresh".to_string()),
        input_schema,
        output_schema: None,
        description: Some(
            "Incrementally refreshes the local repository hybrid-search index (SQLite + FTS + embeddings)."
                .into(),
        ),
        annotations: None,
        icons: None,
        meta: None,
    }
}

pub(crate) async fn handle_repo_index_refresh(
    arguments: Option<JsonObject>,
    config: &Config,
) -> CallToolResult {
    let params = match parse_arguments::<RepoIndexRefreshParams>(arguments) {
        Ok(params) => params,
        Err(result) => return result,
    };

    let repo_root = match resolve_repo_root(params.repo_root.as_deref()) {
        Ok(repo_root) => repo_root,
        Err(err) => return call_tool_error(format!("invalid repo_root: {err}")),
    };

    let file_globs = params
        .file_globs
        .unwrap_or_else(|| config.query_project_index.file_globs.clone());
    let embedding_model = params
        .embedding_model
        .or_else(|| config.query_project_index.embedding_model.clone());
    let require_embeddings = params
        .require_embeddings
        .unwrap_or(config.query_project_index.require_embeddings);

    let outcome = match refresh_repo_index(
        repo_root,
        file_globs,
        embedding_model,
        params.force_full,
        require_embeddings,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => return call_tool_error(format!("index refresh failed: {err}")),
    };

    let payload = json!({
        "repo_root": outcome.repo_root.display().to_string(),
        "stats": outcome.stats,
        "embedding_status": outcome.embedding_status,
    });
    call_tool_success(payload)
}

pub(crate) async fn auto_warm_query_project_index(
    config: &Config,
) -> anyhow::Result<RepoIndexWarmOutcome> {
    let repo_root = resolve_repo_root(None)?;
    refresh_repo_index(
        repo_root,
        config.query_project_index.file_globs.clone(),
        config.query_project_index.embedding_model.clone(),
        false,
        config.query_project_index.require_embeddings,
    )
    .await
}

async fn refresh_repo_index(
    repo_root: PathBuf,
    file_globs: Vec<String>,
    embedding_model: Option<String>,
    force_full: bool,
    require_embeddings: bool,
) -> anyhow::Result<RepoIndexWarmOutcome> {
    let index = RepoHybridIndex::open(&repo_root)
        .await
        .with_context(|| format!("failed to initialize index at `{}`", repo_root.display()))?;
    let embedding_mode = resolve_embedding_mode(require_embeddings)?;
    let stats = index
        .refresh(
            &file_globs,
            force_full,
            embedding_model_or_default(embedding_model),
            embedding_mode.mode,
        )
        .await?;

    Ok(RepoIndexWarmOutcome {
        repo_root,
        stats,
        embedding_status: embedding_mode.status(),
    })
}

pub(crate) async fn handle_query_project(
    arguments: Option<JsonObject>,
    config: &Config,
) -> CallToolResult {
    let params = match parse_arguments::<RepoHybridSearchParams>(arguments) {
        Ok(params) => params,
        Err(result) => return result,
    };

    let query = params.query.trim();
    if query.is_empty() {
        return call_tool_error("query must not be empty");
    }

    if params.limit == 0 {
        return call_tool_error("limit must be greater than zero");
    }

    if !(0.0..=1.0).contains(&params.alpha) {
        return call_tool_error("alpha must be between 0.0 and 1.0");
    }

    let limit = params.limit.min(MAX_LIMIT);
    let repo_root = match resolve_repo_root(params.repo_root.as_deref()) {
        Ok(repo_root) => repo_root,
        Err(err) => return call_tool_error(format!("invalid repo_root: {err}")),
    };

    let file_globs = params
        .file_globs
        .unwrap_or_else(|| config.query_project_index.file_globs.clone());
    let embedding_model = embedding_model_or_default(
        params
            .embedding_model
            .or_else(|| config.query_project_index.embedding_model.clone()),
    );
    let embedding_mode = match resolve_embedding_mode(config.query_project_index.require_embeddings)
    {
        Ok(embedding_mode) => embedding_mode,
        Err(err) => return call_tool_error(format!("index refresh failed: {err}")),
    };

    let index = match RepoHybridIndex::open(&repo_root).await {
        Ok(index) => index,
        Err(err) => {
            return call_tool_error(format!(
                "failed to initialize index at `{}`: {err}",
                repo_root.display()
            ));
        }
    };

    let refresh_stats = match index
        .refresh(
            &file_globs,
            false,
            embedding_model.clone(),
            embedding_mode.mode,
        )
        .await
    {
        Ok(stats) => stats,
        Err(err) => return call_tool_error(format!("index refresh failed: {err}")),
    };

    let results = match index
        .search(
            query,
            limit,
            params.alpha,
            &file_globs,
            embedding_model.clone(),
        )
        .await
    {
        Ok(results) => results,
        Err(err) => return call_tool_error(format!("hybrid search failed: {err}")),
    };

    let payload = json!({
        "repo_root": repo_root.display().to_string(),
        "query": query,
        "limit": limit,
        "alpha": params.alpha,
        "embedding_model": embedding_model,
        "embedding_status": embedding_mode.status(),
        "refresh": refresh_stats,
        "results": results,
    });
    call_tool_success(payload)
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn default_alpha() -> f32 {
    DEFAULT_ALPHA
}

fn embedding_model_or_default(model: Option<String>) -> String {
    match model {
        Some(model) if !model.trim().is_empty() => model,
        _ => DEFAULT_EMBEDDING_MODEL.to_string(),
    }
}

fn resolve_embedding_mode(require_embeddings: bool) -> anyhow::Result<SelectedEmbeddingMode> {
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    resolve_embedding_mode_from_api_key(require_embeddings, api_key.as_deref())
}

fn resolve_embedding_mode_from_api_key(
    require_embeddings: bool,
    api_key: Option<&str>,
) -> anyhow::Result<SelectedEmbeddingMode> {
    if api_key.is_some_and(|value| !value.trim().is_empty()) {
        return Ok(SelectedEmbeddingMode {
            mode: EmbeddingMode::Required,
            reason: None,
        });
    }
    if require_embeddings {
        anyhow::bail!("OPENAI_API_KEY is required when require_embeddings=true");
    }
    Ok(SelectedEmbeddingMode {
        mode: EmbeddingMode::Skip,
        reason: Some(EMBEDDING_REASON_MISSING_API_KEY),
    })
}

fn parse_arguments<T>(arguments: Option<JsonObject>) -> Result<T, CallToolResult>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(arguments) = arguments else {
        return Err(call_tool_error("missing tool arguments"));
    };
    serde_json::from_value::<T>(serde_json::Value::Object(arguments))
        .map_err(|err| call_tool_error(format!("failed to parse tool arguments: {err}")))
}

fn call_tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult {
        content: vec![Content::text(message.into())],
        structured_content: None,
        is_error: Some(true),
        meta: None,
    }
}

fn call_tool_success(payload: serde_json::Value) -> CallToolResult {
    let structured_content = Some(payload.clone());
    CallToolResult {
        content: vec![Content::text(payload.to_string())],
        structured_content,
        is_error: Some(false),
        meta: None,
    }
}

fn resolve_repo_root(repo_root: Option<&str>) -> anyhow::Result<PathBuf> {
    let root = match repo_root {
        Some(repo_root) if !repo_root.trim().is_empty() => {
            let path = PathBuf::from(repo_root);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            }
        }
        _ => std::env::current_dir()?,
    };
    let canonical_root = root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize repository root `{}`",
            root.display()
        )
    })?;
    if !canonical_root.is_dir() {
        anyhow::bail!(
            "repository root must be a directory: `{}`",
            canonical_root.display()
        );
    }
    Ok(canonical_root)
}

fn create_tool_input_schema(schema: schemars::schema::RootSchema) -> Arc<JsonObject> {
    let schema_value = match serde_json::to_value(schema) {
        Ok(value) => value,
        Err(err) => panic!("schema should serialize: {err}"),
    };
    let mut schema_object = match schema_value {
        serde_json::Value::Object(object) => object,
        _ => panic!("tool schema should serialize to a JSON object"),
    };

    let mut input_schema = JsonObject::new();
    for key in ["properties", "required", "type", "$defs", "definitions"] {
        if let Some(value) = schema_object.remove(key) {
            input_schema.insert(key.to_string(), value);
        }
    }
    Arc::new(input_schema)
}

struct RepoHybridIndex {
    repo_root: PathBuf,
    pool: SqlitePool,
}

impl RepoHybridIndex {
    async fn open(repo_root: &Path) -> anyhow::Result<Self> {
        let index_dir = repo_root.join(INDEX_DIR);
        std::fs::create_dir_all(&index_dir).with_context(|| {
            format!("failed to create index directory `{}`", index_dir.display())
        })?;
        let db_path = index_dir.join(DB_FILE_NAME);
        let connect_options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("failed to open SQLite DB `{}`", db_path.display()))?;
        let index = Self {
            repo_root: repo_root.to_path_buf(),
            pool,
        };
        index.ensure_schema().await?;
        Ok(index)
    }

    async fn ensure_schema(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS indexed_files (
                path TEXT PRIMARY KEY,
                modified_sec INTEGER NOT NULL,
                modified_nsec INTEGER NOT NULL DEFAULT 0,
                size_bytes INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        let columns = sqlx::query("PRAGMA table_info(indexed_files)")
            .fetch_all(&self.pool)
            .await?;
        let mut has_modified_nsec = false;
        for row in &columns {
            let name: String = row.try_get("name")?;
            if name == "modified_nsec" {
                has_modified_nsec = true;
                break;
            }
        }
        if !has_modified_nsec {
            sqlx::query(
                "ALTER TABLE indexed_files ADD COLUMN modified_nsec INTEGER NOT NULL DEFAULT 0",
            )
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                snippet TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_chunks_path ON chunks(path)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(content, path UNINDEXED, chunk_id UNINDEXED)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS index_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_metadata(&self, key: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM index_metadata WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        let value = row
            .map(|row| row.try_get::<String, _>("value"))
            .transpose()?;
        Ok(value)
    }

    async fn set_metadata(&self, key: &str, value: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO index_metadata(key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn embedding_ready(&self) -> anyhow::Result<bool> {
        Ok(self
            .load_metadata(METADATA_EMBEDDING_READY)
            .await?
            .as_deref()
            .is_some_and(|value| value == "true"))
    }

    async fn refresh(
        &self,
        file_globs: &[String],
        force_full: bool,
        embedding_model: String,
        embedding_mode: EmbeddingMode,
    ) -> anyhow::Result<RepoIndexRefreshStats> {
        let glob_set = build_glob_set(file_globs)?;
        let stored_model = self.load_metadata(METADATA_EMBEDDING_MODEL).await?;
        let stored_ready = self.embedding_ready().await?;
        let mut force_full = force_full;
        if matches!(embedding_mode, EmbeddingMode::Required)
            && (stored_model.as_deref() != Some(embedding_model.as_str()) || !stored_ready)
        {
            force_full = true;
        }
        if force_full {
            self.clear_all().await?;
        }

        let scanned_files = scan_repo(&self.repo_root, glob_set.as_ref())?;
        let existing_files = self.load_existing_files().await?;

        let mut stats = RepoIndexRefreshStats {
            scanned_files: scanned_files.len(),
            updated_files: 0,
            removed_files: 0,
            indexed_chunks: 0,
        };

        let scanned_paths: HashSet<&str> = scanned_files.keys().map(String::as_str).collect();
        for (path, _existing) in existing_files
            .iter()
            .filter(|(path, _)| !scanned_paths.contains(path.as_str()))
        {
            if let Some(glob_set) = glob_set.as_ref()
                && !glob_set.is_match(path.as_str())
            {
                continue;
            }
            let mut tx = self.pool.begin().await?;
            remove_file_from_index(&mut tx, path).await?;
            tx.commit().await?;
            stats.removed_files += 1;
        }

        for (path, scanned) in &scanned_files {
            let unchanged = existing_files.get(path).is_some_and(|existing| {
                existing.modified_sec == scanned.modified_sec
                    && existing.modified_nsec == scanned.modified_nsec
                    && existing.size_bytes == scanned.size_bytes
            });
            if unchanged {
                continue;
            }

            let file_text = match read_text_file(&scanned.absolute_path)? {
                Some(file_text) => file_text,
                None => {
                    let mut tx = self.pool.begin().await?;
                    remove_file_from_index(&mut tx, path).await?;
                    tx.commit().await?;
                    continue;
                }
            };

            let chunks = chunk_text(&file_text);
            if chunks.is_empty() {
                let mut tx = self.pool.begin().await?;
                remove_file_from_index(&mut tx, path).await?;
                sqlx::query(
                    "INSERT OR REPLACE INTO indexed_files(path, modified_sec, modified_nsec, size_bytes) VALUES (?, ?, ?, ?)",
                )
                .bind(path)
                .bind(scanned.modified_sec)
                .bind(scanned.modified_nsec)
                .bind(scanned.size_bytes)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                stats.updated_files += 1;
                continue;
            }

            let embeddings = if matches!(embedding_mode, EmbeddingMode::Required) {
                let inputs = chunks
                    .iter()
                    .map(|chunk| chunk.content.clone())
                    .collect::<Vec<_>>();
                embed_texts(&embedding_model, &inputs).await?
            } else {
                vec![Vec::new(); chunks.len()]
            };
            if embeddings.len() != chunks.len() {
                anyhow::bail!(
                    "embedding service returned {} vectors for {} chunks",
                    embeddings.len(),
                    chunks.len()
                );
            }

            let mut tx = self.pool.begin().await?;
            remove_file_from_index(&mut tx, path).await?;
            sqlx::query(
                "INSERT OR REPLACE INTO indexed_files(path, modified_sec, modified_nsec, size_bytes) VALUES (?, ?, ?, ?)",
            )
            .bind(path)
            .bind(scanned.modified_sec)
            .bind(scanned.modified_nsec)
            .bind(scanned.size_bytes)
            .execute(&mut *tx)
            .await?;

            for (chunk, embedding) in chunks.into_iter().zip(embeddings.into_iter()) {
                let embedding_json = serde_json::to_string(&embedding)?;
                let content = chunk.content;
                let insert_result = sqlx::query(
                    "INSERT INTO chunks(path, start_line, end_line, snippet, content, embedding) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(path)
                .bind(chunk.start_line as i64)
                .bind(chunk.end_line as i64)
                .bind(chunk.snippet)
                .bind(&content)
                .bind(embedding_json)
                .execute(&mut *tx)
                .await?;
                let chunk_id = insert_result.last_insert_rowid();
                sqlx::query(
                    "INSERT INTO chunks_fts(rowid, content, path, chunk_id) VALUES (?, ?, ?, ?)",
                )
                .bind(chunk_id)
                .bind(content)
                .bind(path)
                .bind(chunk_id)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            stats.updated_files += 1;
        }

        stats.indexed_chunks = self.count_chunks().await?;
        self.set_metadata(METADATA_EMBEDDING_MODEL, &embedding_model)
            .await?;
        let ready = embedding_mode.ready();
        self.set_metadata(
            METADATA_EMBEDDING_READY,
            if ready { "true" } else { "false" },
        )
        .await?;
        Ok(stats)
    }

    async fn clear_all(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM chunks_fts")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM chunks").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM indexed_files")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn load_existing_files(&self) -> anyhow::Result<HashMap<String, ExistingFile>> {
        let rows =
            sqlx::query("SELECT path, modified_sec, modified_nsec, size_bytes FROM indexed_files")
                .fetch_all(&self.pool)
                .await?;
        let mut files = HashMap::with_capacity(rows.len());
        for row in rows {
            let path: String = row.try_get("path")?;
            let modified_sec: i64 = row.try_get("modified_sec")?;
            let modified_nsec: i64 = row.try_get("modified_nsec")?;
            let size_bytes: i64 = row.try_get("size_bytes")?;
            files.insert(
                path,
                ExistingFile {
                    modified_sec,
                    modified_nsec,
                    size_bytes,
                },
            );
        }
        Ok(files)
    }

    async fn count_chunks(&self) -> anyhow::Result<usize> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM chunks")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        Ok(count as usize)
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        alpha: f32,
        file_globs: &[String],
        embedding_model: String,
    ) -> anyhow::Result<Vec<RepoHybridSearchResultItem>> {
        let glob_set = build_glob_set(file_globs)?;
        let mut vector_scores = Vec::new();
        let mut effective_alpha = 0.0;
        if self.embedding_ready().await? {
            match embed_texts(&embedding_model, &[query.to_string()]).await {
                Ok(mut embeddings) => {
                    let query_embedding = embeddings
                        .pop()
                        .context("embedding service returned no query embedding")?;
                    vector_scores = self
                        .vector_scores(&query_embedding, limit, glob_set.as_ref())
                        .await?;
                    effective_alpha = alpha;
                }
                Err(_) => {
                    effective_alpha = 0.0;
                }
            }
        }

        let lexical_scores = self
            .lexical_scores(
                query,
                limit.saturating_mul(LEXICAL_CANDIDATE_MULTIPLIER),
                glob_set.as_ref(),
            )
            .await?;

        if vector_scores.is_empty() && lexical_scores.is_empty() {
            return Ok(Vec::new());
        }

        let lexical_score_pairs = lexical_scores.into_iter().collect::<Vec<_>>();
        let normalized_vector = normalize_scores(&vector_scores);
        let normalized_lexical = normalize_scores(&lexical_score_pairs);

        let mut candidate_ids = HashSet::new();
        candidate_ids.extend(normalized_vector.keys().copied());
        candidate_ids.extend(normalized_lexical.keys().copied());
        let candidate_ids = candidate_ids.into_iter().collect::<Vec<_>>();
        let chunks = self.load_chunks_by_ids(&candidate_ids).await?;

        let mut merged = candidate_ids
            .iter()
            .copied()
            .map(|chunk_id| {
                let vector_score = normalized_vector
                    .get(&chunk_id)
                    .copied()
                    .unwrap_or_default();
                let lexical_score = normalized_lexical
                    .get(&chunk_id)
                    .copied()
                    .unwrap_or_default();
                let score =
                    effective_alpha * vector_score + (1.0 - effective_alpha) * lexical_score;
                (chunk_id, score)
            })
            .collect::<Vec<_>>();
        merged.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        merged.truncate(limit);

        let mut results = Vec::with_capacity(merged.len());
        for (chunk_id, score) in merged {
            if let Some(chunk) = chunks.get(&chunk_id) {
                results.push(RepoHybridSearchResultItem {
                    path: chunk.path.clone(),
                    line_range: LineRange {
                        start: chunk.start_line,
                        end: chunk.end_line,
                    },
                    snippet: chunk.snippet.clone(),
                    score: round_score(score),
                });
            }
        }
        Ok(results)
    }

    async fn vector_scores(
        &self,
        query_embedding: &[f32],
        limit: usize,
        glob_set: Option<&GlobSet>,
    ) -> anyhow::Result<Vec<(i64, f32)>> {
        let candidate_limit = limit.saturating_mul(VECTOR_CANDIDATE_MULTIPLIER).max(limit);
        let buffer_limit = candidate_limit.saturating_mul(4).max(candidate_limit);
        let mut rows =
            sqlx::query("SELECT id, path, embedding FROM chunks ORDER BY id ASC").fetch(&self.pool);
        let mut scores = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let id: i64 = row.try_get("id")?;
            let path: String = row.try_get("path")?;
            if let Some(glob_set) = glob_set
                && !glob_set.is_match(path.as_str())
            {
                continue;
            }
            let embedding_json: String = row.try_get("embedding")?;
            let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json)
                .with_context(|| format!("failed to parse embedding JSON for chunk id {id}"))?;
            let score = cosine_similarity(query_embedding, &embedding);
            scores.push((id, score));
            if scores.len() > buffer_limit {
                scores.sort_by(sort_score_desc);
                scores.truncate(candidate_limit);
            }
        }
        scores.sort_by(sort_score_desc);
        scores.truncate(candidate_limit);
        Ok(scores)
    }

    async fn load_chunks_by_ids(&self, ids: &[i64]) -> anyhow::Result<HashMap<i64, ChunkRecord>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut chunks = HashMap::new();
        for id_chunk in ids.chunks(SQLITE_BIND_CHUNK_SIZE) {
            let mut builder = QueryBuilder::new(
                "SELECT id, path, start_line, end_line, snippet FROM chunks WHERE id IN (",
            );
            let mut separated = builder.separated(", ");
            for id in id_chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");
            let rows = builder.build().fetch_all(&self.pool).await?;
            for row in rows {
                let id: i64 = row.try_get("id")?;
                let path: String = row.try_get("path")?;
                let start_line: i64 = row.try_get("start_line")?;
                let end_line: i64 = row.try_get("end_line")?;
                let snippet: String = row.try_get("snippet")?;
                chunks.insert(
                    id,
                    ChunkRecord {
                        path,
                        start_line: start_line as usize,
                        end_line: end_line as usize,
                        snippet,
                    },
                );
            }
        }
        Ok(chunks)
    }

    async fn lexical_scores(
        &self,
        query: &str,
        limit: usize,
        glob_set: Option<&GlobSet>,
    ) -> anyhow::Result<HashMap<i64, f32>> {
        let query_for_fts = to_fts_query(query);
        let rows = sqlx::query(
            "SELECT chunk_id, bm25(chunks_fts) AS rank FROM chunks_fts WHERE chunks_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(query_for_fts)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await;

        match rows {
            Ok(rows) => {
                let mut scores = HashMap::with_capacity(rows.len());
                for row in rows {
                    let chunk_id: i64 = row.try_get("chunk_id")?;
                    let rank: f64 = row.try_get("rank")?;
                    if let Some(glob_set) = glob_set {
                        let path_row = sqlx::query("SELECT path FROM chunks WHERE id = ?")
                            .bind(chunk_id)
                            .fetch_optional(&self.pool)
                            .await?;
                        let Some(path_row) = path_row else {
                            continue;
                        };
                        let path: String = path_row.try_get("path")?;
                        if !glob_set.is_match(path.as_str()) {
                            continue;
                        }
                    }
                    scores.insert(chunk_id, -(rank as f32));
                }
                Ok(scores)
            }
            Err(_) => {
                self.lexical_scores_with_ripgrep(query, limit, glob_set)
                    .await
            }
        }
    }

    async fn lexical_scores_with_ripgrep(
        &self,
        query: &str,
        limit: usize,
        glob_set: Option<&GlobSet>,
    ) -> anyhow::Result<HashMap<i64, f32>> {
        let mut command = Command::new("rg");
        command
            .current_dir(&self.repo_root)
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color")
            .arg("never")
            .arg("--max-count")
            .arg(FALLBACK_RG_LIMIT.to_string())
            .arg("--")
            .arg(query)
            .arg(".");
        let output = command.output().await?;
        if output.status.code() == Some(1) {
            return Ok(HashMap::new());
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ripgrep lexical fallback failed: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut scores_by_chunk = HashMap::<i64, f32>::new();
        for line in stdout.lines() {
            let mut parts = line.splitn(3, ':');
            let Some(path) = parts.next() else {
                continue;
            };
            let Some(line_number_raw) = parts.next() else {
                continue;
            };
            let Ok(line_number) = line_number_raw.parse::<i64>() else {
                continue;
            };

            let normalized_path = normalize_rel_path(path);
            if let Some(glob_set) = glob_set
                && !glob_set.is_match(normalized_path.as_str())
            {
                continue;
            }

            let row = sqlx::query(
                "SELECT id FROM chunks WHERE path = ? AND start_line <= ? AND end_line >= ? LIMIT 1",
            )
            .bind(&normalized_path)
            .bind(line_number)
            .bind(line_number)
            .fetch_optional(&self.pool)
            .await?;
            let Some(row) = row else {
                continue;
            };
            let chunk_id: i64 = row.try_get("id")?;
            *scores_by_chunk.entry(chunk_id).or_insert(0.0) += 1.0;
        }

        let mut score_pairs = scores_by_chunk.into_iter().collect::<Vec<_>>();
        score_pairs.sort_by(sort_score_desc);
        score_pairs.truncate(limit);
        Ok(score_pairs.into_iter().collect())
    }
}

async fn remove_file_from_index(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    path: &str,
) -> anyhow::Result<usize> {
    let rows = sqlx::query("SELECT id FROM chunks WHERE path = ?")
        .bind(path)
        .fetch_all(&mut **tx)
        .await?;
    for row in &rows {
        let chunk_id: i64 = row.try_get("id")?;
        sqlx::query("DELETE FROM chunks_fts WHERE rowid = ?")
            .bind(chunk_id)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("DELETE FROM chunks WHERE path = ?")
        .bind(path)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM indexed_files WHERE path = ?")
        .bind(path)
        .execute(&mut **tx)
        .await?;
    Ok(rows.len())
}

fn build_glob_set(file_globs: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if file_globs.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for file_glob in file_globs.iter().filter(|glob| !glob.trim().is_empty()) {
        builder.add(Glob::new(file_glob).with_context(|| format!("invalid glob `{file_glob}`"))?);
    }
    Ok(Some(builder.build()?))
}

fn scan_repo(
    repo_root: &Path,
    glob_set: Option<&GlobSet>,
) -> anyhow::Result<HashMap<String, ScannedFile>> {
    let mut files = HashMap::new();
    let mut walker = WalkBuilder::new(repo_root);
    walker
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .require_git(false);

    for entry in walker.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }

        let absolute_path = entry.path().to_path_buf();
        let relative_path = match absolute_path.strip_prefix(repo_root) {
            Ok(relative_path) => normalize_rel_path(relative_path.to_string_lossy().as_ref()),
            Err(_) => continue,
        };
        if should_skip_index_path(relative_path.as_str()) {
            continue;
        }
        if let Some(glob_set) = glob_set
            && !glob_set.is_match(relative_path.as_str())
        {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() > MAX_FILE_SIZE_BYTES {
            continue;
        }
        let (modified_sec, modified_nsec) = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| (duration.as_secs() as i64, duration.subsec_nanos() as i64))
            .unwrap_or((0, 0));
        files.insert(
            relative_path,
            ScannedFile {
                absolute_path,
                modified_sec,
                modified_nsec,
                size_bytes: metadata.len() as i64,
            },
        );
    }
    Ok(files)
}

fn should_skip_index_path(path: &str) -> bool {
    path.starts_with(".git/")
        || path == ".git"
        || path.starts_with("target/")
        || path.starts_with("node_modules/")
        || path.starts_with(".codex/repo_hybrid_index/")
}

fn read_text_file(path: &Path) -> anyhow::Result<Option<String>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read file `{}`", path.display()))?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).to_string()))
}

fn chunk_text(file_text: &str) -> Vec<ChunkDraft> {
    let lines = file_text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    let step = CHUNK_LINE_COUNT.saturating_sub(CHUNK_LINE_OVERLAP).max(1);
    let mut start_index = 0;
    let mut chunks = Vec::new();

    while start_index < lines.len() {
        let end_index = (start_index + CHUNK_LINE_COUNT).min(lines.len());
        let chunk_lines = &lines[start_index..end_index];
        let snippet = chunk_lines
            .iter()
            .take(SNIPPET_LINE_COUNT)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        let content = chunk_lines.join("\n");
        chunks.push(ChunkDraft {
            start_line: start_index + 1,
            end_line: end_index,
            content,
            snippet,
        });
        if end_index == lines.len() {
            break;
        }
        start_index += step;
    }

    chunks
}

fn normalize_rel_path(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

fn to_fts_query(query: &str) -> String {
    let terms = query
        .split_whitespace()
        .map(|part| part.trim_matches('"'))
        .filter(|part| !part.is_empty())
        .map(|part| format!("\"{part}\""))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        "\"\"".to_string()
    } else {
        terms.join(" AND ")
    }
}

fn normalize_scores(scores: &[(i64, f32)]) -> HashMap<i64, f32> {
    if scores.is_empty() {
        return HashMap::new();
    }
    let (min_score, max_score) = scores.iter().fold(
        (f32::MAX, f32::MIN),
        |(min_score, max_score), (_, score)| (min_score.min(*score), max_score.max(*score)),
    );
    if (max_score - min_score).abs() < f32::EPSILON {
        return scores.iter().map(|(id, _)| (*id, 1.0)).collect();
    }
    scores
        .iter()
        .map(|(id, score)| (*id, (*score - min_score) / (max_score - min_score)))
        .collect()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let (dot, norm_left, norm_right) =
        left.iter()
            .zip(right.iter())
            .fold((0.0_f32, 0.0_f32, 0.0_f32), |acc, (left, right)| {
                let (dot, norm_left, norm_right) = acc;
                (
                    dot + (left * right),
                    norm_left + (left * left),
                    norm_right + (right * right),
                )
            });
    if norm_left <= f32::EPSILON || norm_right <= f32::EPSILON {
        return 0.0;
    }
    dot / (norm_left.sqrt() * norm_right.sqrt())
}

fn round_score(score: f32) -> f32 {
    (score * 10_000.0).round() / 10_000.0
}

fn sort_score_desc(left: &(i64, f32), right: &(i64, f32)) -> Ordering {
    right
        .1
        .partial_cmp(&left.1)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.0.cmp(&right.0))
}

#[derive(Serialize)]
struct EmbeddingsRequestBody {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingsResponseBody {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
    index: usize,
}

async fn embed_texts(model: &str, inputs: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = std::env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY is required for query_project embeddings")?;
    let base_url = std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let embeddings_url = format!("{}/embeddings", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    let mut all_embeddings = Vec::<Vec<f32>>::with_capacity(inputs.len());
    for batch in inputs.chunks(EMBED_BATCH_SIZE) {
        let request_body = EmbeddingsRequestBody {
            model: model.to_string(),
            input: batch.to_vec(),
        };
        let response = client
            .post(&embeddings_url)
            .bearer_auth(&api_key)
            .json(&request_body)
            .send()
            .await?;
        if response.status() != StatusCode::OK {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("embedding request failed with status {status}: {body}");
        }

        let mut parsed = response.json::<EmbeddingsResponseBody>().await?;
        parsed.data.sort_by_key(|item| item.index);
        all_embeddings.extend(parsed.data.into_iter().map(|item| item.embedding));
    }
    Ok(all_embeddings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn chunk_text_splits_with_overlap() {
        let file_text = (1..=65)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_text(&file_text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 40);
        assert_eq!(chunks[1].start_line, 33);
        assert_eq!(chunks[1].end_line, 65);
    }

    #[test]
    fn normalize_rel_path_strips_dot_prefix_and_backslashes() {
        assert_eq!(normalize_rel_path("./src\\main.rs"), "src/main.rs");
    }

    #[test]
    fn normalize_scores_handles_constant_values() {
        let normalized = normalize_scores(&[(1, 2.0), (2, 2.0)]);
        assert_eq!(normalized.get(&1).copied(), Some(1.0));
        assert_eq!(normalized.get(&2).copied(), Some(1.0));
    }

    #[test]
    fn resolve_embedding_mode_uses_required_when_api_key_is_present() {
        let mode = resolve_embedding_mode_from_api_key(false, Some("test-key"))
            .expect("mode should resolve");
        assert_eq!(mode.mode, EmbeddingMode::Required);
        assert_eq!(mode.reason, None);
        assert_eq!(mode.status().ready, true);
    }

    #[test]
    fn resolve_embedding_mode_defaults_to_skip_without_api_key() {
        let mode =
            resolve_embedding_mode_from_api_key(false, None).expect("mode should resolve to skip");
        assert_eq!(mode.mode, EmbeddingMode::Skip);
        assert_eq!(mode.reason, Some(EMBEDDING_REASON_MISSING_API_KEY));
        assert_eq!(mode.status().ready, false);
    }

    #[test]
    fn resolve_embedding_mode_requires_api_key_in_strict_mode() {
        let err = resolve_embedding_mode_from_api_key(true, None)
            .expect_err("strict mode should fail without api key");
        assert_eq!(
            err.to_string(),
            "OPENAI_API_KEY is required when require_embeddings=true"
        );
    }

    #[tokio::test]
    async fn refresh_with_globs_preserves_unmatched_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::create_dir_all(repo_root.join("src")).expect("create src dir");
        std::fs::write(repo_root.join("src/a.txt"), "alpha").expect("write a.txt");
        std::fs::write(repo_root.join("src/b.txt"), "beta").expect("write b.txt");

        let index = RepoHybridIndex::open(repo_root).await.expect("open index");
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip)
            .await
            .expect("initial refresh");
        let stats = index
            .refresh(
                &["src/a.txt".to_string()],
                false,
                "model".to_string(),
                EmbeddingMode::Skip,
            )
            .await
            .expect("glob refresh");
        let files = index.load_existing_files().await.expect("load files");

        assert_eq!(stats.removed_files, 0);
        assert!(files.contains_key("src/a.txt"));
        assert!(files.contains_key("src/b.txt"));
    }

    #[tokio::test]
    async fn refresh_with_skip_mode_does_not_rebuild_unchanged_files() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root).await.expect("open index");
        let mode =
            resolve_embedding_mode_from_api_key(false, None).expect("mode should resolve to skip");
        index
            .refresh(&[], false, "model".to_string(), mode.mode)
            .await
            .expect("initial refresh");
        let second = index
            .refresh(&[], false, "model".to_string(), mode.mode)
            .await
            .expect("second refresh");

        assert_eq!(second.updated_files, 0);
        assert_eq!(second.removed_files, 0);
    }

    #[tokio::test]
    async fn search_falls_back_to_lexical_when_embeddings_disabled() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let index = RepoHybridIndex::open(repo_root).await.expect("open index");
        index
            .refresh(&[], false, "model".to_string(), EmbeddingMode::Skip)
            .await
            .expect("refresh");
        let results = index
            .search("needle", 5, 1.0, &[], "model".to_string())
            .await
            .expect("search");

        assert!(
            results
                .iter()
                .any(|result| result.snippet.contains("needle")),
            "expected lexical match in results: {results:?}"
        );
    }

    #[tokio::test]
    async fn auto_warm_query_project_index_is_incremental() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().to_path_buf();
        std::fs::write(repo_root.join("README.md"), "needle").expect("write README");

        let first = refresh_repo_index(repo_root.clone(), vec![], None, false, false)
            .await
            .expect("first warm");
        let second = refresh_repo_index(repo_root, vec![], None, false, false)
            .await
            .expect("second warm");

        assert_eq!(first.stats.updated_files, 1);
        assert_eq!(second.stats.updated_files, 0);
        assert_eq!(second.stats.removed_files, 0);
        assert_eq!(second.embedding_status.ready, first.embedding_status.ready);
    }
}
