# Augment 文件扫描与哈希计算机制深度分析

## 文档信息
- **分析时间**: 2025-12-04
- **源文件**: `chunks.83.mjs`, `chunks.76.mjs`, `chunks.72.mjs`, `chunks.84.mjs`
- **分析范围**: 文件扫描、哈希计算、Blob 上传、codebase-retrieval 工具

---

## 执行摘要

Augment 的本地客户端负责：
1. **文件扫描**: 深度优先遍历工作区，应用多层过滤规则
2. **哈希计算**: 对每个文件计算 `SHA256(relativePath + content)`，**不使用 Merkle Tree**
3. **增量同步**: 通过 mtime 缓存和 find-missing API 实现高效去重
4. **批量上传**: 将文件内容明文上传至 Augment 服务器

---

## 1. 哈希计算实现

### 1.1 核心哈希类 (BlobNameCalculator)

**文件位置**: `chunks.83.mjs:696-719`

```javascript
// jde = BlobNameCalculator
jde = class {
    constructor(t) {
        this.maxBlobSize = t  // 最大文件大小限制（字节）
    }

    _textEncoder = new TextEncoder;

    /**
     * 核心哈希函数
     * @param t - 相对路径 (string)
     * @param r - 文件内容 (Uint8Array)
     * @returns SHA256 哈希值 (hex string)
     */
    _hash(t, r) {
        let n = nGt.createHash("sha256");  // 使用 Node.js crypto
        n.update(t);  // 第一步：更新路径
        n.update(r);  // 第二步：更新内容
        return n.digest("hex");  // 返回 64 字符的十六进制字符串
    }

    /**
     * 计算 Blob 名称（带大小检查）
     * @param t - 相对路径
     * @param r - 文件内容 (string 或 Uint8Array)
     * @param n - 是否检查大小限制 (默认 true)
     * @returns Blob 名称 (SHA256 hex)
     * @throws wY - 文件过大异常
     */
    calculateOrThrow(t, r, n = true) {
        // 字符串转 Uint8Array
        if (typeof r == "string") {
            r = this._textEncoder.encode(r);
        }

        // 大小检查
        if (n && r.length > this.maxBlobSize) {
            throw new wY(this.maxBlobSize);
        }

        return this._hash(t, r);
    }

    /**
     * 安全计算（异常返回 undefined）
     */
    calculate(t, r) {
        try {
            return this.calculateOrThrow(t, r, true);
        } catch {
            return undefined;
        }
    }

    /**
     * 强制计算（不检查大小）
     */
    calculateNoThrow(t, r) {
        return this.calculateOrThrow(t, r, false);
    }
}
```

### 1.2 哈希算法详解

**算法**: SHA-256 (256 位，64 字符十六进制)

**输入格式**:
```
SHA256(relativePath || fileContent)
      ↑              ↑
   字符串        Uint8Array
```

**示例**:
```javascript
// 文件: src/utils/helper.js
// 内容: "export function add(a, b) { return a + b; }"

const path = "src/utils/helper.js";
const content = new TextEncoder().encode("export function add(a, b) { return a + b; }");

// 哈希计算过程
const hash = crypto.createHash("sha256");
hash.update(path);     // "src/utils/helper.js"
hash.update(content);  // <Uint8Array 65 78 70 6f 72 74 ...>
const blobName = hash.digest("hex");
// 结果: "a1b2c3d4e5f6..." (64 字符)
```

### 1.3 为什么包含路径？

**设计原因**:
1. **区分同名文件**: `src/utils.js` 和 `lib/utils.js` 内容相同时产生不同哈希
2. **位置敏感**: 服务端检索需要知道代码位置上下文
3. **防止冲突**: 减少哈希碰撞概率

**对比其他方案**:
| 方案 | 哈希输入 | 特点 |
|------|---------|------|
| **Augment** | path + content | 位置敏感，相同内容不同位置产生不同哈希 |
| **Git** | content only | 内容寻址，相同内容共享存储 |
| **rsync** | rolling checksum | 分块，支持差异传输 |

---

## 2. 文件扫描机制

### 2.1 PathIterator 类 (文件遍历器)

**文件位置**: `chunks.83.mjs:152-201`

