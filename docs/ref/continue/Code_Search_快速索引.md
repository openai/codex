# Continue Code Search 快速索引

## 📚 文档体系

Continue Code Search 文档分为三个层级：

```
Code_Search_分析报告.md (深度分析)
  └─ 完整的技术细节和架构分析

Code_Search_实现指南.md (实战指南)
  └─ 代码示例、配置、最佳实践

Code_Search_快速索引.md (本文档)
  └─ 快速导航和查询表
```

---

## 🎯 快速查询表

### 我想了解...

| 需求 | 章节 | 文档 |
|------|------|------|
| **Code Indexing (索引系统)** |
| 索引是如何工作的 | 1.2 索引更新流程 | 分析报告 |
| 跨分支索引重用 | 1.2 步骤 3 | 分析报告 |
| 索引存储位置 | 1.3 存储架构 | 分析报告 |
| 索引性能如何 | 1.4 性能指标 | 分析报告 |
| **LSP Integration (LSP 集成)** |
| LSP 当前状态 | 2.1 LSP 状态 | 分析报告 |
| 启用 LSP | Section 2 | 实现指南 |
| IDE 提供的功能 | 2.2, 2.3 | 分析报告 + 实现指南 |
| **AST Analysis (AST 分析)** |
| Tree-Sitter 支持 | 3.1 集成 | 分析报告 |
| 支持的语言 | 3.2 26+ 语言 | 分析报告 |
| 提取函数签名 | Section 3.1 | 实现指南 |
| 类结构分析 | Section 3.2 | 实现指南 |
| 依赖关系分析 | Section 3.3 | 实现指南 |
| **Search Implementation (搜索实现)** |
| 4 层索引架构 | 4.1 架构 | 分析报告 |
| 代码片段搜索 | 4.1 Index 1 | 分析报告 |
| 全文搜索 (FTS) | 4.1 Index 2 | 分析报告 |
| 代码块搜索 | 4.1 Index 3 | 分析报告 |
| 向量搜索 (语义) | 4.1 Index 4 | 分析报告 |
| 智能代码分块 | 4.2 分块算法 | 分析报告 |
| **Implementation (实现)** |
| FTS 查询代码 | Section 4.1 | 实现指南 |
| 向量搜索代码 | Section 4.2 | 实现指南 |
| 混合搜索代码 | Section 4.3 | 实现指南 |
| **Performance (性能)** |
| 查询性能指标 | 5.1 性能表 | 分析报告 |
| 自动补全配置 | 5.2 配置 | 分析报告 |
| 索引优化 | Section 6.1 | 实现指南 |
| 搜索优化 | Section 6.2 | 实现指南 |

---

## 🔍 按功能查询

### 索引管理

```
启用/禁用索引          → 实现指南 1.1
配置 Embeddings        → 实现指南 1.2
多仓库配置            → 实现指南 1.4
索引存储位置          → 实现指南 1.3
清除索引              → 实现指南 1.3

关键参数:
- disableIndexing
- selectedModelByRole.embed
- chunkSize, maxChunkTokens
```

### 符号导航 (LSP)

```
启用 LSP                → 实现指南 2.1
LSP 功能利用           → 实现指南 2.2
符号导航流程           → 实现指南 2.3

当前限制:
- LSP 默认禁用
- IDE 提供 LSP 服务
- 可作为回退机制
```

### AST 分析

```
提取函数签名            → 实现指南 3.1
提取类结构            → 实现指南 3.2
依赖关系分析          → 实现指南 3.3

支持: 26+ 语言 (Tree-Sitter)
位置: core/tag-qry/*.scm
```

### 搜索操作

```
全文搜索 (FTS)         → 实现指南 4.1
向量搜索 (语义)        → 实现指南 4.2
混合搜索 (FTS+向量)    → 实现指南 4.3
条件搜索 (过滤)        → 实现指南 4.4

配置:
- nFinal: 返回结果数 (默认 20)
- nRetrieve: 初始候选数 (默认 50)
- bm25Threshold: FTS 阈值 (默认 -2.5)
```

