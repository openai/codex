# Retrieval System Architecture

## 系统架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Application Layer                             │
│              (CLI, TUI, app-server, mcp-server)                     │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                      codex-core Integration                          │
│                                                                      │
│   ┌─────────────────┐    ┌─────────────────┐    ┌────────────────┐  │
│   │ features.rs     │    │ spec_ext.rs     │    │ config.rs      │  │
│   │ CodeSearch      │    │ code_search     │    │ RetrievalConfig│  │
│   │ VectorSearch    │    │ tool handler    │    │                │  │
│   │ QueryRewrite    │    │                 │    │                │  │
│   └─────────────────┘    └─────────────────┘    └────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                      codex-retrieval Crate                           │
├─────────────┬──────────────┬──────────────┬─────────────────────────┤
│  Indexing   │   Storage    │   Search     │   Query Rewrite         │
│  Pipeline   │   Layer      │   Pipeline   │   [Feature]             │
├─────────────┼──────────────┼──────────────┼─────────────────────────┤
│             │              │              │                         │
│  Walker     │  LanceDB     │  BM25        │  CN Detector            │
│  ├─ ignore  │  ├─ chunks   │  Searcher    │  ├─ unicode range       │
│  └─ filter  │  ├─ vectors  │              │  └─ regex               │
│             │  └─ fts      │  Vector      │                         │
│  Metrics    │              │  Searcher    │  LLM Translator         │
│  └─ valid   │  SQLite      │  [Feature]   │  ├─ CN→EN               │
│             │  ├─ catalog  │              │  └─ query expand        │
│  Chunker    │  ├─ tags     │  Hybrid      │                         │
│  └─ text-   │  ├─ snippets │  Searcher    │  RRF Fusion             │
│    splitter │  └─ lock     │  [Feature]   │  └─ score merge         │
│             │              │              │                         │
│  Tags       │              │              │                         │
│  └─ tree-   │              │              │                         │
│    sitter   │              │              │                         │
└─────────────┴──────────────┴──────────────┴─────────────────────────┘
```

## 存储架构

### 双存储设计

| 存储 | 技术 | 职责 |
|------|------|------|
| **LanceDB** | 向量数据库 | 代码块存储、向量索引、BM25 FTS、混合搜索 |
| **SQLite** | 关系数据库 | 索引元数据、变更追踪、代码片段、跨分支 tags、索引锁 |

### LanceDB Schema

```
Table: code_chunks
├── id: String (PK)           # "{workspace}:{filepath}:{chunk_idx}"
├── source_id: String         # workspace identifier
├── filepath: String          # relative path
├── language: String          # "rust", "python", etc.
├── content: String           # chunk text (FTS indexed)
├── start_line: Int32
├── end_line: Int32
├── vector: FixedSizeList[Float32, 1536]  # embedding (optional)
└── updated_at: Int64
```

### SQLite Schema

```sql
-- 索引目录 (增量更新)
CREATE TABLE catalog (
    id INTEGER PRIMARY KEY,
    workspace TEXT NOT NULL,
    branch TEXT,
    filepath TEXT NOT NULL,
    content_hash TEXT NOT NULL,  -- SHA256 前 16 位
    mtime INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL,
    UNIQUE(workspace, branch, filepath)
);

-- 跨分支标签
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    content_hash TEXT NOT NULL,
    workspace TEXT NOT NULL,
    branch TEXT NOT NULL,
    UNIQUE(content_hash, workspace, branch)
);

-- 代码片段 (tree-sitter-tags 提取)
CREATE TABLE snippets (
    id INTEGER PRIMARY KEY,
    workspace TEXT NOT NULL,
    filepath TEXT NOT NULL,
    name TEXT NOT NULL,           -- 函数/类名
    syntax_type TEXT NOT NULL,    -- "function", "class", "method", "struct", "trait"
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    signature TEXT,               -- 函数签名
    docs TEXT,                    -- 文档注释
    content_hash TEXT NOT NULL,
    UNIQUE(workspace, filepath, name, start_line)
);

-- 索引锁 (多进程协调)
CREATE TABLE index_lock (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    holder_id TEXT NOT NULL,      -- 进程标识 (PID + timestamp)
    workspace TEXT NOT NULL,
    locked_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL   -- 超时自动释放
);
```

---

## 代码质量过滤

### 文件有效性检测

在索引前过滤低质量文件 (二进制、生成代码、日志等)：

```
源文件
    │
    ▼
┌──────────────────────────────────┐
│         CodeMetrics 检测          │
├──────────────────────────────────┤
│  • max_line_length ≤ 300        │  ← 过滤超长行 (压缩/混淆)
│  • avg_line_length ≤ 150        │  ← 过滤单行文件
│  • alphanum_fraction ≥ 0.25     │  ← 过滤二进制/非文本
│  • num_lines ≤ 100,000          │  ← 过滤超大文件
│  • number_fraction ≤ 0.50       │  ← 过滤纯数字 (日志/数据)
└──────────────────────────────────┘
    │
    ├─ 通过 → 继续索引
    │
    └─ 失败 → 跳过文件 (记录日志)