```javascript
// RY = PathIterator
RY = class {
    /**
     * @param t - 迭代器名称（用于日志）
     * @param r - 起始 URI (startUri)
     * @param n - 根 URI (rootUri)
     * @param a - 路径过滤器 (PathFilter)
     */
    constructor(t, r, n, a) {
        this._name = t;
        this._startUri = r;
        this._rootUri = n;
        this._pathFilter = a;

        // 验证路径
        if (!CY.isAbsolute(r.fsPath)) {
            throw new Error(`startUri must contain an absolute pathname`);
        }
        if (!CY.isAbsolute(n.fsPath)) {
            throw new Error(`rootUri must contain an absolute pathname`);
        }
        if (!Hh(Ow(n), Ow(r))) {
            throw new Error(`startUri must be inside rootUri`);
        }
    }

    // 性能统计
    stats = new DR("Path metrics");
    _dirsEmitted = this.stats.counterMetric("directories emitted");
    _filesEmitted = this.stats.counterMetric("files emitted");
    _otherEmitted = this.stats.counterMetric("other paths emitted");
    _totalEmitted = this.stats.counterMetric("total paths emitted");
    _readDirMs = this.stats.timingMetric("readDir");
    _filterMs = this.stats.timingMetric("filter");
    _yieldMs = this.stats.timingMetric("yield");
    _totalMs = this.stats.timingMetric("total");

    /**
     * 异步迭代器 - 深度优先遍历
     * @yields [absolutePath, relativePath, type, pathInfo]
     */
    async * [Symbol.asyncIterator]() {
        this._totalMs.start();

        const yieldInterval = 200;  // 每 200ms 让出控制权
        let lastYield = Date.now();

        // 使用数组作为栈（深度优先）
        const stack = new Array;
        stack.push(this._startUri);

        let current;
        while ((current = stack.pop()) !== void 0) {
            // 防止阻塞主线程
            if (Date.now() - lastYield >= yieldInterval) {
                await new Promise(resolve => setTimeout(resolve, 0));
                lastYield = Date.now();
            }

            // 计算相对路径
            const relativePath = DH(this._rootUri, current);

            // 创建该目录的本地过滤器
            const localFilter = this._pathFilter.makeLocalPathFilter(relativePath);

            // 读取目录内容
            this._readDirMs.start();
            const entries = fAe(current.fsPath);  // fs.readdirSync with types
            this._readDirMs.stop();

            // 遍历目录项
            for (const [name, type] of entries) {
                // 跳过 . 和 ..
                if (name === "." || name === "..") continue;

                // 防止阻塞
                if (Date.now() - lastYield >= yieldInterval) {
                    await new Promise(resolve => setTimeout(resolve, 0));
                    lastYield = Date.now();
                }

                // 应用过滤器
                this._filterMs.start();
                const absolutePath = V_.joinPath(current, name);
                const itemRelativePath = X_(relativePath, name, type === "Directory");
                const pathInfo = await localFilter.getPathInfo(itemRelativePath, type);
                this._filterMs.stop();

                // 更新统计
                if (type === "File") {
                    this._filesEmitted.increment();
                } else if (type === "Directory") {
                    this._dirsEmitted.increment();
                } else {
                    this._otherEmitted.increment();
                }
                this._totalEmitted.increment();

                // 发出结果
                this._yieldMs.start();
                yield [absolutePath, itemRelativePath, type, pathInfo];
                this._yieldMs.stop();

                // 如果是被接受的目录，加入栈继续遍历
                if (type === "Directory" && pathInfo.accepted) {
                    stack.push(absolutePath);
                }
            }
        }

        this._totalMs.stop();
    }
}
```

### 2.2 遍历算法分析

**算法**: 深度优先搜索 (DFS)，使用显式栈

```
工作区结构:
project/
├── src/
│   ├── index.js
│   └── utils/
│       └── helper.js
├── tests/
│   └── test.js
└── package.json

遍历顺序 (DFS):
1. project/           (起始)
2. package.json       (文件)
3. tests/             (目录，入栈)
4. src/               (目录，入栈)
5. src/index.js       (文件)
6. src/utils/         (目录，入栈)
7. src/utils/helper.js (文件)
8. tests/test.js      (文件)
```

**特点**:
- **非递归**: 使用显式栈避免调用栈溢出
- **非阻塞**: 每 200ms yield 控制权
- **流式处理**: 使用 AsyncIterator，内存占用低
- **实时过滤**: 边遍历边过滤，跳过不需要的目录

---

## 3. 文件过滤机制

### 3.1 过滤层次结构

