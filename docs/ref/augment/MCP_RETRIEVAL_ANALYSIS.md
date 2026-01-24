# Augment MCP Context Services 与 Codebase Retrieval 深度分析

## 文档信息
- **分析时间**: 2025-12-04
- **分析范围**: MCP Server 模式、codebase-retrieval 工具、索引机制
- **关键发现**: **Codebase-retrieval 是服务端实现，非本地索引**

---

## 执行摘要

Augment 的 Context Engine 通过 MCP (Model Context Protocol) 对外提供代码检索能力。核心机制如下：

1. **本地客户端**: 扫描工作区文件，计算哈希，**上传文件内容到 Augment 服务器**
2. **服务端**: 使用专有的 embedding 模型建立索引，执行语义检索
3. **MCP 接口**: 暴露 `codebase-retrieval` 工具，接受自然语言查询

**关键结论**:
- **索引存储**: 服务端（Augment Cloud）
- **检索计算**: 服务端（专有模型）
- **本地角色**: 文件上传 + 查询中继

---

## 1. MCP Server 模式配置

### 1.1 启动命令

```bash
# 基本模式
auggie --mcp

# 指定工作区
auggie -w /path/to/project --mcp

# 非交互式（CI/CD）
auggie --mcp
```

**环境变量配置**:
```json
{
  "command": "auggie",
  "args": ["-w", "/path/to/project", "--mcp"],
  "env": {
    "AUGMENT_API_TOKEN": "your-access-token",
    "AUGMENT_API_URL": "your-tenant-url"
  }
}
```

### 1.2 CLI 模式解析

**文件**: `chunks.58.mjs:36-103`

```javascript
// 模式判断逻辑
buildConfig(t) {
    let r = t.opts(), n;
    r.acp ? n = "acp" :
    r.mcp ? n = "mcp" :    // ← MCP 模式
    n = r.print || r.quiet ? "text" : "tui";

    return {
        mode: n,  // "mcp" | "acp" | "text" | "tui"
        // ...
    };
}
```

**支持的模式**:
| 模式 | 说明 | 用途 |
|------|------|------|
| `tui` | 终端 UI | 交互式开发 |
| `text` | 纯文本 | CI/CD、脚本 |
| `acp` | Agent Context Protocol | Augment 协议 |
| `mcp` | Model Context Protocol | 第三方 Agent 集成 |

---

## 2. Codebase-Retrieval 工具实现

### 2.1 工具定义

**文件**: `chunks.76.mjs:1092-1126`

```javascript
VF = class extends qo {
    constructor() {
        super("codebase-retrieval", 1)  // 工具名称, 版本
    }

    description = `This tool is Augment's context engine, the world's best
    codebase context engine. It:
    1. Takes in a natural language description of the code you are looking for;
    2. Uses a proprietary retrieval/embedding model suite that produces the
       highest-quality recall of relevant code snippets from across the codebase;
    3. Maintains a real-time index of the codebase, so the results are always
       up-to-date and reflects the current state of the codebase;
    4. Can retrieve across different programming languages;
    5. Only reflects the current state of the codebase on the disk, and has no
       information on version control or code history.`;

    inputSchemaJson = JSON.stringify({
        type: "object",
        properties: {
            information_request: {
                type: "string",
                description: "A description of the information you need."
            }
        },
        required: ["information_request"]
    });

    async call(t, r, n, a, o) {
        let i = Gp();  // 生成请求 ID
        try {
            let c = t.information_request;
            // 调用服务端 API
            let s = await bS().agentCodebaseRetrieval(i, c, r, 0, void 0, n);
            return Ro(s.formattedRetrieval, i);
        } catch (c) {
            return Xr(`Failed to retrieve codebase information: ${c.message}`, i);
        }
    }
}
```

### 2.2 API 调用

**文件**: `chunks.72.mjs:1178-1188`

```javascript
async agentCodebaseRetrieval(t, r, n, a, o, i, c) {
    let s = this._configListener.config;
    return await this.callApi(t, s, "agents/codebase-retrieval", {
        information_request: r,      // 自然语言查询
        blobs: gS(n),                // 上下文 blobs
        dialog: a,                   // 对话历史
        max_output_length: o,        // 最大输出长度
        disable_codebase_retrieval: i?.disableCodebaseRetrieval ?? false,
        enable_commit_retrieval: i?.enableCommitRetrieval ?? false
    }, l => this.convertToAgentCodebaseRetrievalResult(l),
    s.chat.url,  // API URL
    120000,      // 超时 120 秒
    void 0, c);
}

convertToAgentCodebaseRetrievalResult(t) {
    return {
        formattedRetrieval: t.formatted_retrieval  // 服务端返回格式化结果
    }
}
```

**API 请求结构**:
```typescript
// 请求
POST /agents/codebase-retrieval
{
    information_request: string,        // 查询描述
    blobs: BlobReference[],             // 当前上下文的 blob 引用
    dialog: number,                     // 对话历史长度
    max_output_length?: number,         // 输出限制
    disable_codebase_retrieval: boolean,
    enable_commit_retrieval: boolean
}

// 响应
{
    formatted_retrieval: string  // 格式化的检索结果（代码片段 + 文件路径）
}
```

