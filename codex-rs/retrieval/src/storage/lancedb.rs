//! LanceDB storage layer.
//!
//! Provides vector storage and full-text search using LanceDB.
//! Extended schema includes file metadata for tweakcc indexing.
//! Also provides index policy management and BM25 metadata storage.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use arrow::array::Array;
use arrow::array::BooleanArray;
use arrow::array::FixedSizeListArray;
use arrow::array::Float32Array;
use arrow::array::Int32Array;
use arrow::array::Int64Array;
use arrow::array::RecordBatch;
use arrow::array::StringArray;
use arrow::datatypes::DataType;
use arrow::datatypes::Field;
use arrow::datatypes::Schema;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::Table;
use lancedb::connection::Connection;
use lancedb::query::ExecutableQuery;
use lancedb::query::QueryBase;

use crate::config::default_embedding_dimension;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::search::Bm25Metadata;
use crate::search::SparseEmbedding;
use crate::storage::lancedb_types::FileMetadata;
use crate::storage::lancedb_types::IndexPolicy;
use crate::storage::lancedb_types::IndexStatus;
use crate::types::ChunkRef;
use crate::types::CodeChunk;

/// LanceDB store for code chunks and vectors.
pub struct LanceDbStore {
    db: Arc<Connection>,
    table_name: String,
    dimension: i32,
}

impl LanceDbStore {
    /// Get reference to the database connection.
    pub fn db(&self) -> &Connection {
        &self.db
    }

    /// Get the table name.
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// Open or create a LanceDB database.
    pub async fn open(path: &Path) -> Result<Self> {
        Self::open_with_dimension(path, default_embedding_dimension()).await
    }

    /// Open or create a LanceDB database with custom dimension.
    pub async fn open_with_dimension(path: &Path, dimension: i32) -> Result<Self> {
        let uri = path.to_string_lossy().to_string();
        let db = lancedb::connect(&uri).execute().await.map_err(|e| {
            RetrievalErr::LanceDbConnectionFailed {
                uri: uri.clone(),
                cause: e.to_string(),
            }
        })?;

        Ok(Self {
            db: Arc::new(db),
            table_name: "code_chunks".to_string(),
            dimension,
        })
    }

