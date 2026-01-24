# OpenCode 提醒系统

本文档详细分析 OpenCode 的提醒 (Reminder) 系统设计，包括 Plan/Build 提醒、合成文本和 Max Steps 警告。

---

## 目录

1. [系统概览](#1-系统概览)
2. [提醒类型](#2-提醒类型)
3. [实现机制](#3-实现机制)
4. [提醒内容](#4-提醒内容)
5. [与 codex 对比](#5-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       提醒系统架构                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    会话主循环                                 │  │
│  │                                                               │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │  insertReminders()                                    │   │  │
│  │  │    ├─ Plan Agent? → 插入 plan.txt                    │   │  │
│  │  │    └─ Build after Plan? → 插入 build-switch.txt      │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  │                                                               │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │  maxSteps 检查                                        │   │  │
│  │  │    └─ isLastStep? → 注入 max-steps.txt               │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  │                                                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    合成 TextPart                              │  │
│  │    { type: "text", synthetic: true, text: reminder }         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心文件

| 文件 | 职责 |
|------|------|
| `src/session/prompt.ts` | `insertReminders()` 函数 |
| `src/session/prompt/plan.txt` | Plan 模式提醒 |
| `src/session/prompt/build-switch.txt` | Build 切换提醒 |
| `src/session/prompt/max-steps.txt` | 最大步数警告 |

---

## 2. 提醒类型

### 2.1 类型列表

| 类型 | 触发条件 | 文件 |
|------|---------|------|
| Plan 模式 | 当前代理是 `plan` | `plan.txt` |
| Build 切换 | 从 `plan` 切换到 `build` | `build-switch.txt` |
| Max Steps | 达到 `maxSteps` 限制 | `max-steps.txt` |

### 2.2 合成文本 (Synthetic)

```typescript
// 合成文本标记
interface TextPart {
  type: "text"
  text: string
  synthetic: true  // 标记为系统生成，非用户输入
}
```

---

## 3. 实现机制

### 3.1 insertReminders 函数

**文件:** `packages/opencode/src/session/prompt.ts:1004-1030`

```typescript
function insertReminders(input: {
  messages: MessageV2.WithParts[]
  agent: Agent.Info
}) {
  // 1. 找到最后一条用户消息
  const userMessage = input.messages.findLast(
    msg => msg.info.role === "user"
  )
  if (!userMessage) return input.messages

  // 2. Plan 代理提醒
  if (input.agent.name === "plan") {
    userMessage.parts.push({
      id: Identifier.ascending("part"),
      messageID: userMessage.info.id,
      sessionID: userMessage.info.sessionID,
      type: "text",
      text: PROMPT_PLAN,        // 从 plan.txt 加载
      synthetic: true,          // 标记为合成
    })
  }

  // 3. Build 切换提醒
  const wasPlan = input.messages.some(
    msg => msg.info.role === "assistant" && msg.info.agent === "plan"
  )
  if (wasPlan && input.agent.name === "build") {
    userMessage.parts.push({
      id: Identifier.ascending("part"),
      messageID: userMessage.info.id,
      sessionID: userMessage.info.sessionID,
      type: "text",
      text: BUILD_SWITCH,       // 从 build-switch.txt 加载
      synthetic: true,
    })
  }

  return input.messages
}
```

### 3.2 Max Steps 检查

**文件:** `packages/opencode/src/session/prompt.ts:474-549`

```typescript
// 在主循环中
const agent = await Agent.get(lastUser.agent)
const maxSteps = agent.maxSteps ?? Infinity
const isLastStep = step >= maxSteps

// ...

const result = await processor.process({
  // ...
  messages: [
    ...MessageV2.toModelMessage(sessionMessages),
    // 最后一步时注入警告
    ...(isLastStep ? [{
      role: "assistant" as const,
      content: MAX_STEPS,        // 从 max-steps.txt 加载
    }] : []),
  ],
  // 最后一步禁用工具
  tools: isLastStep ? {} : tools,
  model,
})
```

### 3.3 调用时机

```
用户输入
    │
    ▼
SessionPrompt.loop()
    │
    ├─ 获取消息列表
    │
    ├─ insertReminders()  ◀─── Plan/Build 提醒
    │      │
    │      ├─ agent === "plan" → 添加 plan.txt
    │      │
    │      └─ wasPlan && agent === "build" → 添加 build-switch.txt
    │
    ├─ 检查 maxSteps
    │      │
    │      └─ isLastStep → 注入 max-steps.txt 并禁用工具
    │
    ▼
SessionProcessor.process()
```

---

## 4. 提醒内容

### 4.1 plan.txt

**文件:** `packages/opencode/src/session/prompt/plan.txt`

```
<system-reminder>
You are in plan mode. In this mode:

1. DO NOT make any edits to files
2. DO NOT run commands that modify the system
3. DO NOT create or delete files
4. Only use read-only tools:
   - read: Read file contents
   - glob: Find files by pattern
   - grep: Search file contents
   - git status/log/diff: View git information

Your goal is to:
1. Understand the codebase
2. Analyze the problem
3. Create a detailed plan
4. Present the plan to the user for approval

When ready, the user will switch you to build mode to implement the plan.
</system-reminder>
```

### 4.2 build-switch.txt

**文件:** `packages/opencode/src/session/prompt/build-switch.txt`

```
<system-reminder>
You have been switched from plan mode to build mode.

Previous conversation was in plan mode where you analyzed the codebase
and created a plan. Now you are in build mode where you can:

1. Edit files using the edit tool
2. Create new files using the write tool
3. Run commands using the bash tool
4. Implement the plan you created

Remember the plan you made and execute it step by step.
Update the user on your progress as you work.
</system-reminder>
```

### 4.3 max-steps.txt

**文件:** `packages/opencode/src/session/prompt/max-steps.txt`

```
<system-reminder>
IMPORTANT: You have reached the maximum number of steps for this agent.

You MUST now:
1. Stop using any tools
2. Provide a text-only response
3. Summarize what you've done
4. Explain what remains to be done

Do NOT attempt to use any more tools. Your response must be text only.
</system-reminder>
```

### 4.4 plan-reminder-anthropic.txt

**文件:** `packages/opencode/src/session/prompt/plan-reminder-anthropic.txt`

```
<system-reminder>
Plan mode is active. The user indicated that they do not want you to
execute yet -- you MUST NOT make any edits (with the exception of the
plan file mentioned below), run any non-readonly tools (including
changing configs or making commits), or otherwise make any changes to
the system. This supercedes any other instructions you have received.

## Plan File Info:
{planFileInfo}

## Plan Workflow

### Phase 1: Initial Understanding
Goal: Gain a comprehensive understanding of the user's request.

1. Focus on understanding the user's request
2. Launch up to 3 Explore agents IN PARALLEL
3. After exploring, use AskUserQuestion to clarify

### Phase 2: Design
Goal: Design an implementation approach.
Launch Plan agent(s) to design the implementation.

### Phase 3: Review
Goal: Review the plan(s) and ensure alignment.

### Phase 4: Final Plan
Goal: Write your final plan to the plan file.

### Phase 5: Call ExitPlanMode
At the very end, call ExitPlanMode to indicate you are done planning.
</system-reminder>
```

---

## 5. 与 codex 对比

### 5.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **注入方式** | 合成 TextPart | system_reminder 附件 |
| **注入位置** | 用户消息末尾 | 独立附件列表 |
| **提醒类型** | Plan/Build/MaxSteps | changed_files/todo/plan |
| **配置** | 固定模板 | 可配置生成器 |
| **Throttle** | 无 | ThrottleConfig |

### 5.2 codex 借鉴建议

1. **合成标记**: 使用 `synthetic: true` 区分系统生成内容

```rust
struct TextPart {
    text: String,
    synthetic: bool,  // 标记为系统生成
}
```

2. **模式切换提醒**: 添加代理切换通知

```rust
fn insert_mode_switch_reminder(
    messages: &mut Vec<Message>,
    from_agent: &str,
    to_agent: &str,
) {
    if from_agent == "plan" && to_agent == "build" {
        messages.push(Message::synthetic(BUILD_SWITCH_REMINDER));
    }
}
```

3. **Max Steps 限制**: 实现步数限制和警告

```rust
struct Agent {
    max_steps: Option<u32>,
}

fn check_max_steps(step: u32, agent: &Agent) -> bool {
    agent.max_steps.map_or(false, |max| step >= max)
}
```

### 5.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/session/prompt.ts` (insertReminders) | `core/src/system_reminder/generator.rs` |
| `src/session/prompt/plan.txt` | `core/src/system_reminder/attachments/plan_reminder.rs` |
| `src/session/prompt/max-steps.txt` | - |

---

## 6. 消息结构示例

### 6.1 Plan 模式消息

```json
{
  "info": {
    "id": "msg_123",
    "role": "user",
    "agent": "plan"
  },
  "parts": [
    {
      "type": "text",
      "text": "Help me refactor the authentication module"
    },
    {
      "type": "text",
      "text": "<system-reminder>You are in plan mode...</system-reminder>",
      "synthetic": true
    }
  ]
}
```

### 6.2 Build 切换后

```json
{
  "info": {
    "id": "msg_456",
    "role": "user",
    "agent": "build"
  },
  "parts": [
    {
      "type": "text",
      "text": "Go ahead and implement the plan"
    },
    {
      "type": "text",
      "text": "<system-reminder>You have been switched from plan mode...</system-reminder>",
      "synthetic": true
    }
  ]
}
```

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