---

## 3. 文件上传与索引机制

### 3.1 工作流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                        本地客户端 (auggie)                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │ PathFilter   │ -> │ DiskFileMan  │ -> │ Blob Name Calculator │  │
│  │ (.gitignore) │    │ (read files) │    │ (content hash)       │  │
│  └──────────────┘    └──────────────┘    └──────────────────────┘  │
│                                                  │                   │
│                                                  ▼                   │
│                           ┌──────────────────────────────────────┐  │
│                           │ find-missing API                      │  │
│                           │ "Which blobs do you not have?"        │  │
│                           └──────────────────────────────────────┘  │
│                                                  │                   │
│                                                  ▼                   │
│                           ┌──────────────────────────────────────┐  │
│                           │ batch-upload API                      │  │
│                           │ Upload: blob_name + path + content    │  │
│                           └──────────────────────────────────────┘  │
│                                                                      │
└────────────────────────────────────┬────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Augment 服务端                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐    ┌──────────────────┐    ┌───────────────┐ │
│  │ Blob Storage     │ -> │ Embedding Model  │ -> │ Vector Index  │ │
│  │ (content hash)   │    │ (proprietary)    │    │ (similarity)  │ │
│  └──────────────────┘    └──────────────────┘    └───────────────┘ │
│                                                          │          │
│                                                          ▼          │
│                           ┌──────────────────────────────────────┐ │
│                           │ Retrieval Engine                      │ │
│                           │ Query → Embedding → Top-K → Format   │ │
│                           └──────────────────────────────────────┘ │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 DiskFileManager 实现

**文件**: `chunks.83.mjs:800-1100`

```javascript
// 文件摄入
ingestPath(r, n) {
    this._ingestPathMs.start();
    if (!this._stopping) {
        n = Irt(n);  // 规范化路径
        this._enqueueForCalculate(r, n);  // 加入计算队列
    }
    this._ingestPathMs.stop();
}

// Blob 名称计算 (内容哈希)
_calculateBlobName(r, n, a, o) {
    try {
        this._calculateMs.start();
        return this._pathHandler.calculateBlobName(r, n);  // hash(path + content)
    } catch (i) {
        // 处理大文件等错误
    } finally {
        this._calculateMs.stop();
    }
}

// 探测服务端缺少的 blobs
async _probe(r) {
    // 收集批次
    let a = new Set;
    for (let [i, c] of n.items) a.add(c.blobName);

    this._probeMs.start();
    let o = await this._apiServer.findMissing([...a]);  // ← 调用 find-missing API
    this._probeMs.stop();

    // 处理结果
    let unknownBlobs = new Set(o.unknownBlobNames);      // 需要上传
    let nonindexedBlobs = new Set(o.nonindexedBlobNames); // 需要等待索引

    for (let [l, d] of n.items) {
        if (unknownBlobs.has(d.blobName)) {
            this._enqueueForUpload(l, d.folderId, d.relPath, false);
        } else if (nonindexedBlobs.has(d.blobName)) {
            this._enqueueForProbeRetry(l, d);  // 稍后重试
        } else {
            this._pathMapUpdate(...);  // 已存在，更新本地映射
        }
    }
}
```

### 3.3 Batch Upload API

**文件**: `chunks.72.mjs:810-819`

```javascript
async batchUpload(t) {
    let r = this.createRequestId();
    let n = this._configListener.config;

    return await this.callApi(r, n, "batch-upload", {
        blobs: t.map(o => ({
            blob_name: o.blobName,   // 内容哈希
            path: o.pathName,        // 文件路径
            content: o.text          // 文件内容 ← 明文上传
        }))
    }, this.toBatchUploadResult.bind(this));
}
```

**关键发现**:
- 文件**明文内容**上传到服务端
- 使用 blob_name (内容哈希) 去重
- 服务端存储并建立索引

---

## 4. 索引事件与状态

### 4.1 索引阶段

**文件**: `chunks.84.mjs:30-230`

```javascript
// 索引事件类型
interface IndexingEvent {
    workspaceRoot: string;
    phase: "scanning" | "indexing" | "complete" | "workspace-too-large";
    percentage: number;
    filesTracked: number;
    filesProcessed: number;
}

// 触发索引事件
_indexingEmitter.fire({
    workspaceRoot: this.workspaceRoot,
    phase: percentage === 100 ? "complete" : "indexing",
    percentage: percentage,
    filesTracked: totalFiles,
    filesProcessed: processedFiles
});
```

### 4.2 工作区限制

```javascript
// 文件数量限制检查
if (fileCount > maxFilesLimit) {
    this._indexingEmitter.fire({
        phase: "workspace-too-large",
        filesTracked: fileCount,
        filesProcessed: 0
    });
    return false;  // 拒绝索引
}
```

---

## 5. 与其他 Agent 集成

### 5.1 Claude Code 配置

