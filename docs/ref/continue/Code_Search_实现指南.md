# Continue Code Search 实现指南

## 目标

本文档提供 Continue Code Search 系统的实战指南，包括：
- 代码示例和使用场景
- 配置和集成指南
- 性能优化建议
- 常见问题与解决方案

---

## 1. 索引系统使用指南

### 1.1 启用/禁用索引

**场景**: 性能关键环境、仅需 LSP 符号导航

```yaml
# ~/.continue/config.yaml

disableIndexing: false  # 启用索引 (默认)
disableIndexing: true   # 禁用所有索引

# 仅禁用向量搜索 (性能关键)
contextProviders:
  - name: codebase
    disabled: true          # 禁用这个 provider
```

### 1.2 配置 Embeddings 模型

**场景**: 集成不同的 embeddings 提供商

```yaml
# 使用 OpenAI embeddings
selectedModelByRole:
  embed: openai

model_providers:
  openai:
    api_key: "sk-..."
    model_name: "text-embedding-3-small"

# 使用本地 embeddings (离线)
selectedModelByRole:
  embed: ollama

model_providers:
  ollama:
    base_url: "http://localhost:11434"
    model_name: "nomic-embed-text"

# 使用 Jina AI
selectedModelByRole:
  embed: jina

model_providers:
  jina:
    api_key: "jina_..."
    model_name: "jina-embeddings-v2-base-en"
```

### 1.3 索引存储位置

```bash
# 索引存储位置
~/.continue/index.sqlite          # 主数据库
~/.continue/lancedb/              # 向量数据库

# 清除索引 (强制重新索引)
rm ~/.continue/index.sqlite
rm -rf ~/.continue/lancedb/

# 仓库大小
du -sh ~/.continue/               # 总大小
```

### 1.4 多仓库配置

**场景**: 在多个仓库中工作，需要针对性索引

```yaml
# 按仓库配置不同选项
contextProviders:
  - name: codebase
    # 配置 1: 性能优先 (大型仓库)
    chunkSize: 2000           # 较大的 chunk
    maxChunkTokens: 512       # 限制 token 数

  - name: codebase
    # 配置 2: 精度优先 (小型仓库)
    chunkSize: 1000
    maxChunkTokens: 1024
```

---

## 2. LSP 集成指南

### 2.1 启用 LSP 符号导航

**当前状态**: LSP 集成代码存在但默认禁用

**启用方式** (供开发者参考):

```typescript
// core/autocomplete/snippets/getAllSnippets.ts (第 17 行)
// 当前:
const IDE_SNIPPETS_ENABLED = false;

// 启用:
const IDE_SNIPPETS_ENABLED = true;

// 然后在 snippet gathering 中:
if (IDE_SNIPPETS_ENABLED) {
  const lspSnippets = await getDefinitionsFromLsp(
    filepath,
    contents,
    cursorIndex,
    ide,
    lang
  );
  snippets.push(...lspSnippets);
}
```

### 2.2 LSP 功能利用

**IDE 提供的功能**:

```typescript
// VS Code 中获取定义
const definitions = await vscode.commands.executeCommand(
  'vscode.executeDefinitionProvider',
  uri,
  position
);

// 转换为 Continue snippet
const snippet = {
  filepath: definition.uri.fsPath,
  range: {
    start: definition.range.start.line,
    end: definition.range.end.line,
  },
  score: 1.0,  // LSP 结果总是高分
};

// 在 autocomplete 中使用
const results = [...lspSnippets, ...embeddingsSnippets];
```

### 2.3 符号导航流程

```typescript
// 用户按 Ctrl+Click 跳转符号
async function jumpToSymbol(uri: string, position: Position) {
  // 1. 使用 LSP 获取定义
  const definitions = await ide.executeCommand(
    'vscode.executeDefinitionProvider',
    uri,
    position
  );

  // 2. 如果找到，跳转到定义
  if (definitions && definitions.length > 0) {
    const def = definitions[0];
    await ide.openDocument(def.uri);
    await ide.revealRange(def.range);
  }

  // 3. 否则，使用 codebase index 搜索
  const symbol = getSymbolAtPosition(document, position);
  const results = await codebaseIndex.retrieve({
    query: symbol,
    nFinal: 5,
  });
  // 显示搜索结果...
}
```

---

## 3. AST 分析实战