### 自动补全

```
配置参数                → 实现指南 5.1
构建上下文            → 实现指南 5.2
片段排序              → 实现指南 5.3

关键参数:
- debounceDelay: 延迟触发 (默认 350ms)
- modelTimeout: 模型超时 (默认 150ms)
- maxPromptTokens: 上下文大小 (默认 1024)
```

### 性能优化

```
索引优化                → 实现指南 6.1
搜索优化               → 实现指南 6.2
内存优化               → 实现指南 6.3
常见问题               → 实现指南 Section 7

优化工具:
- 批处理: 200 文件/批
- 流式处理: 大文件处理
- 缓存: 热查询结果缓存
```

---

## 📊 架构速查

### 4 层索引架构

```
Layer 1: CodeSnippetsIndex (代码片段)
├─ 存储: SQLite
├─ 查询: SQL 精确匹配
└─ 用途: 顶级符号导航

Layer 2: FullTextSearchCodebaseIndex (全文搜索)
├─ 算法: BM25
├─ 存储: SQLite FTS5
└─ 用途: 关键词搜索

Layer 3: ChunkCodebaseIndex (代码块)
├─ 存储: SQLite
├─ 用途: Embeddings 输入
└─ 特性: 智能分块

Layer 4: LanceDbIndex (向量嵌入)
├─ 存储: LanceDB (Rust 向量 DB)
├─ 算法: 向量相似度
└─ 用途: 语义搜索
```

### 搜索流程

```
用户查询
    ↓
文本预处理 (词干还原、去停用词)
    ↓
┌─ FTS 路径 (全文搜索)
│  └─ 三元组分词 → BM25 排序 → 按阈值过滤
├─ Vector 路径 (语义搜索)
│  └─ Embed 查询 → LanceDB 相似度 → 按距离排序
└─ Snippet 路径 (符号搜索)
   └─ SQL 匹配 → 精确结果
    ↓
合并结果
    ↓
Jaccard 相似度重排
    ↓
去重重叠片段
    ↓
Token 预算打包
    ↓
返回前 20 结果
```

---

## 🔧 配置速查表

### 最常见配置

```yaml
# 启用/禁用索引
disableIndexing: false

# 选择 embeddings 模型
selectedModelByRole:
  embed: openai

# 配置 code search provider
contextProviders:
  - name: codebase
    disabled: false
    dependsOnIndexing:
      - embeddings
      - fullTextSearch

# 自动补全选项
tabAutocompleteOptions:
  maxPromptTokens: 1024
  debounceDelay: 350
  modelTimeout: 150
```

### 性能调优配置

```yaml
# 高性能硬件
completionOptions:
  maxPromptTokens: 2048
  debounceDelay: 200

# 低功耗硬件
completionOptions:
  maxPromptTokens: 512
  debounceDelay: 500
  onlyMyCode: true
```

---

## 📈 性能指标速查

| 操作 | 延迟 | 说明 |
|------|------|------|
| **全文搜索** | <1ms | SQLite FTS5 |
| **向量搜索** | 100-500ms | LanceDB |
| **符号查询** | <1ms | SQLite 精确 |
| **自动补全** | 150-350ms | 包括网络延迟 |
| **索引一个仓库** | 增量 | 内容哈希避免重复 |

---

## ⚠️ 已知限制和注意事项

| 限制 | 解决方案 | 优先级 |
|------|----------|--------|
| **LSP 默认禁用** | 代码片段索引仍可用 | 中 |
| **向量搜索需 embeddings API** | 使用本地模型或 FTS | 低 |
| **CPU 不兼容系统** | 回退到 SQLite JSON | 低 |
| **大文件不索引** | 手动配置大小限制 | 中 |
| **跨语言导航** | 各语言单独索引 | 低 |

---

## 🎓 学习路径

### 初级 (理解基础)

1. 📖 分析报告 Section 1 → 了解索引系统
2. 📖 分析报告 Section 4.1 → 了解搜索架构
3. 📖 实现指南 Section 4 → 简单搜索示例

### 中级 (能够配置和使用)

