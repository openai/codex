# OpenCode 核心工具系统

本文档详细分析 OpenCode 的工具系统设计，包括工具定义、注册表、权限控制和自定义扩展。

---

## 目录

1. [系统概览](#1-系统概览)
2. [工具定义接口](#2-工具定义接口)
3. [内置工具](#3-内置工具)
4. [工具注册表](#4-工具注册表)
5. [权限控制](#5-权限控制)
6. [自定义工具](#6-自定义工具)
7. [MCP 工具集成](#7-mcp-工具集成)
8. [与 codex 对比](#8-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       工具系统架构                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Tool Registry                             │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │                  Built-in Tools                         │ │  │
│  │  │  bash | read | edit | write | glob | grep | task       │ │  │
│  │  │  webfetch | websearch | codesearch | todo | skill | lsp│ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  │                                                               │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │                  Custom Tools                           │ │  │
│  │  │  • config/tool/*.{ts,js}                               │ │  │
│  │  │  • Plugin.tool                                         │ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  │                                                               │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │                  MCP Tools                              │ │  │
│  │  │  • Local MCP Servers                                   │ │  │
│  │  │  • Remote MCP Servers                                  │ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Permission Filter                         │  │
│  │    Agent.permission → Tool Enablement                         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Tool Execution                            │  │
│  │  1. Plugin.trigger("tool.execute.before")                    │  │
│  │  2. tool.execute(args, ctx)                                  │  │
│  │  3. Plugin.trigger("tool.execute.after")                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心文件

| 文件 | 职责 |
|------|------|
| `src/tool/tool.ts` | 工具接口定义 |
| `src/tool/registry.ts` | 工具注册与发现 |
| `src/tool/*.ts` | 内置工具实现 |
| `src/session/prompt.ts` | 工具解析与执行 |

---

## 2. 工具定义接口

### 2.1 Tool.Info 接口

**文件:** `packages/opencode/src/tool/tool.ts`

```typescript
export namespace Tool {
  export interface Context<M extends Metadata = Metadata> {
    sessionID: string              // 当前会话 ID
    messageID: string              // 当前消息 ID
    agent: string                  // 当前代理名称
    abort: AbortSignal             // 中止信号
    callID?: string                // 工具调用 ID
    extra?: { [key: string]: any } // 额外上下文
    metadata(input: {              // 更新元数据回调
      title?: string
      metadata?: M
    }): void
  }

  export interface InitContext {
    agent?: Agent.Info             // 代理配置
  }

  export interface Info<
    Parameters extends z.ZodType,
    M extends Metadata = Metadata
  > {
    id: string                     // 工具唯一标识
    init: (ctx?: InitContext) => Promise<{
      description: string          // 工具描述
      parameters: Parameters       // Zod 参数 Schema
      execute(
        args: z.infer<Parameters>,
        ctx: Context
      ): Promise<{
        title: string              // 执行标题
        metadata: M                // 元数据
        output: string             // 输出内容
        attachments?: MessageV2.FilePart[]  // 附件
      }>
      formatValidationError?(error: z.ZodError): string
    }>
  }

  export function define<
    Parameters extends z.ZodType,
    Result extends Metadata
  >(
    id: string,
    init: Info<Parameters, Result>["init"]
  ): Info<Parameters, Result>
}
```

### 2.2 工具定义示例

```typescript
import { Tool } from "./tool"
import z from "zod"

export const MyTool = Tool.define("my-tool", async () => {
  return {
    description: "A custom tool that does something useful",
    parameters: z.object({
      input: z.string().describe("The input to process"),
      options: z.object({
        flag: z.boolean().optional().describe("Optional flag"),
      }).optional(),
    }),

    async execute(args, ctx) {
      // 更新进度
      ctx.metadata({
        title: "Processing...",
        metadata: { progress: 50 },
      })

      // 检查中止
      if (ctx.abort.aborted) {
        throw new Error("Aborted")
      }

      // 执行逻辑
      const result = await doSomething(args.input)

      return {
        title: "Completed",
        metadata: { processed: true },
        output: result,
      }
    },
  }
})
```

---

## 3. 内置工具

### 3.1 工具列表

| 工具 | ID | 功能 | 关键特性 |
|------|-----|------|---------|
| Bash | `bash` | 命令执行 | tree-sitter 权限检查 |
| Read | `read` | 文件读取 | 支持图片/PDF/Notebook |
| Edit | `edit` | 文件编辑 | diff 方式修改 |
| Write | `write` | 文件写入 | 创建/覆盖 |
| Glob | `glob` | 文件匹配 | 模式搜索 |
| Grep | `grep` | 内容搜索 | ripgrep 后端 |
| Task | `task` | 子代理调用 | 创建子会话 |
| WebFetch | `webfetch` | URL 获取 | HTML 转 Markdown |
| WebSearch | `websearch` | 网页搜索 | Exa/DuckDuckGo |
| CodeSearch | `codesearch` | 代码搜索 | Exa 专用 |
| TodoWrite | `todowrite` | 任务写入 | 任务跟踪 |
| TodoRead | `todoread` | 任务读取 | 任务列表 |
| Skill | `skill` | 技能调用 | 自定义命令 |
| LSP | `lsp` | 语言服务器 | 实验性功能 |
| Batch | `batch` | 批量操作 | 实验性功能 |

### 3.2 Bash 工具

**文件:** `packages/opencode/src/tool/bash.ts`

```typescript
export const BashTool = Tool.define("bash", async () => ({
  description: `Executes a given bash command...`,
  parameters: z.object({
    command: z.string().describe("The command to execute"),
    description: z.string().optional().describe("Brief description"),
    timeout: z.number().optional().describe("Timeout in ms"),
    run_in_background: z.boolean().optional(),
  }),

  async execute(args, ctx) {
    // 1. 权限检查 (tree-sitter 解析)
    const parsed = await parseBashCommand(args.command)
    const permission = await checkPermission(parsed, ctx.agent)

    if (permission === "deny") {
      throw new Permission.RejectedError(...)
    }
    if (permission === "ask") {
      await Permission.ask({
        type: "bash",
        pattern: args.command,
        sessionID: ctx.sessionID,
        // ...
      })
    }

    // 2. 执行命令
    const proc = Bun.spawn({
      cmd: [shell, "-c", args.command],
      cwd: Instance.directory,
      timeout: args.timeout ?? 120000,
    })

    // 3. 收集输出
    const stdout = await Bun.readableStreamToText(proc.stdout)
    const stderr = await Bun.readableStreamToText(proc.stderr)

    return {
      title: args.description ?? args.command.slice(0, 50),
      metadata: { exitCode: proc.exitCode },
      output: formatOutput(stdout, stderr),
    }
  },
}))
```

### 3.3 Read 工具

**文件:** `packages/opencode/src/tool/read.ts`

```typescript
export const ReadTool = Tool.define("read", async () => ({
  description: `Reads a file from the local filesystem...`,
  parameters: z.object({
    file_path: z.string().describe("Absolute path to the file"),
    offset: z.number().optional().describe("Start line (1-based)"),
    limit: z.number().optional().describe("Number of lines"),
  }),

  async execute(args, ctx) {
    const file = Bun.file(args.file_path)
    const stat = await file.stat()

    // 1. 检查文件类型
    if (stat.isDirectory()) {
      return { output: "Cannot read directory" }
    }

    const mime = detectMime(args.file_path)

    // 2. 处理图片
    if (mime.startsWith("image/")) {
      const data = await file.arrayBuffer()
      return {
        output: "Image file",
        attachments: [{
          type: "file",
          mime,
          url: `data:${mime};base64,${Buffer.from(data).toString("base64")}`,
          filename: path.basename(args.file_path),
        }],
      }
    }

    // 3. 处理 PDF
    if (mime === "application/pdf") {
      const pages = await extractPdfPages(file)
      return { output: pages.join("\n\n---\n\n") }
    }

    // 4. 处理 Notebook
    if (args.file_path.endsWith(".ipynb")) {
      const notebook = await parseNotebook(file)
      return { output: formatNotebook(notebook) }
    }

    // 5. 处理文本文件
    const content = await file.text()
    const lines = content.split("\n")
    const start = args.offset ?? 0
    const end = args.limit ? start + args.limit : lines.length
    const selected = lines.slice(start, end)

    return {
      title: path.basename(args.file_path),
      metadata: { lines: selected.length },
      output: selected.map((l, i) =>
        `${(start + i + 1).toString().padStart(6)}→${l}`
      ).join("\n"),
    }
  },
}))
```

### 3.4 Edit 工具

**文件:** `packages/opencode/src/tool/edit.ts`

```typescript
export const EditTool = Tool.define("edit", async () => ({
  description: `Performs exact string replacements in files...`,
  parameters: z.object({
    file_path: z.string().describe("Absolute path to the file"),
    old_string: z.string().describe("Text to replace"),
    new_string: z.string().describe("Replacement text"),
    replace_all: z.boolean().optional().default(false),
  }),

  async execute(args, ctx) {
    // 1. 权限检查
    const permission = await checkEditPermission(ctx.agent)
    if (permission === "deny") {
      throw new Permission.RejectedError(...)
    }

    // 2. 读取文件
    const file = Bun.file(args.file_path)
    const content = await file.text()

    // 3. 验证 old_string 存在且唯一
    const occurrences = countOccurrences(content, args.old_string)
    if (occurrences === 0) {
      throw new Error(`old_string not found in file`)
    }
    if (occurrences > 1 && !args.replace_all) {
      throw new Error(`old_string found ${occurrences} times, use replace_all`)
    }

    // 4. 执行替换
    const newContent = args.replace_all
      ? content.replaceAll(args.old_string, args.new_string)
      : content.replace(args.old_string, args.new_string)

    // 5. 写入文件
    await Bun.write(args.file_path, newContent)

    // 6. 触发 Hook
    await Plugin.trigger("file.edited", { path: args.file_path }, {})

    return {
      title: `Edited ${path.basename(args.file_path)}`,
      metadata: { replaced: args.replace_all ? occurrences : 1 },
      output: generateDiff(content, newContent),
    }
  },
}))
```

---

## 4. 工具注册表

### 4.1 注册表结构

**文件:** `packages/opencode/src/tool/registry.ts`

```typescript
export namespace ToolRegistry {
  // 状态 (懒加载)
  export const state = Instance.state(async () => {
    const custom: Tool.Info[] = []
    const glob = new Bun.Glob("tool/*.{js,ts}")

    // 1. 从配置目录加载自定义工具
    for (const dir of await Config.directories()) {
      for await (const match of glob.scan({
        cwd: dir,
        absolute: true,
        followSymlinks: true,
      })) {
        const mod = await import(match)
        for (const [id, def] of Object.entries<ToolDefinition>(mod)) {
          custom.push(fromPlugin(id, def))
        }
      }
    }

    // 2. 从插件加载工具
    const plugins = await Plugin.list()
    for (const plugin of plugins) {
      for (const [id, def] of Object.entries(plugin.tool ?? {})) {
        custom.push(fromPlugin(id, def))
      }
    }

    return { custom }
  })

  // 注册新工具
  export async function register(tool: Tool.Info) {
    const { custom } = await state()
    const idx = custom.findIndex(t => t.id === tool.id)
    if (idx >= 0) {
      custom.splice(idx, 1, tool)
    } else {
      custom.push(tool)
    }
  }

  // 获取所有工具
  async function all(): Promise<Tool.Info[]> {
    const custom = await state().then(x => x.custom)
    const config = await Config.get()

    return [
      InvalidTool,
      BashTool,
      ReadTool,
      GlobTool,
      GrepTool,
      EditTool,
      WriteTool,
      TaskTool,
      WebFetchTool,
      TodoWriteTool,
      TodoReadTool,
      WebSearchTool,
      CodeSearchTool,
      SkillTool,
      ...(Flag.OPENCODE_EXPERIMENTAL_LSP_TOOL ? [LspTool] : []),
      ...(config.experimental?.batch_tool ? [BatchTool] : []),
      ...custom,
    ]
  }
}
```

### 4.2 工具初始化

**文件:** `packages/opencode/src/tool/registry.ts:117-137`

```typescript
export async function tools(
  providerID: string,
  agent?: Agent.Info
) {
  const tools = await all()

  const result = await Promise.all(
    tools
      .filter(t => {
        // 过滤特殊工具
        if (t.id === "codesearch" || t.id === "websearch") {
          return providerID === "opencode" || Flag.OPENCODE_ENABLE_EXA
        }
        return true
      })
      .map(async t => ({
        id: t.id,
        ...(await t.init({ agent })),
      }))
  )

  return result
}
```

---

## 5. 权限控制

### 5.1 权限过滤

**文件:** `packages/opencode/src/tool/registry.ts:139-160`

```typescript
export async function enabled(
  agent: Agent.Info
): Promise<Record<string, boolean>> {
  const result: Record<string, boolean> = {}

  // 编辑权限
  if (agent.permission.edit === "deny") {
    result["edit"] = false
    result["write"] = false
  }

  // Bash 权限
  if (agent.permission.bash["*"] === "deny" &&
      Object.keys(agent.permission.bash).length === 1) {
    result["bash"] = false
  }

  // WebFetch 权限
  if (agent.permission.webfetch === "deny") {
    result["webfetch"] = false
    result["codesearch"] = false
    result["websearch"] = false
  }

  // Skill 权限
  if (agent.permission.skill["*"] === "deny" &&
      Object.keys(agent.permission.skill).length === 1) {
    result["skill"] = false
  }

  return result
}
```

### 5.2 工具解析

**文件:** `packages/opencode/src/session/prompt.ts:572-648`

```typescript
async function resolveTools(input: {
  agent: Agent.Info
  model: Provider.Model
  sessionID: string
  tools?: Record<string, boolean>
  processor: SessionProcessor.Info
}) {
  const tools: Record<string, AITool> = {}

  // 1. 合并工具启用状态
  const enabledTools = pipe(
    input.agent.tools,                       // 代理配置
    mergeDeep(await ToolRegistry.enabled(input.agent)),  // 权限过滤
    mergeDeep(input.tools ?? {}),            // 调用时覆盖
  )

  // 2. 初始化每个工具
  for (const item of await ToolRegistry.tools(
    input.model.providerID,
    input.agent
  )) {
    // 检查是否启用
    if (Wildcard.all(item.id, enabledTools) === false) continue

    // 转换 Schema
    const schema = ProviderTransform.schema(
      input.model,
      z.toJSONSchema(item.parameters)
    )

    // 创建 AI SDK 工具
    tools[item.id] = tool({
      id: item.id,
      description: item.description,
      inputSchema: jsonSchema(schema),

      async execute(args, options) {
        // 插件 Hook: before
        await Plugin.trigger("tool.execute.before", {
          tool: item.id,
          sessionID: input.sessionID,
          callID: options.toolCallId,
        }, { args })

        // 执行工具
        const result = await item.execute(args, {
          sessionID: input.sessionID,
          abort: options.abortSignal!,
          messageID: input.processor.message.id,
          callID: options.toolCallId,
          agent: input.agent.name,
          metadata: async (val) => {
            // 实时更新元数据
            await Session.updatePart({
              ...match,
              state: { ...val, status: "running" },
            })
          },
        })

        // 插件 Hook: after
        await Plugin.trigger("tool.execute.after", {
          tool: item.id,
          sessionID: input.sessionID,
          callID: options.toolCallId,
        }, result)

        return result
      },
    })
  }

  return tools
}
```

---

## 6. 自定义工具

### 6.1 配置目录工具

```typescript
// ~/.opencode/tool/my-tool.ts
import type { ToolDefinition } from "@opencode-ai/plugin"
import z from "zod"

export default {
  description: "My custom tool",
  args: {
    input: z.string().describe("Input value"),
  },
  async execute(args, ctx) {
    return `Processed: ${args.input}`
  },
} satisfies ToolDefinition
```

### 6.2 插件工具

```typescript
// my-plugin/index.ts
import type { PluginInput, Plugin } from "@opencode-ai/plugin"

export default async function(input: PluginInput): Promise<Plugin> {
  return {
    tool: {
      "my-tool": {
        description: "Tool from plugin",
        args: {
          data: z.object({ key: z.string() }),
        },
        async execute(args, ctx) {
          return JSON.stringify(args.data)
        },
      },
    },
  }
}
```

---

## 7. MCP 工具集成

### 7.1 MCP 工具加载

**文件:** `packages/opencode/src/session/prompt.ts:649-714`

```typescript
// 加载 MCP 工具
for (const [key, item] of Object.entries(await MCP.tools())) {
  if (Wildcard.all(key, enabledTools) === false) continue

  const execute = item.execute
  if (!execute) continue

  // 包装执行函数
  item.execute = async (args, opts) => {
    // 插件 Hook
    await Plugin.trigger("tool.execute.before", {
      tool: key,
      sessionID: input.sessionID,
      callID: opts.toolCallId,
    }, { args })

    const result = await execute(args, opts)

    await Plugin.trigger("tool.execute.after", {
      tool: key,
      sessionID: input.sessionID,
      callID: opts.toolCallId,
    }, result)

    // 处理 MCP 结果格式
    const textParts: string[] = []
    const attachments: MessageV2.FilePart[] = []

    for (const contentItem of result.content) {
      if (contentItem.type === "text") {
        textParts.push(contentItem.text)
      } else if (contentItem.type === "image") {
        attachments.push({
          type: "file",
          mime: contentItem.mimeType,
          url: `data:${contentItem.mimeType};base64,${contentItem.data}`,
        })
      }
    }

    return {
      title: "",
      metadata: result.metadata ?? {},
      output: textParts.join("\n\n"),
      attachments,
    }
  }

  tools[key] = item
}
```

### 7.2 MCP 配置

```jsonc
{
  "mcp": {
    "my-server": {
      "type": "local",
      "command": ["node", "my-mcp-server.js"],
      "environment": { "KEY": "value" },
      "timeout": 5000
    },
    "remote-server": {
      "type": "remote",
      "url": "https://mcp.example.com",
      "headers": { "Authorization": "Bearer xxx" }
    }
  }
}
```

---

## 8. 与 codex 对比

### 8.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **定义方式** | `Tool.define` + Zod | `ToolSpec` trait |
| **注册表** | 动态加载 | `build_specs()` |
| **权限检查** | Agent.permission | (待实现) |
| **插件工具** | 配置目录 + 插件包 | - |
| **MCP 集成** | 完整支持 | - |
| **执行 Hook** | before/after | - |

### 8.2 codex 借鉴建议

1. **权限模式**: 实现 Bash 命令模式匹配

```rust
struct BashPermission {
    patterns: HashMap<String, Permission>,
}

impl BashPermission {
    fn check(&self, command: &str) -> Permission {
        for (pattern, perm) in &self.patterns {
            if minimatch(command, pattern) {
                return *perm;
            }
        }
        Permission::Ask
    }
}
```

2. **动态注册**: 支持运行时注册工具

```rust
impl ToolRegistry {
    fn register(&mut self, tool: Box<dyn ToolSpec>) {
        self.tools.insert(tool.name(), tool);
    }
}
```

3. **执行 Hook**: 添加工具执行前后 Hook

```rust
trait ToolHook {
    async fn before(&self, tool: &str, args: &Value) -> Result<()>;
    async fn after(&self, tool: &str, result: &ToolResult) -> Result<()>;
}
```

### 8.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/tool/tool.ts` | `core/src/tools/spec.rs` |
| `src/tool/registry.ts` | `core/src/tools/mod.rs` |
| `src/tool/bash.ts` | `core/src/tools/handlers/shell.rs` |
| `src/tool/read.ts` | `core/src/tools/handlers/read_file.rs` |
| `src/tool/edit.ts` | `core/src/tools/handlers/edit.rs` |

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