```

### Rust 实现

```rust
pub struct CodeMetrics {
    pub max_line_length: i32,
    pub avg_line_length: f32,
    pub alphanum_fraction: f32,
    pub num_lines: i32,
    pub number_fraction: f32,
}

pub fn is_valid_file(content: &str) -> bool {
    let metrics = compute_metrics(content);

    metrics.max_line_length <= 300
        && metrics.avg_line_length <= 150.0
        && metrics.alphanum_fraction >= 0.25
        && metrics.num_lines <= 100_000
        && metrics.number_fraction <= 0.50
}
```

---

## 分块架构 (text-splitter)

### 设计原则

使用 `text-splitter::CodeSplitter` 替代自定义 AST chunker：
- **内部使用 tree-sitter** 解析代码语法树
- **按语法层级分割**: function > statement > expression
- **自动回退**: 解析失败时回退到 TextSplitter

```
源代码文件
    │
    ├────────────────────────────────────────┐
    │                                        │
    ▼                                        ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│  text-splitter          │    │  tree-sitter-tags       │
│  ::CodeSplitter         │    │                         │
├─────────────────────────┤    ├─────────────────────────┤
│  用途: 代码分块          │    │  用途: 标签提取          │
│                         │    │                         │
│  • 内置 tree-sitter     │    │  • 函数名 + 签名         │
│  • 语法边界分割          │    │  • 类/结构体名           │
│  • Token 计数 (tiktoken)│    │  • 文档注释              │
│  • 自动回退 TextSplitter │    │  • 行号范围              │
└───────────┬─────────────┘    └───────────┬─────────────┘
            │                              │
            ▼                              ▼
     LanceDB chunks                 SQLite snippets
```

### 分块流程

```
源文件
    │
    ▼
┌──────────────────────────────────┐
│  1. 检测语言                      │
│     extension → language          │
└───────────────┬──────────────────┘
                │
                ▼
┌──────────────────────────────────┐
│  2. 创建 CodeSplitter            │
│     language → tree-sitter-lang   │
└───────────────┬──────────────────┘
                │
                ▼
┌──────────────────────────────────┐
│  3. 分块                         │
│     CodeSplitter::chunks()        │
│     • 按语法节点边界分割           │
│     • max_chunk_size: 512        │
│     • 保留完整函数/类             │
└───────────────┬──────────────────┘
                │
                ├─ 成功 → ChunkSpan[]
                │
                └─ 失败 → TextSplitter 回退
```

### 支持语言

| 语言 | tree-sitter crate | 分块支持 | 标签支持 |
|------|-------------------|----------|----------|
| Rust | tree-sitter-rust | ✅ | ✅ |
| Go | tree-sitter-go | ✅ | ✅ |
| Python | tree-sitter-python | ✅ | ✅ |
| Java | tree-sitter-java | ✅ | ✅ |
| TypeScript | tree-sitter-typescript | ✅ | 待添加 |
| 其他 | - | TextSplitter 回退 | - |

---

## 索引流程

```
                           ┌─────────────────┐
                           │ index_workspace │
                           └────────┬────────┘
                                    │
                    ┌───────────────▼───────────────┐
                    │  1. Acquire Lock              │
                    │     • 检查 index_lock 表       │
                    │     • 超时检测 (30s)          │
                    │     • 设置 holder_id          │
                    └───────────────┬───────────────┘
                                    │
                    ┌───────────────▼───────────────┐
                    │  2. Walk Directory            │
                    │     • 遍历文件树               │
                    │     • 应用 .gitignore         │
                    │     • 应用 .ignore            │
                    │     • CodeMetrics 过滤        │
                    └───────────────┬───────────────┘
                                    │
                    ┌───────────────▼───────────────┐
                    │  3. Compute Delta             │
                    │     • 计算 SHA256 content_hash│
                    │     • 比较 catalog 记录        │
                    │     • 分类变更操作             │
                    └───────────────┬───────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         │                          │                          │
         ▼                          ▼                          ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│ Compute         │      │ Delete          │      │ Tag Operations  │
│ • Read file     │      │ • Delete chunks │      │ • Add/remove    │
│ • CodeSplitter  │      │ • Delete snips  │      │   branch tags   │
│ • Extract tags  │      │ • Update catalog│      │ • Zero-cost     │
│ • Embed (opt)   │      │                 │      │   reuse         │
│ • Store chunks  │      │                 │      │                 │
└────────┬────────┘      └────────┬────────┘      └────────┬────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │  4. Commit                │
                    │     • LanceDB commit      │
                    │     • SQLite commit       │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │  5. Release Lock          │
                    │     • 清除 index_lock      │
                    └───────────────────────────┘
```

---

## 并发模型

### 多进程索引锁

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Process A  │     │  Process B  │     │  Process C  │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       ▼                   ▼                   ▼
  try_acquire()       try_acquire()       try_acquire()
       │                   │                   │
       ▼                   │                   │
  ✅ 获取锁                │                   │
  (写入 holder_id)        │                   │
       │                   ▼                   ▼
       │              ⏳ 等待 (30s)        ⏳ 等待 (30s)
       │                   │                   │
       │              检查 expires_at     检查 expires_at
       │                   │                   │
       ▼                   │                   │
  索引工作中...            │                   │
       │                   │                   │
       ▼                   │                   │
  release()               │                   │
       │                   ▼                   │
       │              ✅ 获取锁               │
       │                   │                   │
```

