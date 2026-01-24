# 核心模块详解

## 模块总览

```
crates/tabby-index/src/
├── lib.rs                          # 库入口，暴露公开 API
├── indexer.rs                      # 核心索引引擎 (528 行)
├── tantivy_utils.rs                # Tantivy 工具函数
├── testutils.rs                    # 测试工具
├── indexer_tests.rs                # 索引器测试 (359 行)
├── code/                           # 代码索引模块
│   ├── mod.rs                      # 代码索引主模块 (157 行)
│   ├── types.rs                    # 源代码类型定义 (80 行)
│   ├── index.rs                    # 代码索引构建流程 (242 行)
│   ├── repository.rs               # Git 仓库同步 (116 行)
│   ├── languages.rs                # 语言配置管理 (185 行)
│   └── intelligence/               # AST 智能分析
│       ├── mod.rs                  # 代码智能主模块 (278 行)
│       └── id.rs                   # 源文件 ID 生成 (60 行)
└── structured_doc/                 # 结构化文档索引模块
    ├── mod.rs                      # 构建器主模块 (52 行)
    ├── public.rs                   # 公开 API (229 行)
    ├── types.rs                    # 文档类型定义 (138 行)
    └── types/                      # 具体文档类型实现
        ├── commit.rs               # 提交文档 (55 行)
        ├── issue.rs                # Issue 文档 (58 行)
        ├── pull.rs                 # PR 文档 (68 行)
        ├── web.rs                  # 网页文档 (82 行)
        ├── page.rs                 # 页面文档 (66 行)
        └── ingested.rs             # 自定义文档 (75 行)
```

## 1. indexer.rs - 核心索引引擎

### 概述

提供 Tantivy 索引的高级抽象，支持文档的增删改查，以及垃圾回收。

### 关键结构体

#### IndexId (第 28 行)

```rust
pub struct IndexId {
    pub source_id: String,  // 来源标识 (e.g., "github.com/user/repo")
    pub id: String,         // 文档内唯一ID (e.g., "src/main.rs:0")
}

impl IndexId {
    pub fn encode(&self) -> String {
        format!("{}::{}", self.source_id, self.id)
    }

    pub fn decode(encoded: &str) -> Self {
        let (source_id, id) = encoded.split_once("::").unwrap();
        IndexId {
            source_id: source_id.to_string(),
            id: id.to_string(),
        }
    }
}
```

**用途**：
- 全局唯一索引标识
- Tantivy 文档 ID
- 支持跨索引的去重

#### Trait: ToIndexId (第 33 行)

```rust
pub trait ToIndexId {
    fn to_index_id(&self) -> IndexId;
}

// 实现例子:
impl ToIndexId for SourceCode {
    fn to_index_id(&self) -> IndexId {
        IndexId {
            source_id: format!("code::{}", self.git_url),
            id: format!("{}:{}", self.filepath, self.start_line.unwrap_or(0)),
        }
    }
}
```

#### Trait: IndexAttributeBuilder<T> (第 38 行)

```rust
#[async_trait]
pub trait IndexAttributeBuilder<T>: Send + Sync {
    /// 构建文档级属性 (JSON)
    async fn build_attributes(&self, document: &T) -> serde_json::Value;

    /// 异步构建 chunk 属性流
    /// 返回 (tokens: Vec<String>, attributes: JSON)
    async fn build_chunk_attributes<'a>(
        &self,
        document: &'a T,
    ) -> BoxStream<'a, JoinHandle<Result<(Vec<String>, serde_json::Value)>>>;
}

// 约束条件:
// - Send + Sync: 支持跨线程使用
// - 必须处理异步计算 (embedding 等)
// - chunk 流支持并发处理
```

**实现者**：
- `CodeBuilder` - 代码索引属性构建
- `StructuredDocBuilder` - 文档索引属性构建

#### Struct: TantivyDocBuilder<T> (第 49 行)