### 3.1 提取函数签名

**场景**: 显示函数参数提示

```typescript
// core/indexing/CodeSnippetsIndex.ts

async function extractFunctionSignatures(
  filePath: string,
  content: string
): Promise<FunctionSignature[]> {
  const parser = await getParserForFile(filePath);
  const tree = parser.parse(content);

  // 加载 TypeScript query
  const tsQuery = `
    (function_declaration
      name: (identifier) @name
      parameters: (formal_parameters) @params
      return_type: (type_annotation)? @return
    ) @func
  `;

  const query = language.query(tsQuery);
  const matches = query.matches(tree.rootNode);

  const signatures: FunctionSignature[] = [];
  for (const match of matches) {
    const name = getCapture(match, 'name');
    const params = getCapture(match, 'params');
    const returnType = getCapture(match, 'return');

    signatures.push({
      name: name.text,
      parameters: params.text,
      returnType: returnType?.text,
      line: name.startPosition.row,
    });
  }

  return signatures;
}

// 在自动补全中使用
const signatures = await extractFunctionSignatures(
  activeFile,
  fileContents
);

// 显示参数提示
showParameterHints(signatures);
```

### 3.2 类结构分析

**场景**: 显示类的成员列表

```typescript
async function extractClassMembers(
  content: string
): Promise<ClassMember[]> {
  const parser = await getParserForFile('file.ts');
  const tree = parser.parse(content);

  // 提取类定义
  const classQuery = `
    (class_declaration
      name: (type_identifier) @name
      body: (class_body
        [
          (method_definition
            name: (property_identifier) @method_name
          )
          (property_signature
            name: (property_identifier) @prop_name
          )
        ]
      )
    )
  `;

  const query = language.query(classQuery);
  const matches = query.matches(tree.rootNode);

  const members: ClassMember[] = [];
  for (const match of matches) {
    const className = getCapture(match, 'name');
    const methods = getCaptures(match, 'method_name');
    const props = getCaptures(match, 'prop_name');

    members.push({
      className: className.text,
      methods: methods.map(m => m.text),
      properties: props.map(p => p.text),
    });
  }

  return members;
}

// 在 hover 时显示类信息
onHover(async (position) => {
  const members = await extractClassMembers(fileContent);
  showHoverInfo(members);
});
```

### 3.3 依赖关系分析

**场景**: 找出模块依赖

```typescript
async function extractImports(
  content: string
): Promise<ImportInfo[]> {
  const parser = await getParserForFile('file.ts');
  const tree = parser.parse(content);

  // 提取 import 语句
  const importQuery = `
    [
      (import_statement
        source: (string) @source
        [
          (named_imports
            (import_specifier
              name: (identifier) @name
            )
          )
          (import_clause
            (identifier) @default_import
          )
        ]
      )
      (import_declaration ...)
    ]
  `;

  const query = language.query(importQuery);
  const matches = query.matches(tree.rootNode);

  const imports: ImportInfo[] = [];
  for (const match of matches) {
    const source = getCapture(match, 'source');
    const names = getCaptures(match, 'name');

    imports.push({
      from: source.text.replace(/["']/g, ''),
      items: names.map(n => n.text),
    });
  }

  return imports;
}

// 在代码分析中使用
const deps = await extractImports(fileContent);
analyzeImportCycles(deps);
```

---

## 4. 搜索实现实战

### 4.1 简单的 FTS 查询

**场景**: 快速查找关键词

```typescript
// 直接使用 FullTextSearchCodebaseIndex
async function searchByKeyword(
  keyword: string,
  maxResults: number = 20
): Promise<CodeSnippet[]> {
  const index = new FullTextSearchCodebaseIndex(
    codebaseIndexPath
  );

  const results = await index.retrieve({
    query: keyword,
    nFinal: maxResults,
    nRetrieve: 50,
    bm25Threshold: -2.5,  // 调整 BM25 阈值
  });

  return results;
}

// 用法
const results = await searchByKeyword('handleClick');
// 返回: [
//   { path: 'ui/Button.tsx', content: 'function handleClick(...)', ... },
//   { path: 'ui/Modal.tsx', content: 'const handleClick = (...)', ... },
// ]
```

### 4.2 语义搜索 (向量)

**场景**: 基于含义的搜索

