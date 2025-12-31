# 索引构建流程

## 代码索引完整流程

### 高级流程

```
CodeIndexer::refresh(git_url, commit_sha)
    ↓
[1] Git 仓库同步
    ├─ repository::sync_repository()
    ├─ Git clone 或 pull
    └─ 获取 HEAD 提交 SHA
    ↓
[2] 文件索引处理
    ├─ index::index_repository()
    ├─ Walk file tree
    ├─ Per-file processing (parallel)
    └─ Update Tantivy index
    ↓
[3] 垃圾回收
    ├─ index::garbage_collection()
    ├─ 清理已删除文件的索引
    └─ Commit transaction
    ↓
返回 RefreshStats
```

### 详细步骤

#### 步骤 1: Git 仓库同步 (code/repository.rs)

```rust
pub async fn sync_repository(git_url: &str) -> Result<PathBuf> {
    // 1. 获取本地缓存路径
    let repo_path = cache::repo_path(git_url)?;  // ~/.tabby/repos/{hash}/

    if repo_path.exists() {
        // 2. 拉取最新代码
        let repo = Repository::open(&repo_path)?;
        let mut remote = repo.find_remote("origin")?;

        // 获取所有分支
        remote.fetch(&["*"], None, None)?;

        // 快进合并 main/master
        let branch = repo.find_branch("main", BranchType::Remote)
            .or_else(|_| repo.find_branch("master", BranchType::Remote))?;

        repo.merge(branch.get(), None)?;
    } else {
        // 3. 首次克隆
        Repository::clone(git_url, &repo_path)?;
    }

    // 4. 返回本地路径
    Ok(repo_path)
}

pub fn get_head_commit(repo_path: &Path) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let head = repo.head()?;
    Ok(head.target()?.to_string())
}
```

**关键点**：
- 使用本地缓存避免重复克隆
- 支持增量拉取 (fetch + merge)
- 自动选择 main/master 分支

#### 步骤 2: 文件索引处理 (code/index.rs:85-153)

```rust
pub async fn index_repository(
    repo_path: &Path,
    git_url: &str,
    commit_sha: &str,
    indexer: &Indexer,
    builder: &CodeBuilder,
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();

    // 1. 文件树遍历 (顺序)
    let walker = Walk::new(repo_path)?
        .ignore_rules(vec![".gitignore", ".git", ...]);

    // 2. 批处理 (chunks of 100)
    for file_batch in walker.chunks(100) {
        // 3. 并发处理每个文件
        let futures = file_batch.into_iter().map(|file_path| {
            let git_url = git_url.to_string();
            let commit_sha = commit_sha.to_string();
            let builder = builder.clone();

            tokio::spawn(async move {
                // 4. 读取文件
                let content = tokio::fs::read_to_string(&file_path).await?;

                // 5. 检测语言
                let language = detect_language(&file_path)?;

                // 6. 计算代码指标
                let metrics = compute_metrics(&content)?;

                // 7. 有效性检查
                if !is_valid_file(&metrics) {
                    stats.skipped_files += 1;
                    return Ok(());  // 跳过此文件
                }

                // 8. 提取标签
                let tags = CodeIntelligence::find_tags(&language, &content)?;

                // 9. 构建 SourceCode 文档
                let mut source_code = SourceCode {
                    filepath: file_path.to_string(),
                    language: language.clone(),
                    commit: commit_sha.clone(),
                    git_url: git_url.clone(),
                    body: content.clone(),
                    tags: tags.clone(),
                    max_line_length: metrics.max_line_length,
                    avg_line_length: metrics.avg_line_length,
                    alphanum_fraction: metrics.alphanum_fraction,
                    num_lines: metrics.num_lines,
                    start_line: None,
                    source_file_id: SourceFileId::compute(&file_path, &language, &content)?,
                };

                // 10. 增量更新检查
                if !require_updates(indexer, &source_code).await? {
                    stats.skipped_files += 1;
                    return Ok(());  // 文件未改变，跳过
                }

                // 11. 删除旧索引
                indexer.delete_doc(&source_code.to_index_id()).await?;

                // 12. 添加新索引
                indexer.add(
                    git_url,
                    "code",  // corpus
                    source_code,
                    builder,
                ).await?;

                stats.indexed_files += 1;
                Ok::<_, Error>(())
            })
        });

        // 4. 等待批次完成
        futures::future::join_all(futures).await;
    }

    // 5. 提交所有更改
    indexer.commit().await?;

    Ok(stats)
}

/// 检查文件是否需要重新索引
async fn require_updates(
    indexer: &Indexer,
    source_code: &SourceCode,
) -> Result<bool> {
    // 1. 检查是否已索引
    if !indexer.is_indexed(&source_code.to_index_id()).await? {
        return Ok(true);  // 新文件，需要索引
    }

    // 2. 读取旧的 SourceFileId
    let old_doc = indexer.get_doc(&source_code.to_index_id()).await?;
    let old_source_file_id = parse_source_file_id(old_doc)?;

    // 3. 对比 SourceFileId
    Ok(!old_source_file_id.matches(&source_code.source_file_id))
}

pub struct IndexStats {
    pub indexed_files: usize,
    pub updated_files: usize,
    pub skipped_files: usize,
    pub total_chunks: usize,
    pub failed_chunks: usize,
}
```

