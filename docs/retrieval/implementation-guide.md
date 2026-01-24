# Implementation Guide

## 核心数据结构

### types.rs

```rust
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};

/// 源文件唯一标识
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceFileId {
    pub path: PathBuf,
    pub language: String,
    pub content_hash: String,  // SHA256 前 16 位
}

impl SourceFileId {
    pub fn compute(path: &Path, content: &str) -> Self {
        let hash = Sha256::digest(content.as_bytes());
        Self {
            path: path.to_path_buf(),
            language: detect_language(path).unwrap_or_default(),
            content_hash: format!("{:x}", hash)[..16].to_string(),
        }
    }
}

/// 代码块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub id: String,              // "{workspace}:{filepath}:{idx}"
    pub source_id: String,       // workspace identifier
    pub filepath: String,
    pub language: String,
    pub content: String,
    pub start_line: i32,
    pub end_line: i32,
    pub embedding: Option<Vec<f32>>,
}

/// 代码标签 (tree-sitter-tags 提取)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeTag {
    pub name: String,
    pub syntax_type: SyntaxType,
    pub start_line: i32,
    pub end_line: i32,
    pub signature: Option<String>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyntaxType {
    Function,
    Method,
    Class,
    Struct,
    Trait,
    Interface,
    Enum,
    Constant,
    Variable,
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: CodeChunk,
    pub score: f32,
    pub score_type: ScoreType,
}

#[derive(Debug, Clone, Copy)]
pub enum ScoreType {
    Bm25,
    Vector,
    Hybrid,
    Snippet,
}

/// 索引标签 (workspace + branch)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexTag {
    pub workspace: String,
    pub branch: Option<String>,
}

/// 分块结果
#[derive(Debug, Clone)]
pub struct ChunkSpan {
    pub content: String,
    pub start_line: i32,
    pub end_line: i32,
}
```

---

## Core 集成设计

### 设计原则

**Retrieval 作为独立服务 + Core 最小侵入**

1. **独立配置**: Retrieval 有自己的 `retrieval.toml`，不侵入 Core 的 Config 体系
2. **延迟初始化**: 仅在工具被调用时通过 `RetrievalService::for_workdir()` 初始化
3. **优雅降级**: 未配置时返回友好提示，不阻塞其他功能
4. **扩展模式**: 使用 `error_ext.rs` 等 `*_ext.rs` 模式减少合并冲突

### 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                       Retrieval Service                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Config    │  │   Indexer   │  │      Searcher       │  │
│  │ (独立配置)  │  │  (后台运行)  │  │   (API 暴露)        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│                                                              │
│  配置文件: ~/.codex/retrieval.toml 或 .codex/retrieval.toml  │
│  索引存储: ~/.codex/retrieval/ 或 .retrieval/               │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ RetrievalService::for_workdir(cwd)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                       Core (最小侵入)                        │
│                                                              │
│  core/src/features.rs          - Feature::CodeSearch        │
│  core/src/tools/spec.rs        - include_code_search 字段   │
│  core/src/tools/spec_ext.rs    - register_code_search()     │
│  core/src/tools/handlers/ext/code_search.rs - 无状态 Handler│
│  core/src/error_ext.rs         - From<RetrievalErr> 转换    │
│                                                              │
│  不修改: config/, codex.rs, error.rs 核心文件               │
└─────────────────────────────────────────────────────────────┘
```

### RetrievalService 工厂方法

```rust
// retrieval/src/service.rs

use std::path::Path;
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;

/// 全局服务实例缓存 (按 workdir)
static INSTANCES: Lazy<DashMap<PathBuf, Arc<RetrievalService>>> = Lazy::new(DashMap::new);

impl RetrievalService {
    /// 从配置文件加载并初始化
    /// 1. 先尝试 {workdir}/.codex/retrieval.toml
    /// 2. 再尝试 ~/.codex/retrieval.toml
    /// 3. 未配置或 enabled=false 返回 NotEnabled 错误
    pub async fn for_workdir(workdir: &Path) -> Result<Arc<Self>> {
        let canonical = workdir.canonicalize().unwrap_or_else(|_| workdir.to_path_buf());

        // 尝试从缓存获取
        if let Some(service) = INSTANCES.get(&canonical) {
            return Ok(Arc::clone(&service));
        }

        // 加载配置
        let config = RetrievalConfig::load(workdir)?;

        // 未启用 → 返回 NotEnabled
        if !config.enabled {
            return Err(RetrievalErr::NotEnabled);
        }

        let service = Arc::new(Self::new(config, features).await?);
        INSTANCES.insert(canonical, Arc::clone(&service));

        Ok(service)
    }

    /// 检查是否已配置（不初始化）
    pub fn is_configured(workdir: &Path) -> bool {
        RetrievalConfig::load(workdir)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }
}
```

### CodeSearchHandler (无状态)

```rust
// core/src/tools/handlers/ext/code_search.rs

/// Code Search Handler - 无状态，运行时获取 RetrievalService
pub struct CodeSearchHandler;

#[async_trait]
impl ToolHandler for CodeSearchHandler {
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. 获取工作目录
        let cwd = invocation.turn.cwd.clone();

        // 2. 尝试获取 Retrieval Service (延迟初始化)
        let service = match codex_retrieval::RetrievalService::for_workdir(&cwd).await {
            Ok(s) => s,
            Err(codex_retrieval::RetrievalErr::NotEnabled) => {
                // 优雅降级: 返回友好提示
                return Ok(ToolOutput::Function {
                    content: "Code search is not enabled.\n\n\
                        To enable, create ~/.codex/retrieval.toml with:\n\
                        ```toml\n\
                        [retrieval]\n\
                        enabled = true\n\
                        ```".to_string(),
                    content_items: None,
                    success: Some(false),
                });
            }
            Err(e) => {
                return Err(FunctionCallError::RespondToModel(
                    format!("Failed to initialize code search: {e}")
                ));
            }
        };