```rust
pub struct TantivyDocBuilder<T> {
    document: T,
    builder: Arc<dyn IndexAttributeBuilder<T>>,
    // ... internal fields
}

impl<T: ToIndexId + Send + Sync + 'static> TantivyDocBuilder<T> {
    pub async fn build(self) -> Result<Vec<(IndexId, TantivyDocument)>> {
        // 1. 调用 builder.build_attributes() 构建文档属性
        let doc_attrs = self.builder.build_attributes(&self.document).await?;

        // 2. 并发构建 chunk 属性
        let mut chunks = vec![];
        let chunk_stream = self.builder
            .build_chunk_attributes(&self.document)
            .await?;

        pin_mut!(chunk_stream);
        while let Some(handle) = chunk_stream.next().await {
            match handle.await {
                Ok((tokens, attrs)) => {
                    chunks.push((tokens, attrs));
                },
                Err(e) => {
                    // 记录失败，继续处理其他 chunk
                    self.failed_chunks_count += 1;
                }
            }
        }

        // 3. 构建 TantivyDocument 并返回
        Ok(vec![...])
    }
}

// 关键特性:
// - 并发处理多个 chunk (JoinHandle)
// - 容错处理 (记录失败数)
// - 返回多个索引文档 (chunk per document)
```

**工作流**：
```
TantivyDocBuilder::build()
    ├─ Document → build_attributes() → JSON
    ├─ Document → build_chunk_attributes() → Stream<JoinHandle>
    │   ├─ chunk1 → embedding() [async]
    │   ├─ chunk2 → embedding() [async]
    │   └─ chunk3 → embedding() [async]
    ├─ Collect all chunks
    ├─ Count failures
    └─ Return Vec<(IndexId, TantivyDocument)>
```

#### Struct: Indexer (第 192 行)

```rust
pub struct Indexer {
    reader: IndexReader,
    writer: IndexWriter,
    schema: Schema,
}

impl Indexer {
    pub async fn add<T: ToIndexId + Send + Sync + 'static>(
        &self,
        source_id: &str,
        corpus: &str,
        document: T,
        builder: Arc<dyn IndexAttributeBuilder<T>>,
    ) -> Result<()> {
        // 1. 使用 TantivyDocBuilder 构建
        let docs = TantivyDocBuilder::new(document, builder).build().await?;

        // 2. 写入 Tantivy
        for (index_id, doc) in docs {
            self.writer.add_document(doc)?;
        }

        Ok(())
    }

    pub async fn delete(&self, source_id: &str) -> Result<()> {
        // 删除所有来自 source_id 的文档
        let query = Query::new(format!("source_id:{}", source_id));
        self.writer.delete_query(query)?;
        Ok(())
    }

    pub async fn delete_doc(&self, index_id: &IndexId) -> Result<()> {
        // 删除特定 IndexId 的文档
        let query = Query::new(format!("id:{}", index_id.encode()));
        self.writer.delete_query(query)?;
        Ok(())
    }

    pub async fn get_doc(&self, index_id: &IndexId) -> Result<Option<Document>> {
        // 查询单个文档
        // 返回存储字段 (stored fields)
        Ok(...)
    }

    pub async fn is_indexed(&self, index_id: &IndexId) -> Result<bool> {
        // 检查文档是否已索引
        Ok(self.get_doc(index_id).await?.is_some())
    }

    pub async fn iter_ids(&self, source_id: &str) -> Result<BoxStream<IndexId>> {
        // 遍历来自 source_id 的所有 IndexId
        // 返回异步流，支持大规模索引
        Ok(...)
    }

    pub async fn commit(&self) -> Result<()> {
        // 提交所有待写入的更改
        self.writer.commit()?;
        Ok(())
    }
}
```

**核心操作**：

| 操作 | 说明 | 用途 |
|------|------|------|
| `add()` | 添加/更新文档 | 索引新的文件或文档 |
| `delete()` | 删除所有源文档 | 清理整个仓库的索引 |
| `delete_doc()` | 删除特定文档 | 精细粒度删除 |
| `get_doc()` | 查询文档 | 检查是否已索引 |
| `is_indexed()` | 检查索引状态 | 判断文件是否已处理 |
| `iter_ids()` | 遍历索引ID | 增量更新检测 |
| `commit()` | 提交事务 | 持久化更改 |

#### Struct: IndexGarbageCollector (第 405 行)