**关键特性**：

| 特性 | 实现 | 效果 |
|------|------|------|
| **增量更新** | SourceFileId 变更检测 | 仅更新变更文件 |
| **并发处理** | tokio spawn + join_all | 高吞吐量 |
| **批处理** | chunks(100) | 内存和连接管理 |
| **容错处理** | 失败文件计数 | 不中断整体流程 |
| **有效性检查** | is_valid_file() | 过滤垃圾文件 |

#### 步骤 3: 垃圾回收 (code/index.rs:155-180)

```rust
pub async fn garbage_collection(
    indexer: &Indexer,
    active_git_urls: &[&str],
) -> Result<GcStats> {
    // 1. 获取索引中所有的 source_id
    let indexed_sources = indexer.iter_all_source_ids().await?;

    // 2. 构建活跃源集合
    let active_set: HashSet<&str> = active_git_urls.iter().copied().collect();

    // 3. 找出不活跃的源
    let inactive_sources: Vec<String> = indexed_sources
        .iter()
        .filter(|id| !active_set.contains(id.as_str()))
        .cloned()
        .collect();

    // 4. 删除不活跃源的所有文档
    let mut stats = GcStats::default();
    for source_id in inactive_sources {
        let deleted = indexer.delete(&source_id).await?;
        stats.deleted_docs += deleted;
    }

    // 5. 提交
    indexer.commit().await?;

    Ok(stats)
}

pub struct GcStats {
    pub deleted_docs: usize,
    pub deleted_chunks: usize,
    pub reclaimed_bytes: u64,
}
```

**目的**：
- 清理已删除的仓库索引
- 释放磁盘空间
- 保持索引健康

---

## 文档索引流程

### 高级流程

```
StructuredDocIndexer::sync(doc)
    ↓
[1] 预检查
    ├─ presync(): 检查是否需要更新
    └─ 判断依据: 时间戳、是否已索引
    ↓
[2] 构建属性
    ├─ builder.build(doc)
    ├─ 分块
    └─ 生成 embedding
    ↓
[3] 索引操作
    ├─ delete_doc(): 删除旧版本
    ├─ add(): 添加新版本
    └─ commit(): 提交事务
    ↓
返回 bool (是否已更新)
```

### 详细步骤

#### 步骤 1: 预检查 (structured_doc/public.rs:70-73)

```rust
pub async fn presync(&self, docs: &[StructuredDocState]) -> Result<Vec<bool>> {
    // 返回 bool 向量，表示每个文档是否需要更新

    let mut need_update = vec![];

    for doc in docs {
        // 1. 检查是否已索引
        let is_indexed = self.indexer
            .is_indexed(&doc.to_index_id())
            .await
            .unwrap_or(false);

        // 2. 如果未索引，或时间戳更新，则需要同步
        let needs = !is_indexed
            || doc.updated_at > self.get_last_sync_time(&doc.id).await?;

        need_update.push(needs);
    }

    Ok(need_update)
}

pub struct StructuredDocState {
    pub id: String,
    pub updated_at: i64,        // Unix timestamp
    pub deleted: bool,
}
```

