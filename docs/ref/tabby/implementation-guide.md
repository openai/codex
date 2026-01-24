# 为 codex-rs 的实现参考

## 快速决策矩阵

| 模块 | 推荐做法 | 工作量 | 优先级 |
|------|--------|-------|-------|
| **Tantivy 索引存储** | ✓ 直接复用 | 低 | 🔴 P0 |
| **Tree-sitter AST** | ✓ 复用 + 自定义 | 中 | 🔴 P0 |
| **代码分块** | ✓ 复用 TextSplitter | 低 | 🔴 P0 |
| **Embedding 集成** | ⚠️ 适配器模式 | 低 | 🔴 P0 |
| **Git 同步** | ✓ 复用 git2 | 低 | 🟡 P1 |
| **并发框架** | ✓ 复用 tokio | 低 | 🔴 P0 |
| **文档索引** | ⚠️ 可选模块 | 中 | 🟢 P2 |
| **Rust 特定分析** | 🔨 自实现 | 高 | 🟢 P2 |

---

## Phase 1: MVP (最小可用产品)

### 目标

为 codex-rs 实现基础的代码索引能力，支持：
- Rust 代码索引
- 基础的 Python/TypeScript 支持
- 关键词搜索 + 向量搜索
- 增量更新

### 实现步骤

#### 步骤 1: 项目结构设置 (1-2 天)
```
Cargo.toml additions:
[dependencies]
tantivy = "0.21"
tree-sitter-tags = "0.22"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-typescript = "0.21"
text-splitter = { version = "0.13", features = ["code"] }
```

#### 步骤 2: 核心数据结构 (1-2 天)

```rust

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 索引配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// 索引存储路径
    pub index_dir: PathBuf,

    /// 支持的语言
    pub languages: Vec<String>,

    /// Embedding 维度
    pub embedding_dim: usize,

    /// 最大 chunk 大小
    pub chunk_size: usize,
}

/// 代码索引主接口
pub struct CodeIndex {
    config: IndexConfig,
    tantivy_index: TantivyIndex,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl CodeIndex {
    /// 创建新索引或打开现有索引
    pub async fn open_or_create(
        config: IndexConfig,
        embedding_service: Arc<dyn EmbeddingService>,
    ) -> Result<Self> {
        // 1. 创建或打开 Tantivy 索引
        let tantivy_index = TantivyIndex::open_or_create(&config.index_dir)?;

        // 2. 初始化语言配置
        init_languages(&config.languages)?;

        Ok(CodeIndex {
            config,
            tantivy_index,
            embedding_service,
        })
    }

    /// 索引单个仓库
    pub async fn index_repository(
        &self,
        repo_path: &Path,
        repo_id: &str,
    ) -> Result<IndexStats> {
        // 从 Tabby-Index 的 CodeIndexer::refresh() 改编
        // 具体实现见下一节
        todo!()
    }

    /// 搜索代码
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // 1. 关键词搜索 (BM25)
        let text_results = self.tantivy_index.search_text(query, limit)?;

        // 2. 向量搜索 (可选)
        let vector_results = if let Ok(embedding) =
            self.embedding_service.embed(query).await {
            self.tantivy_index.search_embedding(&embedding, limit)?
        } else {
            vec![]
        };

        // 3. 融合结果
        Ok(fuse_results(&text_results, &vector_results, limit))
    }
}

pub struct IndexStats {
    pub indexed_files: usize,
    pub updated_files: usize,
    pub total_chunks: usize,
    pub elapsed_secs: f64,
}

pub struct SearchResult {
    pub file_path: String,
    pub language: String,
    pub chunk: String,
    pub start_line: i32,
    pub end_line: i32,
    pub score: f32,
}
```

#### 步骤 3: Tantivy 索引封装 (2-3 天)