```rust
pub struct IndexGarbageCollector {
    indexer: Arc<Indexer>,
}

impl IndexGarbageCollector {
    pub async fn garbage_collect(
        &self,
        active_source_ids: &[&str],
    ) -> Result<GarbageCollectionStats> {
        // 1. 获取索引中的所有 source_id
        let indexed_source_ids = self.get_all_source_ids().await?;

        // 2. 找出不活跃的 source_id
        let inactive = indexed_source_ids
            .iter()
            .filter(|id| !active_source_ids.contains(id))
            .collect::<Vec<_>>();

        // 3. 删除不活跃的索引
        let mut stats = GarbageCollectionStats::default();
        for id in inactive {
            stats.deleted_docs += self.indexer.delete(id).await?;
        }

        // 4. 提交
        self.indexer.commit().await?;

        Ok(stats)
    }
}

pub struct GarbageCollectionStats {
    pub deleted_docs: usize,
    pub deleted_chunks: usize,
}
```

**机制**：
- 接收活跃的 source_id 列表
- 删除不在列表中的索引
- 支持跨 corpus 统计

---

## 2. tantivy_utils.rs - Tantivy 工具

```rust
/// 打开或创建索引
pub async fn open_or_create_index(
    index_path: &Path,
    schema: Schema,
) -> Result<(IndexReader, IndexWriter)> {
    // 1. 检查索引是否存在
    if index_path.exists() {
        // 2. 打开并恢复 (处理崩溃)
        let index = Index::open_in_dir(index_path)?;
        index.writer()? // 自动恢复
    } else {
        // 3. 创建新索引
        let index = Index::create_in_dir(index_path, schema)?;
        index.writer()?
    }

    // 返回读写器
    let reader = index.reader()?;
    let writer = index.writer()?;
    Ok((reader, writer))
}

/// Schema 定义
fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // 基础字段 (all corpus)
    schema_builder.add_text_field("field_id", STORED | INDEXED);
    schema_builder.add_text_field("field_source_id", INDEXED);
    schema_builder.add_text_field("field_corpus", INDEXED);

    // Chunk 字段
    schema_builder.add_text_field("field_chunk_id", INDEXED);
    schema_builder.add_bytes_field("field_chunk_embedding", STORED);
    schema_builder.add_text_field("field_chunk_tokens", TEXT);

    // ... 更多字段

    schema_builder.build()
}
```

---

## 3. code/types.rs - 代码文档类型

### SourceCode (第 11 行)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCode {
    /// 绝对文件路径
    pub filepath: String,

    /// 编程语言
    pub language: String,

    /// Git 提交 SHA
    pub commit: String,

    /// Git 仓库 URL
    pub git_url: String,

    /// 代码内容
    pub body: String,

    /// AST 提取的标签 (定义)
    pub tags: Vec<Tag>,

    /// 代码指标 (有效性检查)
    pub max_line_length: i32,
    pub avg_line_length: f32,
    pub alphanum_fraction: f32,
    pub num_lines: i32,

    /// 起始行号 (chunk 场景)
    pub start_line: Option<i32>,

    /// 源文件 ID (用于检测文件变更)
    pub source_file_id: SourceFileId,
}

impl ToIndexId for SourceCode {
    fn to_index_id(&self) -> IndexId {
        IndexId {
            source_id: format!("code::{}", self.git_url),
            id: format!("{}:{}", self.filepath, self.start_line.unwrap_or(0)),
        }
    }
}
```

### Tag (第 70 行)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// 标签在代码中的字符范围
    pub range: (usize, usize),

    /// 在行号中的范围
    pub line_range: (i32, i32),

    /// 是否是定义点 (vs 引用)
    pub is_definition: bool,

    /// 代码语法类型 (function/class/variable/etc)
    pub syntax_type_name: String,

    /// 标签名称 (函数名、类名等)
    pub name: String,

    /// 可选的文档字符串 (doc comments)
    pub docs: Option<String>,
}

// 标签类型示例:
// - "function": 函数定义
// - "class": 类定义
// - "method": 方法定义
// - "variable": 变量定义
// - "constant": 常量定义
```

---

## 4. code/mod.rs - 代码索引协调

### CodeIndexer (第 29 行)

