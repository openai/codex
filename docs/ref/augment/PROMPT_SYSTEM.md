# Augment Prompt 系统分析

## 文档信息
- **分析时间**: 2025-12-04
- **源文件**: `chunks.72.mjs`, `chunks.82.mjs`, `chunks.96.mjs`, `chunks.61.mjs`
- **分析范围**: System Prompt 构建与管理机制

---

## 核心发现

### System Prompt 架构

Augment 使用**可配置的 System Prompt** + **动态替换**机制，而非硬编码的 prompt。

---

## 1. System Prompt 传递机制

### 1.1 API 请求参数

**文件位置**: `chunks.72.mjs:335-372`

```javascript
async chatStream(
    requestId,
    message,
    chatHistory,
    blobs,
    userGuidedBlobs,
    externalSourceIds,
    modelId,
    contextCodeExchangeRequestId,
    // ... 其他参数
    systemPrompt,           // ← System Prompt
    systemPromptReplacements // ← Prompt 替换规则
) {
    const config = this._configListener.config;

    // 选择模型
    if (mode === "AGENT") {
        modelId = modelId ?? config.agent.model;
    } else {
        modelId = modelId ?? config.chat.model;
    }

    const payload = {
        model: modelId,
        message: message,
        chat_history: chatHistory,
        blobs: blobs,
        tool_definitions: toolDefinitions ?? [],
        nodes: nodes ?? [],
        mode: mode ?? "CHAT",
        agent_memories: agentMemories,
        rules: rules ?? [],
        enable_parallel_tool_use: enableParallelToolUse,
        conversation_id: conversationId,
        system_prompt: systemPrompt,              // ← 传递给后端
        ...systemPromptReplacements && {
            system_prompt_replacements: systemPromptReplacements
        }
    };

    return this.callApiStream(
        requestId,
        config,
        "chat-stream",
        payload,
        ...
    );
}
```

### 1.2 配置来源

**文件位置**: `chunks.96.mjs:215`

```javascript
// 从配置中读取 system prompt
systemPrompt = this.config.configuration.systemPrompt;
systemPromptReplacements = this.config.configuration.systemPromptReplacements;
```

**文件位置**: `chunks.61.mjs:1333-1599`

```javascript
class AgentState {
    _systemPrompt = undefined;
    _systemPromptReplacements = undefined;

    constructor(
        remoteAgentId,
        userGuidelines,
        workspaceGuidelines,
        agentMemories,
        modelId,
        rules,
        systemPrompt,              // ← 构造时传入
        systemPromptReplacements,  // ← 构造时传入
        botType
    ) {
        this._systemPrompt = systemPrompt;
        this._systemPromptReplacements = systemPromptReplacements;
    }

    get systemPrompt() {
        return this._systemPrompt;
    }

    get systemPromptReplacements() {
        return this._systemPromptReplacements;
    }
}
```

---

## 2. 已识别的 System Prompts

### 2.1 Orchestrator Agent Prompt

**文件位置**: `chunks.82.mjs:2236`

```
You are an orchestrator agent that manages a sub-agent to complete complex
tasks efficiently. You are a smart but expensive model, while your sub-agent
is a cheaper but less intelligent model. Your role is to provide strategic
direction, detailed instructions, and quality control.

When appropriate, delegate tasks to the subagents who will report back the
work that they've done.
```

**分析**：
- **角色定位**: Orchestrator（协调者）
- **模型定位**: 昂贵但智能
- **职责**: 提供战略方向、详细指令、质量控制
- **工作模式**: 委派任务给 sub-agent

### 2.2 Sub-Agent Prompt

**文件位置**: `chunks.95.mjs:1798`

```
You are a sub-agent working under the direction of an orchestrator agent.
The orchestrator is a smart but expensive model that provides strategic
direction, while you are a cheaper but capable model focused on execution.
Your role is to follow a scoped task to get it done.
```

**分析**：
- **角色定位**: Sub-agent（执行者）
- **模型定位**: 便宜但有能力
- **职责**: 执行特定任务
- **工作模式**: 接收指令并完成

### 2.3 非交互模式 Prompt

**文件位置**: `chunks.82.mjs:2230-2234`

```
 * You are running in an automated workflow and the user is not available.
 * Use the information you have to accomplish the task to the best of your ability.
 * NEVER ask clarifying questions as they cannot be answered.
 * ALWAYS persist until the task is complete without stopping early.
```

**分析**：
- **适用场景**: Remote Agent / 自动化工作流
- **关键约束**: 不能提问、必须坚持完成
- **设计意图**: 完全自主执行