**判断条件**：
- ✓ 文档未在索引中
- ✓ 文档 updated_at > 上次同步时间

#### 步骤 2: 构建和同步 (structured_doc/public.rs:75-120)

```rust
pub async fn sync(&self, doc: StructuredDoc) -> Result<bool> {
    // 1. 预检查
    let need_update = self.presync(&[doc.to_state()]).await?[0];

    if !need_update {
        return Ok(false);  // 已是最新，无需更新
    }

    // 2. 构建文档属性和分块
    //    (由 StructuredDocBuilder 实现，类似 CodeBuilder)
    let chunks = self.builder.build(doc.clone()).await?;
    //   返回: Vec<(chunk_id, chunk_tokens, chunk_attrs)>

    // 3. 删除旧版本
    self.indexer.delete_doc(&doc.to_index_id()).await?;

    // 4. 添加新版本 (并发)
    let futures = chunks.into_iter().map(|chunk| {
        let indexer = self.indexer.clone();
        let builder = self.builder.clone();
        let doc = doc.clone();

        tokio::spawn(async move {
            // 构建 Tantivy 文档
            let tan_doc = build_tantivy_doc(&doc, &chunk)?;

            // 写入索引
            indexer.add(
                doc.source_id(),
                "structured_doc",  // corpus
                doc.clone(),
                builder,
            ).await?;

            Ok::<_, Error>(())
        })
    });

    // 5. 等待所有 chunk 完成
    futures::future::buffer_unordered(12)  // 12 并发任务
        .collect::<Vec<_>>()
        .await;

    // 6. 提交事务
    self.indexer.commit().await?;

    Ok(true)  // 已更新
}
```

#### 步骤 3: 垃圾回收 (structured_doc/public.rs:180-205)

```rust
pub struct StructuredDocGarbageCollector {
    indexer: Arc<Indexer>,
}

impl StructuredDocGarbageCollector {
    pub async fn run<F>(&self, should_keep: F) -> Result<()>
    where
        F: Fn(&str) -> bool,  // 回调: 是否应该保留该文档
    {
        // 1. 遍历索引中的所有文档 ID
        let mut stream = self.indexer.iter_ids("structured_doc").await?;

        while let Some(index_id) = stream.next().await {
            // 2. 解析文档 ID
            let doc_id = parse_doc_id(&index_id)?;

            // 3. 询问是否应该保留
            if !should_keep(&doc_id) {
                // 4. 删除不需要的文档
                self.indexer.delete_doc(&index_id).await?;
            }
        }

        // 5. 提交
        self.indexer.commit().await?;

        Ok(())
    }
}

// 使用示例:
let gc = StructuredDocGarbageCollector::new(indexer);
gc.run(|doc_id| {
    // 保留最近 7 天的 ingested 文档
    if doc_id.starts_with("ingested:") {
        let age_days = get_doc_age_days(doc_id)?;
        age_days <= 7
    } else {
        // 保留其他所有类型
        true
    }
}).await?;
```

**灵活性**：
- 由调用方定义保留策略 (通过回调)
- 支持不同类型的过期政策
- 如 ingested 文档可自动清理

---

## 并发和性能特性

### 并发架构

```
代码索引并发模型:
  ├─ File Walk: Sequential (ignore rules)
  ├─ File Batch: Sequential (100 files/batch)
  │   └─ Per-File: Parallel (tokio spawn)
  │       ├─ AST Parsing: Sequential (tree-sitter not thread-safe)
  │       ├─ Embedding: Parallel (API calls)
  │       ├─ Indexing: Parallel (Tantivy thread-safe)
  │       └─ Metrics: Parallel (independent)
  └─ Commit: Atomic (single thread)

文档索引并发模型:
  ├─ Document Iteration: Sequential
  ├─ Presync: Sequential (metadata checks)
  │   └─ Build & Chunk: Sequential per document
  └─ Per-Chunk: Parallel (buffer_unordered(12))
      ├─ Embedding: Parallel
      ├─ Indexing: Parallel
      └─ Commit: Atomic (single thread)
```