### 锁实现

```rust
pub struct IndexLockGuard {
    holder_id: String,      // "{pid}_{timestamp}"
    workspace: String,
    acquired_at: Instant,
    timeout: Duration,      // 30 seconds
}

impl IndexLockGuard {
    pub async fn try_acquire(
        db: &SqliteStore,
        workspace: &str,
        timeout: Duration,
    ) -> Result<Self, RetrievalErr> {
        let deadline = Instant::now() + timeout;

        loop {
            // 1. 检查现有锁是否过期
            if let Some(lock) = db.get_lock(workspace)? {
                if lock.expires_at < now() {
                    db.force_release(workspace)?;  // 清理过期锁
                } else if Instant::now() > deadline {
                    return Err(RetrievalErr::SqliteLockedTimeout {
                        path: db.path.clone(),
                        waited_ms: timeout.as_millis() as u64,
                    });
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }

            // 2. 尝试获取锁
            let holder_id = format!("{}_{}", std::process::id(), now());
            if db.try_set_lock(workspace, &holder_id, timeout)? {
                return Ok(Self { holder_id, workspace, ... });
            }
        }
    }
}

impl Drop for IndexLockGuard {
    fn drop(&mut self) {
        // 自动释放锁
        let _ = self.db.release_lock(&self.workspace, &self.holder_id);
    }
}
```

### 异步安全存储

```rust
// rusqlite::Connection 不是 Send + Sync
// 需要使用 tokio::task::spawn_blocking 或 Arc<Mutex<>>

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,  // 线程安全封装
}

impl SqliteStore {
    pub async fn query<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap();
            f(&guard)
        })
        .await?
    }
}
```

---

## 搜索流程

```
                           ┌─────────────────┐
                           │     search()    │
                           └────────┬────────┘
                                    │
                    ┌───────────────▼───────────────┐
                    │  1. Query Preprocessing       │
                    │     • 空格规范化              │
                    │     • 分词                   │
                    │     • 停用词移除              │
                    │     • 词干还原 (可选)         │
                    │     • [Feature] 中→英翻译    │
                    └───────────────┬───────────────┘
                                    │
         ┌──────────────────────────┴──────────────────────────┐
         │                          │                          │
         ▼                          ▼                          ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│ BM25 Search     │      │ Vector Search   │      │ Snippet Search  │
│ (LanceDB FTS)   │      │ [Feature]       │      │ (SQLite)        │
│                 │      │                 │      │                 │
│ • Tokenize      │      │ • Embed query   │      │ • Name match    │
│ • BM25 score    │      │ • Cosine sim    │      │ • Type filter   │
│ • Top-K         │      │ • Top-K         │      │ • Signature     │
└────────┬────────┘      └────────┬────────┘      └────────┬────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │  2. RRF Result Fusion     │
                    │     • Reciprocal Rank     │
                    │     • Weight by source    │
                    │     • Jaccard 相似度排序   │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │  3. Dedup & Rerank        │
                    │     • 重叠结果去重         │
                    │     • Snippet boost (1.5x)│
                    │     • Recency decay       │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │  4. Return Top-N          │
                    └───────────────────────────┘
```

### RRF 结果融合

Reciprocal Rank Fusion (RRF) 算法：

```rust
pub struct RrfScorer {
    k: f32,           // 常数，通常 60
    weights: Weights, // 各来源权重
}

impl RrfScorer {
    /// 计算 RRF 分数
    /// score = Σ w_i / (k + rank_i)
    pub fn fuse(&self, results: &[RankedResults]) -> Vec<FusedResult> {
        let mut scores: HashMap<String, f32> = HashMap::new();

        for (source, ranked) in results {
            let weight = self.weights.get(source);
            for (rank, result) in ranked.iter().enumerate() {
                let key = format!("{}:{}", result.filepath, result.start_line);
                *scores.entry(key).or_default() +=
                    weight / (self.k + rank as f32);
            }
        }

        // 排序并返回
        let mut fused: Vec<_> = scores.into_iter().collect();
        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        fused
    }
}
```

### 默认权重配置

```toml
[retrieval.search]
bm25_weight = 0.6      # BM25 搜索权重
vector_weight = 0.3    # 向量搜索权重
snippet_weight = 0.1   # Snippet 匹配权重
rrf_k = 60             # RRF 常数
```

---

## 查询预处理流水线

### 处理步骤

参考 Continue 的查询预处理 (BaseRetrievalPipeline.ts)：

