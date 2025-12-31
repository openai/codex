# Tabby-Index 系统架构

## 概览

Tabby-Index 采用**两层索引架构**，分别处理**代码索引**和**结构化文档索引**，均基于 Tantivy 全文搜索引擎。

```
┌─────────────────────────────────────────────────────────────┐
│                   Tabby-Index System                         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────────┐    ┌──────────────────────────┐   │
│  │   Code Indexing      │    │  Document Indexing       │   │
│  │  (CODE Corpus)       │    │  (STRUCTURED_DOC Corpus) │   │
│  ├──────────────────────┤    ├──────────────────────────┤   │
│  │ • Source files       │    │ • Commits                │   │
│  │ • AST analysis       │    │ • Issues                 │   │
│  │ • Tag extraction     │    │ • Pull requests          │   │
│  │ • Smart chunking     │    │ • Web pages              │   │
│  │ • Embedding          │    │ • Pages                  │   │
│  │ • Incremental update │    │ • Custom documents       │   │
│  │ • Garbage collection │    │ • Incremental sync       │   │
│  └──────────────────────┘    └──────────────────────────┘   │
│           │                              │                   │
│           ├─ Git Repository Sync         │                   │
│           └─ Embedding Service           ├─ Embedding Service
│                                          │                   │
│        ┌──────────────────────────────────┬────────────┐     │
│        │      Tantivy Search Engine       │            │     │
│        │  (Unified Index Storage)         │            │     │
│        ├──────────────────────────────────┼────────────┤     │
│        │  Schema Fields:                  │ Memory-    │     │
│        │  • field_id                      │ Mapped     │     │
│        │  • field_source_id               │ Directory  │     │
│        │  • field_corpus                  │            │     │
│        │  • field_attributes (JSON)       │            │     │
│        │  • field_chunk_*                 │            │     │
│        │  • field_chunk_tokens (vector)   │            │     │
│        └──────────────────────────────────┴────────────┘     │
│                        │                                      │
│        ┌───────────────┴───────────────┐                     │
│        │   Full-Text Search            │                     │
│        │   Vector Similarity Search     │                     │
│        └───────────────────────────────┘                     │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## 索引层级设计

### 层级 1：代码索引 (Code Corpus)

```
Git Repository
    ↓ [repository::sync_repository()]
    ├─ Clone/Pull latest code
    └─ Retrieve commit SHA
    ↓
File Tree Walker
    ├─ Ignore files (.gitignore, etc.)
    └─ Batch files (chunk size: 100)
    ↓
Per-File Processing
    ├─ Parse with TreeSitter (AST)
    ├─ Extract tags (definitions)
    ├─ Compute metrics (validity check)
    ├─ Smart chunking (CodeSplitter)
    └─ Generate embeddings
    ↓
Tantivy Indexing
    ├─ Create SourceCode document
    ├─ Add chunk attributes
    ├─ Write to Tantivy index
    └─ Commit transaction
    ↓
Maintenance
    ├─ Incremental update (source_file_id change)
    ├─ Garbage collection (deleted files)
    └─ Failure tracking
```

**关键特性**：
- **AST-Aware Chunking**：按代码结构分块，保留语义边界
- **Source File ID**：基于 path + language + git_hash，高效检测文件变更
- **Validity Filtering**：过滤掉生成代码、极长行、二进制内容
- **Incremental**：只更新变更的文件，减少重复工作

### 层级 2：结构化文档索引 (Structured Doc Corpus)

```
External Document Sources
    ├─ Git Commits
    ├─ Issue Tracker API
    ├─ Pull Request API
    ├─ Web URLs
    ├─ Documentation Pages
    └─ Custom Ingested Data
    ↓
StructuredDocBuilder
    ├─ Transform to StructuredDoc enum
    ├─ Extract attributes (title, author, time, etc.)
    ├─ Split into chunks
    ├─ Generate embeddings
    └─ Build attributes JSON
    ↓
Sync State Tracking
    ├─ StructuredDocState (id, updated_at, deleted)
    └─ Detect changes (updated_at timestamp)
    ↓
Tantivy Indexing
    ├─ Add/Update/Delete documents
    ├─ Support concurrent buffer operations
    └─ Commit transaction
    ↓
Maintenance
    ├─ Garbage collection (via callback)
    ├─ Stale document cleanup
    └─ Ingested doc retention policy