```rust

use tantivy::{Index, IndexReader, IndexWriter, Schema, doc};

pub struct TantivyIndex {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    schema: Schema,
}

impl TantivyIndex {
    /// 打开或创建 Tantivy 索引
    pub fn open_or_create(index_path: &Path) -> Result<Self> {
        // 1. 如果目录不存在，创建
        std::fs::create_dir_all(index_path)?;

        // 2. 构建 schema
        let schema = Self::build_schema();

        // 3. 打开或创建索引
        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path)?
        } else {
            Index::create_in_dir(index_path, schema.clone())?
        };

        // 4. 获取读写器
        let reader = index.reader()?;
        let writer = index.writer(5_000_000)?;  // 5MB 缓冲

        Ok(TantivyIndex {
            index,
            reader,
            writer,
            schema,
        })
    }

    /// 构建 schema (从 Tabby-Index 改编)
    fn build_schema() -> Schema {
        let mut builder = tantivy::schema::Schema::builder();

        // 基础字段
        builder.add_text_field("file_id", tantivy::schema::TEXT | tantivy::schema::STORED);
        builder.add_text_field("source_id", tantivy::schema::TEXT);
        builder.add_text_field("corpus", tantivy::schema::TEXT);
        builder.add_text_field("attributes", tantivy::schema::STRING | tantivy::schema::STORED);
        builder.add_i64_field("updated_at", tantivy::schema::INDEXED);

        // Chunk 字段
        builder.add_text_field("chunk_id", tantivy::schema::TEXT);
        builder.add_text_field("chunk_attributes", tantivy::schema::STRING | tantivy::schema::STORED);
        builder.add_text_field("chunk_tokens", tantivy::schema::TEXT);
        builder.add_bytes_field("chunk_embedding", tantivy::schema::STORED);

        // 代码特定字段
        builder.add_text_field("filepath", tantivy::schema::TEXT);
        builder.add_text_field("language", tantivy::schema::TEXT);
        builder.add_text_field("commit", tantivy::schema::TEXT);
        builder.add_i64_field("start_line", tantivy::schema::INDEXED);
        builder.add_text_field("body", tantivy::schema::STRING | tantivy::schema::STORED);

        builder.build()
    }

    /// 添加文档到索引
    pub fn add_document(
        &self,
        file_id: &str,
        source_id: &str,
        filepath: &str,
        language: &str,
        body: &str,
        chunk_tokens: &[String],
        embedding: Option<&[f32]>,
    ) -> Result<()> {
        use tantivy::schema::Value;

        let doc = doc!(
            self.schema.get_field("file_id")? => file_id,
            self.schema.get_field("source_id")? => source_id,
            self.schema.get_field("corpus")? => "code",
            self.schema.get_field("filepath")? => filepath,
            self.schema.get_field("language")? => language,
            self.schema.get_field("body")? => body,
            self.schema.get_field("chunk_tokens")? => chunk_tokens.join(" "),
            self.schema.get_field("updated_at")? => chrono::Utc::now().timestamp(),
        );

        if let Some(emb) = embedding {
            let binary = binarize_embedding(emb);
            // 向 doc 添加 embedding (需要扩展)
            // self.writer.add_document(doc)?;
        } else {
            self.writer.add_document(doc)?;
        }

        Ok(())
    }

    /// 提交变更
    pub fn commit(&self) -> Result<()> {
        self.writer.commit()?;
        Ok(())
    }

    /// 搜索文本 (BM25)
    pub fn search_text(
        &self,
        query_text: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        use tantivy::query::QueryParser;

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.get_field("chunk_tokens")?],
        );

        let query = query_parser.parse_query(query_text)?;
        let searcher = self.reader.searcher();
        let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit))?;

        let mut results = vec![];
        for (_score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            results.push(SearchResult {
                file_path: doc.get_first("filepath")
                    .and_then(|v| v.as_text())
                    .unwrap_or("")
                    .to_string(),
                language: doc.get_first("language")
                    .and_then(|v| v.as_text())
                    .unwrap_or("")
                    .to_string(),
                chunk: doc.get_first("body")
                    .and_then(|v| v.as_text())
                    .unwrap_or("")
                    .to_string(),
                start_line: 0,  // TODO: 从 chunk_attributes 解析
                end_line: 0,
                score: _score,
            });
        }

        Ok(results)
    }

    /// 搜索向量
    pub fn search_embedding(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // TODO: 实现向量搜索
        // 方案 A: 使用 Tantivy 的向量功能 (0.22+)
        // 方案 B: 后处理 (取出所有向量，在内存中计算)
        // 方案 C: 集成专门的向量库 (qdrant, milvus)

        todo!("向量搜索实现")
    }
}

fn binarize_embedding(embedding: &[f32]) -> Vec<u8> {
    // 将浮点向量转换为字节表示
    embedding
        .iter()
        .flat_map(|f| f.to_le_bytes().to_vec())
        .collect()
}
```