        // 3. 执行搜索
        let results = service.search(&args.query).await?;

        // 4. 格式化输出
        // ...
    }
}
```

---

## 错误处理设计

### 设计原则

**独立 RetrievalErr + 扩展模式转换**

retrieval crate 使用独立的 `RetrievalErr` 错误类型，在 core 使用 `error_ext.rs` 扩展模式转换，避免修改核心 `error.rs` 文件：

1. **模块独立性**: retrieval crate 可独立编译和测试，不依赖 core
2. **错误信息保留**: 结构化错误保留完整上下文 (uri、path、waited_ms 等)
3. **扩展模式**: 使用 `error_ext.rs` 减少与上游合并冲突
4. **灵活演进**: retrieval 错误变体可独立扩展，不影响 core API

### 扩展模式转换

```rust
// core/src/error_ext.rs - 扩展模式，最小化侵入
//! Error conversions for extension modules.
//!
//! This module contains error type conversions for optional/extension
//! features like retrieval, keeping them separate from core error.rs
//! to minimize invasive changes during upstream syncs.

use crate::error::CodexErr;

impl From<codex_retrieval::RetrievalErr> for CodexErr {
    fn from(err: codex_retrieval::RetrievalErr) -> Self {
        CodexErr::Fatal(err.to_string())
    }
}
```

```rust
// core/src/lib.rs - 添加模块引用
pub mod error;
mod error_ext;  // 扩展模式
pub mod exec;
```

### 特殊错误: NotEnabled

`RetrievalErr::NotEnabled` 在 Handler 中特殊处理，不转换为 `CodexErr`：

```rust
// code_search.rs handler
match codex_retrieval::RetrievalService::for_workdir(&cwd).await {
    Err(codex_retrieval::RetrievalErr::NotEnabled) => {
        // 不抛错，返回友好提示让用户知道如何启用
        return Ok(ToolOutput::Function {
            content: "Code search is not enabled...".to_string(),
            success: Some(false),
        });
    }
    // ...
}
```

---

## 结构化错误类型

### error.rs

```rust
use std::path::PathBuf;
use thiserror::Error;

/// 结构化错误类型
///
/// 设计原则:
/// - 每个变体包含足够的上下文信息
/// - 避免 String-based 错误 (如 `Storage(String)`)
/// - 支持 From 转换常见依赖错误
#[derive(Error, Debug)]
pub enum RetrievalErr {
    // 存储错误
    #[error("LanceDB connection failed: uri={uri}, cause={cause}")]
    LanceDbConnectionFailed { uri: String, cause: String },

    #[error("LanceDB query failed: table={table}, cause={cause}")]
    LanceDbQueryFailed { table: String, cause: String },

    #[error("SQLite lock timeout: path={path:?}, waited={waited_ms}ms")]
    SqliteLockedTimeout { path: PathBuf, waited_ms: u64 },

    #[error("SQLite error: path={path:?}, cause={cause}")]
    SqliteError { path: PathBuf, cause: String },

    // 索引错误
    #[error("Index corrupted: workspace={workspace}, reason={reason}")]
    IndexCorrupted { workspace: String, reason: String },

    #[error("Content hash mismatch: expected={expected}, actual={actual}")]
    ContentHashMismatch { expected: String, actual: String },

    #[error("File not indexable: path={path:?}, reason={reason}")]
    FileNotIndexable { path: PathBuf, reason: String },

    // 搜索错误
    #[error("Search failed: query={query}, cause={cause}")]
    SearchFailed { query: String, cause: String },

    #[error("Embedding dimension mismatch: expected={expected}, actual={actual}")]
    EmbeddingDimensionMismatch { expected: i32, actual: i32 },

    // Feature 错误
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    // 配置错误
    #[error("Config error: field={field}, cause={cause}")]
    ConfigError { field: String, cause: String },

    // 通用错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, RetrievalErr>;

// 从 rusqlite 错误转换
impl From<rusqlite::Error> for RetrievalErr {
    fn from(e: rusqlite::Error) -> Self {
        Self::SqliteError {
            path: PathBuf::new(),
            cause: e.to_string(),
        }
    }
}
```

---

## 代码质量过滤

### metrics.rs

```rust
/// 代码质量指标
#[derive(Debug, Clone)]
pub struct CodeMetrics {
    pub max_line_length: i32,
    pub avg_line_length: f32,
    pub alphanum_fraction: f32,
    pub num_lines: i32,
    pub number_fraction: f32,
}

impl CodeMetrics {
    pub fn compute(content: &str) -> Self {
        let lines: Vec<&str> = content.lines().collect();
        let num_lines = lines.len() as i32;

        if num_lines == 0 {
            return Self {
                max_line_length: 0,
                avg_line_length: 0.0,
                alphanum_fraction: 0.0,
                num_lines: 0,
                number_fraction: 0.0,
            };
        }

        let max_line_length = lines.iter().map(|l| l.len() as i32).max().unwrap_or(0);
        let avg_line_length = content.len() as f32 / num_lines as f32;

        let total_chars = content.len() as f32;
        let alphanum_count = content.chars().filter(|c| c.is_alphanumeric()).count() as f32;
        let number_count = content.chars().filter(|c| c.is_ascii_digit()).count() as f32;

        Self {
            max_line_length,
            avg_line_length,
            alphanum_fraction: if total_chars > 0.0 { alphanum_count / total_chars } else { 0.0 },
            num_lines,
            number_fraction: if total_chars > 0.0 { number_count / total_chars } else { 0.0 },
        }
    }
}

/// 检查文件是否适合索引
pub fn is_valid_file(content: &str) -> bool {
    let metrics = CodeMetrics::compute(content);

    metrics.max_line_length <= 300           // 过滤超长行 (压缩/混淆)
        && metrics.avg_line_length <= 150.0  // 过滤单行文件
        && metrics.alphanum_fraction >= 0.25 // 过滤二进制/非文本
        && metrics.num_lines <= 100_000      // 过滤超大文件
        && metrics.num_lines > 0             // 过滤空文件
        && metrics.number_fraction <= 0.50   // 过滤纯数字 (日志/数据)
}

/// 检查文件是否适合索引，返回详细原因
pub fn validate_file(content: &str) -> std::result::Result<(), &'static str> {
    let metrics = CodeMetrics::compute(content);

