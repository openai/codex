# Tantivy 搜索引擎

## 概览

**Tantivy** 是一个用 Rust 编写的全文搜索引擎库，类似于 Lucene。Tabby-Index 用它作为统一的索引存储和查询后端。

### 核心特性

- **全文搜索**：BM25 评分、TF-IDF、多字段搜索
- **向量搜索**：存储向量、余弦相似度查询
- **混合搜索**：关键词 + 向量联合排序
- **实时索引**：支持增量写入和查询
- **高性能**：内存映射 I/O、并发查询
- **可靠性**：ACID 事务、写入日志 (WAL)

## 索引初始化

### Schema 定义

```rust
// tantivy_utils.rs 中的 build_schema()

pub fn build_schema() -> Schema {
    let mut builder = Schema::builder();

    // ┌─── 基础字段 (所有文档) ──────────────┐
    // │ 用于索引 ID、源标识、分类           │
    // └────────────────────────────────────┘

    // 文档 ID (unique identifier)
    builder.add_text_field(
        "field_id",
        TEXT | STORED,  // INDEXED + STORED
    );

    // 来源标识 (repository, source)
    builder.add_text_field(
        "field_source_id",
        TEXT,           // INDEXED only
    );

    // 索引分类 (corpus: "code" or "structured_doc")
    builder.add_text_field(
        "field_corpus",
        TEXT,           // INDEXED only
    );

    // 文档元数据 (JSON)
    builder.add_text_field(
        "field_attributes",
        STRING | STORED,  // STORED as-is, no indexing
    );

    // 更新时间戳 (用于排序和增量同步)
    builder.add_i64_field(
        "field_updated_at",
        INDEXED,        // INDEXED for range queries
    );

    // 失败 chunk 计数
    builder.add_i64_field(
        "field_failed_chunks_count",
        STORED,         // STORED only
    );

    // ┌─── Chunk 字段 ──────────────────────┐
    // │ 用于代码片段和文档片段               │
    // └────────────────────────────────────┘

    // Chunk 唯一 ID
    builder.add_text_field(
        "field_chunk_id",
        TEXT,           // INDEXED only
    );

    // Chunk 元数据 (JSON)
    // 包含: 行号、语言、路径等
    builder.add_text_field(
        "field_chunk_attributes",
        STRING | STORED,  // STORED as-is
    );

    // Chunk 分词内容 (用于 BM25 搜索)
    builder.add_text_field(
        "field_chunk_tokens",
        TEXT,           // INDEXED, analyzed
    );

    // Chunk 向量 (binary embedding)
    builder.add_bytes_field(
        "field_chunk_embedding",
        STORED,         // STORED for retrieval
    );

    // ┌─── 代码特定字段 ──────────────────┐
    // │ 仅用于 CODE corpus                 │
    // └────────────────────────────────────┘

    builder.add_text_field("chunk_filepath", TEXT);
    builder.add_text_field("chunk_git_url", TEXT);
    builder.add_text_field("chunk_language", TEXT);
    builder.add_text_field("chunk_body", STRING | STORED);
    builder.add_i64_field("chunk_start_line", INDEXED);
    builder.add_text_field("commit", TEXT);

    // ┌─── 文档特定字段 ──────────────────┐
    // │ 仅用于 STRUCTURED_DOC corpus       │
    // └────────────────────────────────────┘

    builder.add_text_field("kind", TEXT);  // "web"/"issue"/"pull"/...

    // commit document fields
    builder.add_text_field("commit_sha", TEXT);
    builder.add_text_field("commit_message", STRING | STORED);
    builder.add_text_field("commit_author_email", TEXT);
    builder.add_i64_field("commit_author_at", INDEXED);

    // issue document fields
    builder.add_text_field("issue_link", TEXT);
    builder.add_text_field("issue_title", TEXT);
    builder.add_text_field("issue_body", TEXT);
    builder.add_text_field("issue_author_email", TEXT);
    builder.add_bool_field("issue_closed", INDEXED);

    // ... 更多文档类型字段

    builder.build()
}
```

### 字段选项详解

```rust
Field Options:
  ├─ INDEXED: 用于全文搜索 (BM25, term queries)
  ├─ STORED: 保存原始值，用于返回结果
  ├─ TEXT: 分词索引 (analyzed)
  │         "hello world" → ["hello", "world"]
  ├─ STRING: 精确匹配索引 (not analyzed)
  │          "hello world" → ["hello world"]
  └─ FAST: 快速范围查询 (numeric fields)

典型组合:
  ├─ INDEXED | STORED (text)
  │  → 可搜索，可返回
  ├─ INDEXED (text)
  │  → 可搜索，不返回原始值
  ├─ STORED (string)
  │  → 不搜索，可返回原始值
  └─ INDEXED | FAST (numeric)
     → 可范围查询，快速聚合
```

