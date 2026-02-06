//! SQLite-vec based vector store.
//!
//! Implements `VectorStore` using sqlite-vec for KNN search.
//! Code content is NOT stored — only metadata (filepath, line range, hash).
//! All data lives in a single `retrieval.db` file.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use rusqlite::Connection;
use rusqlite::params;
use tokio::task::spawn_blocking;
use zerocopy::AsBytes;

use crate::config::default_embedding_dimension;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::search::Bm25Metadata;
use crate::search::SparseEmbedding;
use crate::storage::chunk_types::FileMetadata;
use crate::storage::chunk_types::IndexPolicy;
use crate::storage::chunk_types::IndexStatus;
use crate::storage::vector_store::VectorStore;
use crate::types::ChunkRef;
use crate::types::CodeChunk;

/// SQLite-vec backed vector store.
///
/// Uses sqlite-vec virtual tables for brute-force KNN search and
/// FTS5 for full-text search. All data stored in a single SQLite file.
pub struct SqliteVecStore {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
    dimension: i32,
}

impl SqliteVecStore {
    /// Open or create a SQLite-vec database at the given directory.
    pub fn open(data_dir: &Path) -> Result<Self> {
        Self::open_with_dimension(data_dir, default_embedding_dimension())
    }

    /// Open or create a SQLite-vec database with custom embedding dimension.
    ///
    /// If the database already exists with a different embedding dimension,
    /// the vector data is cleared and the vec0 table is recreated with the
    /// new dimension. Non-vector data (chunks, BM25 metadata) is preserved.
    pub fn open_with_dimension(data_dir: &Path, dimension: i32) -> Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all(data_dir).map_err(|e| RetrievalErr::SqliteError {
            path: data_dir.to_path_buf(),
            cause: format!("failed to create data dir: {e}"),
        })?;

        let db_path = data_dir.join("vector_store.db");