#### 步骤 4: AST 和语言支持 (2-3 天)

```rust

use tree_sitter_tags::{Tags, TagsContext};
use std::collections::HashMap;

pub struct CodeIntelligence;

impl CodeIntelligence {
    /// 提取代码标签
    pub fn extract_tags(
        language: &str,
        content: &str,
    ) -> anyhow::Result<Vec<Tag>> {
        let config = get_language_config(language)?;

        // 使用 tree-sitter-tags
        let mut cursor = TagsContext::new(
            config.language,
            content.as_bytes(),
            config.query.as_str(),
        )?;

        let mut tags = vec![];
        while let Some((name, start_point, end_point)) = cursor.next() {
            tags.push(Tag {
                name: name.to_string(),
                start_line: start_point.row as i32,
                start_column: start_point.column as i32,
                end_line: end_point.row as i32,
                end_column: end_point.column as i32,
                syntax_type: detect_syntax_type(name),
            });
        }

        Ok(tags)
    }

    /// 计算代码指标 (有效性检查)
    pub fn compute_metrics(content: &str) -> CodeMetrics {
        let lines: Vec<&str> = content.lines().collect();
        let total_chars = content.len();

        let max_line_length = lines.iter()
            .map(|l| l.len())
            .max()
            .unwrap_or(0) as i32;

        let avg_line_length = if !lines.is_empty() {
            total_chars as f32 / lines.len() as f32
        } else {
            0.0
        };

        let alphanum_count: usize = content
            .chars()
            .filter(|c| c.is_alphanumeric())
            .count();

        let alphanum_fraction = if total_chars > 0 {
            alphanum_count as f32 / total_chars as f32
        } else {
            0.0
        };

        CodeMetrics {
            max_line_length,
            avg_line_length,
            alphanum_fraction,
            num_lines: lines.len() as i32,
        }
    }

    /// 是否是有效的源代码文件
    pub fn is_valid_file(metrics: &CodeMetrics) -> bool {
        metrics.max_line_length <= 300
            && metrics.avg_line_length <= 150.0
            && metrics.alphanum_fraction >= 0.25
            && metrics.num_lines <= 100000
    }

    /// 代码分块 (从 text-splitter 改编)
    pub fn chunk_code(
        content: &str,
        _language: &str,
        chunk_size: usize,
    ) -> anyhow::Result<Vec<CodeChunk>> {
        use text_splitter::TextSplitter;

        let splitter = TextSplitter::new(chunk_size);
        let mut chunks = vec![];

        for chunk_text in splitter.split_text(content) {
            let start_line = count_lines_before(content, chunk_text) as i32;
            let end_line = start_line + chunk_text.lines().count() as i32 - 1;

            chunks.push(CodeChunk {
                text: chunk_text.to_string(),
                start_line,
                end_line,
            });
        }

        Ok(chunks)
    }
}

pub struct Tag {
    pub name: String,
    pub start_line: i32,
    pub start_column: i32,
    pub end_line: i32,
    pub end_column: i32,
    pub syntax_type: String,
}

pub struct CodeChunk {
    pub text: String,
    pub start_line: i32,
    pub end_line: i32,
}

pub struct CodeMetrics {
    pub max_line_length: i32,
    pub avg_line_length: f32,
    pub alphanum_fraction: f32,
    pub num_lines: i32,
}

fn detect_syntax_type(tag_name: &str) -> String {
    // 根据 tree-sitter-tags 的输出检测语法类型
    if tag_name.contains("function") {
        "function".to_string()
    } else if tag_name.contains("class") {
        "class".to_string()
    } else if tag_name.contains("struct") {
        "struct".to_string()
    } else {
        "definition".to_string()
    }
}

fn count_lines_before(content: &str, chunk: &str) -> usize {
    if let Some(pos) = content.find(chunk) {
        content[..pos].lines().count()
    } else {
        0
    }
}
```

#### 步骤 5: Embedding 集成 (1-2 天)