    if metrics.num_lines == 0 {
        return Err("empty file");
    }
    if metrics.max_line_length > 300 {
        return Err("line too long (>300 chars)");
    }
    if metrics.avg_line_length > 150.0 {
        return Err("avg line length too high (>150)");
    }
    if metrics.alphanum_fraction < 0.25 {
        return Err("likely binary (alphanum < 25%)");
    }
    if metrics.num_lines > 100_000 {
        return Err("file too large (>100k lines)");
    }
    if metrics.number_fraction > 0.50 {
        return Err("likely data file (numbers > 50%)");
    }
    Ok(())
}
```

---

## 代码分块 (text-splitter)

### chunking/splitter.rs

```rust
use text_splitter::CodeSplitter;

/// 使用 text-splitter::CodeSplitter 进行代码分块
/// 内部使用 tree-sitter 解析，按语法边界分割
pub struct CodeChunkerService {
    max_chunk_size: usize,
}

impl CodeChunkerService {
    pub fn new(max_chunk_size: usize) -> Self {
        Self { max_chunk_size }
    }

    /// 分块代码文件
    pub fn chunk(&self, content: &str, language: &str) -> Result<Vec<ChunkSpan>> {
        // 获取 tree-sitter 语言
        let ts_lang = match language {
            "rust" => Some(tree_sitter_rust::LANGUAGE),
            "go" => Some(tree_sitter_go::LANGUAGE),
            "python" => Some(tree_sitter_python::LANGUAGE),
            "java" => Some(tree_sitter_java::LANGUAGE),
            _ => None,
        };

        let chunks = if let Some(lang) = ts_lang {
            // 使用 CodeSplitter (内置 tree-sitter)
            let splitter = CodeSplitter::new(lang, self.max_chunk_size)
                .map_err(|e| RetrievalErr::Parse(e.to_string()))?;

            splitter
                .chunks(content)
                .map(|chunk| self.to_chunk_span(content, chunk))
                .collect()
        } else {
            // 回退到 TextSplitter
            self.fallback_chunk(content)
        };

        Ok(chunks)
    }

    fn to_chunk_span(&self, full_content: &str, chunk: &str) -> ChunkSpan {
        // 计算 chunk 在原文中的行号
        let start_byte = full_content.find(chunk).unwrap_or(0);
        let start_line = full_content[..start_byte].lines().count() as i32;
        let end_line = start_line + chunk.lines().count() as i32 - 1;

        ChunkSpan {
            content: chunk.to_string(),
            start_line,
            end_line: end_line.max(start_line),
        }
    }

    fn fallback_chunk(&self, content: &str) -> Vec<ChunkSpan> {
        use text_splitter::TextSplitter;

        let splitter = TextSplitter::new(self.max_chunk_size);
        splitter
            .chunks(content)
            .enumerate()
            .map(|(i, chunk)| {
                let lines_before: i32 = content[..content.find(chunk).unwrap_or(0)]
                    .lines()
                    .count() as i32;
                let chunk_lines = chunk.lines().count() as i32;
                ChunkSpan {
                    content: chunk.to_string(),
                    start_line: lines_before,
                    end_line: lines_before + chunk_lines - 1,
                }
            })
            .collect()
    }
}
```

---

## 标签提取 (tree-sitter-tags)

### tags/extractor.rs

```rust
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// 使用 tree-sitter-tags 提取代码标签 (函数名、类名等)
pub struct TagExtractor {
    configs: HashMap<String, TagsConfiguration>,
}

impl TagExtractor {
    pub fn new() -> Result<Self> {
        let mut configs = HashMap::new();

        // Rust
        configs.insert(
            "rust".to_string(),
            TagsConfiguration::new(
                tree_sitter_rust::LANGUAGE.into(),
                include_str!("queries/rust.scm"),
                "",  // locals query (optional)
            )?,
        );

        // Go
        configs.insert(
            "go".to_string(),
            TagsConfiguration::new(
                tree_sitter_go::LANGUAGE.into(),
                include_str!("queries/go.scm"),
                "",
            )?,
        );

        // Python
        configs.insert(
            "python".to_string(),
            TagsConfiguration::new(
                tree_sitter_python::LANGUAGE.into(),
                include_str!("queries/python.scm"),
                "",
            )?,
        );

        // Java
        configs.insert(
            "java".to_string(),
            TagsConfiguration::new(
                tree_sitter_java::LANGUAGE.into(),
                include_str!("queries/java.scm"),
                "",
            )?,
        );

        Ok(Self { configs })
    }

    /// 提取代码标签
    pub fn extract(&self, content: &str, language: &str) -> Result<Vec<CodeTag>> {
        let config = self.configs.get(language).ok_or_else(|| {
            RetrievalErr::Parse(format!("unsupported language: {}", language))
        })?;

        let mut context = TagsContext::new();
        let tags = context.generate_tags(config, content.as_bytes(), None)?;

        let mut result = Vec::new();
        for tag in tags {
            let tag = tag?;
            result.push(CodeTag {
                name: String::from_utf8_lossy(&content.as_bytes()[tag.name_range]).to_string(),
                syntax_type: parse_syntax_type(&tag.syntax_type_name),
                start_line: tag.line_range.start as i32,
                end_line: tag.line_range.end as i32,
                signature: None,  // 可从 tag.docs 解析
                docs: tag.docs.map(|d| String::from_utf8_lossy(d).to_string()),
            });
        }

        Ok(result)
    }
}

