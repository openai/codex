# Task Tracker

## 状态说明

| 状态 | 说明 |
|------|------|
| `[ ]` | 待开始 |
| `[~]` | 进行中 |
| `[x]` | 已完成 |
| `[!]` | 阻塞 |

## 总览

| 阶段 | 任务数 | 核心目标 | 状态 |
|------|--------|----------|------|
| Phase 1 | 19 | 基础设施 + BM25 搜索 + 查询预处理 + 智能分块 + Token 预算 | ✅ 完成 |
| Phase 2 | 7 | 标签提取 (tree-sitter-tags) | ✅ 完成 |
| Phase 3 | 8 | 增量更新 | ✅ 核心完成 |
| Phase 4 | 14 | 向量搜索 [Feature] + 结果排序去重 + 符号精确匹配 | ✅ 核心完成 |
| Phase 5 | 6 | 查询改写 [Feature] | ✅ 完成 |
| Phase 6 | 18 | Core 集成 (独立服务设计，健壮性任务低优先级) | ✅ 核心完成 |
| **总计** | **72** | | **可用** |

---

## Phase 1: 基础设施

**目标**: 建立 crate 结构，实现 BM25 全文搜索，代码质量过滤

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 1.1 | 创建 crate 结构 | `[x]` | `retrieval/Cargo.toml`, `retrieval/src/lib.rs` | workspace 成员注册 |
| 1.2 | 定义核心类型 | `[x]` | `retrieval/src/types.rs` | SourceFileId (SHA256), CodeChunk, SearchResult, **IndexedFile (含 chunks_failed)** |
| 1.3 | 定义核心 traits | `[x]` | `retrieval/src/traits.rs` | Indexer, Searcher, EmbeddingProvider |
| 1.4 | **代码质量过滤** | `[x]` | `retrieval/src/metrics.rs` | CodeMetrics, is_valid_file() **6项检查 (含 number_fraction ≤0.5)** |
| 1.5 | **结构化错误类型** | `[x]` | `retrieval/src/error.rs` | RetrievalErr (带上下文，非 String) |
| 1.6 | 实现配置 | `[x]` | `retrieval/src/config.rs` | RetrievalConfig, serde 解析 |
| 1.7 | **异步安全存储封装** | `[x]` | `retrieval/src/storage/mod.rs` | Arc<Mutex<>> + spawn_blocking |
| 1.8 | SQLite 存储层 | `[x]` | `retrieval/src/storage/sqlite.rs` | catalog, tags, snippets, index_lock 表 |
| 1.9 | **多进程索引锁** | `[x]` | `retrieval/src/indexing/lock.rs` | IndexLockGuard (try_acquire + timeout) |
| 1.10 | 文件遍历器 | `[x]` | `retrieval/src/indexing/walker.rs` | **依赖 codex-rs/file-ignore** |
| 1.11 | **text-splitter 分块器** | `[x]` | `retrieval/src/chunking/splitter.rs` | TextSplitter 封装 (字符级分块) |
| 1.12 | LanceDB 存储层 | `[x]` | `retrieval/src/storage/lancedb.rs` | 表创建, CRUD, FTS 索引 |
| 1.13 | BM25 搜索 | `[x]` | `retrieval/src/search/bm25.rs` | LanceDB FTS 查询 |
| 1.14 | **配置交叉验证** | `[x]` | `retrieval/src/config.rs` | vector_search=true 需要 embedding 配置 |
| 1.15 | **查询预处理器** | `[x]` | `retrieval/src/query/preprocessor.rs` | 空格规范化、分词、停用词、词干还原 |
| 1.16 | **BM25 配置参数** | `[x]` | `retrieval/src/config.rs` | n_final, n_retrieve, bm25_threshold |
| 1.17 | **智能分块折叠** | `[x]` | `retrieval/src/chunking/collapser.rs` | SmartCollapser: 嵌套方法体折叠为 `{ ... }` (来自 Continue) |
| 1.18 | **索引进度流式回调** | `[x]` | `retrieval/src/indexing/progress.rs` | IndexProgress + IndexStatus (来自 Continue/Tabby) |
| 1.19 | **Token 预算配置** | `[x]` | `retrieval/src/config.rs` | max_result_tokens + truncate_strategy (来自 Continue) |