```bash
# 用户级别
claude mcp add-json auggie-mcp --scope user '{
    "type": "stdio",
    "command": "auggie",
    "args": ["--mcp"]
}'

# 项目级别
claude mcp add-json auggie-mcp --scope project '{
    "type": "stdio",
    "command": "auggie",
    "args": ["-w", "/path/to/project", "--mcp"]
}'
```

### 5.2 Cursor 配置

```json
// ~/.cursor/mcp.json
{
    "mcpServers": {
        "auggie-mcp": {
            "type": "stdio",
            "command": "auggie",
            "args": ["--mcp"]
        }
    }
}
```

### 5.3 通用 MCP 配置

```json
{
    "command": "auggie",
    "args": ["-w", "/path/to/project", "--mcp"],
    "env": {
        "AUGMENT_API_TOKEN": "your-access-token",
        "AUGMENT_API_URL": "your-tenant-url"
    }
}
```

---

## 6. 架构对比：本地索引 vs 服务端索引

### Augment 方案（服务端索引）

```
优点:
✅ 无需本地计算资源
✅ 专有模型，检索质量高
✅ 跨设备共享索引
✅ 增量更新

缺点:
❌ 代码上传至服务器（隐私考虑）
❌ 依赖网络连接
❌ 需要付费订阅
❌ 大型代码库上传时间长
```

### 本地索引方案（如 Cody）

```
优点:
✅ 代码不离开本地
✅ 离线可用
✅ 无订阅费用

缺点:
❌ 需要本地 GPU/CPU 资源
❌ 索引更新延迟
❌ 模型能力受限
```

---

## 7. 关键代码位置

| 功能 | 文件 | 行号 | 说明 |
|------|------|------|------|
| MCP 模式解析 | chunks.58.mjs | 36-103 | CLI 选项处理 |
| codebase-retrieval 工具 | chunks.76.mjs | 1092-1126 | 工具定义与执行 |
| API 调用 | chunks.72.mjs | 1178-1188 | agents/codebase-retrieval 端点 |
| 文件上传 | chunks.72.mjs | 810-819 | batch-upload 端点 |
| DiskFileManager | chunks.83.mjs | 800-1100 | 文件扫描与上传 |
| 索引事件 | chunks.84.mjs | 30-230 | WorkspaceManager 索引状态 |

---

## 8. 安全与隐私考虑

### 8.1 数据流向

```
本地文件 → 明文上传 → Augment 服务器 → 存储 + 索引
                         ↓
                    Embedding 计算
                         ↓
                    向量存储
```

### 8.2 敏感文件处理

- 遵守 `.gitignore` 规则
- 支持 `.augmentignore` 自定义排除
- 大文件自动跳过

### 8.3 API Token

```bash
# 获取 Token
auggie token print

# 环境变量配置
export AUGMENT_API_TOKEN="your-token"
export AUGMENT_API_URL="https://api.augmentcode.com"
```

---

## 9. 性能指标

**文件**: `chunks.83.mjs:800-830`

```javascript
// DiskFileManager 性能计数器
metrics = new DR("File metrics");
_pathsAccepted = this.metrics.counterMetric("paths accepted");
_pathsNotAccessible = this.metrics.counterMetric("paths not accessible");
_nonFiles = this.metrics.counterMetric("not plain files");
_largeFiles = this.metrics.counterMetric("large files");
_mtimeCacheHits = this.metrics.counterMetric("mtime cache hits");
_mtimeCacheMisses = this.metrics.counterMetric("mtime cache misses");
_probeBatches = this.metrics.counterMetric("probe batches");
_blobNamesProbed = this.metrics.counterMetric("blob names probed");
_filesRead = this.metrics.counterMetric("files read");
_blobsUploaded = this.metrics.counterMetric("blobs uploaded");

// 时间指标
_ingestPathMs = this.metrics.timingMetric("ingestPath");
_probeMs = this.metrics.timingMetric("probe");
_statMs = this.metrics.timingMetric("stat");
_readMs = this.metrics.timingMetric("read");
_uploadMs = this.metrics.timingMetric("upload");
_calculateMs = this.metrics.timingMetric("calculate");
```

---

## 10. 结论

### 核心发现

| 问题 | 答案 |
|------|------|
| **Codebase-retrieval 是本地还是服务端？** | **服务端** - 文件上传到 Augment 服务器 |
| **索引存储在哪里？** | **服务端** - 使用专有 embedding 模型 |
| **本地做什么？** | 文件扫描、哈希计算、上传、查询中继 |
| **如何保证实时性？** | 增量上传 + find-missing 去重机制 |
| **支持哪些语言？** | 多语言（服务端模型决定） |

### 设计权衡

Augment 选择**服务端索引**方案，牺牲本地隐私换取：
1. 更强的检索模型（专有 embedding）
2. 更少的本地资源消耗
3. 跨设备一致的体验

这与 Cursor、Claude Code（本地 grep）形成对比，也与 Cody（本地向量索引）不同。

---

**创建时间**: 2025-12-04
**分析状态**: ✅ 完成