    /// Get the Arrow schema for the chunks table.
    ///
    /// Extended schema includes metadata for tweakcc indexing:
    /// - workspace: workspace identifier
    /// - content_hash: SHA256 hash of file content
    /// - mtime: file modification timestamp
    /// - indexed_at: when the chunk was indexed
    /// - parent_symbol: parent class/struct/impl context
    fn get_schema(&self) -> Schema {
        Schema::new(vec![
            // Core chunk fields
            Field::new("id", DataType::Utf8, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("filepath", DataType::Utf8, false),
            Field::new("language", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("start_line", DataType::Int32, false),
            Field::new("end_line", DataType::Int32, false),
            Field::new(
                "embedding",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, false)),
                    self.dimension,
                ),
                true, // nullable for chunks without embeddings
            ),
            // Extended metadata fields for tweakcc indexing
            Field::new("workspace", DataType::Utf8, false),
            Field::new("content_hash", DataType::Utf8, false),
            Field::new("mtime", DataType::Int64, false),
            Field::new("indexed_at", DataType::Int64, false),
            // Parent symbol context (nullable)
            Field::new("parent_symbol", DataType::Utf8, true),
            // Is overview chunk (nullable, defaults to false for backward compatibility)
            Field::new("is_overview", DataType::Boolean, true),
            // BM25 sparse embedding (JSON string, nullable)
            Field::new("bm25_embedding", DataType::Utf8, true),
        ])
    }

    /// Check if the chunks table exists.
    pub async fn table_exists(&self) -> Result<bool> {
        let tables = self.db.table_names().execute().await.map_err(|e| {
            RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            }
        })?;

        Ok(tables.contains(&self.table_name))
    }

    /// Get or create the chunks table.
    ///
    /// Uses optimistic creation with retry to handle race conditions:
    /// 1. Try to open existing table
    /// 2. If not found, try to create it
    /// 3. If creation fails (another process created it), retry open
    async fn get_or_create_table(&self) -> Result<Table> {
        // First, try to open existing table
        match self.db.open_table(&self.table_name).execute().await {
            Ok(table) => return Ok(table),
            Err(e) => {
                // Check if error is "table not found" - if so, proceed to create
                let err_str = e.to_string().to_lowercase();
                if !err_str.contains("not found")
                    && !err_str.contains("does not exist")
                    && !err_str.contains("no such table")
                {
                    return Err(RetrievalErr::LanceDbQueryFailed {
                        table: self.table_name.clone(),
                        cause: e.to_string(),
                    });
                }
            }
        }

        // Table doesn't exist, try to create it
        let schema = Arc::new(self.get_schema());
        let empty_batch = RecordBatch::new_empty(schema.clone());
        let reader = arrow::record_batch::RecordBatchIterator::new(vec![Ok(empty_batch)], schema);

        match self
            .db
            .create_table(&self.table_name, reader)
            .execute()
            .await
        {
            Ok(table) => Ok(table),
            Err(e) => {
                // Creation failed - might be due to race condition where another
                // process created it. Try to open again.
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("already exists") || err_str.contains("duplicate") {
                    self.db
                        .open_table(&self.table_name)
                        .execute()
                        .await
                        .map_err(|e2| RetrievalErr::LanceDbQueryFailed {
                            table: self.table_name.clone(),
                            cause: format!("create failed ({e}), open retry failed ({e2})"),
                        })
                } else {
                    Err(RetrievalErr::LanceDbQueryFailed {
                        table: self.table_name.clone(),
                        cause: e.to_string(),
                    })
                }
            }
        }
    }

    /// Convert chunks to Arrow RecordBatch.
    ///
    /// Optionally includes BM25 sparse embeddings as JSON strings.
    fn chunks_to_batch(
        &self,
        chunks: &[CodeChunk],
        bm25_embeddings: Option<&[String]>,
    ) -> Result<RecordBatch> {
        // Core chunk fields
        let ids: Vec<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
        let source_ids: Vec<&str> = chunks.iter().map(|c| c.source_id.as_str()).collect();
        let filepaths: Vec<&str> = chunks.iter().map(|c| c.filepath.as_str()).collect();
        let languages: Vec<&str> = chunks.iter().map(|c| c.language.as_str()).collect();
        let contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let start_lines: Vec<i32> = chunks.iter().map(|c| c.start_line).collect();
        let end_lines: Vec<i32> = chunks.iter().map(|c| c.end_line).collect();

        // Build embeddings array
        let embedding_values: Vec<Option<Vec<f32>>> = chunks
            .iter()
            .map(|c| {
                c.embedding.as_ref().map(|e| {
                    // Pad or truncate to dimension
                    let mut vec = e.clone();
                    vec.resize(self.dimension as usize, 0.0);
                    vec
                })
            })
            .collect();

        let embedding_array = self.build_embedding_array(&embedding_values)?;

        // Extended metadata fields
        let workspaces: Vec<&str> = chunks
            .iter()
            .map(|c| {
                if c.workspace.is_empty() {
                    c.source_id.as_str()
                } else {
                    c.workspace.as_str()
                }
            })
            .collect();
        let content_hashes: Vec<&str> = chunks.iter().map(|c| c.content_hash.as_str()).collect();
        let mtimes: Vec<i64> = chunks
            .iter()
            .map(|c| c.modified_time.unwrap_or(0))
            .collect();
        let indexed_ats: Vec<i64> = chunks
            .iter()
            .map(|c| {
                if c.indexed_at == 0 {
                    chrono::Utc::now().timestamp()
                } else {
                    c.indexed_at
                }
            })
            .collect();

        // Parent symbol context (nullable)
        let parent_symbols: Vec<Option<&str>> =
            chunks.iter().map(|c| c.parent_symbol.as_deref()).collect();

        // Is overview chunk (nullable)
        let is_overviews: Vec<Option<bool>> = chunks.iter().map(|c| Some(c.is_overview)).collect();

        // BM25 sparse embeddings (nullable JSON strings)
        let bm25_emb_values: Vec<Option<&str>> = if let Some(embeddings) = bm25_embeddings {
            embeddings
                .iter()
                .map(|e| if e.is_empty() { None } else { Some(e.as_str()) })
                .collect()
        } else {
            vec![None; chunks.len()]
        };

        let schema = Arc::new(self.get_schema());
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(source_ids)),
                Arc::new(StringArray::from(filepaths)),
                Arc::new(StringArray::from(languages)),
                Arc::new(StringArray::from(contents)),
                Arc::new(Int32Array::from(start_lines)),
                Arc::new(Int32Array::from(end_lines)),
                Arc::new(embedding_array),
                Arc::new(StringArray::from(workspaces)),
                Arc::new(StringArray::from(content_hashes)),
                Arc::new(Int64Array::from(mtimes)),
                Arc::new(Int64Array::from(indexed_ats)),
                Arc::new(StringArray::from(parent_symbols)),
                Arc::new(BooleanArray::from(is_overviews)),
                Arc::new(StringArray::from(bm25_emb_values)),
            ],
        )
        .map_err(|e| RetrievalErr::LanceDbQueryFailed {
            table: self.table_name.clone(),
            cause: e.to_string(),
        })
    }

    /// Build a FixedSizeList array from embedding vectors.
    fn build_embedding_array(&self, embeddings: &[Option<Vec<f32>>]) -> Result<FixedSizeListArray> {
        let dim = self.dimension as usize;
        let mut values: Vec<f32> = Vec::with_capacity(embeddings.len() * dim);
        let mut validity: Vec<bool> = Vec::with_capacity(embeddings.len());

        for embedding in embeddings {
            match embedding {
                Some(vec) => {
                    values.extend(vec.iter().take(dim));
                    // Pad if needed
                    if vec.len() < dim {
                        values.extend(std::iter::repeat(0.0).take(dim - vec.len()));
                    }
                    validity.push(true);
                }
                None => {
                    values.extend(std::iter::repeat(0.0).take(dim));
                    validity.push(false);
                }
            }
        }

        let values_array = Float32Array::from(values);
        let field = Arc::new(Field::new("item", DataType::Float32, false));

        FixedSizeListArray::try_new(
            field,
            self.dimension,
            Arc::new(values_array),
            Some(validity.into()),
        )
        .map_err(|e| RetrievalErr::LanceDbQueryFailed {
            table: self.table_name.clone(),
            cause: e.to_string(),
        })
    }

    /// Store a batch of code chunks (without BM25 embeddings).
    pub async fn store_chunks(&self, chunks: &[CodeChunk]) -> Result<()> {
        self.store_chunks_with_bm25(chunks, None).await
    }

    /// Store a batch of code chunks with optional BM25 embeddings.
    ///
    /// If `bm25_embeddings` is provided, it must have the same length as `chunks`.
    /// Each string should be a JSON-serialized SparseEmbedding.
    pub async fn store_chunks_with_bm25(
        &self,
        chunks: &[CodeChunk],
        bm25_embeddings: Option<&[String]>,
    ) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let table = self.get_or_create_table().await?;
        let batch = self.chunks_to_batch(chunks, bm25_embeddings)?;

        // Create a RecordBatchIterator for LanceDB
        let schema = batch.schema();
        let reader = arrow::record_batch::RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    /// Parse a RecordBatch into CodeChunks.
    fn batch_to_chunks(batch: &RecordBatch) -> Result<Vec<CodeChunk>> {
        // Core fields
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid id column".to_string(),
            })?;

        let source_ids = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid source_id column".to_string(),
            })?;

        let filepaths = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid filepath column".to_string(),
            })?;

        let languages = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid language column".to_string(),
            })?;

        let contents = batch
            .column(4)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid content column".to_string(),
            })?;

        let start_lines = batch
            .column(5)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid start_line column".to_string(),
            })?;

        let end_lines = batch
            .column(6)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid end_line column".to_string(),
            })?;

        let embeddings = batch
            .column(7)
            .as_any()
            .downcast_ref::<FixedSizeListArray>();

        // Extended metadata fields (optional for backward compatibility)
        let workspaces = batch
            .column_by_name("workspace")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let content_hashes = batch
            .column_by_name("content_hash")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let mtimes = batch
            .column_by_name("mtime")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
        let indexed_ats = batch
            .column_by_name("indexed_at")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
        let parent_symbols = batch
            .column_by_name("parent_symbol")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let is_overviews = batch
            .column_by_name("is_overview")
            .and_then(|c| c.as_any().downcast_ref::<BooleanArray>());

        let mut chunks = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            let embedding = embeddings.and_then(|emb| {
                if emb.is_null(i) {
                    None
                } else {
                    let values = emb.value(i);
                    let arr = values.as_any().downcast_ref::<Float32Array>()?;
                    Some(arr.values().to_vec())
                }
            });

            // Read extended metadata with fallback defaults
            let workspace = workspaces
                .map(|w| w.value(i).to_string())
                .unwrap_or_else(|| source_ids.value(i).to_string());
            let content_hash = content_hashes
                .map(|h| h.value(i).to_string())
                .unwrap_or_default();
            let mtime = mtimes.map(|m| m.value(i)).unwrap_or(0);
            let indexed_at = indexed_ats.map(|a| a.value(i)).unwrap_or(0);
            let parent_symbol = parent_symbols.and_then(|ps| {
                let val = ps.value(i);
                if val.is_empty() {
                    None
                } else {
                    Some(val.to_string())
                }
            });

            // Read is_overview with fallback to false
            let is_overview = is_overviews.map(|o| o.value(i)).unwrap_or(false);

            chunks.push(CodeChunk {
                id: ids.value(i).to_string(),
                source_id: source_ids.value(i).to_string(),
                filepath: filepaths.value(i).to_string(),
                language: languages.value(i).to_string(),
                content: contents.value(i).to_string(),
                start_line: start_lines.value(i),
                end_line: end_lines.value(i),
                embedding,
                modified_time: if mtime > 0 { Some(mtime) } else { None },
                workspace,
                content_hash,
                indexed_at,
                parent_symbol,
                is_overview,
            });
        }

        Ok(chunks)
    }

    /// Parse a RecordBatch into ChunkRefs (without content).
    ///
    /// This is more efficient than `batch_to_chunks` when you only need
    /// the reference information and will hydrate content later.
    fn batch_to_chunk_refs(batch: &RecordBatch) -> Result<Vec<ChunkRef>> {
        // Core fields (reuse column indices from batch_to_chunks)
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid id column".to_string(),
            })?;

        let source_ids = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid source_id column".to_string(),
            })?;

        let filepaths = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid filepath column".to_string(),
            })?;

        let languages = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid language column".to_string(),
            })?;

        // Skip column 4 (content) - not needed for ChunkRef

        let start_lines = batch
            .column(5)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid start_line column".to_string(),
            })?;

        let end_lines = batch
            .column(6)
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                table: "code_chunks".to_string(),
                cause: "Invalid end_line column".to_string(),
            })?;

        let embeddings = batch
            .column(7)
            .as_any()
            .downcast_ref::<FixedSizeListArray>();

        // Extended metadata fields
        let workspaces = batch
            .column_by_name("workspace")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let content_hashes = batch
            .column_by_name("content_hash")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let indexed_ats = batch
            .column_by_name("indexed_at")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
        let parent_symbols = batch
            .column_by_name("parent_symbol")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let is_overviews = batch
            .column_by_name("is_overview")
            .and_then(|c| c.as_any().downcast_ref::<BooleanArray>());

        let mut refs = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            let embedding = embeddings.and_then(|emb| {
                if emb.is_null(i) {
                    None
                } else {
                    let values = emb.value(i);
                    let arr = values.as_any().downcast_ref::<Float32Array>()?;
                    Some(arr.values().to_vec())
                }
            });

            let workspace = workspaces
                .map(|w| w.value(i).to_string())
                .unwrap_or_else(|| source_ids.value(i).to_string());
            let content_hash = content_hashes
                .map(|h| h.value(i).to_string())
                .unwrap_or_default();
            let indexed_at = indexed_ats.map(|a| a.value(i)).unwrap_or(0);
            let parent_symbol = parent_symbols.and_then(|ps| {
                let val = ps.value(i);
                if val.is_empty() {
                    None
                } else {
                    Some(val.to_string())
                }
            });
            let is_overview = is_overviews.map(|o| o.value(i)).unwrap_or(false);

            refs.push(ChunkRef {
                id: ids.value(i).to_string(),
                source_id: source_ids.value(i).to_string(),
                filepath: filepaths.value(i).to_string(),
                language: languages.value(i).to_string(),
                start_line: start_lines.value(i),
                end_line: end_lines.value(i),
                embedding,
                workspace,
                content_hash,
                indexed_at,
                parent_symbol,
                is_overview,
            });
        }

        Ok(refs)
    }

    /// Search using full-text search (BM25).
    pub async fn search_fts(&self, query: &str, limit: i32) -> Result<Vec<CodeChunk>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Use LanceDB full-text search
        let results = table
            .query()
            .full_text_search(FullTextSearchQuery::new(query.to_string()))
            .limit(limit as usize)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut chunks = Vec::new();
        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;
            chunks.extend(Self::batch_to_chunks(&batch)?);
        }

        Ok(chunks)
    }

    /// Search using vector similarity.
    ///
    /// Returns an error if the embedding dimension doesn't match the configured dimension.
    /// This prevents silent quality degradation from dimension mismatches.
    ///
    /// Note: This method discards distance information. For better search quality,
    /// use `search_vector_with_distance` which preserves the actual similarity scores.
    pub async fn search_vector(&self, embedding: &[f32], limit: i32) -> Result<Vec<CodeChunk>> {
        let results = self.search_vector_with_distance(embedding, limit).await?;
        Ok(results.into_iter().map(|(chunk, _)| chunk).collect())
    }

    /// Search using vector similarity, returning chunks with their distance scores.
    ///
    /// Returns `Vec<(CodeChunk, f32)>` where the f32 is the L2 distance from the query vector.
    /// Lower distance = more similar. Use `1.0 / (1.0 + distance)` to convert to similarity score.
    ///
    /// Returns an error if the embedding dimension doesn't match the configured dimension.
    pub async fn search_vector_with_distance(
        &self,
        embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<(CodeChunk, f32)>> {
        // Validate embedding dimension matches configured dimension
        if embedding.len() != self.dimension as usize {
            return Err(RetrievalErr::EmbeddingDimensionMismatch {
                expected: self.dimension,
                actual: embedding.len() as i32,
            });
        }

        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Use embedding directly - already validated
        let query_vec = embedding.to_vec();

        let results = table
            .vector_search(query_vec)
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?
            .limit(limit as usize)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut results_with_distance = Vec::new();
        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

            // Extract distances from _distance column (added by LanceDB vector search)
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            let chunks = Self::batch_to_chunks(&batch)?;

            for (i, chunk) in chunks.into_iter().enumerate() {
                // Default to 0.0 distance if not available (shouldn't happen for vector search)
                let distance = distances.map(|d| d.value(i)).unwrap_or(0.0);
                results_with_distance.push((chunk, distance));
            }
        }

        Ok(results_with_distance)
    }

    /// Search using full-text search (BM25), returning ChunkRefs.
    ///
    /// Use this when you plan to hydrate content from the file system
    /// to ensure fresh content is returned.
    pub async fn search_fts_refs(&self, query: &str, limit: i32) -> Result<Vec<ChunkRef>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let results = table
            .query()
            .full_text_search(FullTextSearchQuery::new(query.to_string()))
            .limit(limit as usize)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut refs = Vec::new();
        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;
            refs.extend(Self::batch_to_chunk_refs(&batch)?);
        }

        Ok(refs)
    }

    /// Search using vector similarity, returning ChunkRefs.
    ///
    /// Use this when you plan to hydrate content from the file system
    /// to ensure fresh content is returned.
    pub async fn search_vector_refs(&self, embedding: &[f32], limit: i32) -> Result<Vec<ChunkRef>> {
        if embedding.len() != self.dimension as usize {
            return Err(RetrievalErr::EmbeddingDimensionMismatch {
                expected: self.dimension,
                actual: embedding.len() as i32,
            });
        }

        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let query_vec = embedding.to_vec();

        let results = table
            .vector_search(query_vec)
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?
            .limit(limit as usize)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut refs = Vec::new();
        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;
            refs.extend(Self::batch_to_chunk_refs(&batch)?);
        }

        Ok(refs)
    }

    /// Validate a string for safe use in SQL queries.
    ///
    /// Only allows alphanumeric characters, path separators, underscores, hyphens, and dots.
    /// This prevents SQL injection by restricting the character set rather than
    /// trying to escape dangerous patterns.
    ///
    /// Used for both filepaths and workspace identifiers.
    fn validate_sql_identifier(value: &str, field_name: &str) -> Result<()> {
        if value.is_empty() {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("Empty {field_name}"),
            });
        }

        // Whitelist approach: only allow safe characters
        // Restricted set - removed @, +, (, ) which could be problematic
        let is_safe = value.chars().all(|c| {
            c.is_alphanumeric()
                || c == '/'
                || c == '\\'
                || c == '.'
                || c == '_'
                || c == '-'
                || c == ' '
                || c == ':' // For workspace identifiers like "ws:project"
        });

        if !is_safe {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("{field_name} contains potentially unsafe characters"),
            });
        }

        // Reject SQL injection patterns as defense in depth
        let dangerous_patterns = ['\0', ';', '\'', '"'];
        if value.chars().any(|c| dangerous_patterns.contains(&c)) {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("{field_name} contains dangerous SQL characters"),
            });
        }

        // Reject SQL comment patterns
        if value.contains("--") || value.contains("/*") || value.contains("*/") {
            return Err(RetrievalErr::FileNotIndexable {
                path: value.into(),
                reason: format!("{field_name} contains SQL comment markers"),
            });
        }

        Ok(())
    }

    /// Validate a filepath for safe use in SQL queries.
    fn validate_filepath(filepath: &str) -> Result<()> {
        Self::validate_sql_identifier(filepath, "filepath")
    }

    /// Validate a workspace identifier for safe use in SQL queries.
    fn validate_workspace(workspace: &str) -> Result<()> {
        Self::validate_sql_identifier(workspace, "workspace")
    }

    /// Delete chunks by file path.
    ///
    /// Validates the filepath to prevent SQL injection attacks.
    pub async fn delete_by_path(&self, filepath: &str) -> Result<i32> {
        // Validate filepath using whitelist approach (rejects quotes, so no escape needed)
        Self::validate_filepath(filepath)?;

        if !self.table_exists().await? {
            return Ok(0);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Count before delete
        let count_before = table.count_rows(None).await.unwrap_or(0);

        // Safe: filepath validated to not contain quotes or dangerous chars
        table
            .delete(&format!("filepath = '{filepath}'"))
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let count_after = table.count_rows(None).await.unwrap_or(0);

        Ok((count_before - count_after) as i32)
    }

    /// Count total chunks.
    pub async fn count(&self) -> Result<i64> {
        if !self.table_exists().await? {
            return Ok(0);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        table
            .count_rows(None)
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })
            .map(|c| c as i64)
    }

    /// List all chunks in the store with a default safety limit.
    ///
    /// Used for populating BM25 search cache during index loading.
    ///
    /// **Safety:** To prevent OOM on large repositories, a default limit of
    /// 100,000 chunks is applied. For larger repos, use `list_all_chunks_with_limit`
    /// with `None` (be careful!) or process chunks in batches.
    pub async fn list_all_chunks(&self) -> Result<Vec<CodeChunk>> {
        self.list_all_chunks_with_limit(Some(100_000)).await
    }

    /// List chunks with a configurable limit.
    ///
    /// - `limit: Some(n)` - Returns at most `n` chunks
    /// - `limit: None` - No limit (use with caution on large repos!)
    pub async fn list_all_chunks_with_limit(&self, limit: Option<i32>) -> Result<Vec<CodeChunk>> {
        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let query = table.query();
        let results = if let Some(n) = limit {
            query
                .limit(n as usize)
                .execute()
                .await
                .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                    table: self.table_name.clone(),
                    cause: e.to_string(),
                })?
        } else {
            query.execute().await.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?
        };

        let mut chunks = Vec::new();
        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;
            chunks.extend(Self::batch_to_chunks(&batch)?);
        }

        Ok(chunks)
    }

    /// Create a vector index for faster similarity search.
    ///
    /// Uses automatic index type selection (no quantization).
    /// For quantized indexes, use `create_vector_index_with_config`.
    pub async fn create_vector_index(&self) -> Result<()> {
        self.create_vector_index_with_config(None).await
    }

    /// Create a vector index with optional quantization configuration.
    ///
    /// Quantization reduces index size at the cost of some precision:
    /// - `None` or `QuantizationMethod::None`: Full precision (Index::Auto)
    /// - `QuantizationMethod::Scalar`: 4x compression, <1% recall loss
    /// - `QuantizationMethod::Product`: 4-8x compression, 1-3% recall loss
    pub async fn create_vector_index_with_config(
        &self,
        config: Option<&crate::config::QuantizationConfig>,
    ) -> Result<()> {
        use crate::config::QuantizationMethod;
        use lancedb::index::Index;
        use lancedb::index::vector::IvfHnswSqIndexBuilder;
        use lancedb::index::vector::IvfPqIndexBuilder;

        if !self.table_exists().await? {
            return Ok(());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Select index type based on quantization config
        let index = match config.map(|c| c.method).unwrap_or(QuantizationMethod::None) {
            QuantizationMethod::None => Index::Auto,
            QuantizationMethod::Scalar => Index::IvfHnswSq(IvfHnswSqIndexBuilder::default()),
            QuantizationMethod::Product => {
                let cfg = config.expect("config required for Product quantization");
                Index::IvfPq(
                    IvfPqIndexBuilder::default()
                        .num_sub_vectors(cfg.num_sub_vectors as u32)
                        .num_bits(cfg.num_bits as u32),
                )
            }
        };

        table
            .create_index(&["embedding"], index)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    /// Create a full-text search index.
    pub async fn create_fts_index(&self) -> Result<()> {
        if !self.table_exists().await? {
            return Ok(());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Create FTS index on content column
        table
            .create_index(&["content"], lancedb::index::Index::FTS(Default::default()))
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    // ========== Catalog-like operations for tweakcc indexing ==========

    /// Get file metadata for a specific file in a workspace.
    ///
    /// Returns the first chunk's metadata for the file, which contains
    /// content_hash, mtime, and indexed_at for change detection.
    pub async fn get_file_metadata(
        &self,
        workspace: &str,
        filepath: &str,
    ) -> Result<Option<FileMetadata>> {
        // Validate inputs to prevent SQL injection
        Self::validate_workspace(workspace)?;
        Self::validate_filepath(filepath)?;

        if !self.table_exists().await? {
            return Ok(None);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Safe: workspace and filepath validated to not contain quotes or dangerous chars
        let filter = format!("workspace = '{workspace}' AND filepath = '{filepath}'");

        let results = table
            .query()
            .only_if(filter)
            .limit(1)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

            if batch.num_rows() > 0 {
                let chunks = Self::batch_to_chunks(&batch)?;
                if let Some(chunk) = chunks.into_iter().next() {
                    return Ok(Some(FileMetadata {
                        filepath: chunk.filepath,
                        workspace: chunk.workspace,
                        content_hash: chunk.content_hash,
                        mtime: chunk.modified_time.unwrap_or(0),
                        indexed_at: chunk.indexed_at,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Get all file metadata in a workspace.
    ///
    /// Returns unique file entries with their metadata.
    pub async fn get_workspace_files(&self, workspace: &str) -> Result<Vec<FileMetadata>> {
        // Validate workspace to prevent SQL injection
        Self::validate_workspace(workspace)?;

        if !self.table_exists().await? {
            return Ok(Vec::new());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Safe: workspace validated to not contain quotes or dangerous chars
        let filter = format!("workspace = '{workspace}'");

        // Use select() to only fetch metadata columns (avoid loading content/embeddings)
        let results = table
            .query()
            .only_if(filter)
            .select(lancedb::query::Select::Columns(vec![
                "filepath".to_string(),
                "workspace".to_string(),
                "content_hash".to_string(),
                "mtime".to_string(),
                "indexed_at".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        // Collect unique files - use HashMap to deduplicate by filepath
        let mut files: std::collections::HashMap<String, FileMetadata> =
            std::collections::HashMap::new();

        let mut stream = results;
        while let Some(batch) = futures::StreamExt::next(&mut stream).await {
            let batch = batch.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

            // Parse only the columns we selected
            let filepaths = batch
                .column_by_name("filepath")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let workspaces = batch
                .column_by_name("workspace")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let content_hashes = batch
                .column_by_name("content_hash")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let mtimes = batch
                .column_by_name("mtime")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let indexed_ats = batch
                .column_by_name("indexed_at")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());

            if let Some(filepaths) = filepaths {
                for i in 0..batch.num_rows() {
                    let filepath = filepaths.value(i).to_string();
                    files.entry(filepath.clone()).or_insert(FileMetadata {
                        filepath,
                        workspace: workspaces
                            .map(|w| w.value(i).to_string())
                            .unwrap_or_default(),
                        content_hash: content_hashes
                            .map(|h| h.value(i).to_string())
                            .unwrap_or_default(),
                        mtime: mtimes.map(|m| m.value(i)).unwrap_or(0),
                        indexed_at: indexed_ats.map(|a| a.value(i)).unwrap_or(0),
                    });
                }
            }
        }

        Ok(files.into_values().collect())
    }

    /// Delete all chunks for a workspace.
    pub async fn delete_workspace(&self, workspace: &str) -> Result<i32> {
        // Validate workspace to prevent SQL injection
        Self::validate_workspace(workspace)?;

        if !self.table_exists().await? {
            return Ok(0);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let count_before = table.count_rows(None).await.unwrap_or(0);

        // Safe: workspace validated to not contain quotes or dangerous chars
        table
            .delete(&format!("workspace = '{workspace}'"))
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let count_after = table.count_rows(None).await.unwrap_or(0);

        Ok((count_before - count_after) as i32)
    }

    // ========== Index Policy Methods ==========

    /// Get current index status.
    ///
    /// Returns information about table existence, chunk count,
    /// and whether index creation is recommended.
    pub async fn get_index_status(&self, policy: &IndexPolicy) -> Result<IndexStatus> {
        let table_exists = self.table_exists().await?;

        if !table_exists {
            return Ok(IndexStatus::default());
        }

        let chunk_count = self.count().await?;

        let vector_index_recommended =
            policy.chunk_threshold > 0 && chunk_count >= policy.chunk_threshold;

        let fts_index_recommended =
            policy.fts_chunk_threshold > 0 && chunk_count >= policy.fts_chunk_threshold;

        Ok(IndexStatus {
            table_exists,
            chunk_count,
            vector_index_recommended,
            fts_index_recommended,
        })
    }

    /// Apply index policy - create indexes if thresholds are met.
    ///
    /// Returns true if any index was created.
    pub async fn apply_index_policy(
        &self,
        policy: &IndexPolicy,
        quantization_config: Option<&crate::config::QuantizationConfig>,
    ) -> Result<bool> {
        let status = self.get_index_status(policy).await?;

        if !status.table_exists {
            return Ok(false);
        }

        let mut created = false;

        if status.vector_index_recommended || policy.force_rebuild {
            self.create_vector_index_with_config(quantization_config)
                .await?;
            created = true;
        }

        if status.fts_index_recommended || policy.force_rebuild {
            self.create_fts_index().await?;
            created = true;
        }

        Ok(created)
    }

    /// Check if index creation is needed based on policy.
    pub async fn needs_index(&self, policy: &IndexPolicy) -> Result<bool> {
        let status = self.get_index_status(policy).await?;
        Ok(status.needs_indexing())
    }

    // ========== BM25 Metadata Methods ==========

    /// Get the schema for BM25 metadata table.
    fn bm25_metadata_schema() -> Schema {
        Schema::new(vec![
            Field::new("avgdl", DataType::Float32, false),
            Field::new("total_docs", DataType::Int64, false),
            Field::new("updated_at", DataType::Int64, false),
        ])
    }

    /// Check if BM25 metadata table exists.
    pub async fn bm25_metadata_exists(&self) -> Result<bool> {
        let tables = self.db.table_names().execute().await.map_err(|e| {
            RetrievalErr::LanceDbQueryFailed {
                table: BM25_METADATA_TABLE.to_string(),
                cause: e.to_string(),
            }
        })?;
        Ok(tables.contains(&BM25_METADATA_TABLE.to_string()))
    }

    /// Save BM25 metadata.
    pub async fn save_bm25_metadata(&self, metadata: &Bm25Metadata) -> Result<()> {
        let schema = Arc::new(Self::bm25_metadata_schema());

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Float32Array::from(vec![metadata.avgdl])),
                Arc::new(Int64Array::from(vec![metadata.total_docs])),
                Arc::new(Int64Array::from(vec![metadata.updated_at])),
            ],
        )
        .map_err(|e| RetrievalErr::LanceDbQueryFailed {
            table: BM25_METADATA_TABLE.to_string(),
            cause: e.to_string(),
        })?;

        let reader = arrow::record_batch::RecordBatchIterator::new(vec![Ok(batch)], schema.clone());

        if self.bm25_metadata_exists().await? {
            let table = self
                .db
                .open_table(BM25_METADATA_TABLE)
                .execute()
                .await
                .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                    table: BM25_METADATA_TABLE.to_string(),
                    cause: e.to_string(),
                })?;

            table
                .delete("avgdl >= 0")
                .await
                .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                    table: BM25_METADATA_TABLE.to_string(),
                    cause: format!("Failed to delete old metadata: {e}"),
                })?;

            table
                .add(reader)
                .execute()
                .await
                .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                    table: BM25_METADATA_TABLE.to_string(),
                    cause: format!("Failed to add new metadata: {e}"),
                })?;
        } else {
            self.db
                .create_table(BM25_METADATA_TABLE, reader)
                .execute()
                .await
                .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                    table: BM25_METADATA_TABLE.to_string(),
                    cause: e.to_string(),
                })?;
        }

        Ok(())
    }

    /// Load BM25 metadata.
    pub async fn load_bm25_metadata(&self) -> Result<Option<Bm25Metadata>> {
        if !self.bm25_metadata_exists().await? {
            return Ok(None);
        }

        let table = self
            .db
            .open_table(BM25_METADATA_TABLE)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: BM25_METADATA_TABLE.to_string(),
                cause: e.to_string(),
            })?;

        let mut stream = table.query().limit(1).execute().await.map_err(|e| {
            RetrievalErr::LanceDbQueryFailed {
                table: BM25_METADATA_TABLE.to_string(),
                cause: e.to_string(),
            }
        })?;

        use futures::StreamExt;
        if let Some(batch_result) = stream.next().await {
            let batch = batch_result.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: BM25_METADATA_TABLE.to_string(),
                cause: e.to_string(),
            })?;

            if batch.num_rows() == 0 {
                return Ok(None);
            }

            let avgdl = batch
                .column(0)
                .as_any()
                .downcast_ref::<Float32Array>()
                .map(|a| a.value(0))
                .unwrap_or(100.0);

            let total_docs = batch
                .column(1)
                .as_any()
                .downcast_ref::<Int64Array>()
                .map(|a| a.value(0))
                .unwrap_or(0);

            let updated_at = batch
                .column(2)
                .as_any()
                .downcast_ref::<Int64Array>()
                .map(|a| a.value(0))
                .unwrap_or(0);

            return Ok(Some(Bm25Metadata {
                avgdl,
                total_docs,
                updated_at,
            }));
        }

        Ok(None)
    }

    /// Load all chunk contents for BM25 scorer restoration.
    pub async fn load_all_chunk_contents(&self) -> Result<HashMap<String, String>> {
        let mut result = HashMap::new();

        if !self.table_exists().await? {
            return Ok(result);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut stream = table
            .query()
            .select(lancedb::query::Select::Columns(vec![
                "id".to_string(),
                "content".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        use futures::StreamExt;
        while let Some(batch_result) = stream.next().await {
            let batch = batch_result.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

            let ids = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let contents = batch
                .column_by_name("content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            if let (Some(ids_col), Some(contents_col)) = (ids, contents) {
                for i in 0..batch.num_rows() {
                    let id = ids_col.value(i).to_string();
                    let content = contents_col.value(i).to_string();
                    result.insert(id, content);
                }
            }
        }

        Ok(result)
    }

    /// Load all BM25 embeddings from chunks.
    pub async fn load_all_bm25_embeddings(&self) -> Result<HashMap<String, SparseEmbedding>> {
        let mut result = HashMap::new();

        if !self.table_exists().await? {
            return Ok(result);
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        let mut stream = table
            .query()
            .select(lancedb::query::Select::Columns(vec![
                "id".to_string(),
                "bm25_embedding".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

        use futures::StreamExt;
        while let Some(batch_result) = stream.next().await {
            let batch = batch_result.map_err(|e| RetrievalErr::LanceDbQueryFailed {
                table: self.table_name.clone(),
                cause: e.to_string(),
            })?;

            let ids = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| RetrievalErr::LanceDbQueryFailed {
                    table: self.table_name.clone(),
                    cause: "Invalid id column".to_string(),
                })?;

            let embeddings = batch
                .column_by_name("bm25_embedding")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            if let Some(embeddings_col) = embeddings {
                for i in 0..batch.num_rows() {
                    let id = ids.value(i).to_string();
                    let json = embeddings_col.value(i);
                    if !json.is_empty() {
                        if let Some(embedding) = SparseEmbedding::from_json(json) {
                            result.insert(id, embedding);
                        }
                    }
                }
            }
        }

        Ok(result)
    }
}

// ============================================================================
// BM25 Metadata Table Constant
// ============================================================================

const BM25_METADATA_TABLE: &str = "bm25_metadata";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========== LanceDbStore Integration Tests ==========

    /// Helper to create a test chunk with metadata.
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
        let store = LanceDbStore::open(dir.path()).await.unwrap();
        assert!(!store.table_exists().await.unwrap());
    }

    #[tokio::test]
    async fn test_store_and_count() {
        let dir = TempDir::new().unwrap();
        let store = LanceDbStore::open(dir.path()).await.unwrap();

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
        let store = LanceDbStore::open(dir.path()).await.unwrap();

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
        let store = LanceDbStore::open(dir.path()).await.unwrap();

        let chunks = vec![
            test_chunk("ws:test.rs:0", "ws", "test.rs", "fn main() {}", "abc123"),
            test_chunk("ws:test.rs:1", "ws", "test.rs", "fn foo() {}", "abc123"),
        ];

        store.store_chunks(&chunks).await.unwrap();

        // Get metadata for existing file
        let metadata = store.get_file_metadata("ws", "test.rs").await.unwrap();
        assert!(metadata.is_some());
        let meta = metadata.unwrap();
        assert_eq!(meta.filepath, "test.rs");
        assert_eq!(meta.workspace, "ws");
        assert_eq!(meta.content_hash, "abc123");
        assert_eq!(meta.mtime, 1700000000);

        // Get metadata for non-existent file
        let metadata = store
            .get_file_metadata("ws", "nonexistent.rs")
            .await
            .unwrap();
        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn test_get_workspace_files() {
        let dir = TempDir::new().unwrap();
        let store = LanceDbStore::open(dir.path()).await.unwrap();

        let chunks = vec![
            test_chunk("ws:a.rs:0", "ws", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws:a.rs:1", "ws", "a.rs", "fn a2() {}", "hash_a"),
            test_chunk("ws:b.rs:0", "ws", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();

        let files = store.get_workspace_files("ws").await.unwrap();
        assert_eq!(files.len(), 2); // a.rs and b.rs

        // Check that both files are present
        let filepaths: Vec<_> = files.iter().map(|f| f.filepath.as_str()).collect();
        assert!(filepaths.contains(&"a.rs"));
        assert!(filepaths.contains(&"b.rs"));
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let dir = TempDir::new().unwrap();
        let store = LanceDbStore::open(dir.path()).await.unwrap();

        let chunks = vec![
            test_chunk("ws1:a.rs:0", "ws1", "a.rs", "fn a() {}", "hash_a"),
            test_chunk("ws2:b.rs:0", "ws2", "b.rs", "fn b() {}", "hash_b"),
        ];

        store.store_chunks(&chunks).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        let deleted = store.delete_workspace("ws1").await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(store.count().await.unwrap(), 1);

        // Verify ws2 is still there
        let files = store.get_workspace_files("ws2").await.unwrap();
        assert_eq!(files.len(), 1);
    }
}