```
┌─────────────────────────────────────────────────────────────┐
│                       PathFilter                             │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: 默认安全规则 (硬编码)                               │
│    - .git, *.pem, *.key, id_rsa, ...                        │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: .gitignore (项目规则)                              │
│    - node_modules/, dist/, *.log, ...                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: .augmentignore (自定义)                            │
│    - 用户自定义忽略规则                                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: 文件扩展名过滤 (可选)                               │
│    - 仅处理特定扩展名的文件                                   │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 默认安全规则

**文件位置**: `chunks.83.mjs:24-42`

```javascript
// uM = DefaultIgnoreRulesSource
uM = class {
    constructor(t) {
        this._sourceFolderRootPath = t
    }

    getName() {
        return "default Augment rules"
    }

    getRules(t) {
        return new Promise(resolve => {
            // 只在根目录应用
            if (Ow(t) !== this._sourceFolderRootPath) {
                resolve(undefined);
                return;
            }

            // 创建 ignore 实例
            const ignoreInstance = (0, Gwe.default)({
                ignorecase: false  // 大小写敏感
            });

            // 添加默认忽略规则
            ignoreInstance.add([
                // 版本控制
                ".git",

                // 证书和密钥文件
                "*.pem",           // PEM 格式证书
                "*.key",           // 私钥
                "*.pfx",           // PKCS#12
                "*.p12",           // PKCS#12 (别名)
                "*.jks",           // Java KeyStore
                "*.keystore",      // Android KeyStore
                "*.pkcs12",        // PKCS#12 (别名)
                "*.crt",           // 证书
                "*.cer",           // 证书 (DER 格式)

                // SSH 密钥
                "id_rsa",
                "id_ed25519",
                "id_ecdsa",
                "id_dsa",

                // Augment 配置
                ".augment-guidelines"
            ]);

            resolve(ignoreInstance);
        });
    }
}
```

### 3.3 .gitignore 规则处理

**文件位置**: `chunks.83.mjs:12-22`

```javascript
// g6 = GitignoreSource
g6 = class {
    constructor(t) {
        this.filename = t  // ".gitignore"
    }

    getName(t) {
        return tGt(V_.joinPath(t, this.filename))  // 返回完整路径
    }

    async getRules(t, r) {
        // 检查 .gitignore 文件是否存在
        if (r !== undefined) {
            const hasGitignore = r.find(([name, type]) =>
                type === "File" && this.filename === name
            );
            if (!hasGitignore) {
                return undefined;
            }
        }

        // 读取并解析 .gitignore
        return R7n(t, this.filename);
    }
}
```

### 3.4 PathInfo 结构

```javascript
// 文件被接受
const accepted = {
    accepted: true,
    format: () => "Accepted"
};

// 文件被拒绝（各种原因）
const rejectedByIntake = {
    accepted: false,
    format: () => "Rejected by intake service"
};

