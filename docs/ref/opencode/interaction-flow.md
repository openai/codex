# OpenCode 交互流程

本文档详细分析 OpenCode 的交互流程，包括会话生命周期、消息处理循环、流式响应和事件系统。

---

## 目录

1. [系统概览](#1-系统概览)
2. [会话生命周期](#2-会话生命周期)
3. [消息处理循环](#3-消息处理循环)
4. [流式响应处理](#4-流式响应处理)
5. [事件系统](#5-事件系统)
6. [错误处理与重试](#6-错误处理与重试)
7. [与 codex 对比](#7-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       交互流程架构                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    入口层                                     │  │
│  │    ┌─────────┐  ┌─────────┐  ┌─────────┐                    │  │
│  │    │   CLI   │  │   TUI   │  │  Server │                    │  │
│  │    └────┬────┘  └────┬────┘  └────┬────┘                    │  │
│  │         └────────────┼────────────┘                          │  │
│  └──────────────────────┼───────────────────────────────────────┘  │
│                         ▼                                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                 SessionPrompt.prompt()                        │  │
│  │    1. 创建用户消息                                            │  │
│  │    2. 解析文件/代理引用                                       │  │
│  │    3. 触发 chat.message Hook                                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                         │                                           │
│                         ▼                                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                 SessionPrompt.loop()                          │  │
│  │    while (!finished) {                                        │  │
│  │      1. 检查子任务/压缩任务                                   │  │
│  │      2. 检查上下文溢出                                        │  │
│  │      3. 插入提醒                                              │  │
│  │      4. 解析工具                                              │  │
│  │      5. SessionProcessor.process()                            │  │
│  │    }                                                          │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                         │                                           │
│                         ▼                                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                 SessionProcessor.process()                    │  │
│  │    1. LLM.stream()                                            │  │
│  │    2. 处理流式事件                                            │  │
│  │    3. 执行工具调用                                            │  │
│  │    4. 更新消息部分                                            │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                         │                                           │
│                         ▼                                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Bus 事件                                   │  │
│  │    Session.Event | MessageV2.Event | Todo.Event              │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. 会话生命周期

### 2.1 会话创建

**文件:** `packages/opencode/src/session/index.ts`

```typescript
export namespace Session {
  export async function create(input: {
    parentID?: string      // 父会话 (子代理)
    title?: string
  }): Promise<Info> {
    const session: Info = {
      id: Identifier.ascending("session"),
      parentID: input.parentID,
      title: input.title ?? defaultTitle(),
      created: Date.now(),
      updated: Date.now(),
      share: undefined,
    }

    await Storage.write(["session", session.id], session)
    Bus.publish(Event.Created, { session })

    return session
  }
}
```

### 2.2 会话状态

```typescript
interface Session.Info {
  id: string              // 唯一标识
  parentID?: string       // 父会话 ID
  title: string           // 会话标题
  created: number         // 创建时间
  updated: number         // 更新时间
  revert?: string         // 回滚点
  share?: {               // 分享信息
    id: string
    url: string
    expiresAt: number
  }
}
```

### 2.3 会话状态跟踪

**文件:** `packages/opencode/src/session/status.ts`

```typescript
export namespace SessionStatus {
  export type Status =
    | { type: "idle" }
    | { type: "busy" }
    | { type: "retry"; attempt: number; message: string; next: number }

  export function set(sessionID: string, status: Status) {
    state()[sessionID] = status
    Bus.publish(Event.Status, { sessionID, status })
  }

  export function get(sessionID: string): Status {
    return state()[sessionID] ?? { type: "idle" }
  }
}
```

---

## 3. 消息处理循环

### 3.1 SessionPrompt.prompt

**文件:** `packages/opencode/src/session/prompt.ts:140-152`

```typescript
export const prompt = fn(PromptInput, async (input) => {
  // 1. 获取会话
  const session = await Session.get(input.sessionID)

  // 2. 清理回滚点
  await SessionRevert.cleanup(session)

  // 3. 创建用户消息
  const message = await createUserMessage(input)

  // 4. 更新会话时间
  await Session.touch(input.sessionID)

  // 5. 无需回复时直接返回
  if (input.noReply === true) {
    return message
  }

  // 6. 进入主循环
  return loop(input.sessionID)
})
```

### 3.2 SessionPrompt.loop

**文件:** `packages/opencode/src/session/prompt.ts:230-563`

```typescript
export const loop = fn(Identifier.schema("session"), async (sessionID) => {
  // 1. 启动会话 (防止并发)
  const abort = start(sessionID)
  if (!abort) {
    // 已有会话在运行，等待完成
    return new Promise<MessageV2.WithParts>((resolve, reject) => {
      state()[sessionID].callbacks.push({ resolve, reject })
    })
  }

  using _ = defer(() => cancel(sessionID))

  let step = 0
  while (true) {
    SessionStatus.set(sessionID, { type: "busy" })

    if (abort.aborted) break

    // 2. 获取消息列表 (过滤已压缩)
    let msgs = await MessageV2.filterCompacted(MessageV2.stream(sessionID))

    // 3. 查找关键消息
    let lastUser: MessageV2.User | undefined
    let lastAssistant: MessageV2.Assistant | undefined
    let lastFinished: MessageV2.Assistant | undefined
    let tasks: (CompactionPart | SubtaskPart)[] = []

    for (let i = msgs.length - 1; i >= 0; i--) {
      const msg = msgs[i]
      if (!lastUser && msg.info.role === "user")
        lastUser = msg.info as MessageV2.User
      if (!lastAssistant && msg.info.role === "assistant")
        lastAssistant = msg.info as MessageV2.Assistant
      if (!lastFinished && msg.info.role === "assistant" && msg.info.finish)
        lastFinished = msg.info as MessageV2.Assistant
      // 收集待处理任务
      const task = msg.parts.filter(p =>
        p.type === "compaction" || p.type === "subtask"
      )
      if (task && !lastFinished) tasks.push(...task)
    }

    // 4. 检查是否完成
    if (
      lastAssistant?.finish &&
      !["tool-calls", "unknown"].includes(lastAssistant.finish) &&
      lastUser.id < lastAssistant.id
    ) {
      break
    }

    step++

    // 5. 首步生成标题
    if (step === 1) ensureTitle(...)

    const model = await Provider.getModel(...)
    const task = tasks.pop()

    // 6. 处理子任务
    if (task?.type === "subtask") {
      // ... 调用 Task Tool
      continue
    }

    // 7. 处理压缩任务
    if (task?.type === "compaction") {
      const result = await SessionCompaction.process(...)
      if (result === "stop") break
      continue
    }

    // 8. 检查上下文溢出
    if (lastFinished && await SessionCompaction.isOverflow(...)) {
      await SessionCompaction.create(...)
      continue
    }

    // 9. 插入提醒
    msgs = insertReminders({ messages: msgs, agent })

    // 10. 创建处理器
    const processor = SessionProcessor.create({
      assistantMessage: await Session.updateMessage({
        role: "assistant",
        parentID: lastUser.id,
        agent: agent.name,
        ...
      }),
      sessionID,
      model,
      abort,
    })

    // 11. 解析工具
    const tools = await resolveTools({
      agent,
      sessionID,
      model,
      tools: lastUser.tools,
      processor,
    })

    // 12. 执行 LLM 调用
    const result = await processor.process({
      user: lastUser,
      agent,
      abort,
      sessionID,
      system: [
        ...await SystemPrompt.environment(),
        ...await SystemPrompt.custom(),
      ],
      messages: MessageV2.toModelMessage(msgs),
      tools,
      model,
    })

    if (result === "stop") break
  }

  // 13. 修剪旧输出
  SessionCompaction.prune({ sessionID })

  // 14. 返回最终消息
  for await (const item of MessageV2.stream(sessionID)) {
    if (item.info.role === "user") continue
    return item
  }
})
```

---

## 4. 流式响应处理

### 4.1 SessionProcessor.process

**文件:** `packages/opencode/src/session/processor.ts:42-404`

```typescript
export function create(input: {
  assistantMessage: MessageV2.Assistant
  sessionID: string
  model: Provider.Model
  abort: AbortSignal
}) {
  const toolcalls: Record<string, MessageV2.ToolPart> = {}
  let snapshot: string | undefined
  let blocked = false
  let attempt = 0

  return {
    get message() { return input.assistantMessage },

    partFromToolCall(toolCallID: string) {
      return toolcalls[toolCallID]
    },

    async process(streamInput: LLM.StreamInput) {
      while (true) {
        try {
          let currentText: MessageV2.TextPart | undefined
          let reasoningMap: Record<string, MessageV2.ReasoningPart> = {}

          // 1. 开始流式调用
          const stream = await LLM.stream(streamInput)

          // 2. 处理流式事件
          for await (const value of stream.fullStream) {
            input.abort.throwIfAborted()

            switch (value.type) {
              case "start":
                SessionStatus.set(input.sessionID, { type: "busy" })
                break

              // 推理事件
              case "reasoning-start":
                reasoningMap[value.id] = {
                  type: "reasoning",
                  text: "",
                  time: { start: Date.now() },
                }
                break

              case "reasoning-delta":
                reasoningMap[value.id].text += value.text
                await Session.updatePart({ part: reasoningMap[value.id], delta: value.text })
                break

              case "reasoning-end":
                reasoningMap[value.id].time.end = Date.now()
                await Session.updatePart(reasoningMap[value.id])
                break

              // 文本事件
              case "text-start":
                currentText = {
                  type: "text",
                  text: "",
                  time: { start: Date.now() },
                }
                break

              case "text-delta":
                currentText.text += value.text
                await Session.updatePart({ part: currentText, delta: value.text })
                break

              case "text-end":
                currentText.time.end = Date.now()
                await Session.updatePart(currentText)
                break

              // 工具事件
              case "tool-input-start":
                toolcalls[value.id] = {
                  type: "tool",
                  tool: value.toolName,
                  callID: value.id,
                  state: { status: "pending", input: {}, raw: "" },
                }
                await Session.updatePart(toolcalls[value.id])
                break

              case "tool-call":
                toolcalls[value.toolCallId].state = {
                  status: "running",
                  input: value.input,
                  time: { start: Date.now() },
                }
                await Session.updatePart(toolcalls[value.toolCallId])

                // Doom Loop 检测
                const parts = await MessageV2.parts(input.assistantMessage.id)
                const lastThree = parts.slice(-3)
                if (isDoomLoop(lastThree, value)) {
                  // 处理 doom loop
                }
                break

              case "tool-result":
                toolcalls[value.toolCallId].state = {
                  status: "completed",
                  input: value.input,
                  output: value.output.output,
                  title: value.output.title,
                  metadata: value.output.metadata,
                  time: { start: ..., end: Date.now() },
                }
                await Session.updatePart(toolcalls[value.toolCallId])
                break

              case "tool-error":
                toolcalls[value.toolCallId].state = {
                  status: "error",
                  error: value.error.toString(),
                  time: { start: ..., end: Date.now() },
                }
                await Session.updatePart(toolcalls[value.toolCallId])
                break

              // 步骤事件
              case "start-step":
                snapshot = await Snapshot.track()
                break

              case "finish-step":
                // 计算 token 使用
                const usage = Session.getUsage({
                  model: input.model,
                  usage: value.usage,
                })
                input.assistantMessage.cost += usage.cost
                input.assistantMessage.tokens = usage.tokens
                await Session.updateMessage(input.assistantMessage)

                // 记录文件变更
                if (snapshot) {
                  const patch = await Snapshot.patch(snapshot)
                  if (patch.files.length) {
                    await Session.updatePart({
                      type: "patch",
                      files: patch.files,
                    })
                  }
                }

                // 生成摘要
                SessionSummary.summarize({
                  sessionID: input.sessionID,
                  messageID: input.assistantMessage.parentID,
                })
                break

              case "error":
                throw value.error
            }
          }
        } catch (e) {
          // 3. 错误处理
          const error = MessageV2.fromError(e)
          const retry = SessionRetry.retryable(error)

          if (retry !== undefined) {
            // 可重试错误
            attempt++
            const delay = SessionRetry.delay(attempt, error)
            SessionStatus.set(input.sessionID, {
              type: "retry",
              attempt,
              message: retry,
              next: Date.now() + delay,
            })
            await SessionRetry.sleep(delay, input.abort)
            continue
          }

          // 不可重试错误
          input.assistantMessage.error = error
          Bus.publish(Session.Event.Error, {
            sessionID: input.sessionID,
            error: input.assistantMessage.error,
          })
        }

        // 4. 完成处理
        input.assistantMessage.time.completed = Date.now()
        await Session.updateMessage(input.assistantMessage)

        if (blocked) return "stop"
        if (input.assistantMessage.error) return "stop"
        return "continue"
      }
    },
  }
}
```

### 4.2 Doom Loop 检测

**文件:** `packages/opencode/src/session/processor.ts:142-178`

```typescript
const DOOM_LOOP_THRESHOLD = 3

// 检测连续相同的工具调用
if (
  lastThree.length === DOOM_LOOP_THRESHOLD &&
  lastThree.every(p =>
    p.type === "tool" &&
    p.tool === value.toolName &&
    p.state.status !== "pending" &&
    JSON.stringify(p.state.input) === JSON.stringify(value.input)
  )
) {
  const permission = await Agent.get(input.assistantMessage.mode)
    .then(x => x.permission)

  if (permission.doom_loop === "ask") {
    await Permission.ask({
      type: "doom_loop",
      pattern: value.toolName,
      sessionID: input.sessionID,
      title: `Possible doom loop: "${value.toolName}" called ${DOOM_LOOP_THRESHOLD} times with identical arguments`,
    })
  } else if (permission.doom_loop === "deny") {
    throw new Permission.RejectedError(
      input.sessionID,
      "doom_loop",
      value.toolCallId,
      { tool: value.toolName, input: value.input },
      "You seem to be stuck in a doom loop, please stop repeating the same action"
    )
  }
}
```

---

## 5. 事件系统

### 5.1 事件定义

**文件:** `packages/opencode/src/bus/bus-event.ts`

```typescript
export namespace BusEvent {
  export function define<Properties extends z.ZodType>(
    type: string,
    properties: Properties
  ) {
    return {
      type,
      properties,
    }
  }
}
```

### 5.2 主要事件

| 命名空间 | 事件 | 触发时机 |
|---------|------|---------|
| `Session.Event` | Created | 会话创建 |
| | Updated | 会话更新 |
| | Deleted | 会话删除 |
| | Error | 发生错误 |
| | Diff | 文件差异 |
| `MessageV2.Event` | Updated | 消息更新 |
| | Removed | 消息删除 |
| | PartUpdated | 部分更新 |
| | PartRemoved | 部分删除 |
| `SessionCompaction.Event` | Compacted | 压缩完成 |
| `SessionStatus.Event` | Status | 状态变更 |
| | Idle | 空闲状态 |
| `Todo.Event` | Updated | 任务更新 |
| `Command.Event` | Executed | 命令执行 |

### 5.3 发布/订阅

**文件:** `packages/opencode/src/bus/index.ts`

```typescript
export namespace Bus {
  // 发布事件
  export async function publish<D extends BusEvent.Definition>(
    def: D,
    properties: z.output<D["properties"]>
  ) {
    const event = { type: def.type, properties }
    for (const callback of subscribers.get(def.type) ?? []) {
      callback(event)
    }
    for (const callback of globalSubscribers) {
      callback(event)
    }
  }

  // 订阅特定事件
  export function subscribe<D extends BusEvent.Definition>(
    def: D,
    callback: (event: { type: string; properties: any }) => void
  ): () => void {
    const list = subscribers.get(def.type) ?? []
    list.push(callback)
    subscribers.set(def.type, list)
    return () => {
      const idx = list.indexOf(callback)
      if (idx >= 0) list.splice(idx, 1)
    }
  }

  // 订阅所有事件
  export function subscribeAll(callback: (event: any) => void) {
    globalSubscribers.add(callback)
    return () => globalSubscribers.delete(callback)
  }
}
```

---

## 6. 错误处理与重试

### 6.1 可重试错误

**文件:** `packages/opencode/src/session/retry.ts`

```typescript
export namespace SessionRetry {
  // 判断是否可重试
  export function retryable(error: MessageV2.Error): string | undefined {
    switch (error.name) {
      case "APIError":
        if (error.status >= 500) return "Server error, retrying..."
        if (error.status === 429) return "Rate limited, retrying..."
        return undefined
      case "TimeoutError":
        return "Request timed out, retrying..."
      case "NetworkError":
        return "Network error, retrying..."
      default:
        return undefined
    }
  }

  // 计算重试延迟 (指数退避)
  export function delay(attempt: number, error?: APIError): number {
    // 使用 Retry-After 头
    if (error?.headers?.["retry-after"]) {
      return parseInt(error.headers["retry-after"]) * 1000
    }
    // 指数退避: 1s, 2s, 4s, 8s...
    return Math.min(1000 * Math.pow(2, attempt - 1), 30000)
  }

  // 可中断的睡眠
  export async function sleep(ms: number, abort: AbortSignal) {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(resolve, ms)
      abort.addEventListener("abort", () => {
        clearTimeout(timer)
        reject(new Error("Aborted"))
      })
    })
  }
}
```

### 6.2 错误转换

```typescript
export namespace MessageV2 {
  export function fromError(e: any): Error {
    if (e instanceof APIError) {
      return {
        name: "APIError",
        message: e.message,
        status: e.status,
      }
    }
    if (e.name === "TimeoutError") {
      return {
        name: "TimeoutError",
        message: "Request timed out",
      }
    }
    return {
      name: "UnknownError",
      message: e.message ?? String(e),
    }
  }
}
```

---

## 7. 与 codex 对比

### 7.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **会话存储** | Storage (文件) | 内存 + 可选持久化 |
| **消息循环** | while + abort | async stream |
| **事件系统** | Bus (发布/订阅) | mpsc channels |
| **Doom Loop** | 3 次相同调用 | - |
| **重试策略** | 指数退避 | 配置化 |
| **快照** | Snapshot.track | - |

### 7.2 codex 借鉴建议

1. **Doom Loop 检测**: 实现相同工具调用检测

```rust
fn is_doom_loop(recent_calls: &[ToolCall]) -> bool {
    if recent_calls.len() < 3 { return false; }
    let last_three = &recent_calls[recent_calls.len()-3..];
    last_three.windows(2).all(|w| {
        w[0].tool == w[1].tool && w[0].input == w[1].input
    })
}
```

2. **步骤快照**: 在每个步骤记录文件状态

```rust
struct StepSnapshot {
    files: HashMap<PathBuf, FileState>,
}

impl StepSnapshot {
    fn track() -> Self { ... }
    fn diff(&self, other: &Self) -> Vec<FileDiff> { ... }
}
```

3. **会话状态**: 添加 busy/retry 状态跟踪

```rust
enum SessionStatus {
    Idle,
    Busy,
    Retry { attempt: u32, message: String, next: Instant },
}
```

### 7.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/session/prompt.ts` | `core/src/codex_conversation.rs` |
| `src/session/processor.ts` | `core/src/response_processing.rs` |
| `src/session/retry.ts` | - |
| `src/bus/index.ts` | channels |

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