```rust
pub struct CodeIndexer {
    indexer: Arc<Indexer>,
    builder: Arc<CodeBuilder>,
}

impl CodeIndexer {
    pub async fn refresh(
        &self,
        git_url: &str,
        commit: Option<&str>,
    ) -> Result<RefreshStats> {
        // 1. 同步 Git 仓库
        let repo_path = repository::sync_repository(git_url).await?;
        let commit_sha = commit.or_else(|| get_head_commit(&repo_path))?;

        // 2. 索引仓库
        let stats = index::index_repository(
            &repo_path,
            git_url,
            &commit_sha,
            &self.indexer,
            &self.builder,
        ).await?;

        // 3. 垃圾回收
        index::garbage_collection(
            &self.indexer,
            &[git_url],
        ).await?;

        Ok(stats)
    }
}

pub struct RefreshStats {
    pub indexed_files: usize,
    pub updated_files: usize,
    pub deleted_files: usize,
    pub total_chunks: usize,
    pub failed_chunks: usize,
}
```

### CodeBuilder (第 53 行)

```rust
pub struct CodeBuilder {
    embedding: Arc<dyn Embedding>,
}

#[async_trait]
impl IndexAttributeBuilder<SourceCode> for CodeBuilder {
    async fn build_attributes(&self, doc: &SourceCode) -> serde_json::Value {
        json!({
            "filepath": doc.filepath,
            "language": doc.language,
            "commit": doc.commit,
            "git_url": doc.git_url,
            "num_lines": doc.num_lines,
            "tags": doc.tags.iter()
                .filter(|t| t.is_definition)
                .map(|t| json!({
                    "name": t.name,
                    "kind": t.syntax_type_name,
                    "range": [t.range.0, t.range.1],
                }))
                .collect::<Vec<_>>(),
        })
    }

    async fn build_chunk_attributes<'a>(
        &self,
        doc: &'a SourceCode,
    ) -> BoxStream<'a, JoinHandle<Result<(Vec<String>, serde_json::Value)>>> {
        // 1. 分块文档
        let chunks = CodeIntelligence::chunks(&doc.body, &doc.language)?;

        // 2. 为每个 chunk 生成属性
        let stream = stream::iter(chunks).then(|chunk| {
            let body = chunk.text.clone();
            let embedding = self.embedding.clone();

            // 在后台任务中生成 embedding
            tokio::spawn(async move {
                let tokens = tokenize(&body)?;
                let embedding_vec = embedding.embed(&body).await?;
                let attrs = json!({
                    "language": ...,
                    "lines": chunk.start_line..chunk.end_line,
                    "embedding": binarize_embedding(embedding_vec),
                });
                Ok((tokens, attrs))
            })
        });

        Box::pin(stream)
    }
}
```

---

## 5. code/intelligence/mod.rs - AST 分析

### CodeIntelligence (第 19 行)

```rust
pub struct CodeIntelligence;

impl CodeIntelligence {
    /// 提取代码中的 AST 标签
    pub fn find_tags(language: &str, content: &str) -> Result<Vec<Tag>> {
        let config = LANGUAGE_TAGS.get(language)?;
        let tags = config.generate_tags(content)?;

        // 转换为 Tag 结构
        tags.iter().map(|t| Tag {
            range: (t.start_byte, t.end_byte),
            line_range: (t.start_line as i32, t.end_line as i32),
            is_definition: t.is_definition,
            syntax_type_name: t.syntax_type.to_string(),
            name: t.name.clone(),
            docs: t.docs.clone(),
        }).collect()
    }

    /// 智能代码分块
    pub fn chunks(
        content: &str,
        language: &str,
    ) -> Result<Vec<CodeChunk>> {
        // 1. 尝试 CodeSplitter (语义感知)
        match CodeSplitter::new(language).split(content) {
            Ok(chunks) => return Ok(chunks),
            Err(_) => {
                // 2. 降级到 TextSplitter (容错)
                TextSplitter::new(512)
                    .split(content)
                    .map(|s| CodeChunk {
                        text: s.to_string(),
                        start_line: ...,
                        end_line: ...,
                    })
                    .collect()
            }
        }
    }

    /// 计算源文件的有效性
    pub fn compute_source_file(
        filepath: &str,
        language: &str,
        content: &str,
    ) -> Result<SourceCode> {
        let metrics = compute_metrics(content)?;

        Ok(SourceCode {
            filepath: filepath.to_string(),
            language: language.to_string(),
            body: content.to_string(),
            max_line_length: metrics.max_line_length,
            avg_line_length: metrics.avg_line_length,
            alphanum_fraction: metrics.alphanum_fraction,
            num_lines: metrics.num_lines as i32,
            source_file_id: SourceFileId::compute(filepath, language, content)?,
            ..Default::default()
        })
    }
}

pub struct CodeChunk {
    pub text: String,
    pub start_line: i32,
    pub end_line: i32,
}
```