```
原始查询: "用户 认证   逻辑  authenticate"
    │
    ▼
┌──────────────────────────────────────────────────────┐
│  Step 1: 空格规范化                                    │
│  "用户 认证   逻辑  authenticate"                      │
│       ↓                                              │
│  "用户 认证 逻辑 authenticate"                        │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 2: 分词 (Tokenize)                              │
│  "用户 认证 逻辑 authenticate"                        │
│       ↓                                              │
│  ["用户", "认证", "逻辑", "authenticate"]             │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 3: 停用词移除 (Stop Words)                       │
│  ["用户", "认证", "逻辑", "authenticate"]             │
│       ↓ (保留所有，无停用词)                           │
│  ["用户", "认证", "逻辑", "authenticate"]             │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 4: 词干还原 (Stemming) [可选]                    │
│  ["authenticate"]                                    │
│       ↓                                              │
│  ["authent"]                                         │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 5: 去重                                         │
│  移除重复 token                                       │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 6: [Feature:QueryRewrite] 中文翻译              │
│  检测中文 → LLM 翻译为英文                             │
│  "用户 认证 逻辑" → "user authentication logic"       │
└───────────────────────────┬──────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────┐
│  Step 7: N-gram 生成 [可选]                           │
│  ["user", "authentication", "logic"]                 │
│       ↓                                              │
│  ["user auth", "auth logic", ...]  (trigrams)        │
└──────────────────────────────────────────────────────┘
```

### 实现

```rust
pub struct QueryPreprocessor {
    stop_words: HashSet<String>,
    enable_stemming: bool,
    enable_ngrams: bool,
    ngram_size: i32,
}

#[derive(Debug)]
pub struct ProcessedQuery {
    pub original: String,
    pub tokens: Vec<String>,
    pub ngrams: Vec<String>,
}

impl QueryPreprocessor {
    pub fn process(&self, query: &str) -> ProcessedQuery {
        // 1. 规范化空格
        let normalized = normalize_whitespace(query);

        // 2. 分词
        let tokens = tokenize(&normalized);

        // 3. 移除停用词
        let filtered: Vec<_> = tokens
            .into_iter()
            .filter(|t| !self.stop_words.contains(t.to_lowercase().as_str()))
            .collect();

        // 4. 词干还原 (可选)
        let stemmed = if self.enable_stemming {
            stem_tokens(filtered)
        } else {
            filtered
        };

        // 5. 去重
        let unique: Vec<_> = stemmed.into_iter().collect::<HashSet<_>>().into_iter().collect();

        // 6. 生成 n-grams (可选)
        let ngrams = if self.enable_ngrams {
            generate_ngrams(&unique.join(" "), self.ngram_size)
        } else {
            Vec::new()
        };

        ProcessedQuery {
            original: query.to_string(),
            tokens: unique,
            ngrams,
        }
    }
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}\"'".contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
```

### 停用词列表

```rust
lazy_static! {
    static ref STOP_WORDS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // 英文停用词
        set.extend(["the", "a", "an", "is", "are", "was", "were", "be", "been",
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
                    "up", "down", "in", "out", "on", "off", "over", "under"]);
        // 中文停用词
        set.extend(["的", "了", "和", "是", "就", "都", "而", "及", "与", "着",
                    "或", "一个", "没有", "我们", "你们", "他们", "它们", "这个",
                    "那个", "这些", "那些", "什么", "怎么", "如何", "为什么"]);
        set
    };
}
```

### 搜索配置

```toml
[retrieval.search]
# 结果数量
n_final = 20              # 最终返回结果数
n_retrieve = 50           # 初始检索候选数

# BM25 参数
bm25_threshold = -2.5     # BM25 分数截断阈值 (负数，越小越严格)

# 重排序
rerank_threshold = 0.3    # 重排序阈值

# 预处理选项
enable_stemming = true    # 启用词干还原
enable_ngrams = false     # 启用 n-gram
ngram_size = 3            # n-gram 大小
```

---

## Jaccard 相似度排序

### 算法

用于结果排序，计算查询与结果内容的符号级相似度：

```rust
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
```

### 用途

1. **结果排序**: 与当前上下文相似度高的结果排前面
2. **去重辅助**: 相似度过高的结果可能重复
3. **相关性增强**: 结合 BM25/Vector 分数加权

---

## 重叠结果去重

### 问题

多个搜索源可能返回同一文件的重叠代码块：

```
BM25 返回:   file.rs:10-30  (函数定义)
Vector 返回: file.rs:15-35  (函数体部分)
Snippet 返回: file.rs:10-12 (函数签名)
```

### 解决方案

```rust
pub fn deduplicate_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut deduped: Vec<SearchResult> = Vec::new();

    for result in results {
        // 检查是否与已有结果重叠
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
            // 可选: 合并重叠结果，保留最高分
            merge_overlapping(&mut deduped, result);
        }
    }

    deduped
}

fn ranges_overlap(a: Range<i32>, b: Range<i32>) -> bool {
    a.start < b.end && b.start < a.end
}

fn merge_overlapping(results: &mut Vec<SearchResult>, new: SearchResult) {
    // 找到重叠的结果
    if let Some(existing) = results.iter_mut().find(|r| {
        r.chunk.filepath == new.chunk.filepath
            && ranges_overlap(
                r.chunk.start_line..r.chunk.end_line,
                new.chunk.start_line..new.chunk.end_line,
            )
    }) {
        // 扩展范围
        existing.chunk.start_line = existing.chunk.start_line.min(new.chunk.start_line);
        existing.chunk.end_line = existing.chunk.end_line.max(new.chunk.end_line);
        // 保留更高分数
        existing.score = existing.score.max(new.score);
    }
}
```