fn parse_syntax_type(name: &str) -> SyntaxType {
    match name {
        "function" => SyntaxType::Function,
        "method" => SyntaxType::Method,
        "class" => SyntaxType::Class,
        "struct" => SyntaxType::Struct,
        "trait" => SyntaxType::Trait,
        "interface" => SyntaxType::Interface,
        "enum" => SyntaxType::Enum,
        "constant" => SyntaxType::Constant,
        _ => SyntaxType::Variable,
    }
}
```

---

## Tree-Sitter 查询规则 (.scm)

### tags/queries/rust.scm

```scheme
; Functions
(function_item
  name: (identifier) @name) @definition.function

; Methods (impl block)
(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name) @definition.method))

; Structs
(struct_item
  name: (type_identifier) @name) @definition.struct

; Traits
(trait_item
  name: (type_identifier) @name) @definition.trait

; Enums
(enum_item
  name: (type_identifier) @name) @definition.enum

; Constants
(const_item
  name: (identifier) @name) @definition.constant

; Static variables
(static_item
  name: (identifier) @name) @definition.constant
```

### tags/queries/go.scm

```scheme
; Functions
(function_declaration
  name: (identifier) @name) @definition.function

; Methods
(method_declaration
  name: (field_identifier) @name) @definition.method

; Structs
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (struct_type))) @definition.struct

; Interfaces
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (interface_type))) @definition.interface

; Constants
(const_declaration
  (const_spec
    name: (identifier) @name)) @definition.constant
```

### tags/queries/python.scm

```scheme
; Functions
(function_definition
  name: (identifier) @name) @definition.function

; Async functions
(function_definition
  "async"
  name: (identifier) @name) @definition.function

; Classes
(class_definition
  name: (identifier) @name) @definition.class

; Methods (inside class)
(class_definition
  body: (block
    (function_definition
      name: (identifier) @name) @definition.method))

; Decorated functions
(decorated_definition
  (decorator)
  definition: (function_definition
    name: (identifier) @name)) @definition.function
```

### tags/queries/java.scm

```scheme
; Classes
(class_declaration
  name: (identifier) @name) @definition.class

; Interfaces
(interface_declaration
  name: (identifier) @name) @definition.interface

; Methods
(method_declaration
  name: (identifier) @name) @definition.method

; Constructors
(constructor_declaration
  name: (identifier) @name) @definition.method

; Enums
(enum_declaration
  name: (identifier) @name) @definition.enum

; Constants (static final)
(field_declaration
  (modifiers "static" "final")
  declarator: (variable_declarator
    name: (identifier) @name)) @definition.constant
```

---

## 多进程索引锁

### indexing/lock.rs

```rust
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// 索引锁守卫 (RAII)
pub struct IndexLockGuard {
    db: Arc<SqliteStore>,
    holder_id: String,
    workspace: String,
}

impl IndexLockGuard {
    /// 尝试获取锁，超时返回错误
    pub async fn try_acquire(
        db: Arc<SqliteStore>,
        workspace: &str,
        timeout: Duration,
    ) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        let holder_id = format!("{}_{}", std::process::id(), chrono::Utc::now().timestamp_millis());

        loop {
            // 1. 检查现有锁
            let lock_info = db.query(|conn| {
                conn.query_row(
                    "SELECT holder_id, expires_at FROM index_lock WHERE workspace = ?",
                    [workspace],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                ).optional()
            }).await?;

            let now = chrono::Utc::now().timestamp();

            if let Some((existing_holder, expires_at)) = lock_info {
                if expires_at < now {
                    // 锁已过期，清理
                    db.query(|conn| {
                        conn.execute(
                            "DELETE FROM index_lock WHERE workspace = ? AND holder_id = ?",
                            rusqlite::params![workspace, existing_holder],
                        )
                    }).await?;
                } else if Instant::now() > deadline {
                    return Err(RetrievalErr::SqliteLockedTimeout {
                        path: db.path().to_path_buf(),
                        waited_ms: timeout.as_millis() as u64,
                    });
                } else {
                    // 等待后重试
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }

            // 2. 尝试获取锁
            let expires_at = now + 30;  // 30 秒超时
            let inserted = db.query(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO index_lock (id, holder_id, workspace, locked_at, expires_at)
                     VALUES (1, ?, ?, ?, ?)",
                    rusqlite::params![&holder_id, workspace, now, expires_at],
                )
            }).await?;

            if inserted > 0 {
                return Ok(Self {
                    db,
                    holder_id,
                    workspace: workspace.to_string(),
                });
            }

            // 竞争失败，重试
            if Instant::now() > deadline {
                return Err(RetrievalErr::SqliteLockedTimeout {
                    path: db.path().to_path_buf(),
                    waited_ms: timeout.as_millis() as u64,
                });
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    /// 刷新锁 (延长过期时间)
    pub async fn refresh(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 30;

        self.db.query(|conn| {
            conn.execute(
                "UPDATE index_lock SET expires_at = ? WHERE workspace = ? AND holder_id = ?",
                rusqlite::params![expires_at, &self.workspace, &self.holder_id],
            )
        }).await?;

        Ok(())
    }
}

impl Drop for IndexLockGuard {
    fn drop(&mut self) {
        // 同步释放锁 (best effort)
        let db = self.db.clone();
        let workspace = self.workspace.clone();
        let holder_id = self.holder_id.clone();

        // 使用 blocking 释放
        std::thread::spawn(move || {
            let rt = tokio::runtime::Handle::current();
            let _ = rt.block_on(async {
                db.query(|conn| {
                    conn.execute(
                        "DELETE FROM index_lock WHERE workspace = ? AND holder_id = ?",
                        rusqlite::params![workspace, holder_id],
                    )
                }).await
            });
        });
    }
}
```

---

## 异步安全存储封装

### storage/sqlite.rs

```rust
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;

/// 异步安全的 SQLite 存储
/// rusqlite::Connection 不是 Send + Sync，使用 Mutex 封装
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(())
    }

    /// 异步执行查询
    pub async fn query<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();

        spawn_blocking(move || {
            let guard = conn.lock().map_err(|e| {
                RetrievalErr::SqliteError {
                    path: PathBuf::new(),
                    cause: format!("lock poisoned: {}", e),
                }
            })?;
            f(&guard)
        })
        .await
        .map_err(|e| RetrievalErr::SqliteError {
            path: self.path.clone(),
            cause: format!("spawn_blocking failed: {}", e),
        })?
    }

    /// 异步执行事务
    pub async fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.query(|conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
        .await
    }
}
```

---

## RRF 结果融合

### search/fusion.rs

```rust
use std::collections::HashMap;

