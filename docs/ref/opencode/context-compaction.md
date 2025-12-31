# OpenCode 上下文压缩系统

本文档详细分析 OpenCode 的上下文管理和压缩机制，包括溢出检测、修剪策略和摘要生成。

---

## 目录

1. [系统概览](#1-系统概览)
2. [溢出检测](#2-溢出检测)
3. [修剪策略](#3-修剪策略)
4. [压缩流程](#4-压缩流程)
5. [摘要生成](#5-摘要生成)
6. [配置与控制](#6-配置与控制)
7. [与 codex 对比](#7-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                    上下文压缩系统                                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    会话主循环 (prompt.ts)                     │  │
│  │                                                               │  │
│  │  while (!finished) {                                          │  │
│  │    1. 检查子任务                                              │  │
│  │    2. 检查压缩任务 ────────────────────┐                     │  │
│  │    3. 检查上下文溢出 ──────────────────┼──▶ 触发压缩         │  │
│  │    4. 执行 LLM 调用                    │                     │  │
│  │  }                                     │                     │  │
│  │  prune() ◀─────────────────────────────┘                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
│         │                    │                    │                 │
│         ▼                    ▼                    ▼                 │
│  ┌────────────┐      ┌────────────┐      ┌────────────┐            │
│  │ isOverflow │      │   prune    │      │  process   │            │
│  │  (检测)    │      │  (修剪)    │      │  (压缩)    │            │
│  └────────────┘      └────────────┘      └────────────┘            │
│         │                    │                    │                 │
│         ▼                    ▼                    ▼                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    compaction.ts                             │   │
│  │                                                              │   │
│  │  • PRUNE_MINIMUM = 20,000 tokens                            │   │
│  │  • PRUNE_PROTECT = 40,000 tokens                            │   │
│  │  • PRUNE_PROTECTED_TOOLS = ["skill"]                        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心常量

**文件:** `packages/opencode/src/session/compaction.ts:41-44`

| 常量 | 值 | 说明 |
|------|-----|------|
| `PRUNE_MINIMUM` | 20,000 | 触发修剪的最小 token 数 |
| `PRUNE_PROTECT` | 40,000 | 保护最近输出的 token 数 |
| `PRUNE_PROTECTED_TOOLS` | `["skill"]` | 永不修剪的工具 |
| `OUTPUT_TOKEN_MAX` | 32,000 | 预留输出 token 上限 |

---

## 2. 溢出检测

### 2.1 检测逻辑

**文件:** `packages/opencode/src/session/compaction.ts:30-39`

```typescript
export async function isOverflow(input: {
  tokens: MessageV2.Assistant["tokens"]
  model: Provider.Model
}): boolean {
  const config = await Config.get()

  // 1. 检查是否禁用自动压缩
  if (config.compaction?.auto === false) return false

  // 2. 获取模型上下文限制
  const context = input.model.limit.context
  if (context === 0) return false

  // 3. 计算已使用 token
  const count =
    input.tokens.input +
    input.tokens.cache.read +
    input.tokens.output

  // 4. 计算输出预留
  const output = Math.min(
    input.model.limit.output,
    SessionPrompt.OUTPUT_TOKEN_MAX
  ) || SessionPrompt.OUTPUT_TOKEN_MAX

  // 5. 计算可用空间
  const usable = context - output

  // 6. 判断是否溢出
  return count > usable
}
```

### 2.2 溢出公式

```
可用 token = 上下文限制 - min(模型输出限制, 32000)
溢出条件 = (input_tokens + cache_read + output_tokens) > 可用 token
```

**示例计算:**

| 模型 | 上下文限制 | 输出限制 | 可用空间 |
|------|-----------|---------|---------|
| Claude 3.5 Sonnet | 200,000 | 8,192 | 191,808 |
| GPT-4 Turbo | 128,000 | 4,096 | 123,904 |
| Claude 3 Opus | 200,000 | 4,096 | 195,904 |

### 2.3 触发时机

**文件:** `packages/opencode/src/session/prompt.ts:459-471`

```typescript
// 在主循环中检测
if (
  lastFinished &&
  lastFinished.summary !== true &&                    // 非摘要消息
  (await SessionCompaction.isOverflow({
    tokens: lastFinished.tokens,
    model
  }))
) {
  // 创建压缩任务
  await SessionCompaction.create({
    sessionID,
    agent: lastUser.agent,
    model: lastUser.model,
    auto: true,
  })
  continue
}
```

---

## 3. 修剪策略

### 3.1 修剪逻辑

**文件:** `packages/opencode/src/session/compaction.ts:49-90`

```typescript
export async function prune(input: { sessionID: string }) {
  const config = await Config.get()

  // 1. 检查是否禁用修剪
  if (config.compaction?.prune === false) return

  const msgs = await Session.messages({ sessionID: input.sessionID })
  let total = 0
  let pruned = 0
  const toPrune = []
  let turns = 0

  // 2. 从后向前遍历消息
  loop: for (let msgIndex = msgs.length - 1; msgIndex >= 0; msgIndex--) {
    const msg = msgs[msgIndex]

    // 计算用户轮次
    if (msg.info.role === "user") turns++

    // 跳过最近 2 个用户轮次
    if (turns < 2) continue

    // 遇到摘要消息停止
    if (msg.info.role === "assistant" && msg.info.summary) break loop

    // 3. 遍历消息部分
    for (let partIndex = msg.parts.length - 1; partIndex >= 0; partIndex--) {
      const part = msg.parts[partIndex]

      if (part.type === "tool" && part.state.status === "completed") {
        // 跳过受保护的工具
        if (PRUNE_PROTECTED_TOOLS.includes(part.tool)) continue

        // 已经被压缩的部分停止
        if (part.state.time.compacted) break loop

        // 估算 token 数
        const estimate = Token.estimate(part.state.output)
        total += estimate

        // 超过保护阈值后开始标记修剪
        if (total > PRUNE_PROTECT) {
          pruned += estimate
          toPrune.push(part)
        }
      }
    }
  }

  // 4. 执行修剪
  if (pruned > PRUNE_MINIMUM) {
    for (const part of toPrune) {
      if (part.state.status === "completed") {
        part.state.time.compacted = Date.now()   // 标记压缩时间
        await Session.updatePart(part)
      }
    }
  }
}
```

### 3.2 修剪流程图

```
消息列表 (从后向前)
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│  最近 2 个用户轮次                                            │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  [保护] 不修剪                                          │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│  累计 < 40,000 tokens                                         │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  [保护] 不修剪                                          │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│  累计 > 40,000 tokens 且 修剪量 > 20,000                      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  [修剪] 标记 compacted = Date.now()                     │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
    │
    ▼ (遇到摘要消息或已压缩部分停止)
```

### 3.3 修剪后的消息转换

**文件:** `packages/opencode/src/session/message-v2.ts`

```typescript
// 过滤已压缩的部分
export async function filterCompacted(
  messages: AsyncIterable<MessageV2.WithParts>
): Promise<MessageV2.WithParts[]> {
  const result: MessageV2.WithParts[] = []

  for await (const msg of messages) {
    const filteredParts = msg.parts.filter(part => {
      if (part.type !== "tool") return true
      if (part.state.status !== "completed") return true
      return !part.state.time.compacted   // 排除已压缩
    })

    result.push({
      ...msg,
      parts: filteredParts,
    })
  }

  return result
}
```

---

## 4. 压缩流程

### 4.1 创建压缩任务

**文件:** `packages/opencode/src/session/compaction.ts:195-224`

```typescript
export const create = fn(
  z.object({
    sessionID: Identifier.schema("session"),
    agent: z.string(),
    model: z.object({
      providerID: z.string(),
      modelID: z.string(),
    }),
    auto: z.boolean(),
  }),
  async (input) => {
    // 创建用户消息
    const msg = await Session.updateMessage({
      id: Identifier.ascending("message"),
      role: "user",
      model: input.model,
      sessionID: input.sessionID,
      agent: input.agent,
      time: { created: Date.now() },
    })

    // 添加压缩部分标记
    await Session.updatePart({
      id: Identifier.ascending("part"),
      messageID: msg.id,
      sessionID: msg.sessionID,
      type: "compaction",
      auto: input.auto,                  // 自动/手动标记
    })
  },
)
```

### 4.2 执行压缩

**文件:** `packages/opencode/src/session/compaction.ts:92-193`

```typescript
export async function process(input: {
  parentID: string
  messages: MessageV2.WithParts[]
  sessionID: string
  abort: AbortSignal
  auto: boolean
}): Promise<"continue" | "stop"> {
  const userMessage = input.messages.findLast(m =>
    m.info.id === input.parentID
  )!.info as MessageV2.User

  // 1. 获取压缩代理配置
  const agent = await Agent.get("compaction")
  const model = agent.model
    ? await Provider.getModel(agent.model.providerID, agent.model.modelID)
    : await Provider.getModel(userMessage.model.providerID, userMessage.model.modelID)

  // 2. 创建助手消息 (标记为摘要)
  const msg = await Session.updateMessage({
    id: Identifier.ascending("message"),
    role: "assistant",
    parentID: input.parentID,
    sessionID: input.sessionID,
    mode: "compaction",
    agent: "compaction",
    summary: true,                        // 标记为摘要消息
    // ...
  }) as MessageV2.Assistant

  // 3. 创建处理器
  const processor = SessionProcessor.create({
    assistantMessage: msg,
    sessionID: input.sessionID,
    model,
    abort: input.abort,
  })

  // 4. 调用插件获取上下文
  const compacting = await Plugin.trigger(
    "experimental.session.compacting",
    { sessionID: input.sessionID },
    { context: [], prompt: undefined },
  )

  // 5. 构建提示词
  const defaultPrompt = `Provide a detailed prompt for continuing our conversation.
Focus on information helpful for continuation:
- What we did
- What we're doing
- Which files we're working on
- What we're going to do next
The new session won't have access to our conversation.`

  const promptText = compacting.prompt ??
    [defaultPrompt, ...compacting.context].join("\n\n")

  // 6. 执行 LLM 调用
  const result = await processor.process({
    user: userMessage,
    agent,
    abort: input.abort,
    sessionID: input.sessionID,
    tools: {},                            // 无工具
    system: [],
    messages: [
      ...MessageV2.toModelMessage(input.messages),
      {
        role: "user",
        content: [{ type: "text", text: promptText }],
      },
    ],
    model,
  })

  // 7. 自动继续
  if (result === "continue" && input.auto) {
    const continueMsg = await Session.updateMessage({...})
    await Session.updatePart({
      type: "text",
      synthetic: true,
      text: "Continue if you have next steps",
    })
  }

  // 8. 发布事件
  Bus.publish(Event.Compacted, { sessionID: input.sessionID })
  return "continue"
}
```

### 4.3 压缩代理提示词

**文件:** `packages/opencode/src/agent/prompt/compaction.txt`

```
You are a compaction agent. Your job is to summarize the conversation
for continuation in a new context window.

When summarizing, focus on:

1. **What was done**
   - Files created, modified, or deleted
   - Code changes made
   - Commands executed

2. **Current state**
   - Files being worked on
   - Uncommitted changes
   - Current directory/project

3. **Next steps**
   - What was planned but not completed
   - Outstanding tasks
   - User's last request if unfinished

4. **Important context**
   - Decisions made and why
   - Constraints or requirements mentioned
   - User preferences noted

Be concise but comprehensive. The new session will use this
as context to continue the work seamlessly.
```

---

## 5. 摘要生成

### 5.1 会话摘要

**文件:** `packages/opencode/src/session/summary.ts`

```typescript
export namespace SessionSummary {
  // 生成会话差异摘要
  export async function summarizeSession(input: { sessionID: string }) {
    const messages = await Session.messages({ sessionID: input.sessionID })

    // 计算文件差异
    const diffs: Snapshot.FileDiff[] = []
    for (const msg of messages) {
      for (const part of msg.parts) {
        if (part.type === "patch") {
          diffs.push(...part.files)
        }
      }
    }

    // 存储差异
    await Storage.write(["session_diff", input.sessionID], diffs)
    return diffs
  }

  // 生成消息标题和摘要
  export async function summarizeMessage(input: {
    sessionID: string
    messageID: string
  }) {
    const msg = await MessageV2.get(input)
    if (msg.info.role !== "user") return

    // 使用 summary 代理生成摘要
    const agent = await Agent.get("summary")
    const result = await LLM.stream({
      agent,
      messages: [
        { role: "user", content: "Summarize this message:" },
        ...MessageV2.toModelMessage([msg]),
      ],
      // ...
    })

    return result.text
  }
}
```

### 5.2 摘要代理提示词

**文件:** `packages/opencode/src/agent/prompt/summary.txt`

```
Generate a brief summary of the user's request.

Focus on:
- Main intent or goal
- Key files or components mentioned
- Type of task (bug fix, feature, refactor, etc.)

Keep it to 1-2 sentences.
```

---

## 6. 配置与控制

### 6.1 配置选项

**文件:** `packages/opencode/src/config/config.ts:803-807`

```typescript
compaction: z.object({
  auto: z.boolean().optional()
    .describe("Enable automatic compaction when context is full"),
  prune: z.boolean().optional()
    .describe("Enable pruning of old tool outputs"),
}).optional()
```

**配置示例:**

```jsonc
{
  "compaction": {
    "auto": true,    // 自动压缩 (默认 true)
    "prune": true    // 自动修剪 (默认 true)
  }
}
```

### 6.2 环境变量

| 变量 | 说明 |
|------|------|
| `OPENCODE_DISABLE_AUTOCOMPACT` | 禁用自动压缩 |
| `OPENCODE_DISABLE_PRUNE` | 禁用自动修剪 |

### 6.3 插件扩展

```typescript
// 插件可以注入压缩上下文
Plugin.trigger(
  "experimental.session.compacting",
  { sessionID },
  {
    context: [],          // 附加上下文
    prompt: undefined,    // 覆盖默认提示词
  }
)

// 插件实现
export default async function myPlugin(input: PluginInput) {
  return {
    async "experimental.session.compacting"(params, output) {
      // 注入自定义上下文
      output.context.push("Additional context from plugin...")
    }
  }
}
```

### 6.4 手动压缩

```typescript
// 通过命令触发手动压缩
await SessionCompaction.create({
  sessionID: session.id,
  agent: currentAgent,
  model: currentModel,
  auto: false,            // 手动标记
})
```

---

## 7. 与 codex 对比

### 7.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **触发方式** | 自动 + 手动 | 手动触发 |
| **修剪策略** | 时间戳标记 + 过滤 | 直接删除 |
| **保护机制** | 最近 2 轮 + 40K token | - |
| **摘要生成** | 专用 compaction 代理 | system_reminder |
| **配置** | JSON + 环境变量 | - |
| **插件扩展** | Hook 注入上下文 | - |

### 7.2 codex 借鉴建议

1. **自动检测**: 实现类似的溢出检测逻辑

```rust
fn is_overflow(tokens: &TokenCount, model: &Model) -> bool {
    let usable = model.context_limit - model.output_limit.min(32000);
    tokens.total() > usable
}
```

2. **分层保护**: 保护最近的用户轮次和重要工具输出

3. **标记修剪**: 使用时间戳标记而非直接删除，支持恢复

4. **专用代理**: 添加 compaction 代理生成结构化摘要

5. **插件 Hook**: 允许扩展注入压缩上下文

### 7.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/session/compaction.ts` | `core/src/system_reminder/` |
| `src/session/summary.ts` | - |
| `src/agent/prompt/compaction.txt` | `core/src/system_reminder/generator.rs` |

---

## 8. 数据流总结

```
用户输入
    │
    ▼
┌────────────────────────────────────────┐
│ SessionPrompt.loop()                   │
│   检测溢出: isOverflow()               │
│   ├─ 否 → 正常处理                     │
│   └─ 是 → 创建压缩任务                 │
└────────────────────────────────────────┘
    │
    ▼
┌────────────────────────────────────────┐
│ SessionCompaction.create()             │
│   创建 CompactionPart                  │
└────────────────────────────────────────┘
    │
    ▼
┌────────────────────────────────────────┐
│ SessionCompaction.process()            │
│   1. 获取 compaction 代理              │
│   2. 调用插件 Hook                     │
│   3. 生成摘要消息                      │
│   4. 标记 summary: true                │
└────────────────────────────────────────┘
    │
    ▼
┌────────────────────────────────────────┐
│ SessionCompaction.prune()              │
│   1. 遍历旧消息                        │
│   2. 跳过受保护内容                    │
│   3. 标记 compacted 时间戳             │
└────────────────────────────────────────┘
    │
    ▼
┌────────────────────────────────────────┐
│ 后续请求                               │
│   filterCompacted() 过滤已压缩部分     │
│   摘要消息作为上下文起点               │
└────────────────────────────────────────┘
```

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