### 去重策略

| 策略 | 说明 | 适用场景 |
|------|------|----------|
| **丢弃** | 只保留第一个 | 简单快速 |
| **合并** | 扩展范围，保留高分 | 需要完整上下文 |
| **最佳** | 保留分数最高 | 精准匹配 |

---

## 增量更新

### 变更检测

```rust
/// 源文件唯一标识
pub struct SourceFileId {
    pub path: PathBuf,
    pub language: String,
    pub content_hash: String,  // SHA256(content)[..16]
}

impl SourceFileId {
    pub fn compute(path: &Path, content: &str) -> Self {
        let hash = sha2::Sha256::digest(content.as_bytes());
        Self {
            path: path.to_path_buf(),
            language: detect_language(path),
            content_hash: format!("{:x}", hash)[..16].to_string(),
        }
    }
}

/// 变更分类
pub enum ChangeType {
    Compute,    // 新文件或内容变化 → 重新索引
    Delete,     // 文件删除且无其他分支使用 → 删除索引
    AddTag,     // 内容已在其他分支索引 → 零成本复用
    RemoveTag,  // 从当前分支移除标签
}
```

### 跨分支复用

```
Branch A: file.rs (hash: abc123) → 索引
                                    ↓
Branch B: file.rs (hash: abc123) → AddTag (零成本)
                                    ↓
Branch A: 删除 file.rs            → RemoveTag
                                    ↓
Branch B: 仍在使用                 → 索引保留
                                    ↓
Branch B: 删除 file.rs            → Delete (真正删除)
```

---

## Feature 控制

### 定义位置

`core/src/features.rs`:

```rust
pub enum Feature {
    // ... existing ...

    /// 代码搜索工具
    CodeSearch,       // key: "code_search"

    /// 向量语义搜索
    VectorSearch,     // key: "vector_search"

    /// 查询改写/翻译
    QueryRewrite,     // key: "query_rewrite"
}

// FeatureSpec 定义
FeatureSpec {
    id: Feature::CodeSearch,
    key: "code_search",
    stage: Stage::Experimental,
    default_enabled: false,
},
```

### 运行时检查

```rust
if features.enabled(Feature::VectorSearch) {
    // 执行向量搜索
    let vector_results = vector_searcher.search(&query).await?;
    results.extend(vector_results);
}

if features.enabled(Feature::QueryRewrite) && contains_chinese(&query.text) {
    // 翻译查询
    query.text = translator.translate_to_english(&query.text).await?;
}
```

---

## 性能目标

### 索引性能

| 操作 | 目标 |
|------|------|
| 文件遍历 | ~10k files/s |
| 代码质量过滤 | < 1ms/file |
| CodeSplitter 分块 | ~50 MB/s |
| 标签提取 | ~10 MB/s |
| Embedding | ~10 req/s (API) |
| LanceDB 写入 | ~5k docs/s |
| **综合吞吐** | **≥ 350 chunks/sec** |

### 搜索性能

| 操作 | 目标延迟 |
|------|----------|
| BM25 搜索 | < 10ms |
| 向量搜索 | < 50ms |
| 混合搜索 | < 100ms |
| RRF 融合 | < 5ms |

### 优化策略

1. **批量处理**: 100 files/batch 减少 I/O
2. **并发索引**: buffer_unordered(4-8)
3. **代码质量过滤**: 跳过无效文件 (二进制、压缩、生成)
4. **增量更新**: content_hash 变更检测
5. **跨分支复用**: 相同内容零成本复用
6. **延迟 Embedding**: 仅在启用 VectorSearch 时计算
7. **索引锁超时**: 30s 自动释放防止死锁

---

## 健康检查与自修复

### 索引健康检查

定期验证索引完整性，检测损坏：

```
┌─────────────────────────────────────────────┐
│           Index Health Check                │
├─────────────────────────────────────────────┤
│                                             │
│  1. SQLite 完整性                           │
│     • PRAGMA integrity_check               │
│     • 表存在性验证                          │
│                                             │
│  2. LanceDB 一致性                          │
│     • 表可访问性                            │
│     • 向量维度一致                          │
│                                             │
│  3. Catalog 同步                            │
│     • 文件存在性验证 (抽样)                  │
│     • 孤儿记录检测                          │
│                                             │
│  4. 锁状态                                  │
│     • 过期锁清理                            │
│                                             │
└─────────────────────────────────────────────┘
```

### 实现

