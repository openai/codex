# Codex Retrieval System

代码检索系统，为 codex-rs 提供智能代码搜索能力。

## 架构设计

**Retrieval 作为独立服务**，有自己的配置系统和生命周期，与 Core 最小耦合：

```
Core (最小侵入)                    Retrieval (独立服务)
┌────────────────────┐            ┌─────────────────────┐
│ Feature::CodeSearch│──controls──│ code_search tool    │
│ (default: false)   │  register  │ registration        │
└────────────────────┘            └─────────────────────┘
                                           │
                                           ▼
                                  ┌─────────────────────┐
                                  │ retrieval.toml      │
                                  │ - enabled           │
                                  │ - code_search       │
                                  │ - vector_search     │
                                  │ - query_rewrite     │
                                  └─────────────────────┘
```

## 功能概览

| 功能 | 说明 | 配置位置 | 默认 |
|------|------|---------|------|
| **code_search 工具** | 注册工具到 LLM | Core: `[features] code_search` | 关闭 |
| **BM25 全文搜索** | 关键词匹配，高性能 | retrieval.toml | 启用 |
| **向量语义搜索** | Embedding 相似度 | retrieval.toml | 关闭 |
| **查询改写** | 中英双语翻译/强化 | retrieval.toml | 启用 |
| **AST 标签提取** | Go/Rust/Python/Java 符号提取 | - | 启用 |
| **增量更新** | 内容哈希 (SHA256) 变更检测 | - | 启用 |

## 快速开始

### 步骤 1: 启用 Core Feature (控制工具注册)

```toml
# ~/.codex/config.toml

[features]
code_search = true      # 启用代码搜索工具 (实验性)
```

### 步骤 2: 配置 Retrieval 服务

Retrieval 有独立的配置系统，按优先级查找：
1. 项目级：`.codex/retrieval.toml` (相对于 cwd)
2. 全局级：`~/.codex/retrieval.toml`

```toml
# ~/.codex/retrieval.toml 或 .codex/retrieval.toml

[retrieval]
enabled = true
data_dir = "~/.codex/retrieval"  # 索引存储目录

# 搜索功能配置 (Retrieval 内部，非 Core Feature)
[features]
code_search = true      # BM25 全文搜索 (默认开启)
vector_search = false   # 向量搜索 (需要 embedding 配置)
query_rewrite = true    # 查询改写 (默认开启)

# 索引配置
[indexing]
max_file_size_mb = 5        # 跳过超大文件
batch_size = 100            # 批量处理文件数
commit_interval = 100       # 每 N 个操作提交一次

# 分块配置
[chunking]
max_chunk_size = 512        # 最大块大小 (字符)
chunk_overlap = 64          # 块重叠 (字符)

# 搜索配置
[search]
n_final = 20                # 最终返回结果数
n_retrieve = 50             # 初始检索候选数
bm25_weight = 0.6           # BM25 权重
vector_weight = 0.3         # 向量权重
snippet_weight = 0.1        # Snippet 权重

# Embedding 配置 (仅 vector_search = true 时需要)
[embedding]
provider = "openai"
model = "text-embedding-3-small"
dimension = 1536
```

### 使用搜索

代码搜索工具 `code_search` 可被 LLM 自动调用：

```json
{
  "name": "code_search",
  "arguments": {
    "query": "用户认证逻辑",
    "limit": 10,
    "path_filter": ["src/auth/"]
  }
}
```

**返回示例**:
```json
{
  "results": [
    {
      "filepath": "src/auth/handler.rs",
      "start_line": 42,
      "end_line": 78,
      "content": "pub async fn authenticate(...",
      "score": 0.92
    }
  ]
}
```

## 技术栈

| 组件 | 技术选型 | 说明 |
|------|----------|------|
| **向量+混合搜索** | LanceDB | 内置 Tantivy BM25 FTS |
| **元数据存储** | SQLite (rusqlite) | 增量 catalog、跨分支 tags、snippets |
| **代码分块** | text-splitter (CodeSplitter) | **内部使用 tree-sitter**，语法感知分割 |
| **标签提取** | tree-sitter-tags | 函数名、签名、文档注释提取 |

### text-splitter vs tree-sitter

| 用途 | 库 | 说明 |
|------|---|------|
| **代码分块** | `text-splitter::CodeSplitter` | 内置 tree-sitter，按语法边界分割 |
| **标签提取** | `tree-sitter-tags` | 提取函数名、签名、文档，存入 SQLite |

```rust
// text-splitter 内部使用 tree-sitter 解析
// 无需自己实现 AST chunker
let splitter = CodeSplitter::new(tree_sitter_rust::LANGUAGE, 512)?;
let chunks = splitter.chunks(code);
```

## 性能目标

| 指标 | 目标 |
|------|------|
| **索引吞吐** | ≥ 350 chunks/sec |
| **BM25 搜索延迟** | < 10ms |
| **向量搜索延迟** | < 50ms |
| **混合搜索延迟** | < 100ms |

## 文档索引

- [架构设计](./architecture.md) - 系统架构和数据流
- [实现指南](./implementation-guide.md) - 核心接口和代码示例
- [任务追踪](./task-tracker.md) - 开发任务清单

## 目录结构

```
codex-rs/retrieval/
├── Cargo.toml
└── src/
    ├── lib.rs              # 公共 API
    ├── error.rs            # RetrievalErr (结构化错误)
    ├── types.rs            # SourceFileId, CodeChunk, SearchResult
    ├── traits.rs           # Indexer, Searcher, EmbeddingProvider
    ├── config.rs           # RetrievalConfig
    ├── metrics.rs          # CodeMetrics, is_valid_file()
    ├── indexing/           # 索引管道
    │   ├── manager.rs
    │   ├── walker.rs       # 文件遍历 (.gitignore/.ignore)
    │   ├── change_detector.rs
    │   └── lock.rs         # 多进程索引锁
    ├── chunking/           # 代码分块
    │   └── splitter.rs     # text-splitter::CodeSplitter 封装
    ├── tags/               # 标签提取
    │   ├── extractor.rs    # tree-sitter-tags 集成
    │   └── queries/        # .scm 查询规则
    ├── storage/
    │   ├── lancedb.rs      # 向量+FTS 存储
    │   └── sqlite.rs       # 元数据存储
    ├── search/
    │   ├── bm25.rs
    │   ├── vector.rs       # [Feature: VectorSearch]
    │   ├── hybrid.rs       # [Feature: VectorSearch]
    │   └── fusion.rs       # RRF 结果融合
    ├── query/              # [Feature: QueryRewrite]
    │   ├── rewriter.rs
    │   └── translator.rs   # CN↔EN
    └── service.rs          # RetrievalService
```

## 关键依赖

```toml
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
```

## 参考项目

设计参考了以下优秀项目：

- **[Continue](https://github.com/continuedev/continue)** - 4层混合索引架构 (Snippets + FTS + Chunks + Vector)
- **[Tabby](https://github.com/TabbyML/tabby)** - Rust + Tantivy 实现，代码质量过滤
- **[text-splitter](https://github.com/benbrandt/text-splitter)** - tree-sitter 语法感知分块