// 不支持的扩展名
class TY extends sM {
    constructor(extension) {
        this.extension = extension;
    }
    format() {
        return `Unsupported file extension (${this.extension})`;
    }
}
```

---

## 4. DiskFileManager 处理流水线

### 4.1 类定义与配置

**文件位置**: `chunks.83.mjs:761-803`

```javascript
// qde = DiskFileManager
qde = class e extends js {
    /**
     * @param r - 工作区名称
     * @param n - API 服务器
     * @param a - 路径处理器 (PathHandler)
     * @param o - 路径映射表 (PathMap)
     * @param i - 探测批次大小
     */
    constructor(r, n, a, o, i) {
        super();
        this.workspaceName = r;
        this._apiServer = n;
        this._pathHandler = a;
        this._pathMap = o;

        // 日志
        this._logger = Qr(`DiskFileManager[${r}]`);

        // 配置探测批次大小
        if (i === undefined) {
            this._probeBatchSize = e.maxProbeBatchSize;
        } else {
            this._probeBatchSize = Math.max(
                Math.min(i, e.maxProbeBatchSize),
                e.minProbeBatchSize
            );
        }

        // 创建处理队列
        this._toCalculate = new Y8(this._calculate.bind(this));
        this._toProbe = new Y8(this._probe.bind(this));
        this._toUpload = new Y8(this._upload.bind(this));

        // 重试机制
        this._probeRetryWaiters = new Y8(this._enqueueForProbe.bind(this));
        this._probeRetryKicker = new CG(
            this._probeRetryWaiters,
            e.probeRetryPeriodMs
        );

        // 退避重试
        this._probeRetryBackoffWaiters = new Y8(this._enqueueForProbe.bind(this));
        this._probeRetryBackoffKicker = new CG(
            this._probeRetryBackoffWaiters,
            e.probeRetryBackoffPeriodMs
        );
    }

    // 静态配置常量
    static minProbeBatchSize = 1;
    static maxProbeBatchSize = 1000;          // 最大探测批次
    static maxUploadBatchBlobCount = 128;     // 最大上传文件数
    static maxUploadBatchByteSize = 1000000;  // 最大上传大小 (1MB)
    static probeRetryPeriodMs = 3 * 1000;     // 重试间隔 (3秒)
    static probeBackoffAfterMs = 60 * 1000;   // 退避触发时间 (60秒)
    static probeRetryBackoffPeriodMs = 60 * 1000;  // 退避重试间隔 (60秒)

    // 错误消息
    _notAPlainFile = "Not a file";
    _fileNotAccessible = "File not readable";
    _fileNotText = "Binary file";
    _fileUploadFailure = "Upload failed";

    // 性能指标
    metrics = new DR("File metrics");
    _pathsAccepted = this.metrics.counterMetric("paths accepted");
    _pathsNotAccessible = this.metrics.counterMetric("paths not accessible");
    _nonFiles = this.metrics.counterMetric("not plain files");
    _largeFiles = this.metrics.counterMetric("large files");
    _blobNameCalculationFails = this.metrics.counterMetric("blob name calculation fails");
    _encodingErrors = this.metrics.counterMetric("encoding errors");
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
}
```

### 4.2 处理流水线图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        DiskFileManager 处理流水线                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ingestPath(folderId, relativePath)                                         │
│       │                                                                      │
│       ▼                                                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    STAGE 1: Calculate Queue                         │    │
│  │  _toCalculate.insert(seq, [folderId, path])                        │    │
│  │                           │                                         │    │
│  │                           ▼                                         │    │
│  │  ┌─────────────────────────────────────────────────────────────┐   │    │
│  │  │ _calculate(item):                                           │   │    │
│  │  │   1. _getMtime(absolutePath) → 获取文件修改时间              │   │    │
│  │  │   2. _pathMap.getBlobInfo(mtime) → 检查缓存                  │   │    │
│  │  │   3. if (缓存命中 && 有效):                                  │   │    │
│  │  │        return (跳过计算)                                     │   │    │
│  │  │   4. else:                                                   │   │    │
│  │  │        _readAndValidate() → 读取文件                         │   │    │
│  │  │        _calculateBlobName() → SHA256(path + content)         │   │    │
│  │  │   5. _enqueueForProbeRetry(blobInfo)                        │   │    │
│  │  └─────────────────────────────────────────────────────────────┘   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    STAGE 2: Probe Queue                             │    │
│  │  _toProbe.insert(seq, blobInfo)                                    │    │
│  │                           │                                         │    │
│  │                           ▼                                         │    │
│  │  ┌─────────────────────────────────────────────────────────────┐   │    │
│  │  │ _probe(item):                                               │   │    │
│  │  │   1. _probeBatch.addItem(blobName) → 累积批次                │   │    │
│  │  │   2. if (批次满 || 无更多项):                                │   │    │
│  │  │        blobNames = collect(batch)                           │   │    │
│  │  │        result = await _apiServer.findMissing(blobNames)     │   │    │
│  │  │                                                              │   │    │
│  │  │   3. for each blob:                                         │   │    │
│  │  │        if (unknown): _enqueueForUpload()                    │   │    │
│  │  │        if (nonindexed): _enqueueForProbeRetry() (等待索引)   │   │    │
│  │  │        if (known): _pathMapUpdate() (已存在)                 │   │    │
│  │  └─────────────────────────────────────────────────────────────┘   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                     ┌──────────────┴──────────────┐                          │
│                     │                             │                          │
│                     ▼                             ▼                          │
│  ┌──────────────────────────────┐   ┌──────────────────────────────────┐    │
│  │    STAGE 3a: Upload Queue    │   │    STAGE 3b: Probe Retry         │    │
│  │                              │   │                                   │    │
│  │  _toUpload.insert(...)       │   │  _probeRetryWaiters.insert(...)  │    │
│  │           │                  │   │           │                       │    │
│  │           ▼                  │   │           ▼                       │    │
│  │  ┌────────────────────────┐  │   │  ┌─────────────────────────────┐ │    │
│  │  │ _upload(item):         │  │   │  │ 等待 3秒后重新探测          │ │    │
│  │  │   1. 读取文件内容      │  │   │  │ (probeRetryPeriodMs)        │ │    │
│  │  │   2. 累积批次          │  │   │  └─────────────────────────────┘ │    │
│  │  │   3. 批次满时:         │  │   │                                   │    │
│  │  │      _uploadBlobBatch()│  │   │  如果超过 60秒仍未成功:           │    │
│  │  │      → batch-upload API│  │   │  → 进入退避重试 (60秒间隔)        │    │
│  │  └────────────────────────┘  │   │                                   │    │
│  └──────────────────────────────┘   └──────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 mtime 缓存机制

**文件位置**: `chunks.83.mjs:898-931`

```javascript
async _calculate(r) {
    if (r === undefined) return;

    let [seq, [folderId, relPath]] = r;

    // 验证路径是否仍需跟踪
    if (!this._pathMapVerify(folderId, relPath, seq)) return;

    // 构建绝对路径
    const absolutePath = this._makeAbsPath(folderId, relPath);
    if (absolutePath === undefined) {
        this._inflightItemRemove(seq);
        return;
    }

    // 获取文件修改时间
    const mtime = this._getMtime(absolutePath, folderId, relPath, seq);
    if (mtime === undefined) return;

    let blobName;

    // 检查 mtime 缓存
    const cachedInfo = this._pathMap.getBlobInfo(folderId, relPath, mtime);

    if (cachedInfo !== undefined) {
        // 缓存命中
        this._mtimeCacheHits.increment();

        const [cachedBlobName, status] = cachedInfo;

        if (status > 0) {
            // 缓存有效，直接更新路径映射
            this._pathMapUpdate(folderId, relPath, seq, cachedBlobName, mtime);
            return;  // 跳过后续处理
        }

        // 缓存的 blobName 可用，但需要重新探测
        blobName = cachedBlobName;
    } else {
        // 缓存未命中 - 需要读取文件并计算哈希
        this._mtimeCacheMisses.increment();

        // 读取并验证文件
        const content = await this._readAndValidate(absolutePath, folderId, relPath, seq);
        if (content === undefined) return;

        // 计算 blob 名称
        blobName = this._calculateBlobName(relPath, content, folderId, seq);
        if (blobName === undefined) return;
    }

    // 更新统计
    this._pathsAccepted.increment();

    // 加入探测队列
    const blobInfo = {
        folderId: folderId,
        relPath: relPath,
        blobName: blobName,
        mtime: mtime,
        startTime: Date.now()
    };

    this._enqueueForProbeRetry(seq, blobInfo);
}
```

**缓存工作流程**:

```
文件变更检测流程:
                    ┌─────────────────┐
                    │  ingestPath()   │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  获取 mtime     │
                    │  (文件修改时间)  │
                    └────────┬────────┘
                             │
                             ▼
               ┌─────────────────────────────┐
               │ pathMap.getBlobInfo(mtime) │
               │       缓存查询              │
               └─────────────┬───────────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
     ┌─────────────────┐          ┌─────────────────┐
     │   缓存命中       │          │   缓存未命中     │
     │ (mtime 未变)     │          │ (文件已修改)     │
     └────────┬────────┘          └────────┬────────┘
              │                             │
              ▼                             ▼
     ┌─────────────────┐          ┌─────────────────┐
     │ 使用缓存的       │          │ 读取文件内容     │
     │ blobName        │          │ 计算新的         │
     │                 │          │ SHA256 哈希      │
     └────────┬────────┘          └────────┬────────┘
              │                             │
              └──────────────┬──────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  探测/上传流程   │
                    └─────────────────┘