        // Register sqlite-vec extension before opening.
        //
        // SAFETY: `sqlite3_vec_init` is the extension entry point provided by the
        // sqlite-vec crate. `sqlite3_auto_extension` expects a function pointer
        // with the SQLite extension init signature. The transmute converts the
        // concrete fn pointer to the `Option<unsafe extern "C" fn()>` expected by
        // the FFI boundary. This is the officially documented pattern from the
        // sqlite-vec crate README.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(&db_path).map_err(|e| RetrievalErr::SqliteError {
            path: db_path.clone(),
            cause: e.to_string(),
        })?;

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -4000;",
        )
        .map_err(|e| RetrievalErr::SqliteError {
            path: db_path.clone(),
            cause: format!("pragma init failed: {e}"),
        })?;

        // If the vec0 table already exists with a different dimension, drop it
        // and let init_schema recreate it with the correct dimension.
        Self::validate_or_reset_dimension(&conn, dimension, &db_path)?;

        // Initialize schema
        Self::init_schema(&conn, dimension, &db_path)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: db_path,
            dimension,
        })
    }

    /// Initialize the database schema.
    fn init_schema(conn: &Connection, dimension: i32, path: &PathBuf) -> Result<()> {
        // Main chunks table — stores metadata only, no content.
        // Content is read from the file system on demand.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS code_chunks (
                id TEXT PRIMARY KEY,
                source_id TEXT NOT NULL,
                filepath TEXT NOT NULL,
                language TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                workspace TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                mtime INTEGER NOT NULL DEFAULT 0,
                indexed_at INTEGER NOT NULL DEFAULT 0,
                parent_symbol TEXT,
                is_overview INTEGER NOT NULL DEFAULT 0,
                bm25_embedding TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_filepath ON code_chunks(filepath);
            CREATE INDEX IF NOT EXISTS idx_chunks_workspace ON code_chunks(workspace);
            CREATE INDEX IF NOT EXISTS idx_chunks_ws_fp ON code_chunks(workspace, filepath);",
        )
        .map_err(|e| RetrievalErr::SqliteError {
            path: path.clone(),
            cause: format!("schema init failed: {e}"),
        })?;

        // Drop legacy FTS5 table and triggers if they exist (migration from older schema)
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS chunks_fts_ai;
             DROP TRIGGER IF EXISTS chunks_fts_ad;
             DROP TRIGGER IF EXISTS chunks_fts_au;
             DROP TABLE IF EXISTS chunks_fts;",
        )
        .map_err(|e| RetrievalErr::SqliteError {
            path: path.clone(),
            cause: format!("FTS5 cleanup failed: {e}"),
        })?;

        // Drop legacy content column if it exists (migration from older schema)
        // SQLite doesn't support DROP COLUMN in all versions, so we check first
        Self::drop_content_column_if_exists(conn, path)?;

        // Vec0 virtual table for vector search
        let vec_sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(
                chunk_id TEXT PRIMARY KEY,
                embedding float[{dimension}]
            )"
        );
        conn.execute_batch(&vec_sql)
            .map_err(|e| RetrievalErr::SqliteError {
                path: path.clone(),
                cause: format!("vec0 table init failed: {e}"),
            })?;

        // BM25 metadata table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bm25_metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                avgdl REAL NOT NULL,
                total_docs INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| RetrievalErr::SqliteError {
            path: path.clone(),
            cause: format!("bm25_metadata init failed: {e}"),
        })?;

        Ok(())
    }

    /// Drop the `content` column from `code_chunks` if it still exists.
    ///
    /// This handles migration from older schemas that stored content in the DB.
    fn drop_content_column_if_exists(conn: &Connection, path: &PathBuf) -> Result<()> {
        // Check if content column exists via pragma
        let has_content: bool = conn
            .prepare("PRAGMA table_info(code_chunks)")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                let mut found = false;
                for row in rows {
                    if row.map(|n| n == "content").unwrap_or(false) {
                        found = true;
                        break;
                    }
                }
                Ok(found)
            })
            .unwrap_or(false);

        if has_content {
            conn.execute_batch("ALTER TABLE code_chunks DROP COLUMN content")
                .map_err(|e| RetrievalErr::SqliteError {
                    path: path.clone(),
                    cause: format!("failed to drop content column: {e}"),
                })?;
            tracing::info!("Migrated code_chunks: dropped content column");
        }

        Ok(())
    }

    /// Execute a read query asynchronously via spawn_blocking.
    async fn query<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        let path = self.path.clone();

        spawn_blocking(move || {
            let guard = conn.lock().map_err(|_| RetrievalErr::SqliteError {
                path: path.clone(),
                cause: "Mutex poisoned".to_string(),
            })?;
            f(&guard)
        })
        .await
        .map_err(|e| RetrievalErr::SqliteError {
            path: self.path.clone(),
            cause: format!("spawn_blocking failed: {e}"),
        })?
    }

    /// Check if an existing vec0 table has a different dimension than requested.
    /// If so, drop the vec0 table so `init_schema` will recreate it.
    fn validate_or_reset_dimension(
        conn: &Connection,
        dimension: i32,
        path: &PathBuf,
    ) -> Result<()> {
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chunks_vec'",
                [],
                |r| r.get::<_, i32>(0).map(|c| c > 0),
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "check chunks_vec existence".to_string(),
                cause: e.to_string(),
            })?;

        if !table_exists {
            return Ok(());
        }

        // Extract stored dimension from the CREATE statement ("float[N]" pattern)
        let create_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunks_vec'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "read chunks_vec schema".to_string(),
                cause: e.to_string(),
            })?;

        if let Some(stored_dim) = Self::parse_vec0_dimension(&create_sql) {
            if stored_dim != dimension {
                tracing::warn!(
                    stored = stored_dim,
                    requested = dimension,
                    "Embedding dimension mismatch — clearing vector data and reinitializing"
                );
                conn.execute_batch("DROP TABLE IF EXISTS chunks_vec")
                    .map_err(|e| RetrievalErr::SqliteError {
                        path: path.clone(),
                        cause: format!("failed to drop chunks_vec for dimension reset: {e}"),
                    })?;
            }
        }

        Ok(())
    }

    /// Parse the embedding dimension from a vec0 CREATE TABLE statement.
    ///
    /// Looks for the `float[N]` pattern in the SQL string.
    fn parse_vec0_dimension(create_sql: &str) -> Option<i32> {
        let start = create_sql.find("float[")?;
        let after = &create_sql[start + 6..];
        let end = after.find(']')?;
        after[..end].parse::<i32>().ok()
    }

    /// Store embedding in the vec0 virtual table.
    ///
    /// `embedding` is serialized via `zerocopy::AsBytes` which writes the
    /// `f32` slice as raw little-endian bytes (the native byte order on all
    /// platforms SQLite supports). sqlite-vec expects this format.
    fn insert_embedding(
        conn: &Connection,
        chunk_id: &str,
        embedding: &[f32],
        expected_dimension: i32,
    ) -> Result<()> {
        if embedding.len() != expected_dimension as usize {
            return Err(RetrievalErr::EmbeddingDimensionMismatch {
                expected: expected_dimension,
                actual: embedding.len() as i32,
            });
        }
        conn.execute(
            "INSERT OR REPLACE INTO chunks_vec(chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, embedding.as_bytes()],
        )
        .map_err(|e| RetrievalErr::SqliteFailed {
            operation: "insert embedding".to_string(),
            cause: e.to_string(),
        })?;
        Ok(())
    }

    /// Validate a string for safe use in SQL queries (whitelist approach).
    fn validate_sql_value(value: &str, field_name: &str) -> Result<()> {
        if value.is_empty() {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("Empty {field_name}"),
            });
        }

        let is_safe = value.chars().all(|c| {
            c.is_alphanumeric()
                || c == '/'
                || c == '\\'
                || c == '.'
                || c == '_'
                || c == '-'
                || c == ' '
                || c == ':'
        });

        if !is_safe {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("{field_name} contains potentially unsafe characters"),
            });
        }

        if value.contains("--") || value.contains("/*") || value.contains("*/") {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("{field_name} contains SQL comment markers"),
            });
        }

        Ok(())
    }
}