```rust
pub struct HealthChecker {
    sqlite: Arc<SqliteStore>,
    lancedb: Arc<LanceDbStore>,
}

pub struct HealthReport {
    pub status: HealthStatus,
    pub issues: Vec<HealthIssue>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

pub enum HealthStatus {
    Healthy,
    Degraded,  // 部分功能可用
    Corrupted, // 需要重建
}

pub enum HealthIssue {
    SqliteCorrupted { reason: String },
    LanceDbInaccessible { table: String },
    OrphanedRecords { count: i32 },
    DimensionMismatch { expected: i32, actual: i32 },
    StaleLock { workspace: String, age_secs: i64 },
}

impl HealthChecker {
    pub async fn check(&self) -> Result<HealthReport> {
        let mut issues = Vec::new();

        // 1. SQLite 完整性
        let integrity = self.sqlite.query(|conn| {
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        }).await?;

        if integrity != "ok" {
            issues.push(HealthIssue::SqliteCorrupted { reason: integrity });
        }

        // 2. LanceDB 表检查
        if !self.lancedb.table_exists("code_chunks").await? {
            issues.push(HealthIssue::LanceDbInaccessible {
                table: "code_chunks".to_string(),
            });
        }

        // 3. 孤儿记录检测 (抽样)
        let orphan_count = self.check_orphan_records().await?;
        if orphan_count > 0 {
            issues.push(HealthIssue::OrphanedRecords { count: orphan_count });
        }

        // 4. 过期锁检测
        let stale_locks = self.check_stale_locks().await?;
        issues.extend(stale_locks);

        let status = match issues.len() {
            0 => HealthStatus::Healthy,
            n if n <= 2 => HealthStatus::Degraded,
            _ => HealthStatus::Corrupted,
        };

        Ok(HealthReport { status, issues, checked_at: chrono::Utc::now() })
    }
}
```

### 自修复策略

```
问题                    │ 修复策略
───────────────────────┼─────────────────────────
孤儿 catalog 记录       │ 删除孤儿记录
过期索引锁              │ 强制释放锁
维度不匹配              │ 重建向量索引
SQLite 轻微损坏         │ VACUUM + 重建索引
SQLite 严重损坏         │ 删除并重新索引
LanceDB 不可访问        │ 重建 LanceDB 目录
```

---

## 指标收集

### 核心指标

```rust
pub struct RetrievalMetrics {
    // 索引指标
    pub index_files_total: Counter,
    pub index_chunks_total: Counter,
    pub index_bytes_total: Counter,
    pub index_duration_seconds: Histogram,
    pub index_errors_total: Counter,

    // 搜索指标
    pub search_requests_total: Counter,
    pub search_duration_seconds: Histogram,
    pub search_results_count: Histogram,

    // 嵌入指标
    pub embedding_requests_total: Counter,
    pub embedding_tokens_total: Counter,
    pub embedding_latency_seconds: Histogram,
    pub embedding_errors_total: Counter,

    // 存储指标
    pub storage_size_bytes: Gauge,
    pub cache_hit_ratio: Gauge,
}
```

### 指标记录点

```
┌──────────────────────────────────────────────────────────────┐
│                    Metrics Collection Points                  │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Indexing Pipeline                                           │
│  ├─ [START] index_workspace()                               │
│  │     └─ record: index_duration_seconds.start()            │
│  ├─ [EACH] process_file()                                   │
│  │     └─ record: index_files_total.inc()                   │
│  ├─ [EACH] store_chunk()                                    │
│  │     └─ record: index_chunks_total.inc()                  │
│  └─ [END] commit()                                          │
│        └─ record: index_duration_seconds.observe()          │
│                                                              │
│  Search Pipeline                                             │
│  ├─ [START] search()                                        │
│  │     └─ record: search_requests_total.inc()               │
│  ├─ [EACH] bm25/vector/snippet search                       │
│  └─ [END] return results                                    │
│        └─ record: search_duration_seconds.observe()         │
│                   search_results_count.observe()            │
│                                                              │
│  Embedding                                                   │
│  ├─ [EACH] embed_batch()                                    │
│  │     └─ record: embedding_requests_total.inc()            │
│  │                embedding_tokens_total.add(n)             │
│  └─ [ERROR] rate_limit/api_error                            │
│        └─ record: embedding_errors_total.inc()              │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 嵌入 API 速率限制

### 令牌桶算法

```rust
/// 嵌入 API 速率限制器
pub struct EmbeddingRateLimiter {
    /// 每秒请求数限制
    requests_per_second: f64,
    /// 每分钟 Token 数限制
    tokens_per_minute: i64,
    /// 当前令牌数
    tokens: AtomicI64,
    /// 上次刷新时间
    last_refill: AtomicInstant,
}

impl EmbeddingRateLimiter {
    pub fn new(config: &EmbeddingConfig) -> Self {
        Self {
            requests_per_second: config.rate_limit_rps.unwrap_or(10.0),
            tokens_per_minute: config.rate_limit_tpm.unwrap_or(150_000),
            tokens: AtomicI64::new(config.rate_limit_tpm.unwrap_or(150_000)),
            last_refill: AtomicInstant::now(),
        }
    }