## 索引操作

### 打开或创建索引

```rust
// tantivy_utils.rs
pub async fn open_or_create_index(
    index_path: &Path,
    schema: Schema,
) -> Result<(IndexReader, IndexWriter)> {
    // 1. 检查索引是否存在
    if index_path.exists() {
        // 2. 打开现有索引
        let index = Index::open_in_dir(index_path)?;

        // 3. 自动恢复 (处理崩溃)
        // Tantivy 检查 WAL (write-ahead log)
        // 如果存在未提交的更改，自动回滚或恢复

        // 4. 创建读写器
        let reader = index.reader()?;
        let writer = index.writer()?;

        Ok((reader, writer))
    } else {
        // 5. 创建新索引
        let index = Index::create_in_dir(index_path, schema)?;

        let reader = index.reader()?;
        let writer = index.writer()?;

        Ok((reader, writer))
    }
}
```

### 写入文档

```rust
// indexer.rs: Indexer::add()

pub async fn add<T: ToIndexId>(
    &self,
    source_id: &str,
    corpus: &str,
    document: T,
    builder: Arc<dyn IndexAttributeBuilder<T>>,
) -> Result<()> {
    // 1. 构建 Tantivy 文档
    let doc_builder = TantivyDocBuilder::new(document, builder);
    let tan_docs = doc_builder.build().await?;

    // 2. 逐个添加到 Tantivy
    for (index_id, tan_doc) in tan_docs {
        // TantivyDocument 是 Tantivy 的文档表示
        // 包含所有字段值

        self.writer.add_document(tan_doc)?;
    }

    // 注意: 此时只是添加到内存缓冲，未提交到磁盘
    Ok(())
}

// Tantivy Document 构建示例
pub fn build_tan_doc(
    index_id: &IndexId,
    attributes: &serde_json::Value,
    chunk_tokens: &[String],
    embedding: &[f32],
) -> TantivyDocument {
    use tantivy::doc;

    doc!(
        field_id => index_id.encode(),
        field_source_id => index_id.source_id.clone(),
        field_corpus => "code",
        field_attributes => serde_json::to_string(attributes).unwrap(),
        field_updated_at => Utc::now().timestamp(),
        field_chunk_id => "chunk_0",
        field_chunk_attributes => "{}",
        field_chunk_tokens => chunk_tokens.join(" "),
        field_chunk_embedding => binarize_embedding(embedding),
        chunk_filepath => "/path/to/file.rs",
        chunk_language => "rust",
        commit => "abc123def",
    )
}
```

### 删除文档

```rust
// indexer.rs: Indexer::delete()

pub async fn delete(&self, source_id: &str) -> Result<()> {
    // 删除所有来自 source_id 的文档
    // 使用 delete_query: 按条件删除

    let query_parser = QueryParser::for_index(
        &self.index,
        vec![self.schema.get_field("field_source_id").unwrap()],
    );

    let query = query_parser.parse_query(source_id)?;

    self.writer.delete_query(query)?;

    Ok(())
}

pub async fn delete_doc(&self, index_id: &IndexId) -> Result<()> {
    // 删除特定文档

    let query = Query::term(
        self.schema.get_field("field_id").unwrap(),
        Term::from_field_text(
            &index_id.encode()
        ),
    );

    self.writer.delete_query(query)?;

    Ok(())
}
```

### 提交事务

```rust
// indexer.rs: Indexer::commit()

pub async fn commit(&self) -> Result<()> {
    // 提交所有待写入的更改到磁盘

    // 1. 内存缓冲 → 磁盘
    //    Tantivy 内部:
    //    - 刷新内存缓冲 (512 MB 默认)
    //    - 写入段文件 (segment files)
    //    - 更新元数据 (meta.json)

    self.writer.commit()?;

    // 2. 内存映射索引
    //    - 重新加载索引文件
    //    - 更新查询读器 (searcher)
    //    - 允许即时查询

    Ok(())
}
```

## 查询和搜索

### 全文搜索 (BM25)