#[async_trait]
impl VectorStore for SqliteVecStore {
    // ========== Chunk Storage ==========

    async fn store_chunks(&self, chunks: &[CodeChunk]) -> Result<()> {
        self.store_chunks_with_bm25(chunks, None).await
    }

    async fn store_chunks_with_bm25(
        &self,
        chunks: &[CodeChunk],
        bm25_embeddings: Option<&[String]>,
    ) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        // Clone data for the blocking closure
        let chunks_owned: Vec<CodeChunk> = chunks.to_vec();
        let bm25_owned: Option<Vec<String>> = bm25_embeddings.map(|b| b.to_vec());
        let dimension = self.dimension;

        self.query(move |conn| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "begin transaction".to_string(),
                    cause: e.to_string(),
                })?;

            {
                let mut stmt = tx
                    .prepare_cached(
                        "INSERT OR REPLACE INTO code_chunks
                        (id, source_id, filepath, language, start_line, end_line,
                         workspace, content_hash, mtime, indexed_at, parent_symbol, is_overview, bm25_embedding)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    )
                    .map_err(|e| RetrievalErr::SqliteFailed {
                        operation: "prepare insert".to_string(),
                        cause: e.to_string(),
                    })?;

                for (i, chunk) in chunks_owned.iter().enumerate() {
                    let mtime = chunk.modified_time.unwrap_or(0);
                    let indexed_at = if chunk.indexed_at == 0 {
                        chrono::Utc::now().timestamp()
                    } else {
                        chunk.indexed_at
                    };
                    let bm25_emb = bm25_owned
                        .as_ref()
                        .and_then(|b| b.get(i))
                        .filter(|s| !s.is_empty());

                    stmt.execute(params![
                        chunk.id,
                        chunk.source_id,
                        chunk.filepath,
                        chunk.language,
                        chunk.start_line,
                        chunk.end_line,
                        if chunk.workspace.is_empty() {
                            &chunk.source_id
                        } else {
                            &chunk.workspace
                        },
                        chunk.content_hash,
                        mtime,
                        indexed_at,
                        chunk.parent_symbol,
                        chunk.is_overview as i32,
                        bm25_emb,
                    ])
                    .map_err(|e| RetrievalErr::SqliteFailed {
                        operation: "insert chunk".to_string(),
                        cause: e.to_string(),
                    })?;

                    // Store embedding in vec0 if present
                    if let Some(ref emb) = chunk.embedding {
                        Self::insert_embedding(&tx, &chunk.id, emb, dimension)?;
                    }
                }
            }

            tx.commit().map_err(|e| RetrievalErr::SqliteFailed {
                operation: "commit".to_string(),
                cause: e.to_string(),
            })?;

            Ok(())
        })
        .await
    }

    // ========== Vector Search ==========

    async fn search_vector(&self, embedding: &[f32], limit: i32) -> Result<Vec<CodeChunk>> {
        let results = self.search_vector_with_distance(embedding, limit).await?;
        Ok(results.into_iter().map(|(chunk, _)| chunk).collect())
    }

    async fn search_vector_with_distance(
        &self,
        embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<(CodeChunk, f32)>> {
        if embedding.len() != self.dimension as usize {
            return Err(RetrievalErr::EmbeddingDimensionMismatch {
                expected: self.dimension,
                actual: embedding.len() as i32,
            });
        }

        // Serialize query embedding as little-endian f32 bytes for sqlite-vec
        let emb_bytes: Vec<u8> = embedding.as_bytes().to_vec();

        self.query(move |conn| {
            // Quick existence check — vec0 MATCH errors on an empty table
            let has_vectors: bool = conn
                .query_row("SELECT EXISTS(SELECT 1 FROM chunks_vec LIMIT 1)", [], |r| {
                    r.get(0)
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "check vec0 non-empty".to_string(),
                    cause: e.to_string(),
                })?;
            if !has_vectors {
                return Ok(Vec::new());
            }

            let mut stmt = conn
                .prepare(
                    "SELECT v.chunk_id, v.distance,
                            c.id, c.source_id, c.filepath, c.language,
                            c.start_line, c.end_line, c.workspace, c.content_hash,
                            c.mtime, c.indexed_at, c.parent_symbol, c.is_overview
                     FROM chunks_vec v
                     JOIN code_chunks c ON c.id = v.chunk_id
                     WHERE v.embedding MATCH ?1
                       AND k = ?2",
                )
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare vector search".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map(params![emb_bytes, limit], |row| {
                    let distance: f64 = row.get(1)?;
                    let mtime: i64 = row.get(10)?;
                    let is_overview: i32 = row.get(13)?;
                    let chunk = CodeChunk {
                        id: row.get(2)?,
                        source_id: row.get(3)?,
                        filepath: row.get(4)?,
                        language: row.get(5)?,
                        content: String::new(),
                        start_line: row.get(6)?,
                        end_line: row.get(7)?,
                        embedding: None,
                        modified_time: if mtime > 0 { Some(mtime) } else { None },
                        workspace: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(11)?,
                        parent_symbol: row.get(12)?,
                        is_overview: is_overview != 0,
                    };
                    Ok((chunk, distance as f32))
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "vector search".to_string(),
                    cause: e.to_string(),
                })?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "read vector result".to_string(),
                    cause: e.to_string(),
                })?);
            }
            Ok(results)
        })
        .await
    }

    async fn search_vector_refs(&self, embedding: &[f32], limit: i32) -> Result<Vec<ChunkRef>> {
        if embedding.len() != self.dimension as usize {
            return Err(RetrievalErr::EmbeddingDimensionMismatch {
                expected: self.dimension,
                actual: embedding.len() as i32,
            });
        }

        // Serialize query embedding as little-endian f32 bytes for sqlite-vec
        let emb_bytes: Vec<u8> = embedding.as_bytes().to_vec();

        self.query(move |conn| {
            let has_vectors: bool = conn
                .query_row("SELECT EXISTS(SELECT 1 FROM chunks_vec LIMIT 1)", [], |r| {
                    r.get(0)
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "check vec0 non-empty".to_string(),
                    cause: e.to_string(),
                })?;
            if !has_vectors {
                return Ok(Vec::new());
            }

            let mut stmt = conn
                .prepare(
                    "SELECT v.chunk_id, v.distance,
                            c.id, c.source_id, c.filepath, c.language,
                            c.start_line, c.end_line, c.workspace, c.content_hash,
                            c.indexed_at, c.parent_symbol, c.is_overview
                     FROM chunks_vec v
                     JOIN code_chunks c ON c.id = v.chunk_id
                     WHERE v.embedding MATCH ?1
                       AND k = ?2",
                )
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare vector ref search".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map(params![emb_bytes, limit], |row| {
                    let is_overview: i32 = row.get(12)?;
                    Ok(ChunkRef {
                        id: row.get(2)?,
                        source_id: row.get(3)?,
                        filepath: row.get(4)?,
                        language: row.get(5)?,
                        start_line: row.get(6)?,
                        end_line: row.get(7)?,
                        embedding: None,
                        workspace: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
                        parent_symbol: row.get(11)?,
                        is_overview: is_overview != 0,
                    })
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "vector ref search".to_string(),
                    cause: e.to_string(),
                })?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "read vector ref result".to_string(),
                    cause: e.to_string(),
                })?);
            }
            Ok(results)
        })
        .await
    }

    // ========== Full-Text Search ==========
    // FTS5 has been removed. BM25 in-memory index is the primary text search.
    // These methods return empty results for backward compatibility.

    async fn search_fts(&self, _query: &str, _limit: i32) -> Result<Vec<CodeChunk>> {
        Ok(Vec::new())
    }

    async fn search_fts_refs(&self, _query: &str, _limit: i32) -> Result<Vec<ChunkRef>> {
        Ok(Vec::new())
    }

    // ========== CRUD ==========

    async fn delete_by_path(&self, filepath: &str) -> Result<i32> {
        Self::validate_sql_value(filepath, "filepath")?;
        let fp = filepath.to_string();

        self.query(move |conn| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "begin delete_by_path tx".to_string(),
                    cause: e.to_string(),
                })?;

            // Delete from vec0 first (foreign key-like cleanup)
            tx.execute(
                "DELETE FROM chunks_vec WHERE chunk_id IN
                 (SELECT id FROM code_chunks WHERE filepath = ?1)",
                params![fp],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "delete embeddings by path".to_string(),
                cause: e.to_string(),
            })?;

            // Delete from main table
            let deleted = tx
                .execute("DELETE FROM code_chunks WHERE filepath = ?1", params![fp])
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "delete chunks by path".to_string(),
                    cause: e.to_string(),
                })?;

            tx.commit().map_err(|e| RetrievalErr::SqliteFailed {
                operation: "commit delete_by_path".to_string(),
                cause: e.to_string(),
            })?;

            Ok(deleted as i32)
        })
        .await
    }

    async fn delete_workspace(&self, workspace: &str) -> Result<i32> {
        Self::validate_sql_value(workspace, "workspace")?;
        let ws = workspace.to_string();

        self.query(move |conn| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "begin delete_workspace tx".to_string(),
                    cause: e.to_string(),
                })?;

            tx.execute(
                "DELETE FROM chunks_vec WHERE chunk_id IN
                 (SELECT id FROM code_chunks WHERE workspace = ?1)",
                params![ws],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "delete embeddings by workspace".to_string(),
                cause: e.to_string(),
            })?;

            let deleted = tx
                .execute("DELETE FROM code_chunks WHERE workspace = ?1", params![ws])
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "delete chunks by workspace".to_string(),
                    cause: e.to_string(),
                })?;

            tx.commit().map_err(|e| RetrievalErr::SqliteFailed {
                operation: "commit delete_workspace".to_string(),
                cause: e.to_string(),
            })?;

            Ok(deleted as i32)
        })
        .await
    }

    async fn count(&self) -> Result<i64> {
        self.query(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM code_chunks", [], |r| r.get(0))
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "count chunks".to_string(),
                    cause: e.to_string(),
                })?;
            Ok(count)
        })
        .await
    }

    async fn table_exists(&self) -> Result<bool> {
        self.query(|conn| {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='code_chunks'",
                    [],
                    |r| r.get::<_, i32>(0).map(|c| c > 0),
                )
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "check table exists".to_string(),
                    cause: e.to_string(),
                })?;
            Ok(exists)
        })
        .await
    }

    async fn list_all_chunks(&self) -> Result<Vec<CodeChunk>> {
        self.list_all_chunks_with_limit(Some(100_000)).await
    }

    async fn list_all_chunks_with_limit(&self, limit: Option<i32>) -> Result<Vec<CodeChunk>> {
        self.query(move |conn| {
            let base_sql = "SELECT id, source_id, filepath, language,
                            start_line, end_line, workspace, content_hash,
                            mtime, indexed_at, parent_symbol, is_overview
                     FROM code_chunks";

            let row_mapper = |row: &rusqlite::Row| {
                let mtime: i64 = row.get(8)?;
                let is_overview: i32 = row.get(11)?;
                Ok(CodeChunk {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    filepath: row.get(2)?,
                    language: row.get(3)?,
                    content: String::new(),
                    start_line: row.get(4)?,
                    end_line: row.get(5)?,
                    embedding: None,
                    modified_time: if mtime > 0 { Some(mtime) } else { None },
                    workspace: row.get(6)?,
                    content_hash: row.get(7)?,
                    indexed_at: row.get(9)?,
                    parent_symbol: row.get(10)?,
                    is_overview: is_overview != 0,
                })
            };

            let mut chunks = Vec::new();
            if let Some(n) = limit {
                let sql = format!("{base_sql} LIMIT ?1");
                let mut stmt = conn.prepare(&sql).map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare list chunks".to_string(),
                    cause: e.to_string(),
                })?;
                let rows = stmt.query_map(params![n], row_mapper).map_err(|e| {
                    RetrievalErr::SqliteFailed {
                        operation: "list chunks".to_string(),
                        cause: e.to_string(),
                    }
                })?;
                for row in rows {
                    chunks.push(row.map_err(|e| RetrievalErr::SqliteFailed {
                        operation: "read chunk".to_string(),
                        cause: e.to_string(),
                    })?);
                }
            } else {
                let mut stmt = conn
                    .prepare(base_sql)
                    .map_err(|e| RetrievalErr::SqliteFailed {
                        operation: "prepare list chunks".to_string(),
                        cause: e.to_string(),
                    })?;
                let rows =
                    stmt.query_map([], row_mapper)
                        .map_err(|e| RetrievalErr::SqliteFailed {
                            operation: "list chunks".to_string(),
                            cause: e.to_string(),
                        })?;
                for row in rows {
                    chunks.push(row.map_err(|e| RetrievalErr::SqliteFailed {
                        operation: "read chunk".to_string(),
                        cause: e.to_string(),
                    })?);
                }
            }
            Ok(chunks)
        })
        .await
    }

    // ========== File Metadata ==========

    async fn get_file_metadata(
        &self,
        workspace: &str,
        filepath: &str,
    ) -> Result<Option<FileMetadata>> {
        Self::validate_sql_value(workspace, "workspace")?;
        Self::validate_sql_value(filepath, "filepath")?;
        let ws = workspace.to_string();
        let fp = filepath.to_string();

        self.query(move |conn| {
            let result = conn.query_row(
                "SELECT filepath, workspace, content_hash, mtime, indexed_at
                 FROM code_chunks
                 WHERE workspace = ?1 AND filepath = ?2
                 LIMIT 1",
                params![ws, fp],
                |row| {
                    Ok(FileMetadata {
                        filepath: row.get(0)?,
                        workspace: row.get(1)?,
                        content_hash: row.get(2)?,
                        mtime: row.get(3)?,
                        indexed_at: row.get(4)?,
                    })
                },
            );

            match result {
                Ok(meta) => Ok(Some(meta)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(RetrievalErr::SqliteFailed {
                    operation: "get file metadata".to_string(),
                    cause: e.to_string(),
                }),
            }
        })
        .await
    }

    async fn get_workspace_files(&self, workspace: &str) -> Result<Vec<FileMetadata>> {
        Self::validate_sql_value(workspace, "workspace")?;
        let ws = workspace.to_string();

        self.query(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT filepath, workspace, content_hash, mtime, indexed_at
                     FROM code_chunks
                     WHERE workspace = ?1
                     GROUP BY filepath",
                )
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare workspace files".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map(params![ws], |row| {
                    Ok(FileMetadata {
                        filepath: row.get(0)?,
                        workspace: row.get(1)?,
                        content_hash: row.get(2)?,
                        mtime: row.get(3)?,
                        indexed_at: row.get(4)?,
                    })
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "query workspace files".to_string(),
                    cause: e.to_string(),
                })?;

            let mut files = Vec::new();
            for row in rows {
                files.push(row.map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "read workspace file".to_string(),
                    cause: e.to_string(),
                })?);
            }
            Ok(files)
        })
        .await
    }

    // ========== Index Management ==========
    // For sqlite-vec, vector search is brute-force (no index needed).
    // FTS5 has been removed; BM25 in-memory index is the primary text search.

    async fn create_vector_index(&self) -> Result<()> {
        // No-op: sqlite-vec uses brute-force KNN
        Ok(())
    }

    async fn create_fts_index(&self) -> Result<()> {
        // No-op: FTS5 removed, BM25 in-memory index handles text search
        Ok(())
    }

    async fn get_index_status(&self, policy: &IndexPolicy) -> Result<IndexStatus> {
        let chunk_count = self.count().await?;
        let table_exists = self.table_exists().await?;

        if !table_exists {
            return Ok(IndexStatus::default());
        }

        // For sqlite-vec, indexes are always "ready" (brute-force KNN)
        let vector_index_recommended =
            policy.chunk_threshold > 0 && chunk_count >= policy.chunk_threshold;
        // FTS5 removed; always false
        let fts_index_recommended = false;

        Ok(IndexStatus {
            table_exists,
            chunk_count,
            vector_index_recommended,
            fts_index_recommended,
        })
    }

    async fn apply_index_policy(&self, policy: &IndexPolicy) -> Result<bool> {
        let status = self.get_index_status(policy).await?;
        // No actual index creation needed for sqlite-vec
        Ok(status.needs_indexing())
    }

    async fn needs_index(&self, policy: &IndexPolicy) -> Result<bool> {
        let status = self.get_index_status(policy).await?;
        Ok(status.needs_indexing())
    }

    // ========== BM25 Metadata ==========

    async fn save_bm25_metadata(&self, metadata: &Bm25Metadata) -> Result<()> {
        let avgdl = metadata.avgdl;
        let total_docs = metadata.total_docs;
        let updated_at = metadata.updated_at;

        self.query(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO bm25_metadata (id, avgdl, total_docs, updated_at)
                 VALUES (1, ?1, ?2, ?3)",
                params![avgdl, total_docs, updated_at],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "save bm25 metadata".to_string(),
                cause: e.to_string(),
            })?;
            Ok(())
        })
        .await
    }

    async fn load_bm25_metadata(&self) -> Result<Option<Bm25Metadata>> {
        self.query(|conn| {
            let result = conn.query_row(
                "SELECT avgdl, total_docs, updated_at FROM bm25_metadata WHERE id = 1",
                [],
                |row| {
                    Ok(Bm25Metadata {
                        avgdl: row.get(0)?,
                        total_docs: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            );

            match result {
                Ok(meta) => Ok(Some(meta)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(RetrievalErr::SqliteFailed {
                    operation: "load bm25 metadata".to_string(),
                    cause: e.to_string(),
                }),
            }
        })
        .await
    }

    async fn bm25_metadata_exists(&self) -> Result<bool> {
        self.query(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM bm25_metadata WHERE id = 1", [], |r| {
                    r.get(0)
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "check bm25 metadata exists".to_string(),
                    cause: e.to_string(),
                })?;
            Ok(count > 0)
        })
        .await
    }

    // ========== Bulk Load ==========

    async fn load_all_chunk_refs(&self) -> Result<HashMap<String, ChunkRef>> {
        self.query(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, source_id, filepath, language,
                            start_line, end_line, workspace, content_hash,
                            indexed_at, parent_symbol, is_overview
                     FROM code_chunks",
                )
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare load chunk refs".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map([], |row| {
                    let is_overview: i32 = row.get(10)?;
                    Ok(ChunkRef {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        filepath: row.get(2)?,
                        language: row.get(3)?,
                        start_line: row.get(4)?,
                        end_line: row.get(5)?,
                        embedding: None,
                        workspace: row.get(6)?,
                        content_hash: row.get(7)?,
                        indexed_at: row.get(8)?,
                        parent_symbol: row.get(9)?,
                        is_overview: is_overview != 0,
                    })
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "load chunk refs".to_string(),
                    cause: e.to_string(),
                })?;

            let mut result = HashMap::new();
            for row in rows {
                let chunk_ref = row.map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "read chunk ref".to_string(),
                    cause: e.to_string(),
                })?;
                result.insert(chunk_ref.id.clone(), chunk_ref);
            }
            Ok(result)
        })
        .await
    }

    async fn load_all_bm25_embeddings(&self) -> Result<HashMap<String, SparseEmbedding>> {
        self.query(|conn| {
            let mut stmt = conn
                .prepare("SELECT id, bm25_embedding FROM code_chunks WHERE bm25_embedding IS NOT NULL AND bm25_embedding != ''")
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare load bm25 embeddings".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    let json: String = row.get(1)?;
                    Ok((id, json))
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "load bm25 embeddings".to_string(),
                    cause: e.to_string(),
                })?;

            let mut result = HashMap::new();
            for row in rows {
                let (id, json) = row.map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "read bm25 embedding".to_string(),
                    cause: e.to_string(),
                })?;
                if let Some(embedding) = SparseEmbedding::from_json(&json) {
                    result.insert(id, embedding);
                }
            }
            Ok(result)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_chunk(
        id: &str,
        source_id: &str,
        filepath: &str,
        content: &str,
        content_hash: &str,
    ) -> CodeChunk {
        CodeChunk {
            id: id.to_string(),
            source_id: source_id.to_string(),
            filepath: filepath.to_string(),
            language: "rust".to_string(),
            content: content.to_string(),
            start_line: 1,
            end_line: 1,
            embedding: None,
            modified_time: Some(1700000000),
            workspace: source_id.to_string(),
            content_hash: content_hash.to_string(),
            indexed_at: 1700000100,
            parent_symbol: None,
            is_overview: false,
        }
    }

    #[tokio::test]
    async fn test_open_database() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();
        assert!(store.table_exists().await.unwrap());
    }

    #[tokio::test]
    async fn test_store_and_count() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws:test.rs:0", "ws", "test.rs", "fn main() {}", "abc123"),
            test_chunk("ws:test.rs:1", "ws", "test.rs", "fn foo() {}", "abc123"),
        ];

        store.store_chunks(&chunks).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_delete_by_path() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws:a.rs:0", "ws", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws:b.rs:0", "ws", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        let deleted = store.delete_by_path("a.rs").await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_file_metadata() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws:test.rs:0", "ws", "test.rs", "fn main() {}", "abc123"),
            test_chunk("ws:test.rs:1", "ws", "test.rs", "fn foo() {}", "abc123"),
        ];

        store.store_chunks(&chunks).await.unwrap();

        let metadata = store.get_file_metadata("ws", "test.rs").await.unwrap();
        assert!(metadata.is_some());
        let meta = metadata.unwrap();
        assert_eq!(meta.filepath, "test.rs");
        assert_eq!(meta.workspace, "ws");
        assert_eq!(meta.content_hash, "abc123");
        assert_eq!(meta.mtime, 1700000000);

        let metadata = store
            .get_file_metadata("ws", "nonexistent.rs")
            .await
            .unwrap();
        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn test_get_workspace_files() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws:a.rs:0", "ws", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws:a.rs:1", "ws", "a.rs", "fn a2() {}", "hash_a"),
            test_chunk("ws:b.rs:0", "ws", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();

        let files = store.get_workspace_files("ws").await.unwrap();
        assert_eq!(files.len(), 2);

        let filepaths: Vec<_> = files.iter().map(|f| f.filepath.as_str()).collect();
        assert!(filepaths.contains(&"a.rs"));
        assert!(filepaths.contains(&"b.rs"));
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws1:a.rs:0", "ws1", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws2:b.rs:0", "ws2", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        let deleted = store.delete_workspace("ws1").await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(store.count().await.unwrap(), 1);

        let files = store.get_workspace_files("ws2").await.unwrap();
        assert_eq!(files.len(), 1);
    }

    #[tokio::test]
    async fn test_fts_search_returns_empty() {
        // FTS5 has been removed; search_fts always returns empty
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![test_chunk(
            "ws:auth.rs:0",
            "ws",
            "auth.rs",
            "fn authenticate_user(username: &str) -> bool",
            "hash1",
        )];

        store.store_chunks(&chunks).await.unwrap();

        let results = store.search_fts("authenticate", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_load_all_chunk_refs() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        let chunks = vec![
            test_chunk("ws:a.rs:0", "ws", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws:b.rs:0", "ws", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();

        let refs = store.load_all_chunk_refs().await.unwrap();
        assert_eq!(refs.len(), 2);
        assert!(refs.contains_key("ws:a.rs:0"));
        assert!(refs.contains_key("ws:b.rs:0"));

        let a_ref = &refs["ws:a.rs:0"];
        assert_eq!(a_ref.filepath, "a.rs");
        assert_eq!(a_ref.content_hash, "hash_a");
    }

    #[tokio::test]
    async fn test_bm25_metadata() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open(dir.path()).unwrap();

        assert!(!store.bm25_metadata_exists().await.unwrap());

        let metadata = Bm25Metadata {
            avgdl: 100.5,
            total_docs: 42,
            updated_at: 1700000000,
        };
        store.save_bm25_metadata(&metadata).await.unwrap();

        assert!(store.bm25_metadata_exists().await.unwrap());

        let loaded = store.load_bm25_metadata().await.unwrap().unwrap();
        assert!((loaded.avgdl - 100.5).abs() < f32::EPSILON);
        assert_eq!(loaded.total_docs, 42);
        assert_eq!(loaded.updated_at, 1700000000);
    }

    #[tokio::test]
    async fn test_vector_search() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open_with_dimension(dir.path(), 4).unwrap();

        let chunks = vec![
            CodeChunk {
                id: "1".to_string(),
                source_id: "ws".to_string(),
                filepath: "a.rs".to_string(),
                language: "rust".to_string(),
                content: "fn auth() {}".to_string(),
                start_line: 1,
                end_line: 1,
                embedding: Some(vec![0.1, 0.2, 0.3, 0.4]),
                modified_time: None,
                workspace: "ws".to_string(),
                content_hash: "hash1".to_string(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: false,
            },
            CodeChunk {
                id: "2".to_string(),
                source_id: "ws".to_string(),
                filepath: "b.rs".to_string(),
                language: "rust".to_string(),
                content: "fn db() {}".to_string(),
                start_line: 1,
                end_line: 1,
                embedding: Some(vec![0.9, 0.8, 0.7, 0.6]),
                modified_time: None,
                workspace: "ws".to_string(),
                content_hash: "hash2".to_string(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: false,
            },
        ];

        store.store_chunks(&chunks).await.unwrap();

        let query = vec![0.1, 0.2, 0.3, 0.4];
        let results = store.search_vector_with_distance(&query, 2).await.unwrap();
        assert_eq!(results.len(), 2);
        // First result should be closest (id "1")
        assert_eq!(results[0].0.id, "1");
        assert!(results[0].1 < results[1].1); // closer distance
    }

    #[tokio::test]
    async fn test_dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open_with_dimension(dir.path(), 4).unwrap();

        let wrong_dim = vec![0.1, 0.2]; // dimension 2, expected 4
        let result = store.search_vector(&wrong_dim, 10).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dimension_reset_on_open() {
        let dir = TempDir::new().unwrap();

        // Open with dimension 4 and store a chunk with embedding
        {
            let store = SqliteVecStore::open_with_dimension(dir.path(), 4).unwrap();
            let chunk = CodeChunk {
                id: "1".to_string(),
                source_id: "ws".to_string(),
                filepath: "a.rs".to_string(),
                language: "rust".to_string(),
                content: "fn auth() {}".to_string(),
                start_line: 1,
                end_line: 1,
                embedding: Some(vec![0.1, 0.2, 0.3, 0.4]),
                modified_time: None,
                workspace: "ws".to_string(),
                content_hash: "hash1".to_string(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: false,
            };
            store.store_chunks(&[chunk]).await.unwrap();

            // Verify vector search works with dim=4
            let results = store
                .search_vector(&[0.1, 0.2, 0.3, 0.4], 10)
                .await
                .unwrap();
            assert_eq!(results.len(), 1);
        }

        // Re-open with different dimension — vector data should be cleared
        {
            let store = SqliteVecStore::open_with_dimension(dir.path(), 8).unwrap();

            // Non-vector data (code_chunks) should be preserved
            assert_eq!(store.count().await.unwrap(), 1);

            // Vector search with new dimension should return nothing (old embeddings dropped)
            let results = store
                .search_vector(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8], 10)
                .await
                .unwrap();
            assert!(results.is_empty());
        }
    }

    #[tokio::test]
    async fn test_insert_embedding_dimension_check() {
        let dir = TempDir::new().unwrap();
        let store = SqliteVecStore::open_with_dimension(dir.path(), 4).unwrap();

        // Storing a chunk with wrong embedding dimension should fail
        let chunk = CodeChunk {
            id: "1".to_string(),
            source_id: "ws".to_string(),
            filepath: "a.rs".to_string(),
            language: "rust".to_string(),
            content: "fn auth() {}".to_string(),
            start_line: 1,
            end_line: 1,
            embedding: Some(vec![0.1, 0.2]), // dim 2, expected 4
            modified_time: None,
            workspace: "ws".to_string(),
            content_hash: "hash1".to_string(),
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        };
        let result = store.store_chunks(&[chunk]).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_vec0_dimension() {
        let sql = "CREATE VIRTUAL TABLE chunks_vec USING vec0(chunk_id TEXT PRIMARY KEY, embedding float[1536])";
        assert_eq!(SqliteVecStore::parse_vec0_dimension(sql), Some(1536));

        let sql2 = "CREATE VIRTUAL TABLE chunks_vec USING vec0(chunk_id TEXT PRIMARY KEY, embedding float[4])";
        assert_eq!(SqliteVecStore::parse_vec0_dimension(sql2), Some(4));

        assert_eq!(SqliteVecStore::parse_vec0_dimension("no match"), None);
    }
}
