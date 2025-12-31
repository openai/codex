# OpenCode 架构概览

本文档详细分析 OpenCode 的整体架构设计，包括包结构、核心模块、依赖关系和数据流。

---

## 目录

1. [项目结构](#1-项目结构)
2. [核心技术栈](#2-核心技术栈)
3. [模块架构](#3-模块架构)
4. [数据流](#4-数据流)
5. [实例管理](#5-实例管理)
6. [关键设计模式](#6-关键设计模式)

---

## 1. 项目结构

### 1.1 Monorepo 布局

```
opencode/
├── packages/                    # 核心包目录
│   ├── opencode/               # 主 CLI 应用 (36 个 src 子目录)
│   │   ├── src/
│   │   │   ├── agent/          # 代理系统
│   │   │   ├── auth/           # 认证
│   │   │   ├── bus/            # 事件总线
│   │   │   ├── cli/            # CLI 命令
│   │   │   ├── config/         # 配置系统
│   │   │   ├── file/           # 文件操作
│   │   │   ├── flag/           # Feature Flags
│   │   │   ├── global/         # 全局状态
│   │   │   ├── id/             # ID 生成
│   │   │   ├── lsp/            # LSP 集成
│   │   │   ├── mcp/            # MCP 集成
│   │   │   ├── permission/     # 权限管理
│   │   │   ├── plugin/         # 插件系统
│   │   │   ├── project/        # 项目实例
│   │   │   ├── provider/       # LLM Provider
│   │   │   ├── server/         # HTTP 服务器
│   │   │   ├── session/        # 会话管理
│   │   │   ├── shell/          # Shell 执行
│   │   │   ├── skill/          # 技能系统
│   │   │   ├── snapshot/       # 快照管理
│   │   │   ├── tool/           # 工具实现
│   │   │   └── util/           # 工具函数
│   │   └── package.json
│   │
│   ├── app/                    # Web/TUI 前端
│   ├── sdk/                    # JavaScript SDK
│   ├── plugin/                 # 插件 API 定义
│   ├── util/                   # 共享工具库
│   ├── ui/                     # UI 组件库
│   ├── enterprise/             # 企业版功能
│   └── web/                    # Web 应用
│
├── sdks/                       # 多语言 SDK
│   ├── python/
│   └── go/
│
├── infra/                      # 基础设施
├── specs/                      # API 规范
└── package.json                # 根配置
```

### 1.2 核心包职责

| 包名 | 职责 | 主要导出 |
|------|------|----------|
| `opencode` | CLI 应用核心 | 会话管理、工具执行、配置 |
| `@opencode-ai/sdk` | JavaScript 客户端 | `createOpencodeClient` |
| `@opencode-ai/plugin` | 插件 API | `Hooks`, `PluginInput`, `ToolDefinition` |
| `@opencode-ai/util` | 工具函数 | `NamedError`, 日志工具 |
| `app` | 前端应用 | TUI 和 Web UI |

---

## 2. 核心技术栈

### 2.1 运行时与构建

```
┌─────────────────────────────────────────────────────────────┐
│                       技术栈架构                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │     Bun      │    │  TypeScript  │    │    Turbo     │  │
│  │   Runtime    │    │    5.x       │    │   Monorepo   │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                   │                   │           │
│         ▼                   ▼                   ▼           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Package    │    │    Type      │    │    Build     │  │
│  │   Manager    │    │   Checking   │    │   Pipeline   │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `bun` | 1.x | 运行时、包管理、构建 |
| `hono` | 4.x | HTTP 服务器框架 |
| `ai` (Vercel AI SDK) | 4.x | LLM 集成、流式响应 |
| `zod` | 3.x | Schema 定义与验证 |
| `vscode-jsonrpc` | 8.x | LSP 通信协议 |
| `remeda` | 2.x | 函数式工具库 |
| `ink` | 4.x | React 终端 UI |
| `ulid` | 2.x | 唯一 ID 生成 |

---

## 3. 模块架构

### 3.1 核心模块依赖图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         模块依赖架构                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│     ┌─────────────┐          ┌─────────────┐                        │
│     │   CLI/TUI   │◀────────▶│   Server    │                        │
│     │  (cmd/*.ts) │          │ (Hono.js)   │                        │
│     └──────┬──────┘          └──────┬──────┘                        │
│            │                        │                                │
│            ▼                        ▼                                │
│     ┌──────────────────────────────────────┐                        │
│     │              Session                  │                        │
│     │  (prompt.ts, processor.ts, llm.ts)   │                        │
│     └─────────────────┬────────────────────┘                        │
│                       │                                              │
│       ┌───────────────┼───────────────┐                             │
│       ▼               ▼               ▼                             │
│  ┌─────────┐   ┌───────────┐   ┌───────────┐                       │
│  │  Agent  │   │   Tool    │   │  Provider │                       │
│  │(agent.ts)   │(registry) │   │(provider.ts)                       │
│  └────┬────┘   └─────┬─────┘   └─────┬─────┘                       │
│       │              │               │                              │
│       ▼              ▼               ▼                              │
│  ┌─────────┐   ┌───────────┐   ┌───────────┐                       │
│  │ Prompt  │   │   LSP     │   │    MCP    │                       │
│  │Templates│   │(server.ts)│   │(index.ts) │                       │
│  └─────────┘   └───────────┘   └───────────┘                       │
│                                                                      │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │                    共享层                                │    │
│     │  Config | Bus | Plugin | Permission | Instance | Flag   │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 模块职责详解

#### Session 模块 (核心)

**文件:** `packages/opencode/src/session/`

| 文件 | 职责 | 关键函数 |
|------|------|----------|
| `index.ts` | 会话 CRUD | `create`, `get`, `messages`, `updateMessage` |
| `prompt.ts` | 提示执行循环 | `prompt`, `loop`, `resolveTools` |
| `processor.ts` | 消息流处理 | `create`, `process` |
| `llm.ts` | LLM 流式调用 | `stream` |
| `message-v2.ts` | 消息类型定义 | `MessageV2`, `Part` 类型 |
| `system.ts` | 系统提示词 | `header`, `provider`, `environment`, `custom` |
| `compaction.ts` | 上下文压缩 | `isOverflow`, `prune`, `process` |
| `summary.ts` | 摘要生成 | `summarizeSession`, `summarizeMessage` |
| `revert.ts` | 回滚管理 | `cleanup`, `restore` |
| `retry.ts` | 重试策略 | `retryable`, `delay` |
| `status.ts` | 状态跟踪 | `set`, `get`, `Event` |

#### Agent 模块

**文件:** `packages/opencode/src/agent/agent.ts`

```typescript
// Agent.Info 完整结构
interface Info {
  name: string              // 唯一标识符
  description?: string      // 用途描述
  mode: "subagent" | "primary" | "all"  // 运行模式
  native?: boolean          // 是否内置
  hidden?: boolean          // 是否隐藏
  default?: boolean         // 是否默认
  temperature?: number      // 采样温度
  topP?: number            // Top-P 采样
  color?: string           // UI 颜色
  model?: { modelID: string; providerID: string }  // 指定模型
  prompt?: string          // 自定义系统提示词
  tools: Record<string, boolean>     // 工具白名单/黑名单
  options: Record<string, any>       // Provider 特定选项
  maxSteps?: number        // 最大迭代步数
  permission: {            // 权限配置
    edit: "allow" | "deny" | "ask"
    bash: Record<string, Permission>
    skill: Record<string, Permission>
    webfetch?: Permission
    doom_loop?: Permission
    external_directory?: Permission
  }
}
```

#### Tool 模块

**文件:** `packages/opencode/src/tool/`

| 工具 | 文件 | 功能 |
|------|------|------|
| `bash` | `bash.ts` | 命令执行 (tree-sitter 权限检查) |
| `read` | `read.ts` | 文件读取 (支持图片/PDF) |
| `edit` | `edit.ts` | 文件编辑 (diff 方式) |
| `write` | `write.ts` | 文件写入 |
| `glob` | `glob.ts` | 文件模式匹配 |
| `grep` | `grep.ts` | 内容搜索 (ripgrep) |
| `task` | `task.ts` | 子代理调用 |
| `webfetch` | `webfetch.ts` | URL 内容获取 |
| `websearch` | `websearch.ts` | Web 搜索 |
| `codesearch` | `codesearch.ts` | 代码搜索 (Exa) |
| `todo` | `todo.ts` | 任务跟踪 |
| `skill` | `skill.ts` | 技能调用 |
| `lsp` | `lsp.ts` | 语言服务器操作 |
| `batch` | `batch.ts` | 批量操作 (实验性) |

---

## 4. 数据流

### 4.1 用户输入到响应

```
用户输入
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ CLI/TUI/Server 入口                                            │
│   • 解析命令参数                                               │
│   • 验证会话状态                                               │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ SessionPrompt.prompt()                                         │
│   1. 创建用户消息 (createUserMessage)                          │
│   2. 解析文件引用 (@file)                                      │
│   3. 触发 chat.message 插件 Hook                               │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ SessionPrompt.loop()                                           │
│   while (!finished) {                                          │
│     1. 检查子任务 (subtask)                                    │
│     2. 检查压缩任务 (compaction)                               │
│     3. 检查上下文溢出                                          │
│     4. 插入提醒 (insertReminders)                              │
│     5. 解析工具 (resolveTools)                                 │
│     6. 调用 SessionProcessor.process()                         │
│   }                                                            │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ SessionProcessor.process()                                     │
│   1. 调用 LLM.stream()                                         │
│   2. 处理流式事件:                                             │
│      • text-delta → 更新文本部分                               │
│      • tool-call → 执行工具                                    │
│      • reasoning-delta → 更新推理部分                          │
│   3. 检测 Doom Loop (3+ 相同调用)                              │
│   4. 记录快照 (Snapshot.track)                                 │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ 工具执行                                                       │
│   1. Plugin.trigger("tool.execute.before")                     │
│   2. tool.execute(args, ctx)                                   │
│   3. Plugin.trigger("tool.execute.after")                      │
│   4. 更新 ToolPart 状态                                        │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ 响应完成                                                       │
│   1. 修剪旧工具输出 (prune)                                    │
│   2. 生成摘要 (summarize)                                      │
│   3. 发布事件 (Bus.publish)                                    │
│   4. 返回最终消息                                              │
└───────────────────────────────────────────────────────────────┘
```

### 4.2 消息部分 (Part) 类型

**文件:** `packages/opencode/src/session/message-v2.ts`

```typescript
// 11 种 Part 类型
type Part =
  | TextPart           // 文本内容
  | ReasoningPart      // 推理/思考
  | FilePart           // 文件附件
  | ToolPart           // 工具调用
  | StepStartPart      // 步骤开始
  | StepFinishPart     // 步骤完成
  | SnapshotPart       // 快照
  | PatchPart          // 文件差异
  | AgentPart          // 代理引用
  | RetryPart          // 重试
  | CompactionPart     // 压缩标记
  | SubtaskPart        // 子任务

// ToolPart 状态机
type ToolState =
  | { status: "pending"; input: {}; raw: string }
  | { status: "running"; input: any; time: { start: number }; title?: string; metadata?: any }
  | { status: "completed"; input: any; output: string; title: string; metadata: any; time: { start: number; end: number; compacted?: number } }
  | { status: "error"; input: any; error: string; metadata?: any; time: { start: number; end: number } }
```

---

## 5. 实例管理

### 5.1 Instance 模式

**文件:** `packages/opencode/src/project/instance.ts`

```typescript
export namespace Instance {
  // 当前工作目录
  export const directory: string = process.cwd()

  // Git 工作树根目录
  export const worktree: string = findGitRoot()

  // 项目信息
  export const project: Project.Info = detectProject()

  // 状态工厂 (带清理回调)
  export function state<T>(
    init: () => Promise<T>,
    cleanup?: (state: T) => Promise<void>
  ): () => Promise<T>

  // 实例清理
  export async function dispose(): Promise<void>
}
```

### 5.2 状态管理

```typescript
// 使用 Instance.state 创建懒加载单例
const toolState = Instance.state(async () => {
  // 初始化逻辑
  const custom = await loadCustomTools()
  return { custom }
}, async (state) => {
  // 清理逻辑
  await cleanup(state)
})

// 使用状态
const { custom } = await toolState()
```

---

## 6. 关键设计模式

### 6.1 Zod Schema 驱动

所有数据结构使用 Zod 定义，支持:
- 类型推导
- 运行时验证
- JSON Schema 生成
- 默认值处理

```typescript
// 配置 Schema 示例
export const Agent = z.object({
  model: z.string().optional(),
  temperature: z.number().optional(),
  prompt: z.string().optional(),
  tools: z.record(z.string(), z.boolean()).optional(),
  permission: z.object({
    edit: Permission.optional(),
    bash: z.union([Permission, z.record(z.string(), Permission)]).optional(),
  }).optional(),
}).catchall(z.any())

// 类型推导
export type Agent = z.infer<typeof Agent>
```

### 6.2 事件总线

**文件:** `packages/opencode/src/bus/index.ts`

```typescript
export namespace Bus {
  // 发布事件
  export async function publish<D extends BusEvent.Definition>(
    def: D,
    properties: z.output<D["properties"]>
  ): Promise<void>

  // 订阅单个事件
  export function subscribe<D extends BusEvent.Definition>(
    def: D,
    callback: (event: { type: string; properties: any }) => void
  ): () => void

  // 订阅所有事件
  export function subscribeAll(
    callback: (event: any) => void
  ): () => void
}
```

### 6.3 插件 Hook 系统

```typescript
// 可用 Hook 列表
type Hooks = {
  // 聊天相关
  "chat.message": (input, output) => Promise<void>
  "chat.params": (input, output) => Promise<void>

  // 工具相关
  "tool.execute.before": (input, output) => Promise<void>
  "tool.execute.after": (input, output) => Promise<void>

  // 实验性
  "experimental.chat.system.transform": (input, output) => Promise<void>
  "experimental.chat.messages.transform": (input, output) => Promise<void>
  "experimental.session.compacting": (input, output) => Promise<void>
  "experimental.text.complete": (input, output) => Promise<void>

  // 配置
  "config": (config) => Promise<void>

  // 事件
  "event": (input) => void
}
```

### 6.4 Provider 适配

**文件:** `packages/opencode/src/provider/provider.ts`

```typescript
// 30+ 内置 Provider
const BUNDLED_PROVIDERS = {
  "@ai-sdk/anthropic": createAnthropic,
  "@ai-sdk/openai": createOpenAI,
  "@ai-sdk/google": createGoogleGenerativeAI,
  "@ai-sdk/amazon-bedrock": createAmazonBedrock,
  "@ai-sdk/azure": createAzure,
  "@ai-sdk/cohere": createCohere,
  "@ai-sdk/deepinfra": createDeepInfra,
  "@ai-sdk/fireworks": createFireworks,
  "@ai-sdk/groq": createGroq,
  "@ai-sdk/mistral": createMistral,
  "@ai-sdk/perplexity": createPerplexity,
  "@ai-sdk/together": createTogether,
  "@ai-sdk/vertex": createVertex,
  "@ai-sdk/xai": createXai,
  // ... 更多
}
```

---

## 7. 与 codex 架构对比

| 方面 | opencode | codex |
|------|----------|-------|
| **语言** | TypeScript | Rust |
| **运行时** | Bun | Tokio |
| **模块化** | ESM 包 | Cargo crates |
| **状态管理** | Instance.state (懒加载) | Arc<RwLock<T>> |
| **事件系统** | Bus (发布/订阅) | mpsc channels |
| **错误处理** | NamedError + Zod | CodexErr / anyhow |
| **配置** | JSONC + 合并 | TOML + 分层 |
| **LLM 集成** | Vercel AI SDK | 自实现 adapter |

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