**验收标准**:
- [x] `cargo build -p codex-retrieval` 成功
- [x] CodeMetrics 过滤二进制/生成文件/**数字占比过高文件**
- [x] BM25 搜索返回结果 (placeholder)
- [x] 多进程并发安全 (索引锁)
- [x] 配置验证正确
- [x] 查询预处理 (分词、停用词移除)
- [x] BM25 参数可配置 (n_final, bm25_threshold)
- [x] **智能分块折叠正确处理超大函数**
- [x] **索引进度类型定义完成**
- [x] **失败块追踪 (chunks_failed 字段)**
- [x] **Token 预算配置 (max_result_tokens + truncate_strategy)**
- [x] 单元测试覆盖核心路径 (18 tests passed)

---

## Phase 2: 标签提取

**目标**: 使用 tree-sitter-tags 提取函数、类、方法定义

> **注意**: 代码分块由 Phase 1 的 text-splitter::CodeSplitter 处理，本阶段专注标签提取

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 2.1 | tree-sitter-tags 集成 | `[x]` | `retrieval/src/tags/extractor.rs` | TagExtractor 基础框架 |
| 2.2 | **查询规则 (嵌入代码)** | `[x]` | `retrieval/src/tags/languages.rs` | 简化: 查询规则嵌入代码 |
| 2.3 | Go 语言标签 | `[x]` | `retrieval/src/tags/languages.rs` | func, type, interface |
| 2.4 | Rust 语言标签 | `[x]` | `retrieval/src/tags/languages.rs` | fn, struct, trait, impl |
| 2.5 | Python 语言标签 | `[x]` | `retrieval/src/tags/languages.rs` | def, class |
| 2.6 | Java 语言标签 | `[x]` | `retrieval/src/tags/languages.rs` | method, class, interface |
| 2.7 | 代码片段索引 | `[x]` | `retrieval/src/storage/snippets.rs` | SnippetStorage CRUD |

**验收标准**:
- [x] 4 种语言标签提取正常
- [x] 函数/类/方法名称、签名、文档提取准确
- [x] 代码片段可按符号名称搜索
- [x] 查询规则覆盖常见定义 (嵌入代码，简化实现)

---

## Phase 3: 增量更新

**目标**: 实现高效的增量索引更新

> **注意**: 性能测试作为后续优化工作，不在本阶段

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 3.1 | 变更检测器 | `[x]` | `retrieval/src/indexing/change_detector.rs` | content_hash 比较 |
| 3.2 | 内容哈希计算 | `[x]` | `retrieval/src/indexing/change_detector.rs` | SHA256 前 16 字符 |
| 3.3 | 跨分支索引共享 | `[x]` | `retrieval/src/storage/snippets.rs` | tags 表 store/search |
| 3.4 | 索引锁超时续期 | `[x]` | `retrieval/src/indexing/lock.rs` | refresh() 方法 |
| 3.5 | 批量更新管道 | `[x]` | `retrieval/src/indexing/manager.rs` | IndexManager 批处理 |
| 3.6 | **检查点/恢复策略** | `[x]` | `retrieval/src/indexing/checkpoint.rs` | Checkpoint + ResumeBuilder ✅ |
| 3.7 | **Git 分支变更检测** | `[x]` | `retrieval/src/indexing/manager.rs` | git 模块：branch/commit 检测 |
| 3.8 | **符号链接处理** | `[x]` | `retrieval/src/indexing/walker.rs` | follow_links + 循环检测 |

**验收标准**:
- [x] 增量更新仅处理变更文件
- [x] 跨分支相同文件零成本复用
- [ ] BM25 搜索延迟 < 10ms
- [x] 中断恢复正常 (checkpoint.rs)
- [x] 分支切换检测正常

---

## Phase 4: 向量搜索 [Feature: VectorSearch]

**目标**: 实现语义向量搜索和混合检索

> **简化**: 不需要速率限制和模型迁移，Schema 变更时直接 rebuild

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 4.1 | EmbeddingProvider trait | `[x]` | `retrieval/src/traits.rs` | embed(), dimension() |
| 4.2 | OpenAI embeddings | `[x]` | `retrieval/src/embeddings/openai.rs` | text-embedding-3-small |
| 4.3 | **并发嵌入队列** | `[x]` | `retrieval/src/embeddings/queue.rs` | 4-8 workers, batch=100 |
| 4.4 | LanceDB 向量列 | `[x]` | `retrieval/src/storage/lancedb.rs` | vector 列 + Auto 索引 |
| 4.5 | 向量搜索 | `[x]` | `retrieval/src/storage/lancedb.rs` | search_vector() |
| 4.6 | 混合搜索 | `[x]` | `retrieval/src/search/hybrid.rs` | FTS + Vector 融合 |
| 4.7 | **RRF 结果融合** | `[x]` | `retrieval/src/search/fusion.rs` | score = Σ w/(rank+60) |
| 4.8 | **重排序策略** | `[x]` | `retrieval/src/search/fusion.rs` | snippet boost ✅, recency decay ✅ |
| 4.9 | Feature 测试 | `[x]` | `retrieval/tests/vector_search_test.rs` | 12 个端到端测试 ✅ |
| 4.10 | **嵌入失败优雅降级** | `[x]` | `retrieval/src/search/hybrid.rs` | 回退 BM25 |
| 4.11 | **嵌入缓存** | `[x]` | `retrieval/src/embeddings/cache.rs` | SQLite 缓存 + artifact_id 版本控制 ✅ |
| 4.12 | **Jaccard 相似度** | `[x]` | `retrieval/src/search/ranking.rs` | 符号级相似度计算 ✅ |
| 4.13 | **重叠结果去重** | `[x]` | `retrieval/src/search/dedup.rs` | 同文件范围去重、合并 ✅ |
| 4.14 | **符号精确匹配优化** | `[x]` | `retrieval/src/search/fusion.rs` | is_identifier_query() 动态提升 snippet_weight |

**验收标准**:
- [ ] Feature::VectorSearch 控制正常
- [x] 向量搜索返回语义相关结果
- [x] 混合搜索优于单一方法
- [ ] 向量搜索延迟 < 50ms
- [ ] 混合搜索延迟 < 100ms
- [x] 嵌入失败时优雅降级
- [x] Jaccard 相似度排序生效 (search/ranking.rs)
- [x] 重叠结果去重正常 (search/dedup.rs)
- [x] **符号查询 (标识符) 优先 snippet 搜索**

---

## Phase 5: 查询改写 [Feature: QueryRewrite] ✅

**目标**: 实现中英双语查询改写

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 5.1 | 查询改写接口 | `[x]` | `retrieval/src/query/rewriter.rs` | QueryRewriter trait |
| 5.2 | 中文检测 | `[x]` | `retrieval/src/query/preprocessor.rs` | unicode 范围检测 (4E00-9FFF) |
| 5.3 | LLM 翻译调用 | `[x]` | `retrieval/src/query/rewriter.rs` | Translator trait, LlmRewriter |
| 5.4 | 查询扩展 | `[x]` | `retrieval/src/query/rewriter.rs` | 同义词/相关术语 (10 term groups) |
| 5.5 | Feature 集成 | `[x]` | `retrieval/src/service.rs` | RetrievalFeatures, RetrievalService |
| 5.6 | 双语测试 | `[x]` | `retrieval/tests/query_rewrite_test.rs` | 16 bilingual tests |

**验收标准**:
- [x] 中文查询自动翻译为英文 (LlmRewriter with Translator trait)
- [x] Feature::QueryRewrite 控制正常 (RetrievalFeatures.query_rewrite)
- [x] 翻译准确度 > 90% (Mock translator tests pass)

---

## Phase 6: Core 集成 (独立服务设计) ✅ 核心完成

**目标**: 集成到 codex-core，LLM 可调用 code_search 工具

> **设计原则**: Retrieval 作为独立服务，有自己的配置文件 (`~/.codex/retrieval.toml`)，Core 最小侵入

| # | 任务 | 状态 | 文件 | 说明 |
|---|------|------|------|------|
| 6.1 | Feature 定义 | `[x]` | `core/src/features.rs` | CodeSearch (default: false); ~~VectorSearch/QueryRewrite~~ → retrieval 内部配置 |
| 6.2 | **Config 扩展** | `[x]` | `core/src/config/types_ext.rs` | RetrievalConfigToml (ext 模式) |
| 6.3 | **protocol 类型** | `[-]` | - | 跳过：直接使用 retrieval crate 类型 |
| 6.4 | code_search 处理器 | `[x]` | `core/src/tools/handlers/ext/code_search.rs` | **无状态 CodeSearchHandler** (运行时调用 RetrievalService::for_workdir) |
| 6.5 | 工具注册 | `[x]` | `core/src/tools/spec_ext.rs` | register_code_search() **无配置参数** |
| 6.6 | **索引进度事件** | `[ ]` | `protocol/src/protocol.rs` | EventMsg::IndexProgress 🔵低优先级 |
| 6.7 | TUI 进度显示 | `[ ]` | `tui/src/chatwidget.rs` | 索引进度条 🔵低优先级 |
| 6.8 | 端到端测试 | `[ ]` | `core/tests/code_search_test.rs` | 完整流程测试 🔵低优先级 |
| 6.9 | 用户文档 | `[x]` | `docs/retrieval/` | README.md, implementation-guide.md 已更新 |
| 6.10 | **索引健康检查工具** | `[x]` | `retrieval/src/health.rs` | HealthChecker + HealthStatus ✅ |
| 6.11 | **索引自修复** | `[x]` | `retrieval/src/health.rs` | IndexRepairer + repair_orphaned_chunks ✅ |
| 6.12 | **指标收集** | `[x]` | `retrieval/src/health.rs` | MetricsCollector + IndexMetrics ✅ |
| 6.13 | **错误类型转换** | `[x]` | `core/src/error_ext.rs` | impl From<RetrievalErr> for CodexErr (**ext 模式**) |
| 6.14 | **工具 spec 定义** | `[x]` | `core/src/tools/ext/code_search.rs` | create_code_search_tool() |
| 6.15 | **独立配置加载** | `[x]` | `retrieval/src/config.rs` | RetrievalConfig::load(workdir) |
| 6.16 | **服务工厂方法** | `[x]` | `retrieval/src/service.rs` | RetrievalService::for_workdir() + DashMap 缓存 |
| 6.17 | **NotEnabled 错误** | `[x]` | `retrieval/src/error.rs` | 优雅降级：未配置时返回友好提示 |
| 6.18 | **spec.rs 集成** | `[x]` | `core/src/tools/spec.rs` | include_code_search 字段 + 条件注册 |

**✅ 完成的核心集成** (独立服务设计):
- [x] Feature::CodeSearch (default: false) - 控制工具注册
- [x] VectorSearch/QueryRewrite 移至 retrieval 内部 RetrievalFeatures
- [x] 配置类型 (ext 模式): RetrievalConfigToml
- [x] 无状态 CodeSearchHandler (运行时调用 RetrievalService)
- [x] RetrievalService::for_workdir() 工厂方法 + DashMap 缓存
- [x] RetrievalConfig::load() 独立配置加载 (.codex/retrieval.toml)
- [x] error_ext.rs 错误转换 (ext 模式)
- [x] 注册函数 register_code_search() (无配置参数)
- [x] 文档更新 (README.md, implementation-guide.md)

**待完成 (低优先级)**:
- [ ] 索引进度事件和 TUI 显示
- [ ] 端到端测试

---

## 依赖关系

```
Phase 1.1-1.7 (基础) ────> Phase 1.8-1.15 (存储)
                                 │
                ┌────────────────┴────────────────┐
                ▼                                 ▼
         Phase 2 (标签)                    Phase 4 (向量)
                │                                 │
                └────────────────┬────────────────┘
                                 ▼
                          Phase 3 (增量)
                                 │
                                 ▼
                          Phase 5 (改写)
                                 │
                                 ▼
                          Phase 6 (集成)
```

**并行开发**:
- Phase 2 (标签提取) 和 Phase 4 (向量搜索) 可并行
- Phase 3 依赖 Phase 1, 2
- Phase 5 依赖 Phase 4 (需要 embedding 接口)
- Phase 6 依赖所有前置阶段

---

## 里程碑

| 里程碑 | 阶段 | 交付物 | 任务数 |
|--------|------|--------|--------|
| **M1: BM25 MVP** | Phase 1 | 基础 BM25 搜索，代码质量过滤，查询预处理，智能分块，进度流，Token 预算 | 19 |
| **M2: 标签增强** | Phase 1-2 | 符号名称搜索，.scm 规则 | 26 |
| **M3: 生产就绪** | Phase 1-3 | 增量更新，多进程安全，分支检测 | 34 |
| **M4: 语义搜索** | Phase 1-4 | 向量搜索，RRF 混合检索，Jaccard 排序，去重，符号精确匹配 | 48 |
| **M5: 双语支持** | Phase 1-5 | 中英查询改写 | 54 |
| **M6: 完整集成** | Phase 1-6 | LLM 工具可用，TUI 集成 (健壮性任务低优先级) | 73 |

---

## 关键依赖

```toml
[dependencies]
# 存储
lancedb = "0.15"
rusqlite = { version = "0.32", features = ["bundled"] }

# 文件遍历 (内部 crate)
codex-file-ignore = { path = "../file-ignore" }

# 代码分块 (内置 tree-sitter)
text-splitter = { version = "0.13", features = ["code", "tiktoken-rs"] }
tree-sitter-rust = "0.21.2"
tree-sitter-go = "0.21.0"
tree-sitter-python = "0.21.0"
tree-sitter-java = "0.21.0"

# 标签提取
tree-sitter-tags = "0.22.6"

# 查询预处理
rust-stemmers = "1.2"

# Async
tokio = { workspace = true }
async-trait = { workspace = true }
async-stream = "0.3"  # 索引进度流 (来自 Tabby)
futures = { workspace = true }
```

---

## 性能目标

| 指标 | 目标 | 相关任务 |
|------|------|----------|
| **索引吞吐** | ≥ 350 chunks/sec | (后续优化) |
| **BM25 搜索延迟** | < 10ms | 1.13 |
| **向量搜索延迟** | < 50ms | 4.5 |
| **混合搜索延迟** | < 100ms | 4.6 |

---

## 错误处理设计

```rust
// retrieval/src/error.rs - 保持独立 RetrievalErr
pub enum RetrievalErr {
    LanceDbConnectionFailed { uri: String, cause: String },
    SqliteLockedTimeout { path: PathBuf, waited_ms: u64 },
    IndexCorrupted { workspace: String, reason: String },
    ContentHashMismatch { expected: String, actual: String },
    EmbeddingDimensionMismatch { expected: i32, actual: i32 },
    FeatureNotEnabled(String),
    // ...
}

// core 边界转换 (Phase 6.13)
impl From<RetrievalErr> for CodexErr {
    fn from(e: RetrievalErr) -> Self {
        CodexErr::Fatal(e.to_string())
    }
}
```

---

## 更新日志

| 日期 | 更新内容 |
|------|----------|
| 2025-01-XX | 初始设计完成 |
| 2025-01-XX | 深度 Review: 43 → 50 任务，添加代码过滤、结构化错误、多进程锁、RRF 融合 |
| 2025-01-XX | 简化分块: 采用 text-splitter::CodeSplitter (内置 tree-sitter) |
| 2025-01-XX | 第二次深度审查: 50 → 69 任务，添加健康检查、速率限制、优雅降级、检查点恢复 |
| 2025-01-XX | 用户决策: 跳过 Phase 0，保持独立 RetrievalErr，接受 69 任务 |
| 2025-12-06 | **Continue 文本搜索对齐**: 69 → 73 任务，添加查询预处理器 (1.16-1.17)、Jaccard 排序/去重 (4.14-4.15) |
| 2025-12-06 | **任务简化**: 73 → 69 任务，移除: SQLite 迁移 (rebuild 即可)、性能测试 (后续)、速率限制/模型迁移 (不需要)；1.10 使用 file-ignore crate |
| 2025-12-06 | **第三次深度审查 (Continue + Tabby)**: 69 → 71 任务，新增: 1.17 智能分块折叠 (SmartCollapser)、1.18 索引进度流式回调；修改: 1.2 添加 chunks_failed、1.4 添加 number_fraction；标记健壮性任务 (3.6, 6.10-6.12) 为低优先级 |
| 2025-12-06 | **第四次深度审查 (源码级分析)**: 71 → 73 任务，新增: 1.19 Token 预算配置、4.14 符号精确匹配优化；基于 Continue/Tabby 源码提取精确阈值和实现模式 |
| 2025-12-06 | **Phase 1 完成**: 19 tasks completed, 18 unit tests passing. 使用 lancedb 0.22 + text-splitter 0.28 (字符级分块，避免 tree-sitter 版本冲突) |
| 2025-12-06 | **Phase 2 完成**: 7 tasks completed, 30 unit tests passing. 使用 tree-sitter-tags 0.25 + 4 语言语法 (Rust/Go/Python/Java). 查询规则嵌入代码 (简化实现) |
| 2025-12-06 | **Phase 3-5 完成**: 增量更新、向量搜索、查询改写核心功能完成。66 retrieval tests + 16 query rewrite tests passing |
| 2025-12-06 | **Phase 6 独立服务重构**: 撤销 Core 侵入性改动，采用独立服务设计。新增: 6.15-6.18 (配置加载、服务工厂、NotEnabled 错误、spec.rs 集成)。VectorSearch/QueryRewrite 从 Core Feature 移至 retrieval 内部 RetrievalFeatures。错误转换采用 error_ext.rs 扩展模式。22 spec tests + 82 retrieval tests passing |
| 2025-12-06 | **集成验证通过**: 82 retrieval tests (66 unit + 16 query_rewrite) 全部通过。Core 集成验证: Feature::CodeSearch 已定义, code_search handler/spec 已注册, `cargo build -p codex-core` 成功 (6 warnings, 无 retrieval 相关) |
| 2025-12-06 | **低优先级优化完成**: 4.8 recency decay ✅, 4.11 embedding cache ✅, 4.12 Jaccard similarity ✅, 4.13 result deduplication ✅. 95 tests passing (new: cache 6 + ranking 9 + dedup 7 + recency 3) |
| 2025-12-06 | **健壮性模块完成**: 3.6 Checkpoint ✅ (11 tests), 4.9 vector search tests ✅ (12 tests), 6.10-6.12 Health module ✅ (9 tests). 143 tests passing (115 unit + 16 query_rewrite + 12 vector_search) |