```typescript
// 使用 LanceDbIndex
async function semanticSearch(
  query: string,
  maxResults: number = 20,
  directory?: string
): Promise<CodeSnippet[]> {
  const index = new LanceDbIndex(codebaseIndexPath);

  const results = await index.retrieve(
    query,
    maxResults,
    [{ directory: '.', branch: 'main' }],
    directory
  );

  return results;
}

// 用法
const results = await semanticSearch(
  'authenticate user with JWT'  // 语义查询
);
// 返回相关代码，即使不包含 'JWT' 关键词
```

### 4.3 混合搜索 (FTS + 向量)

**场景**: 获得最佳搜索结果

```typescript
// 结合 FTS 和向量搜索
async function hybridSearch(
  query: string,
  maxResults: number = 20
): Promise<CodeSnippet[]> {
  // 1. 并行执行 FTS 和向量搜索
  const [ftsResults, semanticResults] = await Promise.all([
    ftsIndex.retrieve({
      query,
      nFinal: 50,
      bm25Threshold: -2.5,
    }),
    vectorIndex.retrieve(query, 50),
  ]);

  // 2. 合并结果，避免重复
  const merged = new Map<string, CodeSnippet>();
  const addResults = (results: CodeSnippet[], weight: number) => {
    results.forEach((r, i) => {
      const key = `${r.path}:${r.startLine}`;
      if (!merged.has(key)) {
        merged.set(key, { ...r, score: i * weight });
      }
    });
  };

  addResults(ftsResults, 0.4);        // FTS 权重 40%
  addResults(semanticResults, 0.6);   // 向量权重 60%

  // 3. 排序并返回前 N 个
  return Array.from(merged.values())
    .sort((a, b) => a.score - b.score)
    .slice(0, maxResults);
}

// 用法
const results = await hybridSearch('user authentication');
```

### 4.4 条件搜索 (带过滤)

**场景**: 限制搜索范围

```typescript
// 按目录/文件类型搜索
async function restrictedSearch(
  query: string,
  filters: {
    directory?: string;      // 仅搜索特定目录
    fileTypes?: string[];    // 仅搜索特定文件类型
    excludeTests?: boolean;  // 排除测试文件
  } = {}
): Promise<CodeSnippet[]> {
  const results = await vectorIndex.retrieve(
    query,
    20,
    [{ directory: filters.directory || '.', branch: 'main' }],
    filters.directory
  );

  // 过滤结果
  let filtered = results;

  if (filters.fileTypes) {
    filtered = filtered.filter(r =>
      filters.fileTypes!.some(type =>
        r.path.endsWith(type)
      )
    );
  }

  if (filters.excludeTests) {
    filtered = filtered.filter(r =>
      !r.path.includes('__tests__') &&
      !r.path.includes('.test.') &&
      !r.path.includes('.spec.')
    );
  }

  return filtered;
}

// 用法
const results = await restrictedSearch('authenticate', {
  directory: 'src/auth',
  fileTypes: ['.ts', '.tsx'],
  excludeTests: true,
});
```

---

## 5. 自动补全优化指南

### 5.1 配置自动补全参数

**场景**: 针对不同硬件调整

```yaml
# ~/.continue/config.yaml

# 高性能硬件 (16GB+ 内存)
completionOptions:
  maxPromptTokens: 2048      # 更多上下文
  debounceDelay: 200         # 更快响应
  modelTimeout: 300          # 允许更多时间

# 低功耗硬件 (4GB 内存)
completionOptions:
  maxPromptTokens: 512       # 较少上下文
  debounceDelay: 500         # 延迟更长
  modelTimeout: 100          # 超时更短
  onlyMyCode: true           # 仅工作空间

# 针对特定语言优化
tabAutocompleteOptions:
  Python:
    maxPromptTokens: 1024
  TypeScript:
    maxPromptTokens: 1536
    useImports: true
```

### 5.2 构建上下文

**场景**: 构建高质量的补全上下文