1. 📖 分析报告 全部 → 深度理解
2. 📖 实现指南 Section 1-5 → 配置和使用
3. 💻 实践: 配置自己的项目

### 高级 (能够扩展和优化)

1. 📖 分析报告 Section 8-9 → 扩展点
2. 📖 实现指南 Section 6 → 性能优化
3. 💻 实践: 实现自定义索引或分块器

---

## 🚀 常用代码片段

### 启用 LSP (开发者)

```typescript
// core/autocomplete/snippets/getAllSnippets.ts
const IDE_SNIPPETS_ENABLED = true;  // 改为 true
```

### 全文搜索

```typescript
const results = await ftsIndex.retrieve({
  query: 'handleClick',
  nFinal: 20,
  bm25Threshold: -2.5,
});
```

### 向量搜索

```typescript
const results = await vectorIndex.retrieve(
  'authenticate user',
  20,
  [{ directory: '.', branch: 'main' }]
);
```

### 混合搜索

```typescript
const [fts, vec] = await Promise.all([
  ftsIndex.retrieve({ query, nFinal: 50 }),
  vectorIndex.retrieve(query, 50),
]);

const merged = deduplicateAndRank([...fts, ...vec]);
```

---

## ❓ FAQ 快速查找

| 问题 | 答案位置 |
|------|---------|
| 索引为什么慢? | 实现指南 Q1 |
| 搜索结果不好? | 实现指南 Q2 |
| 自动补全延迟高? | 实现指南 Q3 |
| LSP 不工作? | 实现指南 Q4 |
| 索引占用空间大? | 实现指南 Q5 |

---

## 📞 相关资源

| 资源 | 位置 |
|------|------|
| **主 Code Search 分析报告** | Code_Search_分析报告.md |
| **实现和配置指南** | Code_Search_实现指南.md |
| **快速索引 (本文件)** | Code_Search_快速索引.md |
| **主架构分析** | 架构分析.md |
| **模块依赖关系** | 模块依赖关系.md |
| **技术栈详解** | 技术栈与特性.md |

---

## 📋 关键数字速记

```
索引层数:    4 (snippets, FTS, chunks, vector)
语言支持:    26+
内置工具:    22
Context 源:  30+
LLM 提供商:  75+

性能:
- FTS 延迟:    <1ms
- Vector 延迟: 100-500ms
- 补全延迟:    150-350ms

默认参数:
- nFinal:           20
- nRetrieve:        50
- bm25Threshold:    -2.5
- debounceDelay:    350ms
- modelTimeout:     150ms
- maxPromptTokens:  1024
```

---

## 🎯 按使用场景推荐

### 场景: 快速项目审查
```
推荐方案: FTS + 代码片段索引
禁用: 向量搜索 (性能关键)
配置: nFinal=10, debounceDelay=200
```

### 场景: 语义代码搜索
```
推荐方案: 混合搜索 (FTS + 向量)
模型: OpenAI embeddings 或本地
配置: nFinal=20, nRetrieve=50
```

### 场景: 符号导航
```
推荐方案: LSP + 代码片段索引
配置: IDE_SNIPPETS_ENABLED=true
回退: 搜索如果 LSP 失败
```

### 场景: 大型单体项目
```
推荐方案: 增量索引 + 缓存
优化:
  - 增加 chunkSize
  - 启用并行索引
  - 缓存热查询
```

---

## ✅ 检查清单

使用 Code Search 时的检查项：

- [ ] 索引已启用 (`disableIndexing: false`)
- [ ] Embeddings 模型已配置
- [ ] Codebase context provider 已启用
- [ ] 索引初始化完成 (检查 `~/.continue/index.sqlite`)
- [ ] 自动补全延迟在可接受范围内 (<350ms)
- [ ] 大文件已排除或限制
- [ ] 不必要的目录已忽略 (node_modules, dist 等)

---

**索引生成时间**: 2025-12-05
**适用于**: Continue 最新版本
**维护者**: Code Search 分析团队

欢迎跳转到相应的详细文档获取更多信息！