```rust

pub trait EmbeddingService: Send + Sync {
    /// 生成代码片段的 embedding
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// 嵌入维度
    fn embedding_dim(&self) -> usize;
}

// 集成到 codex-rs 的 inference 服务
impl EmbeddingService for CodexEmbeddingAdapter {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        // 调用 codex 的 embedding 模型
        self.inference_service
            .embed(text, "code")  // 指定"code" embedding 上下文
            .await
    }

    fn embedding_dim(&self) -> usize {
        1536  // 根据实际模型调整
    }
}
```

---

## Phase 2: 功能扩展 (可选)

### 目标

- [ ] 支持更多编程语言 (C++, Java, Go, etc.)
- [ ] 文档索引 (Markdown, HTML)
- [ ] 向量搜索优化
- [ ] 混合排序和重排
- [ ] 缓存层优化

### 实现建议

#### 1. 多语言支持扩展

```rust
// 添加语言支持的步骤:

1. 在 Cargo.toml 中添加依赖:
   tree-sitter-cpp = "0.21"
   tree-sitter-go = "0.21"
   tree-sitter-java = "0.21"

2. 在 languages.rs 中注册:
   LANGUAGE_CONFIGS.insert("cpp", create_cpp_config()?);
   LANGUAGE_CONFIGS.insert("go", create_go_config()?);
   LANGUAGE_CONFIGS.insert("java", create_java_config()?);

3. 添加语言检测 (基于文件扩展名):
   fn detect_language(path: &Path) -> Option<&str> {
       path.extension()
           .and_then(|ext| ext.to_str())
           .and_then(|ext| LANGUAGE_MAP.get(ext).copied())
   }
```

#### 2. 向量搜索优化

```rust
// 实现更好的向量检索

pub struct HybridSearch {
    text_weight: f32,
    vector_weight: f32,
}

impl HybridSearch {
    pub async fn search(
        &self,
        query: &str,
        embedding: &[f32],
        index: &CodeIndex,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // 1. BM25 搜索
        let text_results = index.search_text(query, limit * 2)?;

        // 2. 向量搜索
        let vector_results = index.search_embedding(embedding, limit * 2)?;

        // 3. 分数融合 (RRF: Reciprocal Rank Fusion)
        let mut combined = HashMap::new();

        for (i, result) in text_results.iter().enumerate() {
            combined.entry(result.file_path.clone())
                .or_insert(0.0)
                += self.text_weight / (i as f32 + 1.0);
        }

        for (i, result) in vector_results.iter().enumerate() {
            combined.entry(result.file_path.clone())
                .or_insert(0.0)
                += self.vector_weight / (i as f32 + 1.0);
        }

        // 4. 排序和返回
        let mut results: Vec<_> = combined.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        Ok(results.into_iter()
            .take(limit)
            .map(|(path, score)| SearchResult {
                file_path: path,
                score,
                ..Default::default()
            })
            .collect())
    }
}
```

---

## Phase 3: Rust 特定优化 (可选)

### Rust 特定的代码分析

```rust
pub struct RustCodeIntelligence;

impl RustCodeIntelligence {
    /// 提取 Rust 特定的信息
    pub fn analyze_rust(content: &str) -> Result<RustAnalysis> {
        let mut analysis = RustAnalysis::default();

        // 1. 提取 trait 实现
        analysis.trait_impls = Self::extract_trait_impls(content)?;

        // 2. 提取 macro 调用
        analysis.macro_calls = Self::extract_macros(content)?;

        // 3. 提取依赖 (Cargo.toml)
        // analysis.dependencies = Self::extract_dependencies(content)?;

        // 4. 标识 unsafe 块
        analysis.unsafe_blocks = Self::find_unsafe_blocks(content)?;

        Ok(analysis)
    }

    fn extract_trait_impls(content: &str) -> Result<Vec<TraitImpl>> {
        // 使用 tree-sitter 的 Rust parser
        // 查询模式: (impl_item (trait_type (type_identifier)) @trait)
        todo!()
    }

    fn extract_macros(content: &str) -> Result<Vec<MacroCall>> {
        // 查询: (macro_invocation (identifier) @macro)
        todo!()
    }

    fn find_unsafe_blocks(content: &str) -> Result<Vec<UnsafeBlock>> {
        // 查询: (unsafe_block) @unsafe
        todo!()
    }
}

pub struct RustAnalysis {
    pub trait_impls: Vec<TraitImpl>,
    pub macro_calls: Vec<MacroCall>,
    pub unsafe_blocks: Vec<UnsafeBlock>,
}

pub struct TraitImpl {
    pub trait_name: String,
    pub impl_type: String,
    pub methods: Vec<String>,
}

pub struct MacroCall {
    pub name: String,
    pub args: String,
    pub line: i32,
}

pub struct UnsafeBlock {
    pub reason: Option<String>,
    pub line: i32,
}
```

