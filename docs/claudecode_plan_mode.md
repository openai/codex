# Claude Code Plan Mode 设计与实现分析

> 基于 Claude Code v2.0.59 源码分析

## 1. 概述

### 1.1 设计理念

Plan Mode 是 Claude Code 中的一个**只读探索和规划阶段**，在此模式下，助手专注于理解代码库和设计实现策略，而不是直接编写代码。它为需要前期规划的复杂任务提供了一个结构化的工作流程。

**核心设计原则：**

1. **强制前期思考** - 在代码更改之前进行充分的探索和规划
2. **防止过早实现** - 通过只读限制避免仓促编码
3. **鼓励模式探索** - 理解现有代码模式和架构
4. **支持并行探索** - 通过多个子代理高效探索代码库
5. **上下文持久化** - 计划和待办事项在对话压缩后仍然保留

### 1.2 适用场景

**应该使用 Plan Mode 的场景：**
- 多种有效方案的任务（如添加缓存 - Redis vs 内存 vs 文件）
- 重大架构决策（如实时更新 - WebSocket vs SSE vs 轮询）
- 大规模变更（涉及多个文件或系统的重构）
- 需求不明确（需要先探索才能理解全貌）
- 需要用户输入来澄清方案

**不应该使用 Plan Mode 的场景：**
- 简单、直接的任务
- 小型 bug 修复（解决方案明确）
- 添加单个函数或小功能
- 已有信心实现的任务
- 纯研究任务（应使用 Task 工具的 Explore 代理）

---

## 2. 核心架构

### 2.1 状态机设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PLAN MODE STATE MACHINE                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌──────────────┐    EnterPlanMode Tool    ┌───────────────────┐          │
│   │    Normal    │ ─────────────────────────>│    Plan Mode      │          │
│   │    Mode      │                           │    Active         │          │
│   │(mode=default)│                           │   (mode=plan)     │          │
│   └──────────────┘                           └─────────┬─────────┘          │
│         ^                                              │                     │
│         │                                              │                     │
│         │         ExitPlanMode Tool                    │                     │
│         │  ┌───────────────────────────────────────────┘                     │
│         │  │                                                                 │
│         │  ▼                                                                 │
│   ┌─────────────────────────────────────────────────────────────┐           │
│   │               Mode Selection on Exit                         │           │
│   │  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │           │
│   │  │   default   │  │ acceptEdits  │  │ bypassPermissions │   │           │
│   │  │  (ask each) │  │(auto-approve │  │  (skip all asks)  │   │           │
│   │  │             │  │  file edits) │  │                   │   │           │
│   │  └─────────────┘  └──────────────┘  └───────────────────┘   │           │
│   └─────────────────────────────────────────────────────────────┘           │
│                                                                             │
│   RE-ENTRY DETECTION:                                                       │
│   ┌─────────────────────────────────────────────────────────────┐           │
│   │  if (hasExitedPlanMode && planFileExists) {                 │           │
│   │    → Generate "plan_mode_reentry" attachment                │           │
│   │    → Prompt to evaluate: same task vs different task        │           │
│   │  }                                                          │           │
│   └─────────────────────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 状态存储

Plan Mode 状态存储在 `toolPermissionContext` 中：

```javascript
// Location: chunks.16.mjs:1122-1128
// 权限上下文 reducer
function applyPermissionContextAction(currentContext, action) {
  switch (action.type) {
    case "setMode":
      logDebug(`Applying permission update: Setting mode to '${action.mode}'`);
      return {
        ...currentContext,
        mode: action.mode  // "plan" | "default" | "acceptEdits" | "bypassPermissions"
      };
  }
}
```

**模式类型：**

| Mode | Description | Tool Permission |
|------|-------------|-----------------|
| `default` | 正常模式，每个操作都询问 | 逐一询问 |
| `plan` | Plan Mode 激活，只读限制 | 只允许只读工具 |
| `acceptEdits` | 自动批准文件编辑 | 自动批准 Edit/Write |
| `bypassPermissions` | 跳过所有权限询问 | 全部自动批准 |

### 2.3 全局状态标志

```javascript
// Location: chunks.1.mjs:2807-2812
// 全局状态对象 WQ
function hasExitedPlanMode() {
  return globalState.hasExitedPlanMode;
}

function setHasExitedPlanMode(value) {
  globalState.hasExitedPlanMode = value;
}

// 用于检测 Plan Mode 重入
// 如果 hasExitedPlanMode = true 且 plan 文件存在
// → 显示 "plan_mode_reentry" 附件
```

---

## 3. 计划文件持久化

### 3.1 存储位置与命名

**目录：** `~/.claude/plans/`

**文件命名：** `{adjective}-{action}-{noun}.md`

```javascript
// Location: chunks.88.mjs:770-785
// 随机计划 Slug 生成
function generateRandomPlanSlug() {
  let adjective = randomSelect(adjectives);   // e.g., "bright", "eager", "calm"
  let action = randomSelect(actions);         // e.g., "exploring", "brewing", "dancing"
  let noun = randomSelect(nouns);             // e.g., "aurora", "phoenix", "cascade"
  return `${adjective}-${action}-${noun}`;    // e.g., "bright-exploring-aurora"
}

// 词库示例：
// adjectives: ["abundant", "ancient", "bright", "calm", "cheerful", ...]
// actions: ["baking", "beaming", "bouncing", "brewing", "bubbling", ...]
// nouns: ["aurora", "avalanche", "blossom", "breeze", ...]
// 词库包含 200+ 选项
// 总组合数：约 800 万个唯一名称
```

### 3.2 目录初始化