### 吞吐量优化

| 优化技术 | 使用场景 | 效果 |
|--------|--------|------|
| **Batch Processing** | 文件索引 | 100 files/batch |
| **buffer_unordered** | Chunk 处理 | 12 并发任务 |
| **tokio spawn** | 并行计算 | CPU 利用率 |
| **Lazy Static** | 语言配置 | 一次初始化 |
| **Tantivy Buffering** | 索引写入 | 512MB in-mem buffer |

### 内存使用

```
内存配置:
  ├─ Tantivy 缓冲: 512 MB (可配置)
  ├─ 并发任务堆栈: 12 × ~1 MB = 12 MB
  ├─ 单个文件内容: 最多 ~5 MB (大文件拆分)
  └─ Embedding 缓存: ~100 MB (可配置)

总体: ~700 MB-1 GB (可配置)
```

---

## 错误处理和恢复

### 失败场景

```
文件索引失败:
  ├─ 编码错误 (non-UTF8 文件)
  ├─ AST 解析失败 (语法错误)
  ├─ Embedding API 超时
  └─ Disk I/O 错误

处理策略:
  ├─ 单个文件失败: 记录失败数，继续处理下个文件
  ├─ 批次失败: 回退到单个处理
  ├─ 索引写入失败: 重试或抛出错误
  └─ 垃圾回收失败: 记录警告，下次重试
```

### 重试机制

```rust
// 简化示例
async fn index_with_retry(
    file_path: &Path,
    max_retries: usize,
) -> Result<()> {
    let mut last_error = None;

    for attempt in 0..max_retries {
        match index_file(file_path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries - 1 {
                    // 指数退避
                    tokio::time::sleep(
                        Duration::from_millis(100 * 2_u64.pow(attempt as u32))
                    ).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}
```

### 故障恢复

```
索引崩溃恢复:
  1. Tantivy 自动恢复日志
     ├─ 写入日志 (WAL)
     ├─ 检查点 (checkpoints)
     └─ 重放 (replay)

  2. 验证一致性
     ├─ 检查索引完整性
     ├─ 验证段 (segments)
     └─ 重建必要数据

  3. 继续处理
     ├─ 从上次提交点继续
     └─ 跳过已处理文件
```

---

## 性能监控

### 指标收集

```rust
pub struct IndexStats {
    pub indexed_files: usize,        // 新索引的文件
    pub updated_files: usize,        // 更新的文件
    pub skipped_files: usize,        // 跳过的文件 (未改变)
    pub deleted_files: usize,        // 删除的文件
    pub total_chunks: usize,         // 总 chunk 数
    pub failed_chunks: usize,        // 失败的 chunk
    pub elapsed: Duration,           // 总耗时
    pub throughput: f64,             // chunks/sec
}

// 计算示例:
let elapsed = start.elapsed();
let throughput = (stats.total_chunks as f64) / elapsed.as_secs_f64();
```

### 日志示例

```
[INFO] Starting index refresh for https://github.com/user/repo
[INFO] Synced repository to /home/tabby/.tabby/repos/{hash}
[INFO] Processing 1,234 files...
[INFO] Indexed 1,100 files (100 skipped, 34 updated)
[INFO] Generated 15,000 chunks (12 failed)
[INFO] Index refresh completed in 42.5s (353 chunks/sec)
[INFO] Running garbage collection...
[INFO] Removed 50 orphaned documents
[INFO] Total index size: 2.3 GB
```

---

**相关文档**：
- [系统架构](./architecture.md) - 高级设计
- [核心模块详解](./modules.md) - 实现细节
- [Tantivy 搜索引擎](./tantivy.md) - 存储机制