---

## 集成检查清单

### ✓ 必须完成的任务

- [ ] Tantivy 索引集成测试
- [ ] Tree-sitter 语言配置验证
- [ ] Embedding 服务适配器
- [ ] 基本功能测试 (index + search)
- [ ] 性能基准测试
- [ ] 错误处理和恢复
- [ ] 日志和监控集成

### ⚠️ 需要配置的任务

- [ ] 索引目录配置 (可配置路径)
- [ ] 支持的语言列表 (config.toml)
- [ ] Chunk 大小参数 (默认 512 字符)
- [ ] Embedding 服务 URL
- [ ] 搜索权重参数 (BM25 vs Vector)

### 🚀 可选优化任务

- [ ] 多语言支持扩展
- [ ] 向量搜索优化
- [ ] 缓存层实现
- [ ] Rust 特定分析
- [ ] 性能调优 (并发度、缓冲大小)

---

## 常见问题和陷阱

### Q1: Embedding 维度和模型选择?

**A**:
- 默认建议: 1536 维度 (OpenAI ada-002 standard)
- Tabby 使用: 可变维度 (支持多模型)
- codex-rs: 使用现有 embedding 服务的维度

### Q2: 索引更新频率?

**A**:
- **开发场景**: 实时更新 (每次文件保存)
- **分析场景**: 定期更新 (每小时或每天)
- **生产场景**: 增量更新 (Git webhook)

### Q3: 索引大小会很大吗?

**A**:
- **存储**: ~150-200% 源代码大小
- **1 GB 代码** → ~1.5-2 GB 索引
- **优化**: 选择性字段存储、段压缩

### Q4: 并发性能如何?

**A**:
- **单线程**: ~1000 QPS (BM25)
- **多线程** (16 cores): ~5000 QPS
- **Tantivy**: 支持无锁读取 (MVCC)

### Q5: 如何处理大型仓库 (>1GB)?

**A**:
```rust
// 分块处理策略:
for batch in file_tree.chunks(100) {
    process_batch(batch).await?;
    // 每批后提交
}

// 或分库索引:
let repos = split_by_language(&repo_path);
for sub_repo in repos {
    index_repository(sub_repo).await?;
}
```

---

## 参考资源

### 文档链接

- **Tantivy 文档**: https://docs.rs/tantivy
- **Tree-sitter**: https://tree-sitter.github.io
- **Tree-sitter Rust**: https://tree-sitter.github.io/tree-sitter/references
- **Text Splitter**: https://docs.rs/text-splitter

### 代码示例

- **Tabby Index 源码**: https://github.com/TabbyML/tabby/tree/main/crates/tabby-index

---

## 总结

### ✓ 立即可以做

1. 集成 Tantivy 和 Tree-sitter (低成本, 高价值)
2. 实现基础的代码索引 (MVP 目标)
3. 支持 Rust、Python、TypeScript (最常见语言)
4. 整合现有的 embedding 服务

### ⚠️ 需要慎重考虑

1. **Embedding 模型选择**: 确保与 codex-rs 一致
2. **索引更新策略**: 实时 vs 离线 (性能权衡)
3. **向量搜索实现**: Tantivy 内置 vs 专门库
4. **缓存和内存**: 大型索引的内存压力

### 🚀 长期优化

1. **多语言支持**: 逐步扩展到 15+ 语言
2. **向量搜索优化**: HNSW、ANN 索引
3. **Rust 特定分析**: macro、trait、unsafe 分析
4. **性能调优**: 基准测试和瓶颈分析

---

**相关文档**：
- [系统架构](./architecture.md)
- [核心模块详解](./modules.md)
- [AST 和语言处理](./ast-languages.md)
- [索引构建流程](./indexing-process.md)