```typescript
// 构建自动补全上下文
async function buildCompletionContext(
  file: string,
  position: Position
): Promise<CompletionContext> {
  // 1. 获取光标周围的代码
  const prefix = getPrefix(file, position);     // 光标前代码
  const suffix = getSuffix(file, position);     // 光标后代码

  // 2. 获取最近编辑的文件
  const recentFiles = getRecentlyEditedFiles(5);

  // 3. 搜索相关代码片段
  const snippets = await searchRelevantSnippets(
    prefix,    // 使用前缀作为查询
    {
      maxSnippets: 10,
      maxTokens: 800,
    }
  );

  // 4. 打包上下文
  return {
    prefix,
    suffix,
    snippets,
    recentFiles,
    imports: extractImports(file),
  };
}

// 在完成请求中使用
const context = await buildCompletionContext(
  activeFile,
  cursorPosition
);

const completion = await lmm.complete({
  prompt: buildPrompt(context),
  temperature: 0.1,  // 补全应该确定性
  max_tokens: 100,   // 限制生成长度
});
```

### 5.3 片段排序优化

**场景**: 改进自动补全质量

```typescript
// 基于相似度的片段排序
function rankSnippets(
  snippets: CodeSnippet[],
  cursorContext: string
): CodeSnippet[] {
  // 1. 计算相似度
  const scored = snippets.map(snippet => ({
    ...snippet,
    similarity: jaccardSimilarity(
      snippet.content,
      cursorContext
    ),
  }));

  // 2. 按相似度排序
  scored.sort((a, b) => b.similarity - a.similarity);

  // 3. 去重重叠片段
  const deduped = deduplicateOverlapping(scored);

  // 4. 考虑最近性 (最近编辑的文件优先)
  deduped.sort((a, b) => {
    const aRecent = isRecentlyEdited(a.path);
    const bRecent = isRecentlyEdited(b.path);
    if (aRecent !== bRecent) return aRecent ? -1 : 1;
    return b.similarity - a.similarity;
  });

  return deduped;
}

// 在 getAllSnippets 中使用
const allSnippets = [
  ...codebaseSnippets,
  ...importSnippets,
  ...recentlyEditedSnippets,
];

const rankedSnippets = rankSnippets(allSnippets, prefix);
```

---

## 6. 性能优化实战

### 6.1 索引性能优化

**场景**: 加快索引速度

```yaml
# config.yaml - 索引性能优化

# 禁用不需要的索引
contextProviders:
  - name: codebase
    # 只启用最快的索引
    indexTypes: ["fullTextSearch"]  # 跳过向量搜索

# 调整 chunk 大小
contextProviders:
  - name: codebase
    chunkSize: 4000                 # 更大的 chunk
    maxChunkSize: 2048              # 限制 token

# 增加索引批大小 (代码中)
// core/CodebaseIndexer.ts
const BATCH_SIZE = 500;  // 从 200 增加到 500
```

**代码优化**:

```typescript
// 并行索引多个目录
async function* parallelIndex(
  directories: string[]
): AsyncGenerator<IndexingProgress> {
  // 为每个目录创建索引器
  const indexers = directories.map(dir =>
    new CodebaseIndexer(dir)
  );

  // 并行运行
  const generators = indexers.map(idx => idx.refreshDirs());

  // 聚合结果
  yield* aggregateGenerators(generators);
}

// 用法
for await (const progress of parallelIndex(
  ['src', 'lib', 'tests']
)) {
  updateProgress(progress);
}
```

### 6.2 搜索性能优化

**场景**: 加快搜索响应

```typescript
// 缓存经常查询的结果
class CachedCodebaseIndex {
  private cache = new Map<string, CodeSnippet[]>();
  private cacheTTL = 5 * 60 * 1000;  // 5 分钟

  async retrieve(query: string): Promise<CodeSnippet[]> {
    const key = query;

    // 检查缓存
    if (this.cache.has(key)) {
      const cached = this.cache.get(key)!;
      if (Date.now() - cached.timestamp < this.cacheTTL) {
        return cached.results;
      }
    }

    // 缓存未命中，执行搜索
    const results = await this.innerIndex.retrieve({
      query,
      nFinal: 20,
    });

    // 存储在缓存中
    this.cache.set(key, {
      results,
      timestamp: Date.now(),
    });

    return results;
  }
}

// 限制搜索范围
async function limitedSearch(
  query: string,
  scope: 'currentFile' | 'workspace' | 'directory'
): Promise<CodeSnippet[]> {
  let results: CodeSnippet[];

  switch (scope) {
    case 'currentFile':
      // 仅搜索当前文件的代码片段
      results = await index.retrieve({
        query,
        filterDirectory: dirname(currentFile),
      });
      break;

    case 'workspace':
      // 全工作空间搜索
      results = await index.retrieve({ query });
      break;

    case 'directory':
      // 仅搜索当前目录
      results = await index.retrieve({
        query,
        filterDirectory: dirname(currentFile),
      });
      break;
  }

  return results.slice(0, 10);  // 限制结果数
}
```