```

**关键特性**：
- **Multi-Type Support**：6 种文档类型，统一接口
- **Incremental Sync**：基于 updated_at 时间戳
- **Flexible Cleanup**：外部控制保留策略
- **Concurrent Processing**：buffer_unordered 高效批处理

## 数据流

### 完整的索引构建数据流

```
┌─ 代码索引流程 ─────────────────────────────────────────────┐
│                                                               │
│  Repository → File Tree → AST Parser → Tag Extraction       │
│       (git2)       (ignore) (tree-sitter)  (tree-sitter-tags)
│                                                               │
│  Metrics Computation → Validity Check → Smart Chunking      │
│  (code metrics)      (5 criteria)      (CodeSplitter +      │
│                                         TextSplitter)       │
│                                                               │
│  Embedding Generation → Vectorization → Tantivy Indexing   │
│  (tabby-inference)     (binarize)      (schema)             │
│                                                               │
│  Incremental Update Detection → GC → Commit                │
│  (SourceFileId changed?)        (orphan cleanup)             │
│                                                               │
└─────────────────────────────────────────────────────────────┘

┌─ 文档索引流程 ─────────────────────────────────────────────┐
│                                                               │
│  External Source → Transform → Attribute Extraction         │
│  (API/Git)       (enum)       (JSON)                        │
│                                                               │
│  Chunking → Embedding → Vectorization → Tantivy Indexing   │
│  (text)     (model)    (binarize)      (schema)             │
│                                                               │
│  State Tracking → Incremental Sync → GC → Commit           │
│  (timestamp)     (presync check)     (callback)             │
│                                                               │
└─────────────────────────────────────────────────────────────┘

┌─ 查询流程 ─────────────────────────────────────────────────┐
│                                                               │
│  Query → Tantivy Search → Field Scoring → Reranking        │
│  (text)  (full-text)      (BM25 + TF-IDF) (optional)       │
│                                                               │
│  Vector Query → Similarity Search → Score Fusion            │
│  (embedding)   (cosine similarity) (hybrid ranking)         │
│                                                               │
│  Return Results + Metadata → Application                     │
│  (chunk + attributes)                                        │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## 模块交互图

```
┌─────────────────────────────────────────────────────────────┐
│                    Indexer (核心引擎)                        │
│  ─────────────────────────────────────────────────────────  │
│  • Indexer: 索引读写操作                                     │
│  • TantivyDocBuilder<T>: 通用文档构建                      │
│  • IndexGarbageCollector: 垃圾回收                          │
│  • IndexAttributeBuilder<T>: 属性构建 trait                 │
└─────────────────────────────────────────────────────────────┘
                    ▲           ▲           ▲
                    │           │           │
        ┌───────────┴─────┬─────┴────┬──────┴──────┐
        │                 │          │             │
        │                 │          │             │
        │                 │          │             │
    ┌───┴──┐         ┌────┴───┐  ┌──┴────┐  ┌────┴───┐
    │ Code ◄────────►│Structured
    │Index          │ Doc
    │  er │         │ Indexer
    │    │          │       │
    └────┘          └───────┘
        │                 │
        │                 │
    ┌───┴─────────┬───────┴───┐
    │             │           │
    │      Tantivy Index      │
    │   (Unified Storage)     │
    │                         │
    └─────────────────────────┘
        │           │
        │  Schema   │
        │  Fields   │
        │           │
        └─────┬─────┘
              │
        ┌─────▼──────┐
        │   Search   │
        │ + Retrieval│
        └────────────┘
```

## 关键抽象

### 1. IndexId 结构

```
IndexId = (source_id: String, id: String)

purpose: 全局唯一索引标识
examples:
  - ("github.com/user/repo", "src/main.rs:0")  // code chunk
  - ("github.com/user/repo", "commit:abc123:0") // commit doc
  - ("web:docs.example.com", "page:guide:0")    // web page
```

### 2. SourceFileId 结构

```
SourceFileId {
  path: PathBuf,           // "src/main.rs"
  language: String,        // "rust"
  git_hash: String,        // 文件内容 SHA256
}

purpose: 高效的文件变更检测
mechanism:
  1. 索引存储: 解析 SourceFileId
  2. 运行时: 重新计算当前文件 SourceFileId
  3. 对比: 不匹配 → 文件已修改或删除 → 需要更新
```

### 3. Corpus 分类

```
Corpus 用于逻辑分离不同类型的索引数据

| Corpus           | 含义 | 文档类型 |
|------------------|------|--------|
| "code"           | 代码索引 | SourceCode |
| "structured_doc" | 文档索引 | StructuredDoc (6 variants) |

advantage:
  - 可分别查询或联合查询
  - 易于删除某类索引
  - 统计和过滤方便
```

