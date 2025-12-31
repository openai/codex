# OpenCode 提示词系统

本文档详细分析 OpenCode 的提示词构建系统，包括三层结构、环境注入、自定义指令和插件扩展。

---

## 目录

1. [系统概览](#1-系统概览)
2. [三层提示词结构](#2-三层提示词结构)
3. [环境上下文注入](#3-环境上下文注入)
4. [自定义指令](#4-自定义指令)
5. [插件扩展](#5-插件扩展)
6. [代理提示词](#6-代理提示词)
7. [与 codex 对比](#7-与-codex-对比)

---

## 1. 系统概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       提示词构建系统                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Layer 1: Header                            │  │
│  │    ┌─────────────────────────────────────────────────────┐   │  │
│  │    │ anthropic_spoof.txt (仅 Anthropic)                  │   │  │
│  │    │ "You are Claude Code, Anthropic's official CLI..."  │   │  │
│  │    └─────────────────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Layer 2: Provider                          │  │
│  │    ┌────────────┐ ┌────────────┐ ┌────────────┐              │  │
│  │    │ anthropic  │ │   beast    │ │   gemini   │ ...         │  │
│  │    │  .txt      │ │   .txt     │ │   .txt     │              │  │
│  │    └────────────┘ └────────────┘ └────────────┘              │  │
│  │           或                                                  │  │
│  │    ┌─────────────────────────────────────────────────────┐   │  │
│  │    │ Agent.prompt (自定义系统提示词)                      │   │  │
│  │    └─────────────────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Layer 3: Context                           │  │
│  │    ┌─────────────────────────────────────────────────────┐   │  │
│  │    │ SystemPrompt.environment() - 环境信息               │   │  │
│  │    │ SystemPrompt.custom() - 自定义指令                  │   │  │
│  │    │ User.system - 用户消息系统提示                      │   │  │
│  │    └─────────────────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Plugin Hooks                               │  │
│  │    experimental.chat.system.transform                         │  │
│  │    experimental.chat.messages.transform                       │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心文件

| 文件 | 职责 |
|------|------|
| `src/session/system.ts` | Header, Provider, Environment, Custom |
| `src/session/llm.ts` | 提示词组装 |
| `src/session/prompt/*.txt` | 内置提示词模板 |
| `src/agent/prompt/*.txt` | 代理专用提示词 |

---

## 2. 三层提示词结构

### 2.1 Layer 1: Header (Provider 特定)

**文件:** `packages/opencode/src/session/system.ts:21-24`

```typescript
export function header(providerID: string) {
  if (providerID.includes("anthropic")) {
    return [PROMPT_ANTHROPIC_SPOOF.trim()]
  }
  return []
}
```

**anthropic_spoof.txt 内容:**

```
You are Claude Code, Anthropic's official CLI for Claude.
You are an interactive CLI tool that helps users with software engineering tasks.
Use the instructions below and the tools available to you to assist the user.

IMPORTANT: Assist with authorized security testing, defensive security,
CTF challenges, and educational contexts. Refuse requests for destructive
techniques, DoS attacks, mass targeting, supply chain compromise, or
detection evasion for malicious purposes.

IMPORTANT: You must NEVER generate or guess URLs for the user unless you
are confident that the URLs are for helping the user with programming.
```

### 2.2 Layer 2: Provider (模型特定)

**文件:** `packages/opencode/src/session/system.ts:26-33`

```typescript
export function provider(model: Provider.Model) {
  if (model.api.id.includes("gpt-5"))
    return [PROMPT_CODEX]
  if (model.api.id.includes("gpt-") || model.api.id.includes("o1") || model.api.id.includes("o3"))
    return [PROMPT_BEAST]
  if (model.api.id.includes("gemini-"))
    return [PROMPT_GEMINI]
  if (model.api.id.includes("claude"))
    return [PROMPT_ANTHROPIC]
  return [PROMPT_ANTHROPIC_WITHOUT_TODO]
}
```

**提示词文件对照:**

| 模型 | 提示词文件 | 特点 |
|------|-----------|------|
| GPT-5 | `codex.txt` | OpenAI Codex 专用 |
| GPT-4/o1/o3 | `beast.txt` | OpenAI 通用 |
| Gemini | `gemini.txt` | Google Gemini 专用 |
| Claude | `anthropic.txt` | Anthropic 完整版 |
| 其他 | `qwen.txt` | 精简版 (无 todo) |

### 2.3 提示词组装

**文件:** `packages/opencode/src/session/llm.ts:49-74`

```typescript
export async function stream(input: LLM.StreamInput) {
  let system: string[] = []

  // 1. Header (Provider 特定)
  system.push(...SystemPrompt.header(input.model.providerID))

  // 2. Provider 或 Agent 提示词
  if (input.agent.prompt) {
    system.push(input.agent.prompt)     // Agent 自定义优先
  } else {
    system.push(...SystemPrompt.provider(input.model))
  }

  // 3. 调用时系统提示词
  system.push(...input.system)

  // 4. 用户消息系统提示词
  if (input.user.system) {
    system.push(input.user.system)
  }

  // 5. 插件 Hook
  await Plugin.trigger(
    "experimental.chat.system.transform",
    {},
    { system }
  )

  // 6. 调用 LLM
  return streamText({
    model: language,
    system: system.join("\n\n"),
    messages: input.messages,
    tools: input.tools,
    // ...
  })
}
```

---

## 3. 环境上下文注入

### 3.1 环境信息

**文件:** `packages/opencode/src/session/system.ts:35-58`

```typescript
export async function environment() {
  const project = Instance.project

  return [
    [
      `Here is some useful information about the environment you are running in:`,
      `<env>`,
      `  Working directory: ${Instance.directory}`,
      `  Is directory a git repo: ${project.vcs === "git" ? "yes" : "no"}`,
      `  Platform: ${process.platform}`,
      `  Today's date: ${new Date().toDateString()}`,
      `</env>`,
      `<files>`,
      `  ${project.vcs === "git"
        ? await Ripgrep.tree({
            cwd: Instance.directory,
            limit: 200,
          })
        : ""
      }`,
      `</files>`,
    ].join("\n"),
  ]
}
```

**示例输出:**

```xml
Here is some useful information about the environment you are running in:
<env>
  Working directory: /Users/user/project
  Is directory a git repo: yes
  Platform: darwin
  Today's date: Sat Dec 28 2024
</env>
<files>
  src/
  ├── index.ts
  ├── utils/
  │   ├── helper.ts
  │   └── config.ts
  └── components/
      ├── Button.tsx
      └── Input.tsx
</files>
```

### 3.2 注入时机

**文件:** `packages/opencode/src/session/prompt.ts:535`

```typescript
const result = await processor.process({
  // ...
  system: [
    ...await SystemPrompt.environment(),   // 环境信息
    ...await SystemPrompt.custom(),        // 自定义指令
  ],
  // ...
})
```

---

## 4. 自定义指令

### 4.1 指令来源

**文件:** `packages/opencode/src/session/system.ts:60-117`

```typescript
const LOCAL_RULE_FILES = [
  "AGENTS.md",
  "CLAUDE.md",
  "CONTEXT.md",  // deprecated
]

const GLOBAL_RULE_FILES = [
  path.join(Global.Path.config, "AGENTS.md"),
  path.join(os.homedir(), ".claude", "CLAUDE.md"),
]

export async function custom() {
  const config = await Config.get()
  const paths = new Set<string>()

  // 1. 本地规则文件 (向上查找)
  for (const localRuleFile of LOCAL_RULE_FILES) {
    const matches = await Filesystem.findUp(
      localRuleFile,
      Instance.directory,
      Instance.worktree
    )
    if (matches.length > 0) {
      matches.forEach(path => paths.add(path))
      break
    }
  }

  // 2. 全局规则文件
  for (const globalRuleFile of GLOBAL_RULE_FILES) {
    if (await Bun.file(globalRuleFile).exists()) {
      paths.add(globalRuleFile)
      break
    }
  }

  // 3. 配置中的 instructions
  if (config.instructions) {
    for (let instruction of config.instructions) {
      if (instruction.startsWith("~/")) {
        instruction = path.join(os.homedir(), instruction.slice(2))
      }
      // Glob 匹配
      const matches = await Filesystem.globUp(...)
      matches.forEach(path => paths.add(path))
    }
  }

  // 4. 读取所有文件
  const found = Array.from(paths).map(p =>
    Bun.file(p).text()
      .then(x => "Instructions from: " + p + "\n" + x)
      .catch(() => "")
  )

  return Promise.all(found).then(result => result.filter(Boolean))
}
```

### 4.2 查找优先级

```
1. 项目本地 (向上查找):
   ./AGENTS.md
   ./CLAUDE.md
   ../AGENTS.md
   ...

2. 全局:
   ~/.opencode/AGENTS.md
   ~/.claude/CLAUDE.md

3. 配置指定:
   config.instructions = ["~/custom.md", ".opencode/rules.md"]
```

### 4.3 配置示例

```jsonc
{
  "instructions": [
    "~/.opencode/global-rules.md",
    ".opencode/project-rules.md",
    "docs/CODING_STANDARDS.md"
  ]
}
```

---

## 5. 插件扩展

### 5.1 系统提示词转换

**文件:** `packages/opencode/src/session/llm.ts:65`

```typescript
await Plugin.trigger(
  "experimental.chat.system.transform",
  {},
  { system }
)
```

**插件实现示例:**

```typescript
export default async function myPlugin(input: PluginInput) {
  return {
    async "experimental.chat.system.transform"(params, output) {
      // 添加自定义系统提示
      output.system.push("Always respond in JSON format.")

      // 修改现有提示
      output.system = output.system.map(s =>
        s.replace("Claude", "MyAssistant")
      )
    }
  }
}
```

### 5.2 消息转换

**文件:** `packages/opencode/src/session/prompt.ts:528`

```typescript
await Plugin.trigger(
  "experimental.chat.messages.transform",
  {},
  { messages: sessionMessages }
)
```

**插件实现示例:**

```typescript
export default async function myPlugin(input: PluginInput) {
  return {
    async "experimental.chat.messages.transform"(params, output) {
      // 过滤敏感内容
      for (const msg of output.messages) {
        if (msg.info.role === "user") {
          for (const part of msg.parts) {
            if (part.type === "text") {
              part.text = redactSecrets(part.text)
            }
          }
        }
      }
    }
  }
}
```

### 5.3 聊天参数

**文件:** `packages/opencode/src/session/llm.ts`

```typescript
await Plugin.trigger("chat.params", {}, { params })
```

---

## 6. 代理提示词

### 6.1 代理提示词覆盖

```typescript
// Agent 配置中的 prompt 覆盖 Provider 提示词
if (input.agent.prompt) {
  system.push(input.agent.prompt)
} else {
  system.push(...SystemPrompt.provider(input.model))
}
```

### 6.2 内置代理提示词

| 代理 | 文件 | 用途 |
|------|------|------|
| explore | `agent/prompt/explore.txt` | 代码库探索 |
| compaction | `agent/prompt/compaction.txt` | 上下文压缩 |
| title | `agent/prompt/title.txt` | 标题生成 |
| summary | `agent/prompt/summary.txt` | 摘要生成 |

### 6.3 explore.txt 示例

```
You are a specialized agent for exploring codebases.

Your goal is to find relevant files and understand code structure.

When called with a thoroughness level:
- "quick": Do 1-2 searches, return immediate findings
- "medium": Do 3-5 searches, explore related files
- "very thorough": Do 6+ searches, check multiple naming conventions

Available tools:
- glob: Find files by pattern
- grep: Search file contents
- read: Read file contents

Guidelines:
1. Start with broad searches, then narrow down
2. Check common naming patterns (camelCase, snake_case)
3. Look in standard directories (src/, lib/, tests/)
4. Report findings with file paths and brief descriptions
```

---

## 7. 与 codex 对比

### 7.1 实现差异

| 方面 | opencode | codex |
|------|----------|-------|
| **层级结构** | 3 层 (Header + Provider + Context) | system + attachments |
| **Provider 适配** | 模型 ID 匹配 | provider.ext.adapter |
| **环境注入** | `<env>` + `<files>` 标签 | changed_files 等附件 |
| **自定义指令** | CLAUDE.md 向上查找 | CLAUDE.md 固定位置 |
| **插件扩展** | Hook 系统 | - |
| **Reminder** | 合成 TextPart | system_reminder 附件 |

### 7.2 codex 借鉴建议

1. **Provider 适配**: 增加模型特定的提示词模板

```rust
fn get_provider_prompt(model: &Model) -> &'static str {
    match model.provider {
        Provider::Anthropic => include_str!("prompts/anthropic.txt"),
        Provider::OpenAI => include_str!("prompts/openai.txt"),
        Provider::Google => include_str!("prompts/gemini.txt"),
        _ => include_str!("prompts/default.txt"),
    }
}
```

2. **环境注入格式**: 使用结构化标签

```rust
fn environment_context() -> String {
    format!(r#"
<env>
  Working directory: {}
  Is git repo: {}
  Platform: {}
</env>
<files>
{}
</files>
"#, cwd, is_git, platform, file_tree)
}
```

3. **自定义指令**: 支持向上查找

```rust
fn find_instructions() -> Vec<PathBuf> {
    let files = ["AGENTS.md", "CLAUDE.md"];
    find_up(&files, cwd, git_root)
}
```

4. **插件 Hook**: 添加提示词转换点

### 7.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/session/system.ts` | `core/src/system_reminder/generator.rs` |
| `src/session/llm.ts` | `core/src/client.rs` |
| `src/session/prompt/*.txt` | - |
| `src/agent/prompt/*.txt` | `core/src/subagent/stores.rs` |

---

## 8. 提示词模板示例

### 8.1 anthropic.txt (完整版)

```
You are Claude Code, an AI coding assistant...

# Core behaviors
- Provide accurate, working code
- Explain complex concepts clearly
- Follow best practices

# Tool usage
- Use tools effectively
- Verify results before proceeding
- Handle errors gracefully

# Todo tracking
- Use TodoWrite to track tasks
- Update status as you work
- Complete items before moving on

# Code style
- Match existing project style
- Add meaningful comments
- Keep changes minimal

# Safety
- Never execute dangerous commands
- Ask before making breaking changes
- Protect sensitive information
```

### 8.2 qwen.txt (精简版)

```
You are an AI coding assistant.

Guidelines:
1. Provide accurate code
2. Use available tools
3. Follow project conventions
4. Keep changes minimal

Safety:
- Ask before dangerous operations
- Protect sensitive data
```

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