```rust
pub async fn search_text(
    &self,
    query_text: &str,
    field: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // 1. 创建查询解析器
    let query_parser = QueryParser::for_index(
        &self.index,
        vec![self.schema.get_field(field).unwrap()],
    );

    // 2. 解析查询字符串
    // "hello world" → (hello OR world)
    let query = query_parser.parse_query(query_text)?;

    // 3. 创建查询执行器
    let searcher = self.reader.searcher();

    // 4. 执行查询
    // Tantivy 内部:
    // - 遍历倒排索引 (inverted index)
    // - 计算 BM25 相关性分数
    // - 返回前 N 个结果
    let (count, docs) = searcher.search(
        &query,
        &TopDocs::with_limit(limit),
    )?;

    // 5. 检索存储字段
    let mut results = vec![];
    for (_score, doc_address) in docs {
        let doc = searcher.doc(doc_address)?;
        results.push(SearchResult {
            id: doc.get_first(field_id).unwrap().text().to_string(),
            score: _score,
            // ... 其他字段
        });
    }

    Ok(results)
}
```

### 向量搜索 (余弦相似度)

```rust
pub async fn search_embedding(
    &self,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Tantivy 向量搜索示例 (简化)

    // 1. 转换查询向量为 binary 格式
    let query_binary = binarize_embedding(query_embedding);

    // 2. 创建 Term 查询
    // 精确匹配 embedding 的前 N 维
    // (Tantivy 的向量支持有限)

    // 3. 或使用 HNSW 索引 (if available)
    // let index = HnswIndex::new(...);
    // let neighbors = index.search(query_embedding, limit)?;

    // 实际上 Tabby-Index 存储向量但通过额外的
    // embedding service 进行向量搜索
    // 这超出了纯 Tantivy 的范围

    todo!("需要外部向量检索库")
}
```

### 复合查询

```rust
pub async fn search_hybrid(
    &self,
    query_text: &str,
    corpus: &str,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // 1. 文本查询
    let text_results = self.search_text(query_text, "field_chunk_tokens", limit)
        .await?;

    // 2. 过滤 corpus
    let filtered = text_results
        .into_iter()
        .filter(|r| r.corpus == corpus)
        .take(limit)
        .collect();

    Ok(filtered)
}

// 范围查询示例
pub async fn search_by_updated_time(
    &self,
    since: i64,
    until: i64,
) -> Result<Vec<SearchResult>> {
    // 范围查询: since <= updated_at <= until

    let query = RangeQuery::new(
        self.schema.get_field("field_updated_at").unwrap(),
        since..until,
    );

    let searcher = self.reader.searcher();
    let docs = searcher.search(&query, &TopDocs::with_limit(1000))?;

    // ... 处理结果
    Ok(...)
}
```

## 索引性能和优化

### 内存映射 I/O

```
Tantivy 索引存储:
  ├─ .tan 目录
  │   ├─ meta.json          # 索引元数据
  │   ├─ segment_*          # 索引段目录
  │   │   ├─ .fdt           # 存储字段数据
  │   │   ├─ .fdt           # 快速字段数据
  │   │   ├─ .idx           # 倒排索引
  │   │   └─ ...
  │   └─ ...
  └─

访问模式:
  ├─ 顺序读取: 文件 → 内存映射
  │          (虚拟内存管理自动缓存)
  ├─ 随机查询: 内存映射 → CPU cache
  │          (高速缓存命中率高)
  └─ 写入缓冲: 内存 → 512 MB 缓冲
               → 磁盘 (commit 时)

性能特性:
  ├─ 查询延迟: μs 级别
  ├─ 吞吐量: 100k queries/sec (single thread)
  ├─ 并发查询: 无锁读取
  └─ 内存占用: 按需加载
```

### 段合并 (Segment Merging)

```
问题: 大量小更新会产生大量段,查询变慢

解决:
  ├─ 后台合并: Tantivy 自动合并小段
  ├─ 合并策略: 可配置 (default: merge_factor=10)
  └─ 性能权衡: GC pause vs. 查询速度

Tantivy 内部:
  1. 监控段数量
  2. 达到阈值时触发合并
  3. 合并多个小段为一个大段
  4. 重新计算倒排索引
  5. 删除旧段

配置示例:
  let mut index_writer_setting = IndexWriterSettings::default();
  index_writer_setting.merge_policy = MergePolicy::with_merge_factor(10);
  let writer = index.writer_with_settings(index_writer_setting)?;
```

### 缓存策略