```

### 4.4 批量上传实现

**文件位置**: `chunks.83.mjs:1032-1054`

```javascript
async _uploadBlobBatch(r) {
    this._logger.verbose(`upload begin: ${r.length} blobs`);

    // 日志记录待上传文件
    for (const blob of r) {
        this._logger.verbose(`    - ${blob.folderId}:${blob.pathName}; expected blob name ${blob.blobName}`);
    }

    let result;
    try {
        // 批量上传 API 调用
        result = await U_(
            async () => await this._apiServer.batchUpload(r),
            this._logger
        );
    } catch (error) {
        this._logger.error(`batch upload failed: ${nr(error)}`);
    }

    // 构建结果映射
    const uploadedMap = new Map();

    if (result !== undefined) {
        for (let i = 0; i < result.blobNames.length; i++) {
            uploadedMap.set(r[i].blobName, result.blobNames[i]);
        }
    }

    // 顺序上传剩余失败的文件
    await this._uploadBlobsSequentially(
        r,
        result?.blobNames.length ?? 0,
        uploadedMap
    );

    return uploadedMap;
}

async _uploadBlobsSequentially(r, startIndex, resultMap) {
    for (let i = startIndex; i < r.length; i++) {
        const blob = r[i];
        try {
            this._logger.verbose(`sequential upload of ${blob.pathName} -> ${blob.blobName}`);

            const result = await U_(
                async () => this._apiServer.batchUpload([blob]),
                this._logger
            );

            if (result.blobNames.length > 0) {
                resultMap.set(blob.blobName, result.blobNames[0]);
            }
        } catch {
            // 忽略单个文件上传失败
        }
    }
}
```

---

## 5. API 接口详解

### 5.1 find-missing API

**用途**: 检查服务端缺少哪些 blob

**调用位置**: `chunks.83.mjs:952`

```javascript
result = await this._apiServer.findMissing([...blobNames]);
```

**请求**:
```typescript
interface FindMissingRequest {
    blob_names: string[];  // SHA256 哈希列表
}
```

**响应**:
```typescript
interface FindMissingResponse {
    unknownBlobNames: string[];     // 服务端完全没有的 blob
    nonindexedBlobNames: string[];  // 服务端有但未索引的 blob
}
```

**逻辑**:
- `unknownBlobNames` → 需要上传
- `nonindexedBlobNames` → 等待索引完成后重试
- 其他 → 服务端已有，跳过

### 5.2 batch-upload API

**用途**: 批量上传文件内容

**调用位置**: `chunks.72.mjs:810-819`

```javascript
async batchUpload(t) {
    const requestId = this.createRequestId();
    const config = this._configListener.config;

    return await this.callApi(requestId, config, "batch-upload", {
        blobs: t.map(blob => ({
            blob_name: blob.blobName,   // SHA256 哈希
            path: blob.pathName,        // 相对路径
            content: blob.text          // 明文内容
        }))
    }, this.toBatchUploadResult.bind(this));
}
```

**请求**:
```typescript
interface BatchUploadRequest {
    blobs: Array<{
        blob_name: string;  // SHA256(path + content)
        path: string;       // 相对路径
        content: string;    // 文件内容（明文）
    }>;
}
```

**响应**:
```typescript
interface BatchUploadResponse {
    blob_names: string[];  // 成功上传的 blob 名称
}
```

### 5.3 agents/codebase-retrieval API

**用途**: 执行代码库语义检索

**调用位置**: `chunks.72.mjs:1178-1188`

```javascript
async agentCodebaseRetrieval(t, r, n, a, o, i, c) {
    const config = this._configListener.config;

    return await this.callApi(t, config, "agents/codebase-retrieval", {
        information_request: r,      // 自然语言查询
        blobs: gS(n),                // 当前上下文的 blob 引用
        dialog: a,                   // 对话历史长度
        max_output_length: o,        // 最大输出长度
        disable_codebase_retrieval: i?.disableCodebaseRetrieval ?? false,
        enable_commit_retrieval: i?.enableCommitRetrieval ?? false
    },
    l => this.convertToAgentCodebaseRetrievalResult(l),
    config.chat.url,
    120000,  // 超时 120 秒
    undefined,
    c);
}
```

**请求**:
```typescript
interface CodebaseRetrievalRequest {
    information_request: string;         // 自然语言查询
    blobs: BlobReference[];              // 当前上下文
    dialog: number;                      // 对话历史
    max_output_length?: number;          // 输出限制
    disable_codebase_retrieval: boolean; // 禁用代码检索
    enable_commit_retrieval: boolean;    // 启用 Git 提交检索
}
```

**响应**:
```typescript
interface CodebaseRetrievalResponse {
    formatted_retrieval: string;  // 格式化的检索结果
}
```

---

## 6. codebase-retrieval 工具完整定义

### 6.1 工具类实现

**文件位置**: `chunks.76.mjs:1091-1126`

```javascript
// VF = CodebaseRetrievalTool
VF = class extends qo {
    constructor() {
        super("codebase-retrieval", 1)  // 工具名称, 版本号
    }

    // 工具描述 - 这是 LLM 看到的 Prompt
    description = `This tool is Augment's context engine, the world's best codebase context engine. It:
1. Takes in a natural language description of the code you are looking for;
2. Uses a proprietary retrieval/embedding model suite that produces the highest-quality recall of relevant code snippets from across the codebase;
3. Maintains a real-time index of the codebase, so the results are always up-to-date and reflects the current state of the codebase;
4. Can retrieve across different programming languages;
5. Only reflects the current state of the codebase on the disk, and has no information on version control or code history.`;

    // JSON Schema - 定义工具参数
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

    version = 2;

    // 安全检查 - 此工具始终安全
    checkToolCallSafe(t) {
        return true;
    }

    // 工具执行
    async call(t, r, n, a, o) {
        const requestId = Gp();  // 生成唯一请求 ID

        try {
            const query = t.information_request;

            // 调用后端 API
            const result = await bS().agentCodebaseRetrieval(
                requestId,    // 请求 ID
                query,        // 查询内容
                r,            // blobs (上下文)
                0,            // dialog
                undefined,    // max_output_length
                n             // options
            );

            // 返回成功结果
            return Ro(result.formattedRetrieval, requestId);
        } catch (error) {
            // 返回错误结果
            return Xr(
                `Failed to retrieve codebase information: ${error.message}`,
                requestId
            );
        }
    }
}
```

### 6.2 工具 Prompt 分析

**工具描述拆解**:

| 要点 | 内容 | 目的 |
|------|------|------|
| **功能定位** | "Augment's context engine, the world's best codebase context engine" | 建立 LLM 对工具能力的信任 |
| **输入格式** | "natural language description" | 告诉 LLM 可以用自然语言查询 |
| **技术能力** | "proprietary retrieval/embedding model suite" | 暗示有强大的语义理解能力 |
| **实时性** | "real-time index...always up-to-date" | LLM 可以信任结果是最新的 |
| **跨语言** | "retrieve across different programming languages" | 不限于特定语言 |
| **限制声明** | "no information on version control or code history" | 明确边界，防止误用 |

**参数描述**:
```json
{
    "information_request": {
        "type": "string",
        "description": "A description of the information you need."
    }
}
```

- **参数名**: `information_request` (非 `query`)
- **描述**: 简洁开放，鼓励描述性查询而非关键词
- **必填**: 是

### 6.3 工具在 UI 中的显示

**文件位置**: `chunks.86.mjs:395-398`

```javascript
case "codebase-retrieval":
    return {
        title: "Context Engine",           // UI 标题
        description: t.information_request // 显示查询内容
    };