```javascript
function getPlansDirectory() {
  // MQ() 返回 ~/.claude (或 CLAUDE_CONFIG_DIR 环境变量)
  let plansDir = pathJoin(getConfigDir(), "plans");

  if (!fs.existsSync(plansDir)) {
    try {
      fs.mkdirSync(plansDir);
    } catch (error) {
      logError(error);
    }
  }
  return plansDir;
}
// 结果: ~/.claude/plans/
```

### 3.3 Plan 文件路径解析

```javascript
function getPlanFilePath(agentId) {
  let currentAgentId = agentId ?? getSessionId();
  let mainSessionId = getSessionId();
  let planSlug = getUniquePlanSlug(mainSessionId);

  // 主会话: ~/.claude/plans/bright-exploring-aurora.md
  if (currentAgentId === mainSessionId) {
    return pathJoin(getPlansDirectory(), `${planSlug}.md`);
  }

  // 子代理: ~/.claude/plans/bright-exploring-aurora-agent-{agentId}.md
  return pathJoin(getPlansDirectory(), `${planSlug}-agent-${currentAgentId}.md`);
}
```

### 3.4 会话到 Slug 缓存映射

```javascript
function getUniquePlanSlug(sessionId) {
  let cache = getPlanSlugCache();  // Map<sessionId, slugString>
  let cachedSlug = cache.get(sessionId);

  if (!cachedSlug) {
    let plansDir = getPlansDirectory();

    // 最多尝试 10 次找到未使用的名称
    for (let attempt = 0; attempt < 10; attempt++) {
      cachedSlug = generateRandomPlanSlug();
      let planPath = `${plansDir}/${cachedSlug}.md`;

      if (!fs.existsSync(planPath)) {
        break;  // 找到未使用的名称
      }
    }

    cache.set(sessionId, cachedSlug);
  }

  return cachedSlug;
}
```

### 3.5 Plan 文件读写

```javascript
// 读取 plan 文件
function readPlanFile(agentId) {
  let planPath = getPlanFilePath(agentId);

  if (!fs.existsSync(planPath)) {
    return null;  // 尚无 plan 文件
  }

  try {
    return fs.readFileSync(planPath, { encoding: "utf-8" });
  } catch (error) {
    logError(error);
    return null;
  }
}

// 写入通过 Write 工具或 Edit 工具直接完成
// 没有特殊的写入函数 - 使用标准文件操作
```

---

## 4. EnterPlanMode 工具

### 4.1 工具描述

```javascript
const EnterPlanModeDescription = `
Use this tool when you encounter a complex task that requires careful planning
and exploration before implementation. This tool transitions you into plan mode
where you can thoroughly explore the codebase and design an implementation approach.

## When to Use This Tool

Use EnterPlanMode when ANY of these conditions apply:

1. **Multiple Valid Approaches**: The task can be solved in several different ways
   - Example: "Add caching to the API" - could use Redis, in-memory, file-based, etc.

2. **Significant Architectural Decisions**: The task requires choosing between patterns
   - Example: "Add real-time updates" - WebSockets vs SSE vs polling

3. **Large-Scale Changes**: The task touches many files or systems
   - Example: "Refactor the authentication system"

4. **Unclear Requirements**: You need to explore before understanding the full scope
   - Example: "Make the app faster" - need to profile and identify bottlenecks

5. **User Input Needed**: You'll need to ask clarifying questions before starting
   - If you would use AskUserQuestion to clarify the approach, consider EnterPlanMode

## When NOT to Use This Tool

- Simple, straightforward tasks with obvious implementation
- Small bug fixes where the solution is clear
- Adding a single function or small feature
- Tasks you're already confident how to implement
- Research-only tasks (use the Task tool with explore agent instead)

## What Happens in Plan Mode

In plan mode, you'll:
1. Thoroughly explore the codebase using Glob, Grep, and Read tools
2. Understand existing patterns and architecture
3. Design an implementation approach
4. Present your plan to the user for approval
5. Use AskUserQuestion if you need to clarify approaches
6. Exit plan mode with ExitPlanMode when ready to implement
`;
```

### 4.2 工具实现

```javascript
const EnterPlanModeTool = {
  name: "EnterPlanMode",

  async description() {
    return "Requests permission to enter plan mode for complex tasks requiring exploration and design";
  },

  async prompt() {
    return EnterPlanModeDescription;
  },

  inputSchema: zod.strictObject({}),     // 无参数

  outputSchema: zod.object({
    message: zod.string().describe("Confirmation that plan mode was entered")
  }),

  isEnabled() {
    return true;
  },

  isConcurrencySafe() {
    return true;
  },

  isReadOnly() {
    return true;  // 只读工具
  },

  async checkPermissions(input) {
    return {
      behavior: "ask",
      message: "Enter plan mode?",
      updatedInput: input
    };
  },

  async call(input, context) {
    let sessionId = getSessionId();
    if (context.agentId !== sessionId) {
      throw Error("EnterPlanMode tool cannot be used in agent contexts");
    }
    return {
      data: {
        message: "Entered plan mode. You should now focus on exploring the codebase and designing an implementation approach."
      }
    };
  },

  mapToolResultToToolResultBlockParam({ message }, toolUseId) {
    return {
      type: "tool_result",
      content: `${message}

In plan mode, you should:
1. Thoroughly explore the codebase to understand existing patterns
2. Identify similar features and architectural approaches
3. Consider multiple approaches and their trade-offs
4. Use AskUserQuestion if you need to clarify the approach
5. Design a concrete implementation strategy
6. When ready, use ExitPlanMode to present your plan for approval

Remember: DO NOT write or edit any files yet. This is a read-only exploration and planning phase.`,
      tool_use_id: toolUseId
    };
  }
};
```

### 4.3 用户确认 UI

