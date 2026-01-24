# OpenCode 子代理系统

本文档详细分析 OpenCode 的代理 (Agent) 系统设计，包括代理定义、模式、权限、Task Tool 调用机制和会话隔离。

---

## 目录

1. [系统概览](#1-系统概览)
2. [代理定义与模式](#2-代理定义与模式)
3. [原生代理](#3-原生代理)
4. [Task Tool 调用机制](#4-task-tool-调用机制)
5. [会话隔离](#5-会话隔离)
6. [权限系统](#6-权限系统)
7. [代理配置与扩展](#7-代理配置与扩展)
8. [与 codex 对比](#8-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       OpenCode 代理系统                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Primary Agents                            │   │
│  │    ┌─────────┐  ┌─────────┐  ┌──────────────────────┐      │   │
│  │    │  build  │  │  plan   │  │   自定义 primary     │      │   │
│  │    │(default)│  │(只读)   │  │   (用户配置)         │      │   │
│  │    └────┬────┘  └────┬────┘  └──────────────────────┘      │   │
│  └─────────│────────────│───────────────────────────────────────┘   │
│            │            │                                            │
│            ▼            ▼                                            │
│  ┌──────────────────────────────────────┐                           │
│  │           Task Tool 调用              │                           │
│  │     (创建子会话, 执行子代理)          │                           │
│  └──────────────────────────────────────┘                           │
│            │                                                         │
│            ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Subagents                                 │   │
│  │    ┌─────────┐  ┌─────────┐  ┌──────────────────────┐      │   │
│  │    │ general │  │ explore │  │   自定义 subagent    │      │   │
│  │    │(多步骤) │  │(探索)   │  │   (用户配置)         │      │   │
│  │    └─────────┘  └─────────┘  └──────────────────────┘      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  Specialized Agents (Hidden)                 │   │
│  │    ┌───────────┐  ┌─────────┐  ┌─────────┐                 │   │
│  │    │compaction │  │  title  │  │ summary │                 │   │
│  │    │(压缩)     │  │(标题)   │  │(摘要)   │                 │   │
│  │    └───────────┘  └─────────┘  └─────────┘                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心概念

| 概念 | 说明 |
|------|------|
| **Primary Agent** | 可被用户直接选择的主代理 (build, plan) |
| **Subagent** | 只能由其他代理通过 Task Tool 调用 |
| **Mode** | `primary` / `subagent` / `all` |
| **Task Tool** | 子代理调用工具，创建隔离会话 |
| **Permission** | 每个代理独立的权限配置 |

---

## 2. 代理定义与模式

### 2.1 Agent.Info Schema

**文件:** `packages/opencode/src/agent/agent.ts:19-52`

```typescript
export const Info = z.object({
  name: z.string(),                           // 唯一标识符
  description: z.string().optional(),         // 用途描述
  mode: z.enum(["subagent", "primary", "all"]),  // 运行模式

  // 元数据
  native: z.boolean().optional(),             // 是否内置
  hidden: z.boolean().optional(),             // 是否在列表中隐藏
  default: z.boolean().optional(),            // 是否默认代理

  // 模型参数
  topP: z.number().optional(),
  temperature: z.number().optional(),
  color: z.string().optional(),               // UI 显示颜色

  // 自定义配置
  model: z.object({
    modelID: z.string(),
    providerID: z.string(),
  }).optional(),
  prompt: z.string().optional(),              // 自定义系统提示词
  tools: z.record(z.string(), z.boolean()),   // 工具白名单/黑名单
  options: z.record(z.string(), z.any()),     // Provider 特定选项
  maxSteps: z.number().int().positive().optional(),  // 最大迭代步数

  // 权限
  permission: z.object({
    edit: Config.Permission,
    bash: z.record(z.string(), Config.Permission),
    skill: z.record(z.string(), Config.Permission),
    webfetch: Config.Permission.optional(),
    doom_loop: Config.Permission.optional(),
    external_directory: Config.Permission.optional(),
  }),
})
```

### 2.2 运行模式

| 模式 | 可用户选择 | 可被调用 | 说明 |
|------|-----------|---------|------|
| `primary` | ✅ | ❌ | 主代理，用户直接使用 |
| `subagent` | ❌ | ✅ | 子代理，只能通过 Task Tool 调用 |
| `all` | ✅ | ✅ | 两种模式都支持 |

---

## 3. 原生代理

### 3.1 代理列表

**文件:** `packages/opencode/src/agent/agent.ts:117-198`

| 代理 | 模式 | 隐藏 | 描述 |
|------|------|------|------|
| `build` | primary | ❌ | 默认开发代理，全权限 |
| `plan` | primary | ❌ | 只读分析代理 |
| `general` | subagent | ✅ | 通用多步骤任务 |
| `explore` | subagent | ❌ | 代码库探索 |
| `compaction` | primary | ✅ | 上下文压缩 |
| `title` | primary | ✅ | 标题生成 |
| `summary` | primary | ✅ | 摘要生成 |

### 3.2 build 代理

```typescript
build: {
  name: "build",
  mode: "primary",
  native: true,
  tools: { ...defaultTools },
  options: {},
  permission: {
    edit: "allow",
    bash: { "*": "allow" },
    skill: { "*": "allow" },
    webfetch: "allow",
    doom_loop: "ask",
    external_directory: "ask",
  },
}
```

**特点:**
- 默认代理 (`default: true`)
- 全部工具启用
- 全部权限开放

### 3.3 plan 代理

```typescript
plan: {
  name: "plan",
  mode: "primary",
  native: true,
  tools: { ...defaultTools },
  options: {},
  permission: {
    edit: "deny",                    // 禁止编辑
    bash: {
      "cut*": "allow",
      "diff*": "allow",
      "du*": "allow",
      "file *": "allow",
      "find * -delete*": "ask",
      "find * -exec*": "ask",
      "find *": "allow",
      "git diff*": "allow",
      "git log*": "allow",
      "git show*": "allow",
      "git status*": "allow",
      "git branch": "allow",
      "grep*": "allow",
      "ls*": "allow",
      "pwd*": "allow",
      "rg*": "allow",
      "sort*": "allow",
      "stat*": "allow",
      "tail*": "allow",
      "tree*": "allow",
      "wc*": "allow",
      "*": "ask",                    // 其他命令需确认
    },
    webfetch: "allow",
  },
}
```

**特点:**
- 只读模式，禁止文件编辑
- Bash 命令白名单控制
- 适用于代码分析和规划

### 3.4 explore 代理

**文件:** `packages/opencode/src/agent/prompt/explore.txt`

```typescript
explore: {
  name: "explore",
  mode: "subagent",
  native: true,
  description: `Fast agent specialized for exploring codebases...`,
  prompt: PROMPT_EXPLORE,           // 专用系统提示词
  tools: {
    todoread: false,
    todowrite: false,
    edit: false,                    // 禁用编辑
    write: false,                   // 禁用写入
    ...defaultTools,
  },
  options: {},
  permission: agentPermission,
}
```

**系统提示词示例:**

```
You are a specialized agent for exploring codebases.
Your goal is to find relevant files and understand code structure.

When called with a thoroughness level:
- "quick": Do 1-2 searches, return immediate findings
- "medium": Do 3-5 searches, explore related files
- "very thorough": Do 6+ searches, check multiple naming conventions

Focus on:
1. Finding files by patterns (glob)
2. Searching code content (grep)
3. Reading relevant files
4. Summarizing findings
```

### 3.5 general 代理

```typescript
general: {
  name: "general",
  mode: "subagent",
  native: true,
  hidden: true,
  description: `General-purpose agent for researching complex questions...`,
  tools: {
    todoread: false,
    todowrite: false,               // 禁用 todo 工具
    ...defaultTools,
  },
  options: {},
  permission: agentPermission,
}
```

**特点:**
- 通用多步骤任务执行
- 隐藏在代理列表中
- 禁用 todo 工具避免与父会话冲突

### 3.6 专用代理

#### compaction 代理

```typescript
compaction: {
  name: "compaction",
  mode: "primary",
  native: true,
  hidden: true,
  prompt: PROMPT_COMPACTION,
  tools: { "*": false },            // 禁用所有工具
  options: {},
  permission: agentPermission,
}
```

**提示词:** `packages/opencode/src/agent/prompt/compaction.txt`

```
You are a compaction agent. Your job is to summarize the conversation
for continuation in a new context window.

Focus on:
1. What has been done
2. Current files being worked on
3. Next steps planned
4. Important context that should be preserved

Be concise but comprehensive.
```

#### title 代理

```typescript
title: {
  name: "title",
  mode: "primary",
  native: true,
  hidden: true,
  prompt: PROMPT_TITLE,
  tools: {},
  options: {},
  permission: agentPermission,
}
```

**提示词:** `packages/opencode/src/agent/prompt/title.txt`

```
Generate a short, descriptive title for this conversation.
Keep it under 50 characters.
Focus on the main topic or task.
```

---

## 4. Task Tool 调用机制

### 4.1 Task Tool 定义

**文件:** `packages/opencode/src/tool/task.ts:14-136`

```typescript
export const TaskTool = Tool.define("task", async () => {
  // 获取可调用的子代理列表
  const agents = await Agent.list().then(x =>
    x.filter(a => a.mode !== "primary")
  )

  const description = DESCRIPTION.replace(
    "{agents}",
    agents.map(a =>
      `- ${a.name}: ${a.description ?? "Manual only"}`
    ).join("\n")
  )

  return {
    description,
    parameters: z.object({
      description: z.string().describe("A short (3-5 words) description"),
      prompt: z.string().describe("The task for the agent to perform"),
      subagent_type: z.string().describe("The agent type to use"),
      session_id: z.string().describe("Existing session to continue").optional(),
      command: z.string().describe("Command that triggered this").optional(),
    }),

    async execute(params, ctx) {
      // 1. 获取代理配置
      const agent = await Agent.get(params.subagent_type)
      if (!agent) throw new Error(`Unknown agent type: ${params.subagent_type}`)

      // 2. 创建或恢复子会话
      const session = await iife(async () => {
        if (params.session_id) {
          const found = await Session.get(params.session_id).catch(() => {})
          if (found) return found
        }
        return await Session.create({
          parentID: ctx.sessionID,              // 关联父会话
          title: params.description + ` (@${agent.name} subagent)`,
        })
      })

      // 3. 更新工具元数据
      ctx.metadata({
        title: params.description,
        metadata: { sessionId: session.id },
      })

      // 4. 订阅工具调用事件 (用于实时更新)
      const parts: Record<string, ToolSummary> = {}
      const unsub = Bus.subscribe(MessageV2.Event.PartUpdated, async (evt) => {
        if (evt.properties.part.sessionID !== session.id) return
        if (evt.properties.part.type !== "tool") return
        parts[evt.properties.part.id] = summarize(evt.properties.part)
        ctx.metadata({
          title: params.description,
          metadata: { summary: Object.values(parts), sessionId: session.id },
        })
      })

      // 5. 执行子代理
      const result = await SessionPrompt.prompt({
        sessionID: session.id,
        model: agent.model ?? msg.info.model,
        agent: agent.name,
        tools: {
          todowrite: false,
          todoread: false,
          task: false,                          // 禁用嵌套 task
          ...agent.tools,
        },
        parts: await SessionPrompt.resolvePromptParts(params.prompt),
      })

      unsub()

      // 6. 返回结果
      const text = result.parts.findLast(x => x.type === "text")?.text ?? ""
      return {
        title: params.description,
        metadata: {
          summary: Object.values(parts),
          sessionId: session.id,
        },
        output: text + `\n\n<task_metadata>\nsession_id: ${session.id}\n</task_metadata>`,
      }
    },
  }
})
```

### 4.2 调用流程

```
Primary Agent (build)
    │
    │ 生成 Task Tool 调用
    │ { subagent_type: "explore", prompt: "Find all API endpoints" }
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ TaskTool.execute()                                             │
│   1. 验证代理存在                                              │
│   2. 创建子会话 (parentID 关联)                                │
│   3. 订阅 PartUpdated 事件                                     │
│   4. 调用 SessionPrompt.prompt()                               │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ 子会话执行 (Explore Agent)                                     │
│   • 使用 explore 代理配置                                      │
│   • 使用 explore 专用提示词                                    │
│   • 工具受限 (无 edit/write/task)                              │
│   • 独立的消息历史                                             │
└───────────────────────────────────────────────────────────────┘
    │
    │ 执行完成
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│ 结果返回                                                       │
│   • 最后的文本输出                                             │
│   • 工具调用摘要                                               │
│   • 子会话 ID (可恢复)                                         │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
Primary Agent 继续处理
```

### 4.3 工具描述

**文件:** `packages/opencode/src/tool/task.txt`

```
Launch a new agent to handle complex, multi-step tasks autonomously.

The Task tool launches specialized agents (subprocesses) that
autonomously handle complex tasks. Each agent type has specific
capabilities and tools available to it.

Available agent types and the tools they have access to:
{agents}

When using the Task tool, you must specify a subagent_type parameter.

When NOT to use the Task tool:
- If you want to read a specific file path, use the Read tool
- If you are searching for a specific class definition, use Glob
- Tasks that are simple and don't require multiple steps

Usage notes:
- Always include a short description (3-5 words)
- Launch multiple agents concurrently when possible
- Agents can be resumed using the session_id parameter
- Provide clear, detailed prompts for autonomous work
```

---

## 5. 会话隔离

### 5.1 父子会话关系

```typescript
// 创建子会话
const session = await Session.create({
  parentID: ctx.sessionID,                    // 关联父会话
  title: params.description + ` (@${agent.name} subagent)`,
})

// 会话结构
interface Session.Info {
  id: string
  parentID?: string                           // 父会话 ID
  title: string
  created: number
  updated: number
  // ...
}
```

### 5.2 隔离内容

| 项目 | 是否隔离 | 说明 |
|------|---------|------|
| 消息历史 | ✅ | 子会话有独立的消息列表 |
| 工具状态 | ✅ | 工具调用独立跟踪 |
| 快照 | ✅ | 独立的文件快照 |
| Token 计数 | ✅ | 独立的上下文计数 |
| 代理配置 | ✅ | 使用子代理的配置 |
| 工作目录 | ❌ | 共享项目目录 |
| 文件系统 | ❌ | 共享文件访问 |

### 5.3 会话恢复

```typescript
// 通过 session_id 恢复子会话
const session = await iife(async () => {
  if (params.session_id) {
    const found = await Session.get(params.session_id).catch(() => {})
    if (found) return found                   // 恢复已有会话
  }
  return await Session.create({...})          // 创建新会话
})
```

---

## 6. 权限系统

### 6.1 权限类型

```typescript
export const Permission = z.enum(["ask", "allow", "deny"])

// ask: 执行前询问用户
// allow: 自动允许
// deny: 自动拒绝
```

### 6.2 权限继承与合并

**文件:** `packages/opencode/src/agent/agent.ts:333-398`

```typescript
function mergeAgentPermissions(
  basePermission: any,
  overridePermission: any
): Agent.Info["permission"] {
  // 1. 规范化 bash 权限格式
  if (typeof basePermission.bash === "string") {
    basePermission.bash = { "*": basePermission.bash }
  }
  if (typeof overridePermission.bash === "string") {
    overridePermission.bash = { "*": overridePermission.bash }
  }

  // 2. 深度合并
  const merged = mergeDeep(basePermission ?? {}, overridePermission ?? {})

  // 3. 确保 bash 有默认通配符
  let mergedBash = merged.bash
  if (typeof mergedBash === "object") {
    mergedBash = mergeDeep({ "*": "allow" }, mergedBash)
  }

  return {
    edit: merged.edit ?? "allow",
    webfetch: merged.webfetch ?? "allow",
    bash: mergedBash ?? { "*": "allow" },
    skill: mergedSkill ?? { "*": "allow" },
    doom_loop: merged.doom_loop,
    external_directory: merged.external_directory,
  }
}
```

### 6.3 Bash 权限模式匹配

```typescript
// 配置示例
permission: {
  bash: {
    "git diff*": "allow",     // git diff 及其参数
    "git log*": "allow",
    "find * -delete*": "ask", // find 删除需确认
    "rm*": "deny",            // 禁止 rm
    "*": "ask",               // 默认询问
  }
}

// 匹配逻辑 (使用 minimatch)
function matchBashPermission(command: string, patterns: Record<string, Permission>) {
  for (const [pattern, permission] of Object.entries(patterns)) {
    if (minimatch(command, pattern)) {
      return permission
    }
  }
  return patterns["*"] ?? "ask"
}
```

### 6.4 工具权限过滤

**文件:** `packages/opencode/src/tool/registry.ts:139-160`

```typescript
export async function enabled(agent: Agent.Info): Promise<Record<string, boolean>> {
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

---

## 7. 代理配置与扩展

### 7.1 配置文件

```jsonc
// opencode.json
{
  "agent": {
    // 覆盖内置代理
    "build": {
      "temperature": 0.7,
      "model": "anthropic/claude-3-opus"
    },

    // 创建新的 Primary Agent
    "code-reviewer": {
      "mode": "primary",
      "description": "Specialized code review agent",
      "prompt": "You are a code review expert...",
      "temperature": 0.3,
      "tools": {
        "edit": false,
        "write": false
      },
      "permission": {
        "edit": "deny"
      }
    },

    // 创建新的 Subagent
    "test-generator": {
      "mode": "subagent",
      "description": "Generate unit tests for code",
      "prompt": "You are a test generation specialist...",
      "tools": {
        "todoread": false,
        "todowrite": false
      }
    }
  }
}
```

### 7.2 Markdown 配置

```markdown
<!-- .opencode/agent/code-reviewer.md -->
---
mode: primary
description: Specialized code review agent
temperature: 0.3
tools:
  edit: false
  write: false
permission:
  edit: deny
---

You are a code review expert. Focus on:
1. Code quality and best practices
2. Potential bugs and edge cases
3. Performance considerations
4. Security vulnerabilities

Be constructive and provide specific suggestions.
```

### 7.3 AI 生成代理

**文件:** `packages/opencode/src/agent/agent.ts:294-330`

```typescript
export async function generate(input: {
  description: string
  model?: { providerID: string; modelID: string }
}) {
  const existing = await list()

  const result = await generateObject({
    temperature: 0.3,
    messages: [
      { role: "system", content: PROMPT_GENERATE },
      {
        role: "user",
        content: `Create an agent for: "${input.description}"
          Existing names (cannot use): ${existing.map(i => i.name).join(", ")}`
      },
    ],
    schema: z.object({
      identifier: z.string(),
      whenToUse: z.string(),
      systemPrompt: z.string(),
    }),
  })

  return result.object
}
```

---

## 8. 与 codex 对比

### 8.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **代理定义** | Zod Schema + JSON/Markdown | Rust struct + TOML |
| **模式区分** | `mode: primary/subagent/all` | `Agent` vs `Subagent` 类型 |
| **调用机制** | Task Tool + 子会话 | `delegate` 模式 |
| **会话隔离** | 完全隔离 (独立 Session) | 共享会话 (标记区分) |
| **权限系统** | 模式匹配 (minimatch) | (待实现) |
| **配置** | JSON/Markdown + 合并 | TOML |

### 8.2 codex 借鉴建议

1. **权限模式匹配**: 实现类似 minimatch 的 Bash 命令模式匹配

2. **专用代理**: 添加 compaction、title、summary 等专用代理

3. **代理配置**: 支持 Markdown 格式的代理定义

4. **会话隔离**: 考虑完全隔离的子会话机制

5. **动态生成**: 实现 AI 生成代理功能

### 8.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/agent/agent.ts` | `core/src/subagent/mod.rs` |
| `src/tool/task.ts` | `core/src/subagent/delegate.rs` |
| `src/agent/prompt/*.txt` | `core/src/subagent/stores.rs` |
| `src/session/prompt.ts` | `core/src/codex_conversation.rs` |

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