---

## 3. Prompt 替换机制

### 3.1 工作原理

```typescript
interface SystemPromptReplacements {
    [key: string]: string;
}
```

**示例**：
```javascript
const systemPrompt = `
Hello {{user_name}}, you are working on project {{project_name}}.
Your task is to {{task_description}}.
`;

const replacements = {
    "user_name": "Alice",
    "project_name": "MyApp",
    "task_description": "fix the authentication bug"
};

// 后端会将 {{key}} 替换为对应的值
// 最终 prompt: "Hello Alice, you are working on project MyApp..."
```

### 3.2 使用场景

**文件位置**: `chunks.84.mjs:1480, 1548`

```javascript
// 场景 1: Agent 模式
await apiServer.chatStream(
    requestId,
    message,
    chatHistory,
    blobs,
    userGuidelines,
    workspaceGuidelines,
    toolDefinitions,
    requestNodes,
    chatMode,
    agentMemories,
    rules,
    conversationId,
    abortSignal,
    this.state.systemPrompt,              // ← 传递
    this.state.systemPromptReplacements   // ← 传递
);

// 场景 2: Silent 模式
await apiServer.chatStream(
    ...,
    silent,
    enableParallelTools,
    conversationId,
    abortSignal,
    this.state.systemPrompt,
    this.state.systemPromptReplacements
);
```

---

## 4. Prompt 组成要素

根据代码分析，完整的 Prompt 包括以下部分：

### 4.1 基础 System Prompt

```
[Base System Prompt - 角色和职责定义]
```

### 4.2 User Guidelines（用户指南）

**文件位置**: `chunks.72.mjs:351, 509`

```javascript
payload.user_guidelines = userGuidelines;
```

- 来源：用户配置或 API 参数
- 作用：自定义 Agent 行为

### 4.3 Workspace Guidelines（工作空间指南）

```javascript
payload.workspace_guidelines = workspaceGuidelines;
```

- 来源：项目配置文件（如 `.augment/guidelines.md`）
- 作用：项目特定的规则和约定

### 4.4 Agent Memories（Agent 记忆）

```javascript
payload.agent_memories = agentMemories;
```

- 来源：持久化的 Agent 记忆
- 作用：跨会话的上下文保持

### 4.5 Rules（规则）

```javascript
payload.rules = rules ?? [];
```

- 来源：配置的规则列表
- 作用：约束 Agent 行为

### 4.6 Tool Definitions（工具定义）

```javascript
payload.tool_definitions = toolDefinitions ?? [];
```

- 来源：根据 mode 动态加载的工具集
- 作用：告诉 LLM 可以使用哪些工具

---

## 5. Prompt 构建流程

```
┌─────────────────────────────────────────────┐
│ 1. 加载配置                                   │
│    - systemPrompt (from config)              │
│    - systemPromptReplacements (from config)  │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ 2. 收集动态内容                               │
│    - User Guidelines                         │
│    - Workspace Guidelines                    │
│    - Agent Memories                          │
│    - Rules                                   │
│    - Chat History                            │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ 3. 构建 Tool Definitions                     │
│    - 根据 chatMode 选择工具集                 │
│    - AGENT mode: 完整工具                    │
│    - CHAT mode: 基础工具                     │
│    - REMOTE_AGENT mode: 远程工具             │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ 4. 应用 Prompt 替换                          │
│    - {{key}} → value                         │
│    - 动态插入用户名、项目名等                  │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ 5. 组装完整 Payload                          │
│    {                                         │
│      model: "...",                           │
│      message: "...",                         │
│      chat_history: [...],                    │
│      tool_definitions: [...],                │
│      system_prompt: "...",                   │
│      system_prompt_replacements: {...},      │
│      user_guidelines: "...",                 │
│      workspace_guidelines: "...",            │
│      agent_memories: "...",                  │
│      rules: [...]                            │
│    }                                         │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ 6. 发送到 Backend                            │
│    POST /chat-stream                         │
└─────────────────────────────────────────────┘
```

---

## 6. 不同模式的 Prompt 差异

| 模式 | System Prompt | Tools | 特点 |
|------|--------------|-------|------|
| **CHAT** | 基础对话 prompt | 基础工具（View, Search） | 简单问答 |
| **AGENT** | Agent prompt | 完整工具（Edit, Execute, Task等） | 自主执行 |
| **REMOTE_AGENT** | 自动化 prompt | 远程工具 | 无需用户交互 |
| **CLI_AGENT** | CLI prompt | CLI 工具 + Task | 命令行模式 |
| **CLI_NONINTERACTIVE** | 非交互 prompt | 基础 CLI 工具 | 脚本执行 |
| **MEMORIES** | 记忆管理 prompt | 记忆相关工具 | 记忆操作 |
| **ORIENTATION** | 方向引导 prompt | 引导工具 | 项目理解 |