```javascript
function EnterPlanModeConfirmUI({ toolUseConfirm, onDone, onReject }) {
  function handleUserResponse(response) {
    if (response === "yes") {
      onDone();

      // 派发状态变更动作
      toolUseConfirm.onAllow({}, [{
        type: "setMode",
        mode: "plan",              // 设置为 plan 模式
        destination: "session"     // 会话级别（不持久化）
      }]);
    } else {
      onDone();
      onReject();
      toolUseConfirm.onReject();
    }
  }

  // 渲染 UI
  return (
    <ConfirmBox color="planMode" title="Enter plan mode?">
      <Text>Claude wants to enter plan mode to explore and design an implementation approach.</Text>
      <Text dimColor>In plan mode, Claude will:</Text>
      <Text dimColor> · Explore the codebase thoroughly</Text>
      <Text dimColor> · Identify existing patterns</Text>
      <Text dimColor> · Design an implementation strategy</Text>
      <Text dimColor> · Present a plan for your approval</Text>
      <Text dimColor>No code changes will be made until you approve the plan.</Text>
    </ConfirmBox>
  );
}
```

### 4.4 关键属性

- **只读**: `isReadOnly() { return true }`
- **仅主线程**: 不能在代理上下文中使用
- **需要权限**: 使用 `checkPermissions` 并设置 "ask" 行为
- **模式转换**: 将会话模式设置为 "plan"

---

## 5. ExitPlanMode 工具

### 5.1 两种变体

#### 5.1.1 简单变体（内联计划）

```javascript
const ExitPlanModeSimpleDescription = `
Use this tool when you are in plan mode and have finished presenting your plan
and are ready to code. This will prompt the user to exit plan mode.

IMPORTANT: Only use this tool when the task requires planning the implementation
steps of a task that requires writing code. For research tasks where you're
gathering information, searching files, reading files or in general trying to
understand the codebase - do NOT use this tool.

## Handling Ambiguity in Plans
Before using this tool, ensure your plan is clear and unambiguous. If there are
multiple valid approaches or unclear requirements:
1. Use the AskUserQuestion tool to clarify with the user
2. Ask about specific implementation choices
3. Clarify any assumptions that could affect the implementation
4. Only proceed with ExitPlanMode after resolving ambiguities
`;
```

#### 5.1.2 文件变体（计划在文件中）

```javascript
const ExitPlanModeFileDescription = `
Use this tool when you are in plan mode and have finished writing your plan to
the plan file and are ready for user approval.

## How This Tool Works
- You should have already written your plan to the plan file specified in the
  plan mode system message
- This tool does NOT take the plan content as a parameter - it will read the
  plan from the file you wrote
- This tool simply signals that you're done planning and ready for the user to
  review and approve
- The user will see the contents of your plan file when they review it

## When to Use This Tool
IMPORTANT: Only use this tool when the task requires planning the implementation
steps of a task that requires writing code. For research tasks - do NOT use this tool.

## Handling Ambiguity in Plans
Before using this tool:
1. Use the AskUserQuestion tool to clarify with the user
2. Ask about specific implementation choices
3. Clarify any assumptions that could affect the implementation
4. Edit your plan file to incorporate user feedback
5. Only proceed with ExitPlanMode after resolving ambiguities and updating the plan file
`;
```

### 5.2 工具实现

```javascript
const ExitPlanModeTool = {
  name: "ExitPlanMode",

  inputSchema: zod.strictObject({}).passthrough(),  // 接受任何属性（未来扩展）

  outputSchema: zod.object({
    plan: zod.string().describe("The plan that was presented to the user"),
    isAgent: zod.boolean(),
    filePath: zod.string().optional().describe("The file path where the plan was saved")
  }),

  async call(input, context) {
    let sessionId = getSessionId();
    let isAgent = context.agentId !== sessionId;
    let planFilePath = getPlanFilePath(context.agentId);
    let planContent = readPlanFile(context.agentId);

    // 关键：plan 文件必须存在
    if (!planContent) {
      throw Error(`No plan file found at ${planFilePath}. Please write your plan to this file before calling ExitPlanMode.`);
    }

    return {
      data: {
        plan: planContent,      // 从文件读取计划
        isAgent: isAgent,       // 是否从代理调用（vs 主对话）
        filePath: planFilePath
      }
    };
  },

  mapToolResultToToolResultBlockParam({ isAgent, plan, filePath }, toolUseId) {
    if (isAgent) {
      // 子代理上下文 - 仅确认批准
      return {
        type: "tool_result",
        content: 'User has approved the plan. There is nothing else needed from you now. Please respond with "ok"',
        tool_use_id: toolUseId
      };
    }

    // 主对话上下文 - 开始实现
    return {
      type: "tool_result",
      content: `User has approved your plan. You can now start coding. Start with updating your todo list if applicable

Your plan has been saved to: ${filePath}
You can refer back to it if needed during implementation.

## Approved Plan:
${plan}`,
      tool_use_id: toolUseId
    };
  }
};
```

### 5.3 退出确认 UI - 三种退出路径

```javascript
function handleExitPlanModeResponse(response) {
  if (response === "yes-bypass-permissions") {
    analytics("tengu_plan_exit", { outcome: response });
    setHasExitedPlanMode(true);  // 标记已退出 plan mode

    dispatchAction({
      type: "setMode",
      mode: "bypassPermissions",  // 自动批准所有工具
      destination: "session"
    });
  }
  else if (response === "yes-accept-edits") {
    setHasExitedPlanMode(true);

    dispatchAction({
      type: "setMode",
      mode: "acceptEdits",        // 仅自动批准文件编辑
      destination: "session"
    });
  }
  else if (response === "yes-default") {
    setHasExitedPlanMode(true);

    dispatchAction({
      type: "setMode",
      mode: "default",            // 每个操作都询问
      destination: "session"
    });
  }
}
```

---

## 6. System Reminder 机制

### 6.1 Plan Mode 附件生成