/// RRF (Reciprocal Rank Fusion) 结果融合
pub struct RrfScorer {
    k: f32,              // 常数，通常 60
    bm25_weight: f32,    // BM25 权重
    vector_weight: f32,  // 向量权重
    snippet_weight: f32, // Snippet 权重
}

impl Default for RrfScorer {
    fn default() -> Self {
        Self {
            k: 60.0,
            bm25_weight: 0.6,
            vector_weight: 0.3,
            snippet_weight: 0.1,
        }
    }
}

impl RrfScorer {
    /// 融合多个搜索结果
    /// score = Σ w_i / (k + rank_i)
    pub fn fuse(
        &self,
        bm25_results: &[SearchResult],
        vector_results: &[SearchResult],
        snippet_results: &[SearchResult],
    ) -> Vec<SearchResult> {
        let mut scores: HashMap<String, (f32, Option<SearchResult>)> = HashMap::new();

        // BM25 结果
        for (rank, result) in bm25_results.iter().enumerate() {
            let key = format!("{}:{}", result.chunk.filepath, result.chunk.start_line);
            let score = self.bm25_weight / (self.k + rank as f32);
            scores
                .entry(key)
                .and_modify(|(s, _)| *s += score)
                .or_insert((score, Some(result.clone())));
        }

        // Vector 结果
        for (rank, result) in vector_results.iter().enumerate() {
            let key = format!("{}:{}", result.chunk.filepath, result.chunk.start_line);
            let score = self.vector_weight / (self.k + rank as f32);
            scores
                .entry(key)
                .and_modify(|(s, _)| *s += score)
                .or_insert((score, Some(result.clone())));
        }

        // Snippet 结果
        for (rank, result) in snippet_results.iter().enumerate() {
            let key = format!("{}:{}", result.chunk.filepath, result.chunk.start_line);
            let score = self.snippet_weight / (self.k + rank as f32);
            scores
                .entry(key)
                .and_modify(|(s, _)| *s += score)
                .or_insert((score, Some(result.clone())));
        }

        // 排序并返回
        let mut fused: Vec<_> = scores
            .into_iter()
            .filter_map(|(_, (score, result))| {
                result.map(|mut r| {
                    r.score = score;
                    r.score_type = ScoreType::Hybrid;
                    r
                })
            })
            .collect();

        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        fused
    }
}
```

---

## 查询改写 (Feature)

### query/translator.rs

```rust
/// 检测文本是否包含中文
pub fn contains_chinese(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'))
}

/// 查询改写器
pub struct QueryRewriter {
    llm_client: Arc<dyn LlmClient>,
}

impl QueryRewriter {
    pub fn new(llm_client: Arc<dyn LlmClient>) -> Self {
        Self { llm_client }
    }

    /// 翻译中文查询为英文
    pub async fn translate_to_english(&self, query: &str) -> Result<String> {
        if !contains_chinese(query) {
            return Ok(query.to_string());
        }

        let prompt = format!(
            "Translate the following Chinese code search query to English. \
             Keep technical terms, function names, and code identifiers unchanged.\n\n\
             Query: {}\n\nEnglish:",
            query
        );

        self.llm_client
            .complete(&prompt)
            .await
            .map_err(|e| RetrievalErr::SearchFailed {
                query: query.to_string(),
                cause: e.to_string(),
            })
    }
}
```

---

## 主服务接口

### service.rs

```rust
use std::sync::Arc;

/// 检索服务
pub struct RetrievalService {
    config: RetrievalConfig,
    lancedb: Arc<LanceDbStore>,
    sqlite: Arc<SqliteStore>,
    chunker: CodeChunkerService,
    tag_extractor: TagExtractor,
    features: Features,
}

impl RetrievalService {
    pub async fn new(config: RetrievalConfig, features: Features) -> Result<Self> {
        let lancedb = Arc::new(LanceDbStore::open(&config.data_dir.join("lancedb")).await?);
        let sqlite = Arc::new(SqliteStore::open(&config.data_dir.join("metadata.db"))?);
        let chunker = CodeChunkerService::new(config.chunking.max_chunk_size);
        let tag_extractor = TagExtractor::new()?;

        Ok(Self {
            config,
            lancedb,
            sqlite,
            chunker,
            tag_extractor,
            features,
        })
    }

    /// 索引工作空间
    pub async fn index_workspace(
        &self,
        workspace: &Path,
    ) -> impl Stream<Item = IndexProgress> {
        // 1. 获取锁
        let lock = IndexLockGuard::try_acquire(
            self.sqlite.clone(),
            &workspace.to_string_lossy(),
            Duration::from_secs(30),
        )
        .await?;

        // 2. 遍历文件
        // 3. 计算变更
        // 4. 索引
        // 5. 提交
        // (lock 自动释放)
    }

    /// 搜索代码
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        // 1. 查询预处理
        let processed_query = if self.features.enabled(Feature::QueryRewrite)
            && contains_chinese(&query.text)
        {
            let rewriter = QueryRewriter::new(self.llm_client.clone());
            let translated = rewriter.translate_to_english(&query.text).await?;
            SearchQuery {
                text: translated,
                ..query
            }
        } else {
            query
        };