### 6.3 内存优化

**场景**: 处理大型仓库

```typescript
// 流式处理大型索引
async function* streamIndexChunks(
  filePath: string,
  chunkSize: number = 1000
): AsyncGenerator<CodeSnippet[]> {
  const content = readFileSync(filePath, 'utf-8');
  const parser = await getParserForFile(filePath);
  const tree = parser.parse(content);

  let batch: CodeSnippet[] = [];

  // 遍历 AST，流式 yield chunks
  for (const snippet of extractSnippets(tree)) {
    batch.push(snippet);

    if (batch.length >= chunkSize) {
      yield batch;
      batch = [];  // 清空，允许 GC
    }
  }

  if (batch.length > 0) {
    yield batch;
  }
}

// 使用流式处理
for await (const snippets of streamIndexChunks(
  'large-file.ts'
)) {
  // 处理每个 chunk，然后忘记它
  await storeSnippets(snippets);
}
```

---

## 7. 常见问题与解决方案

### Q: 索引很慢，如何加速?

**A**: 多个优化策略

```yaml
# 方案 1: 禁用向量搜索 (最快)
disableIndexing: false
contextProviders:
  - name: codebase
    indexTypes: ["fullTextSearch", "snippets"]  # 跳过 LanceDbIndex

# 方案 2: 增加 chunk 大小
contextProviders:
  - name: codebase
    chunkSize: 4000              # 更大 chunks = 更快索引

# 方案 3: 排除不必要的目录
contextProviders:
  - name: codebase
    ignore: ["node_modules", ".git", "dist", "build"]
```

### Q: 搜索结果不相关，如何改进?

**A**: 调整搜索参数

```typescript
// 增加 nRetrieve (初始候选数)
const results = await index.retrieve({
  query: 'user authentication',
  nFinal: 20,
  nRetrieve: 100,              // 从 50 增加到 100
  bm25Threshold: -3.0,         // 降低阈值 (更多结果)
});

// 改进查询
// ❌ 不好
const query = 'foo';           // 太通用

// ✅ 更好
const query = 'authenticate user with JWT token';  // 具体查询
```

### Q: 自动补全延迟太高，如何优化?

**A**: 调整超时和 debounce

```yaml
# 方案 1: 更激进的超时
completionOptions:
  debounceDelay: 100           # 从 350ms 降到 100ms
  modelTimeout: 50             # 从 150ms 降到 50ms
  showWhateverWeHaveAtXMs: 100 # 提前返回部分结果

# 方案 2: 减少上下文
completionOptions:
  maxPromptTokens: 512         # 从 1024 降到 512
  prefixPercentage: 0.2        # 减少光标前的上下文

# 方案 3: 仅本地代码
completionOptions:
  onlyMyCode: true             # 排除外部库
```

### Q: LSP 导航不工作？

**A**: 检查配置和文件类型

```typescript
// 确保配置了语言服务
// VS Code: 需要安装对应的语言扩展
// IntelliJ: 内置语言支持
// CLI: 使用 Tree-Sitter 作为备选

// 如果 LSP 失败，使用 codebase index
async function jumpToDefinition(symbol: string) {
  try {
    // 首先尝试 LSP
    const lspDef = await ide.getLspDefinition(symbol);
    if (lspDef) return navigateTo(lspDef);
  } catch (e) {
    // 回退到搜索
    const results = await codebaseIndex.retrieve({
      query: symbol,
      nFinal: 1,
    });
    if (results.length > 0) return navigateTo(results[0]);
  }
}
```

### Q: 索引占用空间太大？

**A**: 清理或配置

```bash
# 查看索引大小
du -sh ~/.continue/

# 清理不需要的索引
rm ~/.continue/index.sqlite        # 删除 SQLite
rm -rf ~/.continue/lancedb/        # 删除向量 DB

# 配置排除大文件
contextProviders:
  - name: codebase
    maxFileSize: 1000000           # 1MB (跳过更大的)
    ignore:
      - "*.log"
      - "*.min.js"
      - "*.bundle.js"
```

---

## 8. 实战案例

### 案例 1: React 项目中的快速组件搜索