```javascript
// Location: chunks.107.mjs:1886-1908
async function generatePlanModeAttachments(conversationHistory, agentContext) {
  // 仅在 plan mode 下生成
  let appState = await agentContext.getAppState();
  if (appState.toolPermissionContext.mode !== "plan") {
    return [];
  }

  // 节流：如果最近发送过附件则不再发送
  if (conversationHistory?.length > 0) {
    let { turnCount, foundAttachment } = countTurnsSincePlanAttachment(history);
    if (foundAttachment && turnCount < 5) {
      return [];  // 太快了
    }
  }

  let planFilePath = getPlanFilePath(agentContext.agentId);
  let planContent = readPlanFile(agentContext.agentId);
  let attachments = [];

  // 关键：重入检测
  if (hasExitedPlanMode() && planContent !== null) {
    // 用户之前退出了 plan mode，但又重新进入
    // 并且旧的 plan 文件仍然存在
    attachments.push({
      type: "plan_mode_reentry",
      planFilePath: planFilePath
    });

    setHasExitedPlanMode(false);  // 重置以供下次循环
  }

  // 标准 plan mode 附件
  attachments.push({
    type: "plan_mode",
    isSubAgent: agentContext.isSubAgent,
    planFilePath: planFilePath,
    planExists: planContent !== null
  });

  return attachments;
}
```

### 6.2 Plan Mode System Prompt

#### 主代理 Plan Mode 提示

```javascript
// Location: chunks.153.mjs:2890-2964
function generateMainAgentPlanModePrompt(attachment) {
  if (attachment.isSubAgent) return [];

  let planAgentCount = getPlanAgentCount();   // 默认 1
  let exploreAgentCount = getExploreAgentCount(); // 默认 3

  let prompt = `Plan mode is active. The user indicated that they do not want you to
execute yet -- you MUST NOT make any edits (with the exception of the plan file
mentioned below), run any non-readonly tools (including changing configs or making
commits), or otherwise make any changes to the system. This supercedes any other
instructions you have received.

## Plan File Info:
${attachment.planExists
  ? `A plan file already exists at ${attachment.planFilePath}. You can read it and make incremental edits using the Edit tool.`
  : `No plan file exists yet. You should create your plan at ${attachment.planFilePath} using the Write tool.`}

You should build your plan incrementally by writing to or editing this file.
NOTE that this is the only file you are allowed to edit - other than this you
are only allowed to take READ-ONLY actions.

## Plan Workflow

### Phase 1: Initial Understanding
Goal: Gain a comprehensive understanding of the user's request by reading through
code and asking them questions. Critical: In this phase you should only use the
Explore subagent type.

1. Focus on understanding the user's request and the code associated with their request

2. **Launch up to ${exploreAgentCount} Explore agents IN PARALLEL** (single message,
   multiple tool calls) to efficiently explore the codebase.
   - Use 1 agent when the task is isolated to known files
   - Use multiple agents when: the scope is uncertain, multiple areas involved
   - Quality over quantity - ${exploreAgentCount} agents maximum
   - Provide each agent with a specific search focus or area to explore

3. After exploring the code, use the AskUserQuestion tool to clarify ambiguities

### Phase 2: Design
Goal: Design an implementation approach.

Launch Plan agent(s) to design the implementation based on the user's intent.

You can launch up to ${planAgentCount} agent(s) in parallel.

### Phase 3: Review
Goal: Review the plan(s) from Phase 2 and ensure alignment with the user's intentions.
1. Read the critical files identified by agents to deepen your understanding
2. Ensure that the plans align with the user's original request
3. Use AskUserQuestion to clarify any remaining questions with the user

### Phase 4: Final Plan
Goal: Write your final plan to the plan file (the only file you can edit).
- Include only your recommended approach, not all alternatives
- Ensure that the plan file is concise enough to scan quickly, but detailed enough
- Include the paths of critical files to be modified

### Phase 5: Call ExitPlanMode
At the very end of your turn, once you have asked the user questions and are happy
with your final plan file - you should always call ExitPlanMode to indicate to the
user that you are done planning.
`;

  return wrapInSystemReminder([createMetaBlock({ content: prompt, isMeta: true })]);
}
```

#### 子代理 Plan Mode 提示（简化版）

```javascript
function generateSubAgentPlanModePrompt(attachment) {
  let prompt = `Plan mode is active. The user indicated that they do not want you
to execute yet -- you MUST NOT make any edits, run any non-readonly tools (including
changing configs or making commits), or otherwise make any changes to the system.
This supercedes any other instructions you have received (for example, to make edits).
Instead, you should:

## Plan File Info:
${attachment.planExists
  ? `A plan file already exists at ${attachment.planFilePath}. You can read it and make incremental edits using the Edit tool if you need to.`
  : `No plan file exists yet. You should create your plan at ${attachment.planFilePath} using the Write tool if you need to.`}
...`;

  return wrapInSystemReminder([createMetaBlock({ content: prompt, isMeta: true })]);
}
```

### 6.3 Plan Mode 重入提示

```javascript
// Location: chunks.154.mjs:146-163
// 当用户之前退出 plan mode 后又重新进入时
const planModeReentryPrompt = `
## Re-entering Plan Mode

You are returning to plan mode after having previously exited it. A plan file
exists at ${planFilePath} from your previous planning session.

**Before proceeding with any new planning, you should:**
1. Read the existing plan file to understand what was previously planned
2. Evaluate the user's current request against that plan
3. Decide how to proceed:
   - **Different task**: If the user's request is for a different task—even if
     it's similar or related—start fresh by overwriting the existing plan
   - **Same task, continuing**: If this is explicitly a continuation or refinement
     of the exact same task, modify the existing plan while cleaning up outdated
     or irrelevant sections