```rust
// Tantivy 缓存机制

// 1. Query Cache
//    缓存最近执行的查询结果
//    避免重复计算
//    大小: 可配置 (default: 100)

// 2. Block Cache
//    缓存块读取 (BM25 计算)
//    加速评分
//    大小: ~10% 索引大小

// 3. Field Cache
//    缓存字段值 (排序、聚合)
//    大小: 可配置

// 配置示例
let index = Index::open_in_dir(path)?;
index.set_query_cache_num_blocks(1000);  // 增加缓存
```

## 故障恢复和可靠性

### 写入日志 (WAL)

```
Tantivy 确保持久性:

  ┌─ 正常操作 ─────────────────┐
  │                            │
  │ 1. 写入内存缓冲            │
  │ 2. 写入 WAL (disk)        │
  │ 3. 返回成功                │
  │ 4. Commit 时刷新到索引    │
  │                            │
  └────────────────────────────┘

  崩溃恢复:
    ├─ 发现未完成的事务
    ├─ 重放 WAL 日志
    ├─ 恢复内存状态
    └─ 继续处理

可靠性保证:
  ├─ 原子性: commit 成功或全部回滚
  ├─ 一致性: 索引始终有效
  ├─ 隔离性: 读取不受写入影响 (MVCC)
  └─ 持久性: commit 后数据永久保存
```

### 检查点 (Checkpoints)

```rust
// 创建检查点以便恢复

pub async fn create_checkpoint(&self) -> Result<CheckpointId> {
    // 1. 当前提交所有更改
    self.writer.commit()?;

    // 2. 记录当前索引状态
    let checkpoint = CheckpointId {
        timestamp: Utc::now(),
        commit_hash: self.get_current_commit_hash()?,
        documents_indexed: self.get_doc_count()?,
    };

    // 3. 保存检查点元数据
    checkpoint.save()?;

    Ok(checkpoint)
}

pub async fn rollback_to_checkpoint(
    &self,
    checkpoint: CheckpointId,
) -> Result<()> {
    // 恢复到指定检查点
    // 需要重新打开索引到特定版本
    todo!()
}
```

## 索引大小和存储

### 存储占用估算

```
存储计算:
  ├─ 原始文本: ~1 byte/char
  ├─ 倒排索引: ~10-20% 原始大小 (高压缩)
  ├─ 存储字段: ~50-100% 原始大小 (可选压缩)
  ├─ 向量数据: ~40-100 bytes/vector (维度 × 4)
  └─ 总计: ~150-200% 原始大小

示例:
  1 GB 源代码
    ├─ 倒排索引: ~150 MB
    ├─ 存储字段: ~500 MB
    └─ 总索引: ~700 MB

优化策略:
  ├─ 压缩: Tantivy 自动压缩段
  ├─ 去重: 避免重复文本
  ├─ 选择性存储: 只存储必要字段
  └─ 分段清理: 垃圾回收
```

## 对 codex-rs 的实现参考

### 1. Schema 设计

```rust
// 建议: 为 Rust 代码索引扩展 schema

builder.add_text_field("chunk_crate_name", TEXT);      // crate 名称
builder.add_text_field("chunk_module_path", TEXT);     // 模块路径
builder.add_text_field("chunk_visibility", TEXT);      // pub/private
builder.add_text_field("chunk_trait_impls", TEXT);     // 实现的 trait

// 这些字段可用于精确过滤和相关性排序
```

### 2. 查询优化

```rust
// 实现混合查询: 关键词 + 向量
pub async fn search_rust_code(
    &self,
    query: &str,
    embedding: &[f32],
) -> Result<Vec<SearchResult>> {
    // 1. 关键词搜索
    let text_results = self.search_text(query, "field_chunk_tokens", 100)?;

    // 2. 向量搜索 (通过外部服务)
    let vector_results = self.vector_search(embedding, 100)?;

    // 3. 融合结果 (加权平均)
    Ok(fuse_results(&text_results, &vector_results))
}
```

### 3. 性能调优

```rust
// 为 codex-rs 优化:

// 增加缓冲大小 (处理大型仓库)
let settings = IndexWriterSettings {
    buffer_size: 1024 * 1024 * 1024,  // 1 GB
    merge_factor: 10,
};

// 启用查询缓存
index.set_query_cache_num_blocks(10000);

// 异步提交 (降低延迟)
writer.commit_and_prepare()?;
```

---

**相关文档**：
- [系统架构](./architecture.md) - 搜索引擎在整体架构中的位置
- [索引构建流程](./indexing-process.md) - 如何向 Tantivy 写入数据