        // 2. 执行搜索
        let bm25_results = self.lancedb.search_fts(&processed_query.text, 20).await?;

        let vector_results = if self.features.enabled(Feature::VectorSearch) {
            // 执行向量搜索
            let embedding = self.embed_query(&processed_query.text).await?;
            self.lancedb.search_vector(&embedding, 20).await?
        } else {
            vec![]
        };

        let snippet_results = self.search_snippets(&processed_query).await?;

        // 3. 融合结果
        let scorer = RrfScorer::default();
        let fused = scorer.fuse(&bm25_results, &vector_results, &snippet_results);

        // 4. 截取 Top-N
        Ok(fused.into_iter().take(query.limit as usize).collect())
    }
}
```

---

## 搜索配置

### config.rs (SearchConfig)

```rust
use serde::{Deserialize, Serialize};

/// 搜索配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// 最终返回结果数
    #[serde(default = "default_n_final")]
    pub n_final: i32,

    /// 初始检索候选数
    #[serde(default = "default_n_retrieve")]
    pub n_retrieve: i32,

    /// BM25 分数截断阈值 (负数，越小越严格)
    #[serde(default = "default_bm25_threshold")]
    pub bm25_threshold: f32,

    /// 重排序阈值
    #[serde(default = "default_rerank_threshold")]
    pub rerank_threshold: f32,

    /// 启用词干还原
    #[serde(default = "default_enable_stemming")]
    pub enable_stemming: bool,

    /// 启用 n-gram
    #[serde(default)]
    pub enable_ngrams: bool,

    /// n-gram 大小
    #[serde(default = "default_ngram_size")]
    pub ngram_size: i32,

    /// BM25 权重
    #[serde(default = "default_bm25_weight")]
    pub bm25_weight: f32,

    /// 向量搜索权重
    #[serde(default = "default_vector_weight")]
    pub vector_weight: f32,

    /// Snippet 匹配权重
    #[serde(default = "default_snippet_weight")]
    pub snippet_weight: f32,

    /// RRF 常数 k
    #[serde(default = "default_rrf_k")]
    pub rrf_k: f32,

    /// 路径匹配权重 (Continue: 10.0)
    #[serde(default = "default_path_weight_multiplier")]
    pub path_weight_multiplier: f32,

    /// 最大结果 Token 数 (Continue: 8000)
    #[serde(default = "default_max_result_tokens")]
    pub max_result_tokens: i32,

    /// Token 预算截断策略
    #[serde(default)]
    pub truncate_strategy: TruncateStrategy,

    /// 最大 Token 长度 (Tabby: 64)
    #[serde(default = "default_max_token_length")]
    pub max_token_length: i32,
}

/// Token 预算截断策略
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum TruncateStrategy {
    /// 从尾部截断
    #[default]
    Tail,
    /// 智能截断 (保留完整块)
    Smart,
}

fn default_n_final() -> i32 { 20 }
fn default_n_retrieve() -> i32 { 50 }
fn default_bm25_threshold() -> f32 { -2.5 }
fn default_rerank_threshold() -> f32 { 0.3 }
fn default_enable_stemming() -> bool { true }
fn default_ngram_size() -> i32 { 3 }
fn default_bm25_weight() -> f32 { 0.6 }
fn default_vector_weight() -> f32 { 0.3 }
fn default_snippet_weight() -> f32 { 0.1 }
fn default_rrf_k() -> f32 { 60.0 }
fn default_path_weight_multiplier() -> f32 { 10.0 }
fn default_max_result_tokens() -> i32 { 8000 }
fn default_max_token_length() -> i32 { 64 }

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            n_final: default_n_final(),
            n_retrieve: default_n_retrieve(),
            bm25_threshold: default_bm25_threshold(),
            rerank_threshold: default_rerank_threshold(),
            enable_stemming: default_enable_stemming(),
            enable_ngrams: false,
            ngram_size: default_ngram_size(),
            bm25_weight: default_bm25_weight(),
            vector_weight: default_vector_weight(),
            snippet_weight: default_snippet_weight(),
            rrf_k: default_rrf_k(),
            path_weight_multiplier: default_path_weight_multiplier(),
            max_result_tokens: default_max_result_tokens(),
            truncate_strategy: TruncateStrategy::default(),
            max_token_length: default_max_token_length(),
        }
    }
}
```

---

## 符号精确匹配优化

### search/identifier.rs

```rust
use regex::Regex;
use once_cell::sync::Lazy;

/// 标识符正则 (函数名/类名/变量名)
static IDENTIFIER_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap()
});

/// 检测查询是否为标识符
pub fn is_identifier_query(query: &str) -> bool {
    let trimmed = query.trim();
    // 单个词且匹配标识符模式
    !trimmed.contains(' ') && IDENTIFIER_REGEX.is_match(trimmed)
}