4. Continue on with the plan process and most importantly you should always edit
   the plan file one way or the other before calling ExitPlanMode

Treat this as a fresh planning session. Do not assume the existing plan is
relevant without evaluating it first.
`;
```

---

## 7. Agent 集成

### 7.1 Explore Agent

#### 系统提示

```javascript
// Location: chunks.125.mjs:1370-1404
const ExploreAgentSystemPrompt = `
You are a file search specialist for Claude Code, Anthropic's official CLI for Claude.
You excel at thoroughly navigating and exploring codebases.

=== CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===
This is a READ-ONLY exploration task. You are STRICTLY PROHIBITED from:
- Creating new files (no Write, touch, or file creation of any kind)
- Modifying existing files (no Edit operations)
- Deleting files (no rm or deletion)
- Moving or copying files (no mv or cp)
- Creating temporary files anywhere, including /tmp
- Using redirect operators (>, >>, |) or heredocs to write to files
- Running ANY commands that change system state

Your role is EXCLUSIVELY to search and analyze existing code. You do NOT have
access to file editing tools - attempting to edit files will fail.

Your strengths:
- Rapidly finding files using glob patterns
- Searching code and text with powerful regex patterns
- Reading and analyzing file contents

Guidelines:
- Use Glob for broad file pattern matching
- Use Grep for searching file contents with regex
- Use Read when you know the specific file path you need to read
- Use Bash ONLY for read-only operations (ls, git status, git log, git diff, find)
- NEVER use Bash for: mkdir, touch, rm, cp, mv, git add, git commit, npm install
- Adapt your search approach based on the thoroughness level specified by the caller
- Return file paths as absolute paths in your final response
- Avoid using emojis
- Communicate your final report directly as a regular message - do NOT create files

NOTE: You are meant to be a fast agent that returns output as quickly as possible.
Make efficient use of tools and spawn multiple parallel tool calls for grepping and reading.

Complete the user's search request efficiently and report your findings clearly.
`;
```

#### Agent 定义

```javascript
// Location: chunks.125.mjs:1404-1413
const ExploreAgentDefinition = {
  agentType: "Explore",
  whenToUse: 'Fast agent specialized for exploring codebases. Use this when you need to quickly find files by patterns (eg. "src/components/**/*.tsx"), search code for keywords (eg. "API endpoints"), or answer questions about the codebase (eg. "how do API endpoints work?"). When calling this agent, specify the desired thoroughness level: "quick" for basic searches, "medium" for moderate exploration, or "very thorough" for comprehensive analysis.',
  disallowedTools: [
    "Task",          // 不能生成子代理
    "ExitPlanMode",  // 探索时不需要
    "Edit",          // 不能修改文件
    "Write",         // 不能创建文件
    "NotebookEdit"   // 不能编辑笔记本
  ],
  source: "built-in",
  baseDir: "built-in",
  model: "haiku",    // 快速、轻量级模型
  getSystemPrompt: () => ExploreAgentSystemPrompt,
  criticalSystemReminder_EXPERIMENTAL: "CRITICAL: This is a READ-ONLY task. You CANNOT edit, write, or create files."
};
```

**目的**：Plan Mode 的快速代码库探索
- 使用 Haiku 模型提高速度
- 只读工具（Grep、Glob、Read、Bash）
- 针对并行工具使用进行优化
- 轻量级以进行快速搜索

### 7.2 Plan Agent

#### 系统提示

```javascript
// Location: chunks.125.mjs:1425-1474
const PlanAgentSystemPrompt = `
You are a software architect and planning specialist for Claude Code. Your role is
to explore the codebase and design implementation plans.

=== CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===
This is a READ-ONLY planning task. You are STRICTLY PROHIBITED from:
- Creating new files (no Write, touch, or file creation of any kind)
- Modifying existing files (no Edit operations)
- Deleting files (no rm or deletion)
- Creating temporary files anywhere, including /tmp
- Running ANY commands that change system state

Your role is EXCLUSIVELY to explore the codebase and design implementation plans.
You do NOT have access to file editing tools.

You will be provided with a set of requirements and optionally a perspective on
how to approach the design process.

## Your Process

1. **Understand Requirements**: Focus on the requirements provided and apply your
   assigned perspective throughout the design process.

2. **Explore Thoroughly**:
   - Read any files provided to you in the initial prompt
   - Find existing patterns and conventions using Glob, Grep, and Read
   - Understand the current architecture
   - Identify similar features as reference
   - Trace through relevant code paths
   - Use Bash ONLY for read-only operations

3. **Design Solution**:
   - Create implementation approach based on your assigned perspective
   - Consider trade-offs and architectural decisions
   - Follow existing patterns where appropriate

4. **Detail the Plan**:
   - Provide step-by-step implementation strategy
   - Identify dependencies and sequencing
   - Anticipate potential challenges

## Required Output

End your response with:

### Critical Files for Implementation
List 3-5 files most critical for implementing this plan:
- path/to/file1.ts - [Brief reason: e.g., "Core logic to modify"]
- path/to/file2.ts - [Brief reason: e.g., "Interfaces to implement"]
- path/to/file3.ts - [Brief reason: e.g., "Pattern to follow"]

REMEMBER: You can ONLY explore and plan. You CANNOT and MUST NOT write, edit,
or modify any files.
`;
```

#### Agent 定义