### SourceFileId (code/intelligence/id.rs 第 15 行)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceFileId {
    pub path: PathBuf,
    pub language: String,
    pub git_hash: String,  // SHA256(file_content)
}

impl SourceFileId {
    pub fn compute(
        path: &str,
        language: &str,
        content: &str,
    ) -> Result<Self> {
        Ok(SourceFileId {
            path: PathBuf::from(path),
            language: language.to_string(),
            git_hash: compute_sha256(content),
        })
    }

    /// 检测文件是否被修改
    pub fn matches(&self, other: &SourceFileId) -> bool {
        self.path == other.path
            && self.language == other.language
            && self.git_hash == other.git_hash
    }
}

// 变更检测流程:
// 1. 从索引读取旧 SourceFileId
// 2. 计算当前文件的 SourceFileId
// 3. 比较: 不匹配 → 文件已修改 → 需要重新索引
```

---

## 6. code/languages.rs - 语言配置

### 支持的语言

```rust
lazy_static! {
    static ref LANGUAGE_TAGS: HashMap<&'static str, TagsConfigurationSync> = {
        HashMap::from([
            ("python", TagsConfiguration::from_language(Language::Python)),
            ("rust", TagsConfiguration::from_language(Language::Rust)),
            ("java", TagsConfiguration::from_language(Language::Java)),
            ("kotlin", TagsConfiguration::from_language(Language::Kotlin)),
            ("scala", TagsConfiguration::from_language(Language::Scala)),
            ("typescript", TagsConfiguration::from_language(Language::TypeScript)),
            ("javascript", TagsConfiguration::from_language(Language::JavaScript)),
            ("tsx", TagsConfiguration::from_language(Language::TSX)),
            ("go", TagsConfiguration::from_language(Language::Go)),
            ("ruby", TagsConfiguration::from_language(Language::Ruby)),
            ("c", TagsConfiguration::from_language(Language::C)),
            ("cpp", TagsConfiguration::from_language(Language::Cpp)),
            ("c_sharp", TagsConfiguration::from_language(Language::CSharp)),
            ("solidity", TagsConfiguration::from_language(Language::Solidity)),
            ("lua", TagsConfiguration::from_language(Language::Lua)),
            ("elixir", TagsConfiguration::from_language(Language::Elixir)),
            ("gdscript", TagsConfiguration::from_language(Language::GDScript)),
        ])
    }
}

// 每个语言配置包含:
// - TreeSitter parser
// - .scm 查询规则 (提取标签的规则)
// - 分块策略
```

---

## 7. structured_doc/types.rs - 文档类型

### StructuredDoc (第 19 行)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StructuredDoc {
    Web(WebDoc),
    Issue(IssueDoc),
    Pull(PullDoc),
    Commit(CommitDoc),
    Page(PageDoc),
    Ingested(IngestedDoc),
}

impl StructuredDoc {
    pub fn kind(&self) -> &'static str {
        match self {
            StructuredDoc::Web(_) => "web",
            StructuredDoc::Issue(_) => "issue",
            StructuredDoc::Pull(_) => "pull",
            StructuredDoc::Commit(_) => "commit",
            StructuredDoc::Page(_) => "page",
            StructuredDoc::Ingested(_) => "ingested",
        }
    }
}

impl ToIndexId for StructuredDoc {
    fn to_index_id(&self) -> IndexId {
        IndexId {
            source_id: self.source_id().to_string(),
            id: self.doc_id().to_string(),
        }
    }
}
```

### 具体文档类型

#### CommitDoc (第 55 行, commit.rs)

```rust
pub struct CommitDoc {
    pub source_id: String,           // 来源 (repo URL)
    pub commit_sha: String,          // 提交 SHA
    pub message: String,             // 提交信息
    pub author_email: String,        // 作者邮箱
    pub author_name: String,         // 作者名
    pub authored_at: i64,            // 提交时间戳
    pub diff: Option<String>,        // 变更内容
}
```

#### IssueDoc (第 58 行, issue.rs)