/// 动态权重调整 (基于 Continue CodeSnippetsIndex)
pub fn get_dynamic_weights(query: &str, config: &SearchConfig) -> (f32, f32, f32) {
    if is_identifier_query(query) {
        // 标识符查询: 提升 snippet 权重
        let snippet_weight = 0.4;  // 从 0.1 提升
        let remaining = 1.0 - snippet_weight;
        let bm25_weight = remaining * 0.6;
        let vector_weight = remaining * 0.4;
        (bm25_weight, vector_weight, snippet_weight)
    } else {
        // 普通查询: 使用配置权重
        (config.bm25_weight, config.vector_weight, config.snippet_weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_detection() {
        assert!(is_identifier_query("getUserById"));
        assert!(is_identifier_query("_private_var"));
        assert!(is_identifier_query("MAX_SIZE"));
        assert!(!is_identifier_query("get user by id"));
        assert!(!is_identifier_query("123abc"));
        assert!(!is_identifier_query("how to use async"));
    }
}
```

---

## Token 预算管理

### search/truncate.rs

```rust
use crate::types::{CodeChunk, SearchResult};
use crate::config::{SearchConfig, TruncateStrategy};

/// 根据 Token 预算截断结果
pub fn truncate_results(
    results: Vec<SearchResult>,
    config: &SearchConfig,
    count_tokens: impl Fn(&str) -> i32,
) -> Vec<SearchResult> {
    match config.truncate_strategy {
        TruncateStrategy::Tail => truncate_tail(results, config.max_result_tokens, count_tokens),
        TruncateStrategy::Smart => truncate_smart(results, config.max_result_tokens, count_tokens),
    }
}

/// 尾部截断: 超过预算立即停止
fn truncate_tail(
    results: Vec<SearchResult>,
    budget: i32,
    count_tokens: impl Fn(&str) -> i32,
) -> Vec<SearchResult> {
    let mut total = 0;
    results
        .into_iter()
        .take_while(|r| {
            let tokens = count_tokens(&r.chunk.content);
            if total + tokens > budget {
                false
            } else {
                total += tokens;
                true
            }
        })
        .collect()
}

/// 智能截断: 保留完整块，跳过超大块
fn truncate_smart(
    results: Vec<SearchResult>,
    budget: i32,
    count_tokens: impl Fn(&str) -> i32,
) -> Vec<SearchResult> {
    let mut total = 0;
    let mut output = Vec::new();

    for result in results {
        let tokens = count_tokens(&result.chunk.content);

        // 跳过超大块 (> 50% 预算)
        if tokens > budget / 2 {
            continue;
        }

        if total + tokens <= budget {
            total += tokens;
            output.push(result);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_count_tokens(s: &str) -> i32 {
        // 简单估算: 4 字符 = 1 token
        (s.len() / 4) as i32
    }

    #[test]
    fn test_truncate_tail() {
        let results = vec![
            make_result("a".repeat(100).as_str()),  // 25 tokens
            make_result("b".repeat(100).as_str()),  // 25 tokens
            make_result("c".repeat(100).as_str()),  // 25 tokens
        ];

        let config = SearchConfig {
            max_result_tokens: 60,
            truncate_strategy: TruncateStrategy::Tail,
            ..Default::default()
        };

        let truncated = truncate_results(results, &config, mock_count_tokens);
        assert_eq!(truncated.len(), 2);  // 只保留前 2 个
    }
}
```

---

## 查询预处理器

### query/preprocessor.rs

```rust
use std::collections::HashSet;

/// 查询预处理器
/// 参考 Continue BaseRetrievalPipeline.ts 的 getCleanedTrigrams 实现
pub struct QueryPreprocessor {
    stop_words: HashSet<String>,
    config: SearchConfig,
}

/// 预处理后的查询
#[derive(Debug, Clone)]
pub struct ProcessedQuery {
    /// 原始查询
    pub original: String,
    /// 处理后的 tokens
    pub tokens: Vec<String>,
    /// n-grams (可选)
    pub ngrams: Vec<String>,
}

impl QueryPreprocessor {
    pub fn new(config: SearchConfig) -> Self {
        Self {
            stop_words: Self::default_stop_words(),
            config,
        }
    }

    /// 预处理查询
    /// 1. 空格规范化
    /// 2. 分词
    /// 3. 移除停用词
    /// 4. 词干还原 (可选)
    /// 5. 去重
    /// 6. 生成 n-gram (可选)
    pub fn process(&self, query: &str) -> ProcessedQuery {
        // Step 1: 空格规范化
        let normalized = normalize_whitespace(query);

        // Step 2: 分词
        let tokens = tokenize(&normalized);

        // Step 3: 移除停用词
        let filtered: Vec<_> = tokens
            .into_iter()
            .filter(|t| !self.stop_words.contains(&t.to_lowercase()))
            .collect();

        // Step 4: 词干还原 (可选)
        let stemmed = if self.config.enable_stemming {
            stem_tokens(&filtered)
        } else {
            filtered
        };

        // Step 5: 去重
        let unique: Vec<_> = stemmed
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Step 6: 生成 n-gram (可选)
        let ngrams = if self.config.enable_ngrams {
            generate_ngrams(&unique.join(" "), self.config.ngram_size)
        } else {
            Vec::new()
        };

        ProcessedQuery {
            original: query.to_string(),
            tokens: unique,
            ngrams,
        }
    }

    fn default_stop_words() -> HashSet<String> {
        let words = [
            // 英文停用词
            "the", "a", "an", "is", "are", "was", "were", "be", "been",
            "being", "have", "has", "had", "do", "does", "did", "will",
            "would", "could", "should", "may", "might", "can", "this",
            "that", "these", "those", "i", "you", "he", "she", "it",
            "we", "they", "what", "which", "who", "whom", "how", "when",
            "where", "why", "all", "each", "every", "both", "few", "more",
            "most", "other", "some", "such", "no", "not", "only", "same",
            "so", "than", "too", "very", "just", "but", "and", "or", "if",
            "because", "as", "until", "while", "of", "at", "by", "for",
            "with", "about", "against", "between", "into", "through",
            "during", "before", "after", "above", "below", "to", "from",
            "up", "down", "in", "out", "on", "off", "over", "under",
            // 中文停用词
            "的", "了", "和", "是", "就", "都", "而", "及", "与", "着",
            "或", "一个", "没有", "我们", "你们", "他们", "它们", "这个",
            "那个", "这些", "那些", "什么", "怎么", "如何", "为什么",
        ];
        words.iter().map(|s| s.to_string()).collect()
    }
}

/// 空格规范化
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 分词
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}\"'".contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 词干还原 (使用 rust-stemmers)
fn stem_tokens(tokens: &[String]) -> Vec<String> {
    use rust_stemmers::{Algorithm, Stemmer};

    let en_stemmer = Stemmer::create(Algorithm::English);

    tokens
        .iter()
        .map(|t| {
            // 只对英文 token 进行词干还原
            if t.chars().all(|c| c.is_ascii_alphabetic()) {
                en_stemmer.stem(t).to_string()
            } else {
                t.clone()
            }
        })
        .collect()
}

/// 生成 n-grams
fn generate_ngrams(text: &str, n: i32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < n as usize {
        return vec![text.to_string()];
    }

    words
        .windows(n as usize)
        .map(|w| w.join(" "))
        .collect()
}
```

### 使用示例

```rust
// 在 service.rs 的 search 方法中
pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
    // 1. 查询预处理
    let preprocessor = QueryPreprocessor::new(self.config.search.clone());
    let processed = preprocessor.process(&query.text);

    // 2. 中文翻译 (Feature: QueryRewrite)
    let search_text = if self.features.enabled(Feature::QueryRewrite)
        && contains_chinese(&processed.original)
    {
        let rewriter = QueryRewriter::new(self.llm_client.clone());
        rewriter.translate_to_english(&processed.original).await?
    } else {
        processed.tokens.join(" ")
    };

    // 3. 执行搜索 (使用预处理后的文本)
    let bm25_results = self.lancedb
        .search_fts(&search_text, self.config.search.n_retrieve)
        .await?
        .into_iter()
        .filter(|r| r.score >= self.config.search.bm25_threshold)
        .collect::<Vec<_>>();

    // ... (向量搜索、Snippet 搜索)

    // 4. RRF 融合
    let scorer = RrfScorer::new(&self.config.search);
    let fused = scorer.fuse(&bm25_results, &vector_results, &snippet_results);

    // 5. Jaccard 重排序
    let ranked = rank_by_jaccard(&fused, &processed.original);

    // 6. 去重
    let deduped = deduplicate_results(ranked);

    // 7. 截取 Top-N
    Ok(deduped.into_iter().take(self.config.search.n_final as usize).collect())
}
```

---

## Jaccard 相似度

### search/ranking.rs

```rust
use std::collections::HashSet;

/// 计算 Jaccard 相似度
/// J(A, B) = |A ∩ B| / |A ∪ B|
pub fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let a_symbols: HashSet<&str> = split_symbols(a).collect();
    let b_symbols: HashSet<&str> = split_symbols(b).collect();

    let intersection = a_symbols.intersection(&b_symbols).count();
    let union = a_symbols.union(&b_symbols).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// 按符号分割文本 (保留 camelCase)
fn split_symbols(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| {
        c.is_whitespace() || ".,;:!?()[]{}\"'`~@#$%^&*-+=<>/\\|".contains(c)
    })
    .filter(|s| !s.is_empty())
}

/// 使用 Jaccard 相似度对结果重排序
pub fn rank_by_jaccard(results: Vec<SearchResult>, query: &str) -> Vec<SearchResult> {
    let mut ranked: Vec<_> = results
        .into_iter()
        .map(|mut r| {
            let jaccard = jaccard_similarity(query, &r.chunk.content);
            // 组合原始分数和 Jaccard 相似度
            r.score = r.score * 0.7 + jaccard * 0.3;
            r
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
    });

    ranked
}
```

---

## 重叠结果去重

### search/dedup.rs

```rust
use std::ops::Range;

/// 去重重叠结果
/// 同一文件中行范围有重叠的结果只保留一个
pub fn deduplicate_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut deduped: Vec<SearchResult> = Vec::new();

    for result in results {
        let overlaps = deduped.iter().any(|existing| {
            existing.chunk.filepath == result.chunk.filepath
                && ranges_overlap(
                    existing.chunk.start_line..existing.chunk.end_line,
                    result.chunk.start_line..result.chunk.end_line,
                )
        });

        if !overlaps {
            deduped.push(result);
        } else {
            // 合并重叠结果
            merge_overlapping(&mut deduped, result);
        }
    }

    deduped
}