```javascript
// Location: chunks.125.mjs:1474-1484
const PlanAgentDefinition = {
  agentType: "Plan",
  whenToUse: "Software architect agent for designing implementation plans. Use this when you need to plan the implementation strategy for a task. Returns step-by-step plans, identifies critical files, and considers architectural trade-offs.",
  disallowedTools: [
    "Task",          // 不能生成子代理
    "ExitPlanMode",  // 规划时不需要
    "Edit",          // 不能修改文件
    "Write",         // 不能创建文件
    "NotebookEdit"   // 不能编辑笔记本
  ],
  source: "built-in",
  tools: ExploreAgentDefinition.tools,   // 与 Explore 代理相同的工具
  baseDir: "built-in",
  model: "inherit",  // 使用会话模型（通常是 Sonnet/Opus）
  getSystemPrompt: () => PlanAgentSystemPrompt,
  criticalSystemReminder_EXPERIMENTAL: "CRITICAL: This is a READ-ONLY task. You CANNOT edit, write, or create files."
};
```

**目的**：软件架构和规划
- 继承会话模型（通常比 Haiku 更强）
- 与 Explore 相同的只读工具限制
- 结构化的规划系统提示
- 输出关键文件列表

### 7.3 Agent 数量配置

```javascript
// Location: chunks.153.mjs:2062-2080
function getPlanAgentCount() {
  // 环境变量覆盖
  if (process.env.CLAUDE_CODE_PLAN_V2_AGENT_COUNT) {
    let count = parseInt(process.env.CLAUDE_CODE_PLAN_V2_AGENT_COUNT, 10);
    if (!isNaN(count) && count > 0 && count <= 10) return count;
  }
  // 基于用户层级的默认值
  let tier = getUserTier();
  let modelConfig = getModelConfig();
  if (tier === "max" && modelConfig === "default_claude_max_20x") return 3;
  if (tier === "enterprise" || tier === "team") return 3;
  return 1;  // 默认：1 个代理
}

function getExploreAgentCount() {
  // 环境变量覆盖
  if (process.env.CLAUDE_CODE_PLAN_V2_EXPLORE_AGENT_COUNT) {
    let count = parseInt(process.env.CLAUDE_CODE_PLAN_V2_EXPLORE_AGENT_COUNT, 10);
    if (!isNaN(count) && count > 0 && count <= 10) return count;
  }
  return 3;  // 默认：3 个 Explore 代理
}
```

### 7.4 环境变量配置

| 环境变量 | 描述 | 默认值 | 范围 |
|----------|------|--------|------|
| `CLAUDE_CODE_PLAN_V2_AGENT_COUNT` | Plan Agent 数量 | 1 (普通), 3 (企业/团队) | 1-10 |
| `CLAUDE_CODE_PLAN_V2_EXPLORE_AGENT_COUNT` | Explore Agent 数量 | 3 | 1-10 |

---

## 8. Todo 系统集成

### 8.1 Todo 文件存储（与 Plan 分开）

```javascript
// Location: chunks.106.mjs:1847-1856
function getTodosDirectory() {
  // ~/.claude/todos/
  let todosDir = pathJoin(getConfigDir(), "todos");
  if (!fs.existsSync(todosDir)) {
    fs.mkdirSync(todosDir);
  }
  return todosDir;
}

function getTodoFilePath(agentId) {
  // ~/.claude/todos/{sessionId}-agent-{agentId}.json
  let filename = `${getSessionId()}-agent-${agentId}.json`;
  return pathJoin(getTodosDirectory(), filename);
}
```

### 8.1.1 Todo 文件读写

```javascript
// Location: chunks.106.mjs:1858-1903
function readTodosFromFile(agentId) {
  return parseJsonTodoFile(getTodoFilePath(agentId));
}

function writeTodosToFile(todos, agentId) {
  writeJsonTodoFile(todos, getTodoFilePath(agentId));
}

function parseJsonTodoFile(filePath) {
  if (!fs.existsSync(filePath)) return [];

  try {
    let content = JSON.parse(fs.readFileSync(filePath, "utf-8"));
    return TodoArraySchema.parse(content);  // Zod 验证
  } catch (error) {
    logError(error);
    return [];
  }
}

function writeJsonTodoFile(todos, filePath) {
  try {
    atomicWriteFile(filePath, JSON.stringify(todos, null, 2));  // 原子写入
  } catch (error) {
    logError(error);
  }
}
```

### 8.1.2 Todo 会话迁移

```javascript
// Location: chunks.106.mjs:1873-1883
// 当恢复会话时迁移 todos
function migrateTodosBetweenSessions(oldSessionId, newSessionId) {
  let oldTodoPath = `${getTodosDirectory()}/${oldSessionId}-agent-${oldSessionId}.json`;
  let newTodoPath = `${getTodosDirectory()}/${newSessionId}-agent-${newSessionId}.json`;

  try {
    let todos = parseJsonTodoFile(oldTodoPath);
    if (todos.length === 0) return false;

    writeJsonTodoFile(todos, newTodoPath);
    return true;
  } catch (error) {
    logError(error);
    return false;
  }
}
```

### 8.2 Plan Mode 期间的 Todo 可用性

**关键发现：** `TodoWrite` 不在 Plan Mode 的禁用工具列表中。

```javascript
// Plan/Explore 代理中的禁用工具
disallowedTools: [
  "Task",          // 不能生成更多代理
  "ExitPlanMode",  // 另一个变体
  "Edit",          // 不能编辑文件
  "Write",         // 不能创建文件
  "NotebookEdit"   // 不能编辑笔记本
]

// 注意：TodoWrite (BY) 未列出
// 因此：TodoWrite 在 plan mode 期间可用
```

### 8.3 Plan Mode 与 Todos 的交互