    /// 等待并获取许可
    pub async fn acquire(&self, estimated_tokens: i64) -> Result<()> {
        loop {
            // 刷新令牌
            self.maybe_refill();

            // 检查是否有足够令牌
            let current = self.tokens.load(Ordering::Relaxed);
            if current >= estimated_tokens {
                // 原子扣减
                if self.tokens.compare_exchange(
                    current,
                    current - estimated_tokens,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_ok() {
                    return Ok(());
                }
            }

            // 等待后重试 (指数退避)
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

### 退避策略

```
请求失败 (429 Too Many Requests)
    │
    ▼
┌───────────────────────────────┐
│  指数退避 + 抖动               │
│                               │
│  wait = min(base * 2^n, max)  │
│       + random(0, jitter)     │
│                               │
│  base = 1s                    │
│  max  = 60s                   │
│  jitter = 0.5s                │
│  n = retry_count              │
└───────────────────────────────┘
    │
    ▼
重试请求 (最多 5 次)
```

---

## 优雅降级

### 降级策略

当向量搜索不可用时，回退到 BM25 搜索：

```
┌─────────────────────────────────────────────────────────────┐
│                    Graceful Degradation                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  正常模式 (VectorSearch 启用)                                │
│  ├─ BM25 搜索 (LanceDB FTS)                                 │
│  ├─ Vector 搜索 (LanceDB)                                   │
│  ├─ Snippet 搜索 (SQLite)                                   │
│  └─ RRF 融合 → 混合结果                                     │
│                                                             │
│  降级模式 (Embedding 失败)                                   │
│  ├─ BM25 搜索 (LanceDB FTS)                                 │
│  ├─ ❌ Vector 搜索 (跳过)                                    │
│  ├─ Snippet 搜索 (SQLite)                                   │
│  └─ RRF 融合 (仅 BM25 + Snippet)                            │
│                                                             │
│  最小模式 (LanceDB 不可用)                                   │
│  ├─ ❌ BM25 搜索 (跳过)                                      │
│  ├─ ❌ Vector 搜索 (跳过)                                    │
│  └─ Snippet 搜索 (SQLite) → 仅符号匹配                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 实现

```rust
pub struct SearchContext {
    pub capabilities: SearchCapabilities,
    pub degradation_reason: Option<DegradationReason>,
}

pub struct SearchCapabilities {
    pub bm25_available: bool,
    pub vector_available: bool,
    pub snippet_available: bool,
}

pub enum DegradationReason {
    EmbeddingApiUnavailable,
    EmbeddingRateLimited,
    LanceDbCorrupted,
    VectorIndexMissing,
}

impl RetrievalService {
    pub async fn search_with_fallback(&self, query: SearchQuery) -> Result<SearchResponse> {
        let ctx = self.check_capabilities().await;

        let (results, degraded) = match ctx.capabilities {
            // 完整模式
            SearchCapabilities { bm25_available: true, vector_available: true, snippet_available: true } => {
                (self.hybrid_search(&query).await?, false)
            }

            // 无向量模式
            SearchCapabilities { bm25_available: true, vector_available: false, snippet_available: true } => {
                tracing::warn!("Vector search unavailable, falling back to BM25 + Snippet");
                (self.bm25_snippet_search(&query).await?, true)
            }

            // 仅 Snippet 模式
            SearchCapabilities { snippet_available: true, .. } => {
                tracing::warn!("Only snippet search available");
                (self.snippet_only_search(&query).await?, true)
            }

            // 完全不可用
            _ => {
                return Err(RetrievalErr::SearchFailed {
                    query: query.text,
                    cause: "All search backends unavailable".to_string(),
                });
            }
        };

        Ok(SearchResponse {
            results,
            degraded,
            degradation_reason: ctx.degradation_reason,
        })
    }
}
```

---

## 检查点恢复

### 索引检查点

支持大型仓库索引中断后恢复：

```sql
-- 添加 checkpoint 表
CREATE TABLE checkpoint (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    workspace TEXT NOT NULL,
    phase TEXT NOT NULL,           -- 'walk', 'chunk', 'embed', 'commit'
    total_files INTEGER NOT NULL,
    processed_files INTEGER NOT NULL,
    last_file TEXT,
    started_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 恢复流程

```
启动索引
    │
    ▼
┌───────────────────────────────┐
│  检查 checkpoint 表            │
└───────────────┬───────────────┘
                │
        ┌───────┴───────┐
        │               │
        ▼               ▼
   无检查点          有检查点
        │               │
        ▼               ▼
   从头开始         恢复提示
        │               │
        │         "发现未完成索引:
        │          workspace: /path
        │          进度: 1234/5678 文件
        │          是否继续? [Y/n]"
        │               │
        │         ┌─────┴─────┐
        │         │           │
        │         ▼           ▼
        │       继续         重新开始
        │         │           │
        │         ▼           │
        │    跳过已处理文件     │
        │         │           │
        └─────────┴───────────┘
                  │
                  ▼
             继续索引
```

### 实现

```rust
pub struct IndexCheckpoint {
    pub workspace: String,
    pub phase: IndexPhase,
    pub total_files: i32,
    pub processed_files: i32,
    pub last_file: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

pub enum IndexPhase {
    Walk,    // 文件遍历中
    Chunk,   // 分块处理中
    Embed,   // 嵌入计算中
    Commit,  // 提交中
}

impl IndexingManager {
    /// 保存检查点 (每处理 100 个文件)
    async fn save_checkpoint(&self, phase: IndexPhase, last_file: &str) -> Result<()> {
        self.sqlite.query(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO checkpoint
                 (id, workspace, phase, total_files, processed_files, last_file, started_at, updated_at)
                 VALUES (1, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &self.workspace,
                    phase.as_str(),
                    self.total_files,
                    self.processed_files,
                    last_file,
                    self.started_at.timestamp(),
                    chrono::Utc::now().timestamp(),
                ],
            )
        }).await
    }

    /// 尝试恢复检查点
    pub async fn try_resume(&self) -> Result<Option<IndexCheckpoint>> {
        self.sqlite.query(|conn| {
            conn.query_row(
                "SELECT workspace, phase, total_files, processed_files, last_file, started_at
                 FROM checkpoint WHERE workspace = ?",
                [&self.workspace],
                |row| Ok(IndexCheckpoint {
                    workspace: row.get(0)?,
                    phase: IndexPhase::from_str(row.get::<_, String>(1)?.as_str()),
                    total_files: row.get(2)?,
                    processed_files: row.get(3)?,
                    last_file: row.get(4)?,
                    started_at: chrono::DateTime::from_timestamp(row.get(5)?, 0).unwrap(),
                }),
            ).optional()
        }).await
    }

    /// 清除检查点 (索引成功完成后)
    async fn clear_checkpoint(&self) -> Result<()> {
        self.sqlite.query(|conn| {
            conn.execute("DELETE FROM checkpoint WHERE workspace = ?", [&self.workspace])
        }).await
    }
}
```

---

## 配置交叉验证

### 验证规则

```rust
impl RetrievalConfig {
    /// 验证配置一致性
    pub fn validate(&self, features: &Features) -> Result<(), Vec<ConfigWarning>> {
        let mut warnings = Vec::new();

        // 1. VectorSearch 需要 embedding 配置
        if features.enabled(Feature::VectorSearch) {
            if self.embedding.provider.is_none() {
                warnings.push(ConfigWarning::MissingDependency {
                    feature: "vector_search",
                    required: "retrieval.embedding.provider",
                });
            }
            if self.embedding.dimension.is_none() {
                warnings.push(ConfigWarning::MissingDependency {
                    feature: "vector_search",
                    required: "retrieval.embedding.dimension",
                });
            }
        }

        // 2. QueryRewrite 需要 LLM 配置 (或使用默认 provider)
        if features.enabled(Feature::QueryRewrite) {
            // 可使用默认 model_provider，但需要 API key
        }

        // 3. data_dir 存在性
        if !self.data_dir.exists() {
            warnings.push(ConfigWarning::PathNotExists {
                field: "retrieval.data_dir",
                path: self.data_dir.clone(),
            });
        }

        // 4. 维度一致性 (如果已有索引)
        // 在运行时检查

        if warnings.is_empty() {
            Ok(())
        } else {
            Err(warnings)
        }
    }
}

pub enum ConfigWarning {
    MissingDependency { feature: &'static str, required: &'static str },
    PathNotExists { field: &'static str, path: PathBuf },
    DimensionMismatch { configured: i32, indexed: i32 },
}
```

### 启动时验证

```
RetrievalService::new()
    │
    ▼
┌───────────────────────────────┐
│  1. 加载配置                   │
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│  2. 配置交叉验证               │
│     • Feature 依赖检查         │
│     • 路径存在性检查           │
└───────────────┬───────────────┘
                │
        ┌───────┴───────┐
        │               │
        ▼               ▼
     验证通过         验证失败
        │               │
        ▼               ▼
   继续初始化      返回错误 + 修复建议
```

---

## SQLite Schema 迁移

### 版本管理

```sql
-- 添加 schema_version 表
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

-- 初始版本
INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (1, strftime('%s', 'now'));
```

### 迁移实现

```rust
const CURRENT_SCHEMA_VERSION: i32 = 3;

pub struct SchemaMigrator {
    conn: Arc<Mutex<Connection>>,
}

impl SchemaMigrator {
    pub fn migrate(&self) -> Result<()> {
        let current_version = self.get_current_version()?;

        for version in (current_version + 1)..=CURRENT_SCHEMA_VERSION {
            self.apply_migration(version)?;
            self.set_version(version)?;
            tracing::info!("Applied schema migration v{}", version);
        }

        Ok(())
    }

    fn apply_migration(&self, version: i32) -> Result<()> {
        match version {
            2 => {
                // v2: 添加 checkpoint 表
                self.conn.lock().unwrap().execute_batch(include_str!("migrations/v2_checkpoint.sql"))?;
            }
            3 => {
                // v3: 添加 metrics 表
                self.conn.lock().unwrap().execute_batch(include_str!("migrations/v3_metrics.sql"))?;
            }
            _ => {
                return Err(RetrievalErr::ConfigError {
                    field: "schema_version",
                    cause: format!("Unknown migration version: {}", version),
                });
            }
        }
        Ok(())
    }
}
```

### 迁移文件

```
retrieval/src/storage/migrations/
├── v1_initial.sql      # 初始 schema
├── v2_checkpoint.sql   # 添加 checkpoint 表
└── v3_metrics.sql      # 添加 metrics 表
```