/// 检查两个范围是否重叠
fn ranges_overlap(a: Range<i32>, b: Range<i32>) -> bool {
    a.start < b.end && b.start < a.end
}

/// 合并重叠结果
fn merge_overlapping(results: &mut Vec<SearchResult>, new: SearchResult) {
    if let Some(existing) = results.iter_mut().find(|r| {
        r.chunk.filepath == new.chunk.filepath
            && ranges_overlap(
                r.chunk.start_line..r.chunk.end_line,
                new.chunk.start_line..new.chunk.end_line,
            )
    }) {
        // 扩展范围到覆盖两者
        existing.chunk.start_line = existing.chunk.start_line.min(new.chunk.start_line);
        existing.chunk.end_line = existing.chunk.end_line.max(new.chunk.end_line);
        // 保留更高分数
        existing.score = existing.score.max(new.score);
    }
}
```

---

## 依赖配置

### Cargo.toml

```toml
[package]
name = "codex-retrieval"
version.workspace = true
edition.workspace = true

[dependencies]
# 存储
lancedb = "0.15"
rusqlite = { version = "0.32", features = ["bundled"] }

# 代码分块 (内置 tree-sitter)
text-splitter = { version = "0.13", features = ["code", "tiktoken-rs"] }
tree-sitter-rust = "0.21.2"
tree-sitter-go = "0.21.0"
tree-sitter-python = "0.21.0"
tree-sitter-java = "0.21.0"

# 标签提取
tree-sitter-tags = "0.22.6"

# 查询预处理
rust-stemmers = "1.2"   # 词干还原

# Async
tokio = { workspace = true, features = ["full"] }
async-trait = { workspace = true }
futures = { workspace = true }
async-stream = { workspace = true }

# Utils
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = "0.10"
ignore = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile = { workspace = true }
tokio-test = { workspace = true }
```