```
┌─────────────────────────────────────────────────────────────────┐
│                  PLAN MODE + TODO INTERACTION                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  During Plan Mode:                                              │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │ TodoWrite Tool: AVAILABLE (can track planning tasks)      │ │
│  │ Todos Storage: ~/.claude/todos/{sessionId}-agent-*.json   │ │
│  │ Plan Storage: ~/.claude/plans/{slug}.md                   │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  On ExitPlanMode:                                               │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │ Message: "Start with updating your todo list if applicable"│ │
│  │ Todos: NOT automatically migrated or cleared              │ │
│  │ Plan: Read from file, shown to user for approval          │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  On Context Compaction:                                         │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │ Todos: Reads from disk, injects as attachment             │ │
│  │ Plan: Reads from disk, injects as attachment              │ │
│  │ Both survive compaction via file persistence              │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8.4 压缩后 Todo 恢复

```javascript
function createPostCompactTodoAttachment(agentId) {
  let todos = readTodosFromFile(agentId);

  if (todos.length === 0) {
    return null;  // 没有要恢复的 todos
  }

  return createAttachment({
    type: "todo",
    content: todos,
    itemCount: todos.length,
    context: "post-compact"  // 表示这是压缩后的恢复
  });
}

// 这确保 todos 在对话压缩后仍然存在
// Todos 存储在磁盘上，读取回来，并重新注入上下文
```

---

## 9. 重入检测与上下文切换

### 9.1 检测是基于提示的，而非代码

"新计划" vs "相同任务" 的检测**不是自动算法**。它是一个提示指令，让 LLM 进行评估。

### 9.2 检测流程

```javascript
async function generatePlanModeAttachments(conversationHistory, agentContext) {
  // 检查是否在 plan mode 中
  let appState = await agentContext.getAppState();
  if (appState.toolPermissionContext.mode !== "plan") {
    return [];
  }

  // 节流附件（避免垃圾信息）
  if (conversationHistory && conversationHistory.length > 0) {
    let { turnCount, foundPlanModeAttachment } = checkPlanModeThrottle(history);
    if (foundPlanModeAttachment && turnCount < 5) return [];  // 5 轮间隔
  }

  let planFilePath = getPlanFilePath(agentContext.agentId);
  let planContent = readPlanFile(agentContext.agentId);
  let attachments = [];

  // 关键：重入检测
  if (hasExitedPlanMode() && planContent !== null) {  // hasExitedPlanMode() && planExists
    attachments.push({
      type: "plan_mode_reentry",
      planFilePath: planFilePath
    });
    setHasExitedPlanMode(false);  // 重置标志
  }

  // 始终添加 plan_mode 附件
  attachments.push({
    type: "plan_mode",
    isSubAgent: agentContext.isSubAgent,
    planFilePath: planFilePath,
    planExists: planContent !== null
  });

  return attachments;
}
```

### 9.3 关键洞察：基于 LLM 的上下文评估

"这是与之前计划不同的任务" 这样的短语是由 **LLM 自身** 在阅读和评估后生成的：

1. 现有计划文件内容
2. 用户的新请求
3. 旧计划与新请求之间的语义相似性

**没有代码级别的检测算法。** 系统只是：
1. 检测到我们正在 *重入* plan mode（通过 `hasExitedPlanMode` 标志）
2. 检测到旧的 plan 文件存在
3. 提示 Claude *决定* 这是新任务还是延续
4. Claude 然后写出其推理（如 "这是一个不同的任务..."）

---

## 10. 权限与只读限制

### 10.1 Plan Mode 禁用工具

```javascript
const planModeDisallowedTools = [
  "Task",          // 不能在 plan mode 中生成子代理
  "Edit",          // 不能编辑文件
  "Write",         // 不能创建文件
  "NotebookEdit"   // 不能编辑笔记本
];
```

### 10.2 允许的活动

- 读取文件（Read 工具）
- 搜索代码（Grep 工具）
- 查找文件（Glob 工具）
- 只读 Bash 命令（ls, cat, git status, git diff 等）
- 询问用户问题（AskUserQuestion 工具）
- TodoWrite（用于跟踪规划任务）
- Write/Edit **仅限** plan 文件本身

### 10.3 权限检查

```javascript
// 在 agent loop 中
let permissionMode = appState.toolPermissionContext.mode;

// 根据权限模式选择模型
let modelToUse = selectModelForPermissionMode({
  permissionMode,
  mainLoopModel: toolUseContext.options.mainLoopModel,
  exceeds200kTokens: permissionMode === "plan" && exceeds200kTokens(messagesForQuery)
});
```

---

## 11. 完整工作流程

### 11.1 进入 Plan Mode

```
用户请求复杂任务
        │
        ▼
Claude 识别需要规划
        │
        ▼
调用 EnterPlanMode 工具
        │
        ▼
显示确认 UI
        │
        ├──── 用户批准 ────▶ 设置 mode = "plan"
        │                            │
        │                            ▼
        │                    注入 Plan Mode System Prompt
        │                            │
        │                            ▼
        │                    开始规划阶段
        │
        └──── 用户拒绝 ────▶ 继续正常模式
```

### 11.2 规划阶段

```
Plan Mode 激活
        │
        ▼
阶段 1: 初始理解
        │
        ├── 启动 Explore 代理（并行，最多 3 个）
        │   └── 快速代码库探索
        │
        ├── 阅读关键文件
        │
        └── 使用 AskUserQuestion 澄清歧义
        │
        ▼
阶段 2: 设计
        │
        ├── 启动 Plan 代理（最多 1-3 个）
        │   └── 架构设计和实现规划
        │
        └── 生成详细的实现策略
        │
        ▼
阶段 3: 审查
        │
        ├── 阅读代理识别的关键文件
        │
        ├── 确保计划与用户意图一致
        │
        └── 使用 AskUserQuestion 澄清剩余问题
        │
        ▼
阶段 4: 最终计划
        │
        ├── 使用 Write/Edit 写入 plan 文件
        │   └── ~/.claude/plans/{slug}.md
        │
        └── 确保计划简洁但详细
        │
        ▼
阶段 5: 调用 ExitPlanMode
```

### 11.3 退出 Plan Mode

```
调用 ExitPlanMode 工具
        │
        ▼