```

---

## 7. 与 Merkle Tree 的对比分析

### 7.1 Augment 的扁平哈希方案

```
文件结构:
project/
├── src/
│   ├── index.js    → SHA256("src/index.js" + content1) = "abc123..."
│   └── utils.js    → SHA256("src/utils.js" + content2) = "def456..."
├── lib/
│   └── helper.js   → SHA256("lib/helper.js" + content3) = "ghi789..."
└── package.json    → SHA256("package.json" + content4) = "jkl012..."

存储结构 (PathMap):
┌────────────────────┬──────────────────┬─────────────┐
│ 相对路径            │ blobName (SHA256) │ mtime      │
├────────────────────┼──────────────────┼─────────────┤
│ src/index.js       │ abc123...        │ 1701648000  │
│ src/utils.js       │ def456...        │ 1701648100  │
│ lib/helper.js      │ ghi789...        │ 1701648200  │
│ package.json       │ jkl012...        │ 1701648300  │
└────────────────────┴──────────────────┴─────────────┘
```

### 7.2 Merkle Tree 方案（假设实现）

```
                    root_hash = H(dir1_hash + dir2_hash + file_hash)
                   /              |              \
            dir1_hash         dir2_hash      file_hash
           = H(f1 + f2)       = H(f3)        = H(content4)
           /        \             |
      file1_hash  file2_hash  file3_hash
      = H(c1)     = H(c2)     = H(c3)