```typescript
// 自定义搜索函数
async function searchReactComponents(
  pattern: string
): Promise<ComponentInfo[]> {
  const results = await codebaseIndex.retrieve({
    query: `React component ${pattern}`,
    nFinal: 20,
  });

  // 过滤 TSX/JSX 文件
  const components = results
    .filter(r => /\.(tsx?|jsx?)$/.test(r.path))
    .map(r => ({
      name: extractComponentName(r.content),
      path: r.path,
      hooks: extractHooks(r.content),
      props: extractPropsInterface(r.content),
    }));

  return components;
}

// 在 VS Code 中使用
const components = await searchReactComponents('button');
// 返回:
// [
//   {
//     name: 'Button',
//     path: 'src/components/Button.tsx',
//     hooks: ['useState', 'useCallback'],
//     props: 'interface ButtonProps { ... }'
//   }
// ]
```

### 案例 2: 跨文件函数调用追踪

```typescript
// 追踪函数调用关系
async function traceCallChain(
  functionName: string,
  depth: number = 3
): Promise<CallChain> {
  // 1. 找到函数定义
  const definition = await codebaseIndex.retrieve({
    query: `function ${functionName}`,
    nFinal: 1,
  });

  if (!definition) return null;

  // 2. 找到调用该函数的地方
  const callers = await codebaseIndex.retrieve({
    query: functionName,
    nFinal: 20,
  });

  // 3. 递归追踪上级调用者
  const chain: CallChain = {
    function: functionName,
    definition: definition[0],
    callers: [],
  };

  if (depth > 0) {
    for (const caller of callers) {
      const callerName = extractFunctionName(caller.content);
      chain.callers.push(
        await traceCallChain(callerName, depth - 1)
      );
    }
  }

  return chain;
}

// 调用追踪
const chain = await traceCallChain('handleSubmit');
console.log(chain);
// 输出: handleSubmit <- onFormSubmit <- handleButtonClick <- ...
```

### 案例 3: API 端点自动补全

```typescript
// 为 API 调用自动补全
async function getApiCompletions(
  endpoint: string
): Promise<ApiCallCompletion[]> {
  // 搜索类似的 API 调用
  const results = await codebaseIndex.retrieve({
    query: `fetch ${endpoint} API call`,
  });

  // 提取请求方式、参数等
  const completions = results.map(r => ({
    code: r.content,
    method: extractHttpMethod(r.content),
    params: extractRequestParams(r.content),
    responseType: extractResponseType(r.content),
  }));

  return completions;
}

// 在编辑器中使用
const completions = await getApiCompletions('users');
// 返回所有 users API 调用的示例
```

---

## 9. 性能基准测试

```typescript
// 性能测试框架
async function benchmarkSearch() {
  const queries = [
    'authenticate user',
    'render component',
    'database query',
    'error handling',
  ];

  for (const query of queries) {
    const start = performance.now();

    const results = await codebaseIndex.retrieve({
      query,
      nFinal: 20,
    });

    const duration = performance.now() - start;

    console.log(`Query: "${query}" | Duration: ${duration}ms | Results: ${results.length}`);
  }
}

// 预期结果:
// Query: "authenticate user" | Duration: 45ms | Results: 20
// Query: "render component" | Duration: 38ms | Results: 20
// Query: "database query" | Duration: 52ms | Results: 15
// Query: "error handling" | Duration: 41ms | Results: 20
```

---

## 10. 最佳实践总结

| 最佳实践 | 说明 |
|---------|------|
| **使用混合搜索** | FTS + 向量搜索获得最佳结果 |
| **限制搜索范围** | 按目录/文件类型过滤提高速度 |
| **缓存热查询** | 常见查询结果缓存 5 分钟 |
| **调整 chunk 大小** | 大仓库用更大 chunks，小仓库精细度 |
| **定期清理索引** | 每月清理一次陈旧数据 |
| **监控索引大小** | 保持在 1-5GB 范围内 |
| **并行索引** | 多目录并行构建索引 |
| **使用 LSP** | 符号导航优先使用 LSP |
| **回退机制** | LSP 失败时回退到搜索 |
| **流式处理** | 大文件使用流式处理 |

---

*指南生成时间: 2025-12-05*
*适用于: Continue 最新版本*
*难度级别: 中级 (需要基础的编程知识)*