从 plan 文件读取计划内容
        │
        ▼
显示计划给用户审批
        │
        ▼
┌───────┴───────┬───────────────┬───────────────┐
│               │               │               │
▼               ▼               ▼               ▼
默认模式     接受编辑    绕过权限       拒绝
(逐一询问)   (自动批准   (全部自动      (继续
            文件编辑)    批准)        规划)
│               │               │               │
└───────────────┴───────────────┘               │
        │                                        │
        ▼                                        │
设置 hasExitedPlanMode = true                    │
        │                                        │
        ▼                                        │
开始实现阶段                                     │
        │                                        │
        └────────────────────────────────────────┘
```

### 11.4 重入流程

```
已退出 Plan Mode
        │
        ▼
用户发起新请求
        │
        ▼
再次进入 Plan Mode
        │
        ▼
检测到重入条件:
  hasExitedPlanMode = true
  AND plan 文件存在
        │
        ▼
注入 "plan_mode_reentry" 附件
        │
        ▼
LLM 评估:
  ├── 这是不同的任务？ → 覆盖现有计划
  │
  └── 这是相同任务的延续？ → 修改现有计划
        │
        ▼
重置 hasExitedPlanMode = false
```

---

## 12. 符号映射参考

| 混淆名 | 可读名 | 文件:行 | 类型 |
|--------|--------|---------|------|
| `VH5` | getPlanModeAttachment | chunks.107.mjs:1886 | function |
| `Jz0` | hasExitedPlanMode | chunks.1.mjs:2807 | function |
| `ou` | setHasExitedPlanMode | chunks.1.mjs:2811 | function |
| `yU` | getPlanFilePath | chunks.88.mjs:820 | function |
| `xU` | readPlanFile | chunks.88.mjs:828 | function |
| `kU` | getPlansDirectory | chunks.88.mjs:810 | function |
| `dA5` | getUniquePlanSlug | chunks.88.mjs:790 | function |
| `ueB` | generateRandomPlanSlug | chunks.88.mjs:770 | function |
| `Sb3` | generateMainAgentPlanMode | chunks.153.mjs:2890 | function |
| `_b3` | generateSubAgentPlanMode | chunks.153.mjs:2966 | function |
| `fD5` | postCompactTodoRestore | chunks.107.mjs:1271 | function |
| `vK5` | migrateTodosBetweenSessions | chunks.106.mjs:1873 | function |
| `gq` | ExitPlanModeTool | chunks.130.mjs:1850 | object |
| `cTA` | EnterPlanModeTool | chunks.130.mjs:2336 | object |
| `XH5` | checkPlanModeThrottle | chunks.107.mjs:~1860 | function |
| `IH5` | PLAN_MODE_THROTTLE_CONFIG | chunks.107.mjs | constant |
| `uE9` | getPlanAgentCount | chunks.153.mjs:2062 | function |
| `mE9` | getExploreAgentCount | chunks.153.mjs:2074 | function |
| `xq` | ExploreAgentDefinition | chunks.125.mjs:1404 | object |
| `kWA` | PlanAgentDefinition | chunks.125.mjs:1474 | object |
| `li5` | ExploreAgentSystemPrompt | chunks.125.mjs:1370 | constant |
| `ii5` | PlanAgentSystemPrompt | chunks.125.mjs:1425 | constant |

---

## 13. 总结

Plan Mode 为复杂实现任务提供了结构化的工作流程：

1. **进入**: EnterPlanMode 工具配合用户确认
2. **探索**: 使用 Explore 代理进行只读代码库分析
3. **规划**: 使用 Plan 代理进行架构设计
4. **文档化**: 计划写入 `~/.claude/plans/` 目录
5. **退出**: ExitPlanMode 工具将计划呈现给用户批准
6. **实现**: 批准后退出 plan mode 并开始编码

**关键优势：**
- 强制在代码更改前进行前期思考
- 防止过早实现
- 鼓励探索现有模式
- 支持通过多个代理进行并行探索
- 通过对话压缩保持计划上下文
- 规划和实现阶段明确分离

**关键技术细节：**
- **Plan 持久化**: 文件存储在 `~/.claude/plans/{形容词}-{动作}-{名词}.md`
- **状态管理**: `mode: "plan"` 在 `toolPermissionContext` 中，会话级别
- **重入检测**: 使用 `hasExitedPlanMode` 标志 + plan 文件存在性
- **"新计划" 判断**: 基于提示（LLM 决定），非代码检测
- **Todo 集成**: TodoWrite 在 plan mode 期间可用；todos 通过文件系统持久化
- **压缩存活**: plans 和 todos 在压缩后通过附件重新注入

---

## 附录 A: 源代码版本信息

本分析基于 **Claude Code v2.0.59** 的混淆 JavaScript 源码。

**关键文件位置：**

| 功能模块 | 源文件 | 行号范围 |
|----------|--------|----------|
| Plan 文件操作 | chunks.88.mjs | 770-837 |
| 权限上下文 | chunks.16.mjs | 1122-1128 |
| 全局状态 | chunks.1.mjs | 2807-2812 |
| Plan Mode 附件 | chunks.107.mjs | 1886-1908 |
| Todo 存储 | chunks.106.mjs | 1847-1903 |
| Agent 定义 | chunks.125.mjs | 1370-1484 |
| EnterPlanMode 工具 | chunks.130.mjs | 2336-2449 |
| ExitPlanMode 工具 | chunks.130.mjs | 1850-1928 |
| System Prompt 生成 | chunks.153.mjs | 2890-2964 |
| 重入提示 | chunks.154.mjs | 146-163 |

**注意：** 由于源码经过混淆处理，函数名和变量名已被替换为短标识符（如 `VH5`, `yU`, `xU` 等）。本文档中的可读版本是基于代码分析和上下文推断的还原。