### 4. Chunk 设计

```
Chunk = 代码/文档分块后的单位

attributes:
  - chunk_id: 唯一标识 (在文档内)
  - chunk_tokens: 分词索引字符串 (for BM25)
  - chunk_attributes: JSON 元数据
  - source_file_id: 文件身份标识 (仅代码)

benefits:
  - 精细粒度: 可返回具体片段而不是整个文件
  - 向量化: 每个 chunk 单独 embedding
  - 去重: 避免相同内容多次索引
```

## 存储设计

### Tantivy 索引字段

```
BASE FIELDS (所有文档):
  ├─ field_id: String (Stored + Indexed)
  │  └─ IndexId 字符串表示
  ├─ field_source_id: String (Indexed)
  │  └─ 来源标识 (仓库/源)
  ├─ field_corpus: String (Indexed, Enum)
  │  └─ "code" 或 "structured_doc"
  ├─ field_attributes: String (Stored)
  │  └─ JSON 格式元数据
  ├─ field_updated_at: i64 (Indexed)
  │  └─ 时间戳，用于排序
  └─ field_failed_chunks_count: i64 (Stored)
     └─ 失败块数计数

CHUNK FIELDS (分块数据):
  ├─ field_chunk_id: String (Indexed)
  ├─ field_chunk_attributes: String (Stored, JSON)
  ├─ field_chunk_tokens: String (Indexed, TEXT)
  │  └─ 分词内容，BM25 搜索
  └─ field_chunk_embedding: Vec<u8> (可选，binary vector)
     └─ 向量表示，余弦相似度搜索

CODE-SPECIFIC FIELDS:
  ├─ chunk_filepath: String (Indexed)
  ├─ chunk_git_url: String (Indexed)
  ├─ chunk_language: String (Indexed, Enum)
  ├─ chunk_body: String (Stored, code content)
  ├─ chunk_start_line: Optional<i32> (Indexed)
  └─ commit: String (Indexed, git sha)

DOC-SPECIFIC FIELDS:
  ├─ kind: String (Indexed, Enum)
  │  └─ "web" | "issue" | "pull" | "commit" | "page" | "ingested"
  ├─ commit: {SHA, MESSAGE, AUTHOR_EMAIL, AUTHOR_AT}
  ├─ issue: {LINK, TITLE, AUTHOR_EMAIL, BODY, CLOSED}
  ├─ pull: {LINK, TITLE, AUTHOR_EMAIL, BODY, DIFF, MERGED}
  ├─ web: {TITLE, LINK, CHUNK_TEXT}
  ├─ page: {LINK, TITLE, CHUNK_CONTENT}
  └─ ingested: {TITLE, LINK, CHUNK_BODY}
```

## 并发和性能特性

### 并发处理

```
代码索引:
  ├─ File tree walk: sequential (ignore rules application)
  ├─ Batch processing: chunks(100)
  ├─ Per-chunk: 并发处理 (tokio spawn)
  │   ├─ AST parsing: sequential (tree-sitter)
  │   ├─ Embedding: parallel (concurrent API calls)
  │   └─ Indexing: parallel (Tantivy supports concurrent writes)
  └─ Commit: atomic transaction

文档索引:
  ├─ Document iteration: sequential
  ├─ State checking: parallel (presync)
  ├─ Chunk building: buffer_unordered (concurrent)
  │   └─ 12 concurrent tasks (default)
  ├─ Embedding: parallel
  └─ Batch write: Tantivy atomic

Tantivy:
  ├─ In-memory: 512MB buffer (configurable)
  ├─ Merge threads: parallel segment merge
  └─ Readers: lock-free (MVCC)
```

### 性能优化

| 优化技术 | 应用场景 | 效果 |
|--------|--------|------|
| **Incremental Update** | 文件变更检测 | 仅更新变更文件 (~5-10% overhead) |
| **Batch Processing** | 索引写入 | 减少事务次数，提升吞吐 |
| **Concurrent Chunking** | embedding 计算 | 并行 API 调用，减少延迟 |
| **Memory-Mapped IO** | 索引存储 | 零拷贝读取，高性能查询 |
| **CodeSplitter** | 代码分块 | 保留语义边界，减少不必要的chunk |
| **Lazy Static** | 语言配置 | 一次性初始化，无重复开销 |

---

**相关文档**：
- [核心模块详解](./modules.md) - 各模块具体实现
- [索引构建流程](./indexing-process.md) - 详细流程步骤
- [Tantivy 搜索引擎](./tantivy.md) - 搜索和查询机制