```rust
pub struct IssueDoc {
    pub source_id: String,
    pub issue_id: String,            // Issue ID
    pub link: String,                // GitHub Issue 链接
    pub title: String,
    pub body: String,
    pub author_email: String,
    pub author_name: String,
    pub created_at: i64,
    pub closed: bool,
}
```

#### PullDoc (第 68 行, pull.rs)

```rust
pub struct PullDoc {
    pub source_id: String,
    pub pull_id: String,
    pub link: String,                // GitHub PR 链接
    pub title: String,
    pub body: String,
    pub author_email: String,
    pub diff: String,                // PR diff 内容
    pub merged: bool,
    pub created_at: i64,
}
```

#### WebDoc, PageDoc, IngestedDoc

类似结构，分别对应网页、文档页面和自定义文档。

---

## 8. structured_doc/public.rs - 文档索引 API

### StructuredDocIndexer (第 41 行)

```rust
pub struct StructuredDocIndexer {
    indexer: Arc<Indexer>,
    builder: Arc<StructuredDocBuilder>,
}

impl StructuredDocIndexer {
    pub async fn presync(
        &self,
        docs: &[StructuredDocState],
    ) -> Result<Vec<bool>> {
        // 检查哪些文档需要更新
        // 返回 bool 向量: true 表示需要更新
        Ok(docs.iter().map(|doc| {
            !self.indexer.is_indexed(&doc.to_index_id()).await.unwrap_or(false)
                || doc.updated_at > last_indexed_time(&doc.id)
        }).collect())
    }

    pub async fn sync(
        &self,
        doc: StructuredDoc,
    ) -> Result<bool> {
        // 同步单个文档

        // 1. 检查是否需要更新
        if !self.presync(&[doc.to_state()]).await?[0] {
            return Ok(false);
        }

        // 2. 构建文档属性和 chunks
        let chunks = self.builder.build(doc.clone()).await?;

        // 3. 删除旧版本
        self.indexer.delete_doc(&doc.to_index_id()).await?;

        // 4. 添加新版本 (并发)
        let futures = chunks.into_iter().map(|chunk| {
            let indexer = self.indexer.clone();
            tokio::spawn(async move {
                indexer.add(..., chunk, ...).await
            })
        });

        futures::future::join_all(futures).await;

        // 5. 提交
        self.indexer.commit().await?;

        Ok(true)
    }
}

pub struct StructuredDocState {
    pub id: String,
    pub updated_at: i64,             // Unix 时间戳
    pub deleted: bool,
}

pub struct StructuredDocGarbageCollector {
    indexer: Arc<Indexer>,
}

impl StructuredDocGarbageCollector {
    pub async fn run<F>(
        &self,
        should_keep: F,  // 回调: id -> bool
    ) -> Result<()>
    where
        F: Fn(&str) -> bool,
    {
        // 1. 遍历所有索引的文档
        for doc_id in self.indexer.iter_ids("structured_doc").await? {
            // 2. 检查是否应该保留
            if !should_keep(&doc_id) {
                // 3. 删除
                self.indexer.delete_doc(&doc_id).await?;
            }
        }

        // 4. 提交
        self.indexer.commit().await?;
        Ok(())
    }
}
```

---

## 模块依赖关系

```
lib.rs (入口)
    ↓
indexer.rs (核心引擎)
    ├─ tantivy_utils.rs (Tantivy 操作)
    ├─ code/
    │   ├─ mod.rs (CodeIndexer)
    │   ├─ index.rs (构建流程)
    │   ├─ types.rs (SourceCode)
    │   ├─ repository.rs (Git 同步)
    │   ├─ languages.rs (语言配置)
    │   └─ intelligence/
    │       ├─ mod.rs (CodeIntelligence)
    │       └─ id.rs (SourceFileId)
    └─ structured_doc/
        ├─ mod.rs (StructuredDocBuilder)
        ├─ public.rs (StructuredDocIndexer)
        ├─ types.rs (StructuredDoc enum)
        └─ types/
            ├─ commit.rs
            ├─ issue.rs
            ├─ pull.rs
            ├─ web.rs
            ├─ page.rs
            └─ ingested.rs
```

---

**相关文档**：
- [系统架构](./architecture.md) - 模块如何交互
- [索引构建流程](./indexing-process.md) - 具体使用流程
- [AST 和语言处理](./ast-languages.md) - 语言支持详解