存储结构:
┌────────────────────┬──────────────────┬─────────────────────┐
│ 节点                │ hash             │ children            │
├────────────────────┼──────────────────┼─────────────────────┤
│ /                  │ root_hash        │ [src/, lib/, pkg]   │
│ src/               │ dir1_hash        │ [index.js, utils.js]│
│ lib/               │ dir2_hash        │ [helper.js]         │
│ src/index.js       │ file1_hash       │ []                  │
│ src/utils.js       │ file2_hash       │ []                  │
│ lib/helper.js      │ file3_hash       │ []                  │
│ package.json       │ file_hash        │ []                  │
└────────────────────┴──────────────────┴─────────────────────┘
```

### 7.3 方案对比

| 特性 | Augment (扁平哈希) | Merkle Tree |
|------|-------------------|-------------|
| **哈希计算** | 每文件独立 | 层层聚合 |
| **变更检测** | O(n) 遍历 | O(log n) 从根开始 |
| **增量同步** | find-missing API | 树遍历差异 |
| **存储开销** | n 个哈希 | n + 目录数 个哈希 |
| **实现复杂度** | 低 | 高 |
| **适用场景** | 内容检索 | 完整性验证 |
| **部分验证** | 不支持 | 支持 |

### 7.4 为什么 Augment 不用 Merkle Tree？

**原因分析**:

1. **目标不同**
   - Augment: 代码检索和语义搜索
   - Merkle Tree: 完整性验证和差异同步

2. **服务端架构**
   - Augment 的索引在服务端，不需要客户端验证完整性
   - `find-missing` API 已经实现了高效的增量同步

3. **简化实现**
   - 扁平结构更容易实现和维护
   - 避免目录重命名导致的大量哈希重算

4. **检索粒度**
   - 代码检索是文件级别或代码块级别
   - 不需要目录级别的哈希聚合

---

## 8. 性能优化策略

### 8.1 已实现的优化

| 优化策略 | 实现方式 | 效果 |
|---------|---------|------|
| **mtime 缓存** | 文件未修改时跳过哈希计算 | 减少 90%+ 的重复计算 |
| **批量探测** | 最多 1000 个 blob 一次探测 | 减少 API 调用次数 |
| **批量上传** | 最多 128 个文件或 1MB | 减少网络往返 |
| **异步非阻塞** | 每 200ms yield | 保持 UI 响应 |
| **深度优先** | 使用显式栈 | 避免递归栈溢出 |
| **流式处理** | AsyncIterator | 低内存占用 |

### 8.2 潜在优化空间

1. **并行上传**: 当前是顺序回退，可以改为并行
2. **压缩传输**: 文本内容可以 gzip 压缩
3. **增量哈希**: 对于追加型文件，可以使用滚动哈希
4. **本地缓存**: 缓存服务端已有的 blob 列表

---

## 9. 关键代码位置索引

| 功能 | 文件 | 行号 | 类/函数名 |
|------|------|------|----------|
| 哈希计算 | chunks.83.mjs | 696-719 | `jde` (BlobNameCalculator) |
| 文件遍历 | chunks.83.mjs | 152-201 | `RY` (PathIterator) |
| 默认忽略规则 | chunks.83.mjs | 24-42 | `uM` (DefaultIgnoreRulesSource) |
| gitignore 处理 | chunks.83.mjs | 12-22 | `g6` (GitignoreSource) |
| 文件管理器 | chunks.83.mjs | 761-1100 | `qde` (DiskFileManager) |
| 计算队列 | chunks.83.mjs | 898-931 | `_calculate()` |
| 探测队列 | chunks.83.mjs | 940-963 | `_probe()` |
| 上传队列 | chunks.83.mjs | 972-1054 | `_upload()` |
| batch-upload API | chunks.72.mjs | 810-819 | `batchUpload()` |
| codebase-retrieval API | chunks.72.mjs | 1178-1188 | `agentCodebaseRetrieval()` |
| retrieval 工具定义 | chunks.76.mjs | 1091-1126 | `VF` (CodebaseRetrievalTool) |

---

## 10. 总结

### 核心发现

1. **哈希算法**: `SHA256(relativePath + content)`，每个文件独立计算
2. **无 Merkle Tree**: 使用扁平的文件→哈希映射
3. **增量同步**: 通过 mtime 缓存 + find-missing API 实现
4. **三阶段流水线**: Calculate → Probe → Upload
5. **批量处理**: 最大 1000 个探测，128 个上传

### 设计权衡

| 选择 | 优点 | 代价 |
|------|------|------|
| 扁平哈希 | 实现简单，易于调试 | 无法高效验证目录完整性 |
| 包含路径 | 区分同名文件 | 重命名需要重新上传 |
| mtime 缓存 | 快速跳过未修改文件 | 依赖文件系统时间准确性 |
| 服务端索引 | 强大的语义检索 | 隐私和网络依赖 |

---

**创建时间**: 2025-12-04
**分析状态**: ✅ 完成