---

## 7. Prompt 工程技巧（从代码推断）

### 7.1 角色定位清晰

```
✅ "You are an orchestrator agent..."
❌ "You help users..."
```

- 明确角色（orchestrator vs sub-agent）
- 明确能力边界（smart but expensive）

### 7.2 行为约束

```
✅ "NEVER ask clarifying questions"
✅ "ALWAYS persist until complete"
❌ 模糊的建议
```

- 使用绝对化语言（NEVER, ALWAYS）
- 明确禁止和要求的行为

### 7.3 工具使用指导

在工具定义的 `description` 字段中：
- 详细说明工具用途
- 提供使用示例
- 明确参数要求

---

## 8. System Prompt 最佳实践（从 Augment 学习）

### 8.1 分层设计

```
Base Prompt (不变)
  ↓
+ User Guidelines (用户自定义)
  ↓
+ Workspace Guidelines (项目特定)
  ↓
+ Dynamic Context (当前会话)
```

### 8.2 使用替换变量

```python
# 而非硬编码：
"Hello Alice, you are working on MyApp..."

# 使用模板：
"Hello {{user_name}}, you are working on {{project_name}}..."
```

**优点**：
- 可配置
- 可复用
- 易于测试

### 8.3 明确工作模式

```
Automated mode: "You are running in an automated workflow..."
Interactive mode: "You can ask the user for clarification..."
```

---

## 9. 待深入分析的问题

### 已回答 ✅

1. **System Prompt 如何传递？** → 通过 `chatStream` API 的 `system_prompt` 参数
2. **Prompt 是否可配置？** → 是，从 `config.configuration.systemPrompt` 读取
3. **是否有 Prompt 模板？** → 是，使用 `{{key}}` 替换机制
4. **不同模式有不同 Prompt 吗？** → 是，orchestrator / sub-agent / automated

### 待回答 ❓

5. **Backend 如何处理 system_prompt？** → 需要查看后端代码
6. **实际的完整 System Prompt 内容？** → 需要运行时抓包或配置文件
7. **Prompt 长度限制？** → 未在代码中找到明确限制
8. **Few-shot 示例在哪里？** → 未在当前代码中发现

---

## 10. 与其他 Agent 系统对比

| 特性 | Augment | Cursor | GitHub Copilot |
|------|---------|--------|----------------|
| **Prompt 配置** | 可配置 + 替换机制 | 固定 | 固定 |
| **分层设计** | ✅ 多层（base+user+workspace） | ✅ | ❌ |
| **动态替换** | ✅ {{key}} 模式 | ❌ | ❌ |
| **多模式支持** | ✅ 7种模式 | ✅ 2-3种 | ❌ |
| **工具集成** | ✅ 动态加载 | ✅ | ✅ |
| **用户自定义** | ✅ User Guidelines | ✅ | ❌ |
| **项目规则** | ✅ Workspace Guidelines | ✅ .cursorrules | ❌ |

---

## 11. 关键代码位置总结

| 功能 | 文件 | 行号 | 说明 |
|------|------|------|------|
| chatStream API | chunks.72.mjs | 335-372 | 主要的 LLM 调用接口 |
| System Prompt 配置 | chunks.96.mjs | 215 | 从配置读取 |
| Agent State | chunks.61.mjs | 1333-1599 | systemPrompt 状态管理 |
| Orchestrator Prompt | chunks.82.mjs | 2236 | Orchestrator 角色定义 |
| Sub-agent Prompt | chunks.95.mjs | 1798 | Sub-agent 角色定义 |
| Automated Prompt | chunks.82.mjs | 2230-2234 | 自动化模式约束 |

---

## 12. 下一步分析

1. **查找完整 System Prompt**
   - 搜索配置文件
   - 运行时抓包

2. **分析 Prompt 效果**
   - 测试不同 prompt 对 Agent 行为的影响
   - A/B 测试不同的指令方式

3. **研究 Tool Description**
   - 工具描述如何影响 LLM 使用工具
   - Few-shot 示例的最佳实践

---

**创建时间**: 2025-12-04
**分析状态**: ✅ 基础分析完成 → 🔄 等待完整 Prompt 提取
