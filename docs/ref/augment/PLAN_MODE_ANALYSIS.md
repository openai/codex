# Augment Plan Mode 深度分析

## 文档信息
- **分析时间**: 2025-12-05
- **源文件**: `chunks.97.mjs`, `chunks.77.mjs`, `chunks.78.mjs`, `chunks.96.mjs`
- **分析范围**: Plan Mode 支持与实现机制
- **文档版本**: v1.0

---

## 核心发现

### ❌ Augment 没有独立的 PLAN 或 PLANNING chat mode

经过全面的代码分析，Augment 定义了 8 种 chat mode，但**不包含** PLAN 或 PLANNING mode：

1. **CHAT** - 基础对话模式
2. **AGENT** - Agent 自主执行模式
3. **REMOTE_AGENT** - 远程 Agent 模式
4. **MEMORIES** - 记忆管理模式
5. **ORIENTATION** - 方向引导模式
6. **MEMORIES_COMPRESSION** - 记忆压缩模式
7. **CLI_AGENT** - CLI Agent 模式
8. **CLI_NONINTERACTIVE** - 非交互式 CLI 模式

### ✅ 但 Augment 有完整的 Plan 功能系统

虽然没有独立的 plan mode，但 Augment 通过以下机制实现了完整的 plan 功能：

- **Session Update Type**: "plan" 类型的会话更新
- **4个任务管理工具**: view_tasklist, update_tasks, add_tasks, reorganize_tasklist
- **Plan Entries 生成机制**: 将任务树递归转换为 plan entries
- **实时 Plan 更新推送**: 通过 session update 机制实时推送给客户端

### 🎯 实现方式：跨 Mode 功能

Plan 是一种**跨 mode 功能**，而非独立 mode：
- 在 AGENT、CLI_AGENT、CLI_NONINTERACTIVE 等模式中可用
- 通过 `enableTaskList` feature flag 控制
- 基于任务管理工具系统实现
- LLM 自主决定何时使用

---

## 1. Plan Mode 存在性分析

### 1.1 Chat Mode 完整列表

**文件位置**: `chunks.78.mjs`

```javascript
// Mode 验证函数
validateChatMode(mode) {
    const supportedModes = [
        "CHAT",
        "AGENT",
        "REMOTE_AGENT",
        "MEMORIES",
        "ORIENTATION",
        "MEMORIES_COMPRESSION",
        "CLI_AGENT",
        "CLI_NONINTERACTIVE"
    ];

    if (!supportedModes.includes(mode)) {
        throw new Error(
            `Unsupported chat mode: ${String(mode)}. ` +
            `Supported modes: ${supportedModes.join(", ")}`
        );
    }
}
```

**结论**:
- ❌ 没有 PLAN 或 PLANNING mode
- ✅ 但有 plan 相关的 session update 类型

### 1.2 Session Update Type "plan"

**文件位置**: `chunks.96.mjs:2348`

```javascript
// Session update 类型定义
sessionUpdate: z.literal("plan")
```

Session update 支持多种类型，其中包括 "plan"：
- `user_message_chunk` - 用户消息片段
- `agent_message_chunk` - Agent 消息片段
- `agent_thought_chunk` - Agent 思考片段
- `tool_call` - 工具调用
- `tool_call_update` - 工具调用更新
- **`plan`** - **计划更新** ← Plan 功能的核心
- `available_commands_update` - 可用命令更新
- `current_mode_update` - 当前模式更新

**结论**: Plan 是通过 session update 机制实现的，不是独立的 chat mode。

---

## 2. Plan 功能核心组件

### 2.1 架构概览

```
┌─────────────────────────────────────────────────────────┐
│                     LLM (Claude)                        │
│  决定何时使用任务管理工具来规划和追踪任务                  │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              4个任务管理工具                             │
│  • view_tasklist    • update_tasks                      │
│  • add_tasks        • reorganize_tasklist               │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│                 TaskManager                             │
│  • 创建/更新/查询任务                                     │
│  • 维护任务树结构                                        │
│  • 持久化任务状态                                        │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              工具响应处理 (Xur 函数)                     │
│  • 检测任务工具调用                                       │
│  • 提取 plan 参数                                        │
│  • 生成 plan entries                                    │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│         Plan Entries 生成 (Yur 函数)                    │
│  • 递归遍历任务树                                        │
│  • 计算 priority (基于 depth)                           │
│  • 映射 status (基于 task state)                        │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│           Session Update 推送                           │
│  {                                                      │
│    sessionUpdate: "plan",                               │
│    entries: [                                           │
│      { content, priority, status },                     │
│      ...                                                │
│    ]                                                    │
│  }                                                      │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│               客户端 UI 渲染                             │
│  • 实时显示任务列表                                       │
│  • 可视化任务状态                                        │
│  • 支持交互操作                                          │
└─────────────────────────────────────────────────────────┘
```

### 2.2 核心组件详解

#### 组件 1: Session Update 机制

**定义位置**: `chunks.96.mjs:2348`

**作用**: 定义 "plan" 作为有效的 session update 类型。

**Payload 结构**:
```typescript
{
  sessionUpdate: "plan",
  entries: PlanEntry[]
}

interface PlanEntry {
  content: string;      // 任务名称
  priority: "high" | "medium" | "low";
  status: "pending" | "in_progress" | "completed";
}
```

#### 组件 2: 任务管理工具（4个）

**文件位置**: `chunks.77.mjs:1957-2276`

| 工具名 | 类名 | 代码行 | 说明 |
|--------|------|--------|------|
| view_tasklist | xZ | 1957-1987 | 查看当前任务列表 |
| update_tasks | yZ | 1989-2091 | 批量更新任务属性 |
| add_tasks | RZ | 2152-2276 | 批量创建新任务 |
| reorganize_tasklist | CZ | 2093-2150 | 通过 markdown 重组任务结构 |

#### 组件 3: Plan Entries 生成

**函数**: `Yur(task, entries=[], depth=0)`

**文件位置**: `chunks.97.mjs:915-943`

**功能**: 递归遍历任务树，将每个任务转换为 plan entry。

**代码**:
```javascript
function Yur(e, t = [], r = 0) {
    // 跳过已取消的任务
    if (e.state === "CANCELLED") return t;

    // 深度 > 0 时才添加（跳过根任务）
    if (r > 0) {
        t.push({
            content: e.name,
            priority: uea(r),      // 根据深度计算 priority
            status: dea(e.state)   // 映射任务状态到 plan status
        });
    }

    // 递归处理子任务
    if (e.subTasksData && Array.isArray(e.subTasksData)) {
        for (let n of e.subTasksData) {
            Yur(n, t, r + 1);
        }
    }

    return t;
}
```

**优先级计算** (`uea` 函数, `chunks.97.mjs:926-927`):
```javascript
function uea(depth) {
    return depth <= 1 ? "high"
         : depth === 2 ? "medium"
         : "low";
}
```

**状态映射** (`dea` 函数, `chunks.97.mjs:930-942`):
```javascript
function dea(state) {
    switch (state) {
        case "NOT_STARTED":
            return "pending";
        case "IN_PROGRESS":
            return "in_progress";
        case "COMPLETE":
            return "completed";
        case "CANCELLED":
            return "pending";
        default:
            return "pending";
    }
}
```

#### 组件 4: 工具响应处理

**函数**: `Xur(toolName, toolResponse)`

**文件位置**: `chunks.97.mjs:285-343`

**功能**: 处理工具返回的响应，检测并提取 plan 参数。

**代码**:
```javascript
function Xur(toolName, toolResponse) {
    let {
        text: responseText,
        isError: isError,
        plan: planData  // ← 提取 plan 参数
    } = toolResponse;

    // 错误处理
    if (isError) {
        return { content: X2e(responseText) };
    }

    // 如果有 plan 数据且是任务工具
    if (planData && Sqn(toolName)) {
        let planEntries = vqn(planData);

        // update_tasks 特殊处理：添加文本内容
        if (toolName === "update_tasks") {
            let updateText = _qn(responseText);
            if (updateText) {
                planEntries.content = [{
                    type: "content",
                    content: {
                        type: "text",
                        text: updateText
                    }
                }];
            }
        }

        return planEntries;
    }

    // 其他工具的格式化逻辑...
    switch (toolName) {
        case "view": return yqn(responseText);
        case "apply_patch": return wqn(responseText, isError);
        // ...
        default: return {};
    }
}
```

**任务工具检测** (`Sqn` 函数, `chunks.97.mjs:336-337`):
```javascript
function Sqn(toolName) {
    return ["add_tasks", "update_tasks", "reorganize_tasklist"].includes(toolName);
}
```

**Plan Entries 提取** (`vqn` 函数, `chunks.97.mjs:340-343`):
```javascript
function vqn(planData) {
    return {
        planEntries: Yur(planData)  // 调用 Yur 递归转换
    };
}
```

#### 组件 5: Plan 推送机制

**文件位置**: `chunks.97.mjs:1026-1032`

**触发条件**: `r.plan && s.planEntries`

**代码**:
```javascript
// 在 ACPEventListener 的工具结果处理中
r.plan && s.planEntries && this.conn.sessionUpdate({
    sessionId: this.sessionId,
    update: {
        sessionUpdate: "plan",
        entries: s.planEntries
    }
});
```

**说明**:
- 当工具返回包含 `plan` 参数
- 且处理后生成了 `planEntries`
- 立即通过 WebSocket 连接推送给客户端

---

## 3. 任务管理工具详解

### 3.1 ViewTaskListTool (view_tasklist)

**文件位置**: `chunks.77.mjs:1957-1987`

**类名**: `xZ extends qo`

**描述**: "View the current task list for the conversation."

**输入参数**: 无

**输入 Schema**:
```json
{
  "type": "object",
  "properties": {},
  "required": []
}
```

**核心逻辑**:
```javascript
async call(params, chatHistory, abortSignal, toolHost, conversationId) {
    try {
        // 获取当前根任务 UUID
        let rootTaskUuid = this._taskManager.getCurrentRootTaskUuid();
        if (!rootTaskUuid) {
            return ErrorResponse("No root task found.");
        }

        // 获取完整的任务树（hydrated task）
        let taskTree = await this._taskManager.getHydratedTask(rootTaskUuid);
        if (!taskTree) {
            return ErrorResponse(`Task with UUID ${rootTaskUuid} not found.`);
        }

        // 格式化任务列表
        let formattedList = fW(taskTree);

        // 记录任务查看事件
        let requestId = chatHistory.length > 0
            ? chatHistory[chatHistory.length - 1].request_id
            : "";
        mW(10, requestId, formattedList);

        // 生成最终响应
        let taskListView = gie(taskTree);
        return SuccessResponse(Jg.formatTaskListViewResponse(taskListView));
    } catch (error) {
        this._logger.error("Error in ViewTaskListTool:", error);
        return ErrorResponse(
            `Failed to view task list: ${error instanceof Error ? error.message : String(error)}`
        );
    }
}
```

**输出示例**:
```
Current Task List:
[ ] Task 1: 设计认证架构
[/] Task 2: 实现 OAuth2 集成
    [x] Subtask 2.1: 配置 OAuth2 provider
    [/] Subtask 2.2: 实现授权流程
[ ] Task 3: 实现 JWT token 管理
[ ] Task 4: 编写单元测试

Legend:
[ ] = NOT_STARTED
[/] = IN_PROGRESS
[x] = COMPLETE
[-] = CANCELLED
```

### 3.2 UpdateTasksTool (update_tasks)

**文件位置**: `chunks.77.mjs:1989-2091`

**类名**: `yZ extends qo`

**描述**: "Update one or more tasks' properties (state, name, description). Can update a single task or multiple tasks in one call. Use this on complex sequences of work to plan, track progress, and manage work."

**输入 Schema**:
```json
{
  "type": "object",
  "properties": {
    "tasks": {
      "type": "array",
      "description": "Array of tasks to update. Each task should have a task_id and the properties to update.",
      "items": {
        "type": "object",
        "properties": {
          "task_id": {
            "type": "string",
            "description": "The UUID of the task to update."
          },
          "state": {
            "type": "string",
            "enum": ["NOT_STARTED", "IN_PROGRESS", "CANCELLED", "COMPLETE"],
            "description": "New task state. Use NOT_STARTED for [ ], IN_PROGRESS for [/], CANCELLED for [-], COMPLETE for [x]."
          },
          "name": {
            "type": "string",
            "description": "New task name."
          },
          "description": {
            "type": "string",
            "description": "New task description."
          }
        },
        "required": ["task_id"]
      }
    }
  },
  "required": ["tasks"]
}
```

**核心逻辑**:
```javascript
async call(params, chatHistory, abortSignal, toolHost, conversationId) {
    try {
        let tasksToUpdate = params.tasks;

        // 验证输入
        if (!tasksToUpdate || tasksToUpdate.length === 0) {
            return ErrorResponse("tasks array is required and must not be empty.");
        }

        // 批量更新任务
        let result = await this.handleBatchUpdate(tasksToUpdate);

        if (!result.isError) {
            // 获取根任务并记录更新
            let rootTaskUuid = this._taskManager.getCurrentRootTaskUuid();
            if (rootTaskUuid) {
                let updatedTaskTree = await this._taskManager.getHydratedTask(rootTaskUuid);
                let formattedList = fW(updatedTaskTree);
                let requestId = chatHistory.length > 0
                    ? chatHistory[chatHistory.length - 1].request_id
                    : "";
                mW(updateType, requestId, formattedList);
            }
        }

        // 返回响应（包含 plan 参数）
        return result;
    } catch (error) {
        this._logger.error("Error in UpdateTasksTool:", error);
        return ErrorResponse(`Failed to update tasks: ${error.message}`);
    }
}
```

**输入示例**:
```json
{
  "tasks": [
    {
      "task_id": "abc-123",
      "state": "IN_PROGRESS"
    },
    {
      "task_id": "def-456",
      "state": "COMPLETE"
    }
  ]
}
```

**输出**:
- 文本响应描述更新结果
- **`plan` 参数**: 更新后的完整任务树

### 3.3 AddTasksTool (add_tasks)

**文件位置**: `chunks.77.mjs:2152-2276`

**类名**: `RZ extends qo`

**描述**: "Add one or more new tasks to the task list. Can add a single task or multiple tasks in one call. Tasks can be added as subtasks or after specific tasks. Use this when planning complex sequences of work."

**输入 Schema**:
```json
{
  "type": "object",
  "properties": {
    "tasks": {
      "type": "array",
      "description": "Array of tasks to add.",
      "items": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Task name."
          },
          "description": {
            "type": "string",
            "description": "Task description."
          },
          "parent_task_id": {
            "type": "string",
            "description": "UUID of parent task for subtasks (optional)."
          },
          "after_task_id": {
            "type": "string",
            "description": "UUID of task after which to insert (optional)."
          },
          "state": {
            "type": "string",
            "enum": ["NOT_STARTED", "IN_PROGRESS", "CANCELLED", "COMPLETE"],
            "description": "Initial state (optional, defaults to NOT_STARTED)."
          }
        },
        "required": ["name", "description"]
      }
    }
  },
  "required": ["tasks"]
}
```

**核心逻辑**:
```javascript
async call(params, chatHistory, abortSignal, toolHost, conversationId) {
    try {
        let tasksToAdd = params.tasks;

        if (!tasksToAdd || tasksToAdd.length === 0) {
            return ErrorResponse("tasks array is required and must not be empty.");
        }

        // 批量创建任务
        let result = await this.handleBatchAdd(tasksToAdd);

        if (!result.isError) {
            // 获取更新后的任务树
            let rootTaskUuid = this._taskManager.getCurrentRootTaskUuid();
            if (rootTaskUuid) {
                let taskTree = await this._taskManager.getHydratedTask(rootTaskUuid);
                let formattedList = fW(taskTree);
                let requestId = chatHistory.length > 0
                    ? chatHistory[chatHistory.length - 1].request_id
                    : "";
                mW(addType, requestId, formattedList);
            }
        }

        return result;  // 包含 plan 参数
    } catch (error) {
        this._logger.error("Error in AddTasksTool:", error);
        return ErrorResponse(`Failed to add tasks: ${error.message}`);
    }
}
```

**输入示例**:
```json
{
  "tasks": [
    {
      "name": "设计认证架构",
      "description": "设计整体的认证和授权架构"
    },
    {
      "name": "实现 OAuth2 集成",
      "description": "集成 OAuth2 provider",
      "parent_task_id": "abc-123"
    }
  ]
}
```

**输出**:
- 文本响应描述创建结果（包含新任务的 UUID）
- **`plan` 参数**: 包含新任务的完整任务树

### 3.4 ReorganizeTaskListTool (reorganize_tasklist)

**文件位置**: `chunks.77.mjs:2093-2150`

**类名**: `CZ extends qo`

**描述**: "Reorganize the task list structure for the current conversation. Use this only for major restructuring like reordering tasks, changing hierarchy. For individual task updates, use update_tasks tool."

**输入 Schema**:
```json
{
  "type": "object",
  "properties": {
    "markdown": {
      "type": "string",
      "description": "Markdown representation of task list. New tasks should have UUID: 'NEW_UUID'. Must contain exactly one root task with proper hierarchy using dash indentation."
    }
  },
  "required": ["markdown"]
}
```

**Markdown 格式示例**:
```markdown
- [/] 实现用户认证系统 (abc-root)
  - [x] 设计认证架构 (task-123)
  - [/] 实现 OAuth2 集成 (task-456)
    - [x] 配置 OAuth2 provider (task-789)
    - [ ] 实现授权流程 (NEW_UUID)
  - [ ] 实现 JWT token 管理 (NEW_UUID)
  - [ ] 编写单元测试 (NEW_UUID)
```

**格式规则**:
- 使用 `-` 表示列表项
- 使用空格缩进表示层级（每层 2 或 4 个空格）
- 状态标记：`[ ]` (NOT_STARTED), `[/]` (IN_PROGRESS), `[x]` (COMPLETE), `[-]` (CANCELLED)
- UUID 在括号中：`(task-uuid)` 或 `(NEW_UUID)` 表示新任务

**核心逻辑**:
```javascript
async call(params, chatHistory, abortSignal, toolHost, conversationId) {
    try {
        let markdown = params.markdown;

        // 解析 markdown
        let parsedTasks = this.parseMarkdown(markdown);
        if (!parsedTasks.success) {
            return ErrorResponse(`Failed to parse markdown: ${parsedTasks.error}`);
        }

        // 应用重组
        let result = await this._taskManager.reorganizeTaskList(parsedTasks.taskTree);

        if (!result.isError) {
            // 获取重组后的任务树
            let rootTaskUuid = this._taskManager.getCurrentRootTaskUuid();
            if (rootTaskUuid) {
                let taskTree = await this._taskManager.getHydratedTask(rootTaskUuid);
                let formattedList = fW(taskTree);
                let requestId = chatHistory.length > 0
                    ? chatHistory[chatHistory.length - 1].request_id
                    : "";
                mW(reorganizeType, requestId, formattedList);
            }
        }

        return result;  // 包含 plan 参数
    } catch (error) {
        this._logger.error("Error in ReorganizeTaskListTool:", error);
        return ErrorResponse(`Failed to reorganize task list: ${error.message}`);
    }
}
```

**使用场景**:
- 大规模任务重排序
- 改变任务层级结构
- 批量删除任务（不在 markdown 中的任务会被删除）

---

## 4. 完整工作流程

### 4.1 流程图

```
┌──────────────────────────────────────────────────────────────┐
│ 1. 用户请求                                                    │
│    "实现用户认证系统，支持 OAuth2 和 JWT"                       │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 2. LLM 分析请求                                               │
│    • 识别为复杂任务，需要规划                                  │
│    • 决定使用 add_tasks 工具                                  │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 3. 调用 add_tasks 工具                                        │
│    {                                                          │
│      "tasks": [                                               │
│        {"name": "设计认证架构", "description": "..."},         │
│        {"name": "实现 OAuth2", "parent_task_id": "...", ...}, │
│        {"name": "实现 JWT", ...},                             │
│        {"name": "编写测试", ...}                               │
│      ]                                                        │
│    }                                                          │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 4. TaskManager 执行                                           │
│    • 创建根任务（如果不存在）                                   │
│    • 创建 4 个子任务                                           │
│    • 设置任务属性（name, description, state, parent）          │
│    • 构建任务树结构                                            │
│    • 持久化到存储                                              │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 5. 工具返回响应                                                │
│    {                                                          │
│      text: "Successfully created 4 tasks...",                 │
│      isError: false,                                          │
│      plan: {  // ← 完整的任务树                                │
│        uuid: "root-uuid",                                     │
│        name: "Root Task",                                     │
│        state: "IN_PROGRESS",                                  │
│        subTasksData: [                                        │
│          {uuid: "...", name: "设计认证架构", state: "NOT_...},│
│          {uuid: "...", name: "实现 OAuth2", subTasksData: ...},│
│          ...                                                  │
│        ]                                                      │
│      }                                                        │
│    }                                                          │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 6. Xur() 处理工具响应                                          │
│    • 检测 toolName === "add_tasks" → Sqn() 返回 true          │
│    • 检测到 plan 参数存在                                      │
│    • 调用 vqn(planData) → Yur(planData)                       │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 7. Yur() 递归生成 Plan Entries                                │
│    遍历任务树，生成：                                          │
│    [                                                          │
│      {                                                        │
│        content: "设计认证架构",                                │
│        priority: "high",    // depth=1                        │
│        status: "pending"    // NOT_STARTED → pending          │
│      },                                                       │
│      {                                                        │
│        content: "实现 OAuth2 集成",                            │
│        priority: "high",    // depth=1                        │
│        status: "pending"                                      │
│      },                                                       │
│      ...                                                      │
│    ]                                                          │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 8. ACPEventListener 处理工具结果                               │
│    • 接收到 toolResult 包含 planEntries                        │
│    • 检测 r.plan && s.planEntries 为 true                     │
│    • 准备 session update                                      │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 9. 发送 Session Update                                        │
│    conn.sessionUpdate({                                       │
│      sessionId: "...",                                        │
│      update: {                                                │
│        sessionUpdate: "plan",                                 │
│        entries: [                                             │
│          {content: "...", priority: "high", status: "..."},   │
│          ...                                                  │
│        ]                                                      │
│      }                                                        │
│    })                                                         │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 10. 客户端接收并渲染                                           │
│     • WebSocket 接收 session update                           │
│     • 解析 plan entries                                       │
│     • 更新 UI 显示任务列表                                     │
│     • 高亮显示任务状态和优先级                                  │
└──────────────────────────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│ 11. LLM 继续执行任务                                           │
│     • 开始第一个任务："设计认证架构"                            │
│     • 调用 update_tasks 更新状态为 IN_PROGRESS                 │
│     • 完成后再次调用 update_tasks 更新为 COMPLETE              │
│     • 每次更新都触发新的 plan session update                    │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 时序图

```
User          LLM          Tool System    TaskManager    Response Handler    Client
  │             │                │              │                │              │
  │─Request────▶│                │              │                │              │
  │             │                │              │                │              │
  │             │─add_tasks─────▶│              │                │              │
  │             │                │              │                │              │
  │             │                │─createTask()─▶│                │              │
  │             │                │◀─task tree───│                │              │
  │             │                │              │                │              │
  │             │◀─response + plan──────────────│                │              │
  │             │                │              │                │              │
  │             │─────────────────────────────────▶Xur(toolName, response)      │
  │             │                │              │   • Detect plan                │
  │             │                │              │   • Call Yur()                 │
  │             │                │              │   • Generate entries           │
  │             │◀──────────────────────────────────{planEntries}│              │
  │             │                │              │                │              │
  │             │─────────────────────────────────────────────────▶sessionUpdate│
  │             │                │              │                │              │
  │             │                │              │                │   [Render Plan UI]
  │◀────────────────────────────────────────────────────────────────display tasks
  │             │                │              │                │              │
  │             │─update_tasks──▶│              │                │              │
  │             │                │─updateTask()─▶│                │              │
  │             │◀─response + plan──────────────│                │              │
  │             │─────────────────────────────────▶Xur() → Yur()                │
  │             │─────────────────────────────────────────────────▶sessionUpdate│
  │◀────────────────────────────────────────────────────────────────update UI   │
```

---

## 5. Feature Flag 控制

### 5.1 enableTaskList

**作用**: 控制任务管理工具的加载。

**影响的 Mode**:
- CLI_AGENT
- CLI_NONINTERACTIVE
- AGENT

**工具加载逻辑** (`chunks.78.mjs`):

```javascript
// 在 SidecarToolHost 构造函数中
if (mode === "CLI_AGENT" || mode === "CLI_NONINTERACTIVE") {
    // 加载基础工具...

    // 条件加载任务管理工具
    if (enableTaskList) {
        tools.push(
            new xZ(taskManager),   // view_tasklist
            new CZ(taskManager),   // reorganize_tasklist
            new yZ(taskManager),   // update_tasks
            new RZ(taskManager)    // add_tasks
        );
    }
}
else if (mode === "AGENT") {
    // AGENT 模式也支持任务工具
    if (enableTaskList) {
        tools.push(
            new xZ(taskManager),
            new CZ(taskManager),
            new yZ(taskManager),
            new RZ(taskManager)
        );
    }
}
```

### 5.2 配置来源

Feature flags 来自后端 API 响应：

```
Backend API: /get-models
    ↓
Response.feature_flags: {
  enable_task_list: true,
  ...
}
    ↓
Client parses and stores
    ↓
Used in tool loading
```

---

## 6. 数据结构详解

### 6.1 Task 数据结构

```typescript
interface Task {
  uuid: string;                // 任务唯一标识
  name: string;                // 任务名称
  description: string;         // 任务描述
  state: TaskState;            // 任务状态
  parentTaskId?: string;       // 父任务 ID（可选）
  subTasksData?: Task[];       // 子任务数组（可选）
  createdAt: number;           // 创建时间戳
  updatedAt: number;           // 更新时间戳
}

type TaskState =
  | "NOT_STARTED"
  | "IN_PROGRESS"
  | "COMPLETE"
  | "CANCELLED";
```

### 6.2 Plan Entry 数据结构

```typescript
interface PlanEntry {
  content: string;      // 任务名称
  priority: Priority;   // 优先级（基于任务深度）
  status: Status;       // 状态（从任务状态映射）
}

type Priority = "high" | "medium" | "low";
type Status = "pending" | "in_progress" | "completed";
```

### 6.3 状态映射规则

| Task State | Plan Status | 说明 |
|-----------|-------------|------|
| NOT_STARTED | pending | 未开始 |
| IN_PROGRESS | in_progress | 进行中 |
| COMPLETE | completed | 已完成 |
| CANCELLED | pending | 已取消（但在 plan entries 中会被跳过） |

### 6.4 优先级计算规则

| Task Depth | Priority | 说明 |
|-----------|----------|------|
| 0 | - | 根任务，不显示在 plan entries 中 |
| 1 | high | 顶层任务，最高优先级 |
| 2 | medium | 二级任务，中等优先级 |
| ≥3 | low | 三级及以下任务，低优先级 |

### 6.5 Tool Response 结构

```typescript
interface ToolResponse {
  text: string;           // 文本响应内容
  isError: boolean;       // 是否为错误响应
  plan?: Task;            // 任务树（仅任务工具返回）
}
```

### 6.6 Session Update Payload

```typescript
interface SessionUpdatePayload {
  sessionId: string;
  update: {
    sessionUpdate: "plan";
    entries: PlanEntry[];
  };
}
```

---

## 7. 与其他系统的集成

### 7.1 与 Chat History 的关系

```
┌─────────────────────────────────────────────────┐
│              Chat History                       │
│  [                                              │
│    { role: "user", content: "实现认证系统" },    │
│    { role: "assistant", content: "好的..." },   │
│    {                                            │
│      role: "assistant",                         │
│      tool_calls: [{                             │
│        name: "add_tasks",                       │
│        input: {...}                             │
│      }]                                         │
│    },                                           │
│    {                                            │
│      role: "tool",                              │
│      tool_call_id: "...",                       │
│      content: "Successfully created 4 tasks"    │
│      // plan 参数不存储在这里                     │
│    }                                            │
│  ]                                              │
└─────────────────────────────────────────────────┘
                       │
                       │ 工具响应包含 plan 参数
                       ▼
┌─────────────────────────────────────────────────┐
│          Plan Entries (Session Update)          │
│  • 不存储在 chat history 中                      │
│  • 通过 WebSocket 实时推送                       │
│  • 客户端单独维护和渲染                           │
└─────────────────────────────────────────────────┘
```

**关键点**:
- 任务工具调用**会记录**在 chat history 中
- Plan entries **不存储**在 chat history 中
- Plan entries 通过 session update **实时推送**
- 两个系统独立但协调工作

### 7.2 与 Checkpoint 的集成

**Checkpoint 机制**: Augment 使用 checkpoint 系统来追踪文件修改状态，支持回滚。

**任务操作与 Checkpoint**:

```javascript
// 在任务更新时创建 checkpoint
async updateTask(taskId, updates) {
    // 1. 创建 checkpoint
    let checkpoint = await this.checkpointManager.createCheckpoint();

    // 2. 更新任务
    let task = await this.storage.updateTask(taskId, updates);

    // 3. 关联 checkpoint 和任务
    await this.storage.linkCheckpointToTask(checkpoint.id, taskId);

    return task;
}
```

**回滚场景**:
- 当文件操作需要回滚时
- 相关的任务状态也应该回滚
- 保持任务状态与代码状态的一致性

```
File Edit → Checkpoint Created
    ↓
Task Updated (state: COMPLETE)
    ↓
User Requests Rollback
    ↓
File Reverted to Checkpoint
    ↓
Task State Reverted (state: IN_PROGRESS)
```

### 7.3 与 Agent Memory 的关系

```
┌─────────────────────────────────────────────────┐
│           Agent Memory System                   │
│  • 长期记忆存储                                  │
│  • 跨会话保持                                    │
│  • 记忆压缩（MEMORIES_COMPRESSION mode）         │
└─────────────────────────────────────────────────┘
                       ║
                       ║ 独立系统
                       ║
┌─────────────────────────────────────────────────┐
│            Task Management                      │
│  • TaskManager 持久化                           │
│  • 任务状态存储                                  │
│  • 跨会话保持任务状态                            │
└─────────────────────────────────────────────────┘
```

**关键区别**:

| 特性 | Agent Memory | Task Management |
|------|-------------|----------------|
| **存储内容** | 重要上下文、决策、学习 | 任务列表、状态、层级 |
| **数据结构** | 文本段落 | 结构化任务树 |
| **压缩机制** | MEMORIES_COMPRESSION mode | Plan entries 生成 |
| **更新频率** | 较低（关键时刻） | 高（每次任务变更） |
| **查询方式** | 语义搜索 | UUID 直接查询 |
| **生命周期** | 长期（可能永久） | 中期（项目周期） |

**协作场景**:
- Agent Memory 可能记录："用户偏好使用 TypeScript 和 Jest"
- Task Management 记录："编写 Jest 测试用例 - IN_PROGRESS"

---

## 8. 代码位置总结

| 功能 | 文件 | 行号 | 类/函数 | 说明 |
|------|------|------|---------|------|
| **Session Update 定义** | chunks.96.mjs | 2348 | z.literal("plan") | 定义 plan 类型 |
| **工具响应处理** | chunks.97.mjs | 285-343 | Xur() | 处理工具返回的 plan |
| **任务工具检测** | chunks.97.mjs | 336-337 | Sqn() | 判断是否为任务工具 |
| **Plan entries 提取** | chunks.97.mjs | 340-343 | vqn() | 调用 Yur 转换 |
| **递归转换函数** | chunks.97.mjs | 915-943 | Yur() | 任务树 → plan entries |
| **优先级计算** | chunks.97.mjs | 926-927 | uea() | depth → priority |
| **状态映射** | chunks.97.mjs | 930-942 | dea() | task state → status |
| **Plan 推送逻辑** | chunks.97.mjs | 1026-1032 | - | session update 发送 |
| **ViewTaskListTool** | chunks.77.mjs | 1957-1987 | xZ class | view_tasklist 工具 |
| **UpdateTasksTool** | chunks.77.mjs | 1989-2091 | yZ class | update_tasks 工具 |
| **ReorganizeTaskListTool** | chunks.77.mjs | 2093-2150 | CZ class | reorganize_tasklist 工具 |
| **AddTasksTool** | chunks.77.mjs | 2152-2276 | RZ class | add_tasks 工具 |
| **Tool Descriptions** | chunks.77.mjs | 1858-1865 | Jg.getToolDescriptions() | 工具描述 |
| **Chat Mode 验证** | chunks.78.mjs | 多处 | validateChatMode() | Mode 验证和切换 |
| **Tool Host** | chunks.78.mjs | 多处 | DZ class (SidecarToolHost) | 工具加载逻辑 |

---

## 9. 关键设计模式

### 9.1 跨 Mode 功能

**设计理念**: Plan 功能不绑定到特定的 chat mode，而是作为一个可选功能在多个 mode 中可用。

**优点**:
- ✅ **灵活性**: 可以在不同场景下使用 plan 功能
- ✅ **解耦**: Plan 逻辑与 mode 逻辑分离
- ✅ **可扩展**: 轻松添加到新的 mode

**实现机制**:
```javascript
// 在多个 mode 中条件加载任务工具
if (mode === "AGENT" || mode === "CLI_AGENT" || mode === "CLI_NONINTERACTIVE") {
    if (enableTaskList) {
        tools.push(...taskManagementTools);
    }
}
```

**对比**:
- ❌ **独立 Mode 方式**: 需要单独的 PLAN mode，切换麻烦
- ✅ **跨 Mode 功能**: 在需要时自然使用，无需切换

### 9.2 工具驱动

**设计理念**: Plan 功能完全由工具实现，LLM 自主决定何时使用。

**优点**:
- ✅ **智能决策**: LLM 根据任务复杂度决定是否需要规划
- ✅ **无需额外状态**: 不需要维护 "planning state"
- ✅ **与对话流程自然融合**: 工具调用是对话的一部分

**工作流程**:
```
User: "实现一个复杂的功能"
    ↓
LLM 思考: "这个任务很复杂，我应该先规划"
    ↓
LLM 决策: 调用 add_tasks 工具
    ↓
Tool 执行 + Plan 推送
    ↓
LLM 继续: "好的，我已经创建了计划，现在开始第一步..."
```

**对比其他方式**:
- ❌ **命令驱动**: 用户必须明确说 "创建计划"
- ❌ **状态驱动**: 系统维护复杂的状态机
- ✅ **工具驱动**: LLM 自然决策，用户无感知

### 9.3 实时推送

**设计理念**: Plan entries 通过 session update 实时推送，不等待任务完成。

**优点**:
- ✅ **即时反馈**: 用户立即看到计划
- ✅ **增量更新**: 任务状态变化时实时更新
- ✅ **不阻塞对话**: Plan 更新与对话并行

**实现机制**:
```javascript
// 工具返回后立即推送
toolResponse.plan && planEntries &&
    conn.sessionUpdate({
        sessionUpdate: "plan",
        entries: planEntries
    });

// 不等待任务完成，立即继续对话
```

**时序**:
```
T+0s: LLM 调用 add_tasks
T+0.1s: 工具返回响应
T+0.2s: Plan entries 推送 ← 实时
T+0.3s: 客户端渲染 ← 用户立即看到
T+1s: LLM 继续对话 ← 不阻塞
```

### 9.4 层级管理

**设计理念**: 支持任务树结构，递归转换保持层级，Priority 反映任务深度。

**任务树示例**:
```
Root Task (depth=0, 不显示)
├─ Task A (depth=1, priority=high)
│  ├─ Task A1 (depth=2, priority=medium)
│  └─ Task A2 (depth=2, priority=medium)
├─ Task B (depth=1, priority=high)
│  └─ Task B1 (depth=2, priority=medium)
│     └─ Task B1a (depth=3, priority=low)
└─ Task C (depth=1, priority=high)
```

**Plan Entries 输出**:
```javascript
[
  { content: "Task A", priority: "high", status: "pending" },
  { content: "Task A1", priority: "medium", status: "pending" },
  { content: "Task A2", priority: "medium", status: "pending" },
  { content: "Task B", priority: "high", status: "pending" },
  { content: "Task B1", priority: "medium", status: "pending" },
  { content: "Task B1a", priority: "low", status: "pending" },
  { content: "Task C", priority: "high", status: "pending" }
]
```

**优点**:
- ✅ **清晰的层级关系**: 通过 priority 反映
- ✅ **扁平化输出**: 客户端易于渲染
- ✅ **保持顺序**: 深度优先遍历

---

## 10. 与其他 Agent 系统对比

| 特性 | Augment | Claude Code | Cursor | GitHub Copilot |
|------|---------|-------------|--------|----------------|
| **独立 Plan Mode** | ❌ 无 | ✅ 有 | ❌ 无 | ❌ 无 |
| **任务管理工具** | ✅ 4个 (view, update, add, reorganize) | ✅ TodoWrite | ✅ Task List | ❌ 无 |
| **实时 Plan 更新** | ✅ Session Update | ✅ 实时更新 | ❌ 无 | ❌ 无 |
| **层级任务支持** | ✅ 完整支持（任务树） | ✅ 支持 | ✅ 有限支持 | ❌ 无 |
| **工具驱动设计** | ✅ 完全工具驱动 | ✅ 工具驱动 | ⚠️ 部分工具 | ❌ 命令驱动 |
| **跨 Mode 可用** | ✅ AGENT, CLI_AGENT 等 | ✅ 所有 mode | ❌ 特定场景 | ❌ 无 |
| **任务状态追踪** | ✅ 4种状态 | ✅ 3种状态 | ⚠️ 简单状态 | ❌ 无 |
| **批量操作** | ✅ 支持 | ✅ 支持 | ❌ 逐个操作 | ❌ 无 |
| **Markdown 重组** | ✅ reorganize_tasklist | ❌ 无 | ❌ 无 | ❌ 无 |
| **Checkpoint 集成** | ✅ 集成 | ✅ 集成 | ⚠️ 部分 | ❌ 无 |
| **优先级计算** | ✅ 自动（基于深度） | ⚠️ 手动 | ❌ 无 | ❌ 无 |

**总结**:
- **Augment**: 工具丰富，设计灵活，但没有独立 plan mode
- **Claude Code**: 有专门的 plan mode，但工具相对简单
- **Cursor**: 任务管理功能有限
- **GitHub Copilot**: 基本没有任务管理功能

---

## 11. 使用场景与示例

### 场景 1: 复杂功能开发

**用户请求**:
```
"实现用户认证系统，支持 OAuth2 和 JWT token，需要单元测试"
```

**LLM 响应流程**:

```
1. LLM 分析: "这是一个复杂任务，需要规划"

2. 调用 add_tasks 工具:
{
  "tasks": [
    {
      "name": "设计认证架构",
      "description": "设计整体的认证和授权架构，包括 OAuth2 和 JWT 的集成方案"
    },
    {
      "name": "实现 OAuth2 集成",
      "description": "集成第三方 OAuth2 provider（Google, GitHub 等）",
      "parent_task_id": null
    },
    {
      "name": "配置 OAuth2 provider",
      "description": "设置 OAuth2 客户端 ID、secret 和回调 URL",
      "parent_task_id": "<Task 2 UUID>"
    },
    {
      "name": "实现授权流程",
      "description": "实现 OAuth2 authorization code flow",
      "parent_task_id": "<Task 2 UUID>"
    },
    {
      "name": "实现 JWT token 管理",
      "description": "实现 JWT token 生成、验证和刷新逻辑"
    },
    {
      "name": "编写单元测试",
      "description": "为认证模块编写全面的单元测试"
    }
  ]
}

3. Plan Entries 推送到客户端:
[
  { content: "设计认证架构", priority: "high", status: "pending" },
  { content: "实现 OAuth2 集成", priority: "high", status: "pending" },
  { content: "配置 OAuth2 provider", priority: "medium", status: "pending" },
  { content: "实现授权流程", priority: "medium", status: "pending" },
  { content: "实现 JWT token 管理", priority: "high", status: "pending" },
  { content: "编写单元测试", priority: "high", status: "pending" }
]

4. LLM 开始执行:
"好的，我已经创建了实现计划。现在让我开始第一步：设计认证架构。"

5. 更新任务状态:
调用 update_tasks({ task_id: "<Task 1 UUID>", state: "IN_PROGRESS" })
→ Plan UI 实时更新

6. 完成第一步:
调用 update_tasks({ task_id: "<Task 1 UUID>", state: "COMPLETE" })
→ Plan UI 显示 ✓

7. 继续下一个任务...
```

### 场景 2: Bug 修复追踪

**用户请求**:
```
"登录页面有个 bug，用户输入错误密码后卡住了"
```

**LLM 响应流程**:

```
1. 调用 add_tasks:
{
  "tasks": [
    {"name": "重现 bug", "description": "在本地环境重现登录卡住的问题"},
    {"name": "定位问题代码", "description": "通过调试找到卡住的原因"},
    {"name": "修复 bug", "description": "修改代码解决卡住问题"},
    {"name": "验证修复", "description": "测试确认 bug 已解决"}
  ]
}

2. 开始执行:
update_tasks({ task_id: "<Task 1>", state: "IN_PROGRESS" })

3. "我正在重现这个 bug..."
[执行调试步骤]

4. 完成重现:
update_tasks({ task_id: "<Task 1>", state: "COMPLETE" })
update_tasks({ task_id: "<Task 2>", state: "IN_PROGRESS" })

5. "找到问题了，是在密码验证失败后没有解除 loading 状态"
[继续修复...]

6. 每完成一步都更新 plan
```

### 场景 3: 重构任务

**用户请求**:
```
"重构这个组件，拆分成更小的子组件"
```

**LLM 响应流程**:

```
1. 先查看现有代码理解结构

2. 创建重构计划:
add_tasks([
  {"name": "分析组件职责", "description": "..."},
  {"name": "设计子组件结构", "description": "..."},
  {"name": "创建 Header 子组件", "description": "..."},
  {"name": "创建 Content 子组件", "description": "..."},
  {"name": "创建 Footer 子组件", "description": "..."},
  {"name": "重构主组件", "description": "..."},
  {"name": "更新测试", "description": "..."}
])

3. 按计划执行，每完成一个子组件更新状态

4. 如果需要调整计划:
调用 reorganize_tasklist 通过 markdown 重组
```

### 场景 4: 大规模任务重组

**初始计划**:
```
- [ ] 实现功能 A
- [ ] 实现功能 B
- [ ] 实现功能 C
```

**用户反馈**: "我们需要先做 B，而且 A 需要拆分成更小的任务"

**LLM 调用** `reorganize_tasklist`:
```markdown
- [/] 实现功能 B (task-b-uuid)
- [ ] 实现功能 A (task-a-uuid)
  - [ ] A 的设计阶段 (NEW_UUID)
  - [ ] A 的实现阶段 (NEW_UUID)
  - [ ] A 的测试阶段 (NEW_UUID)
- [ ] 实现功能 C (task-c-uuid)
```

**结果**: 任务顺序调整，A 拆分为子任务，plan UI 立即更新。

---

## 12. 最佳实践

### 12.1 任务粒度控制

**推荐做法**:
- ✅ **最多 3 层深度**: depth 1 (high), depth 2 (medium), depth 3 (low)
- ✅ **每个任务目标明确**: 可测试、可完成的单元
- ✅ **避免过度拆分**: 太细的任务增加管理开销

**示例**:

✅ **好的粒度**:
```
- 实现用户认证 (depth=1, high)
  - OAuth2 集成 (depth=2, medium)
    - 配置 provider (depth=3, low)
    - 实现授权流程 (depth=3, low)
  - JWT 管理 (depth=2, medium)
  - 测试 (depth=2, medium)
```

❌ **过度拆分**:
```
- 实现用户认证
  - OAuth2 集成
    - 创建 OAuth2 配置文件
      - 添加 client_id 字段
      - 添加 client_secret 字段
      - 添加 redirect_uri 字段  ← 太细了！
```

### 12.2 状态转换规则

**标准流程**:
```
NOT_STARTED → IN_PROGRESS → COMPLETE
```

**取消任务**:
```
任意状态 → CANCELLED
```

**注意**: CANCELLED 任务不会出现在 plan entries 中（被 Yur 函数跳过）。

**示例**:
```javascript
// 开始任务
update_tasks({ task_id: "...", state: "IN_PROGRESS" })

// 完成任务
update_tasks({ task_id: "...", state: "COMPLETE" })

// 取消任务
update_tasks({ task_id: "...", state: "CANCELLED" })
```

### 12.3 批量操作优化

**推荐**: 使用批量操作减少工具调用次数。

✅ **推荐**:
```javascript
// 一次更新多个任务
update_tasks({
  tasks: [
    { task_id: "task-1", state: "COMPLETE" },
    { task_id: "task-2", state: "IN_PROGRESS" },
    { task_id: "task-3", name: "新任务名" }
  ]
})
```

❌ **不推荐**:
```javascript
// 多次调用工具
update_tasks({ tasks: [{ task_id: "task-1", state: "COMPLETE" }] })
update_tasks({ tasks: [{ task_id: "task-2", state: "IN_PROGRESS" }] })
update_tasks({ tasks: [{ task_id: "task-3", name: "新任务名" }] })
```

**优点**:
- 减少 LLM 调用次数
- 减少 plan update 次数
- 提高性能

### 12.4 重组时机

**reorganize_tasklist 适用场景**:
- ✅ 大规模结构调整（改变多个任务的层级关系）
- ✅ 任务重排序（改变多个任务的顺序）
- ✅ 批量删除任务（不在 markdown 中的任务会被删除）

**update_tasks 适用场景**:
- ✅ 单个或少量任务更新
- ✅ 仅修改任务属性（state, name, description）
- ✅ 不改变结构

**示例**:

✅ **使用 reorganize_tasklist**:
```
需求: "把任务 B 移到 A 下面作为子任务，并删除任务 C"
→ 使用 reorganize_tasklist 重建整个结构
```

✅ **使用 update_tasks**:
```
需求: "把任务 A 标记为完成"
→ 使用 update_tasks 更新单个任务
```

---

## 13. 限制与注意事项

### 13.1 不是独立 Mode

**限制**:
- ❌ 无法单独进入 "plan mode"
- ❌ 需要在支持的 mode 中使用（AGENT、CLI_AGENT、CLI_NONINTERACTIVE）
- ❌ 依赖 `enableTaskList` feature flag

**影响**:
- 如果 feature flag 未启用，任务工具不可用
- 如果在 CHAT mode，任务工具可能不加载

**检查方法**:
```javascript
// 确认当前 mode 是否支持任务工具
if (mode === "AGENT" || mode === "CLI_AGENT" || mode === "CLI_NONINTERACTIVE") {
    if (featureFlags.enableTaskList) {
        // 任务工具可用
    }
}
```

### 13.2 依赖 LLM 决策

**限制**:
- ❌ LLM 决定何时使用任务工具
- ❌ 无法强制 LLM 创建计划
- ❌ Prompt 设计影响 plan 功能使用

**影响**:
- 简单任务 LLM 可能不创建计划
- 需要在 system prompt 中引导 LLM 使用任务工具

**缓解方法**:
- 在 system prompt 中明确说明任务工具的用途
- 提供 few-shot 示例展示何时使用
- 用户可以明确要求："请先创建一个计划"

### 13.3 任务持久化

**特性**:
- ✅ 任务存储在 TaskManager
- ✅ 会话结束后任务状态保留
- ⚠️ 需要考虑任务清理策略

**注意事项**:
- 任务可能跨多个会话累积
- 需要定期清理已完成或取消的任务
- 大量任务可能影响性能

**建议**:
```javascript
// 在合适的时机清理任务
if (taskList.length > 100 && allTasksComplete()) {
    taskManager.archiveTasks();
}
```

### 13.4 UI 渲染依赖

**限制**:
- ❌ Plan entries 通过 session update 推送
- ❌ 客户端需要实现 plan UI 渲染
- ❌ 无法在纯 CLI 环境中显示 plan UI

**影响**:
- 没有 WebSocket 连接时 plan 功能不可用
- 客户端必须实现 plan UI
- 纯文本界面（如 SSH）无法显示 plan

**替代方案**:
- 使用 `view_tasklist` 工具查看任务列表
- 输出为文本格式显示在对话中

---

## 14. 待深入分析的问题

### 14.1 TaskManager 实现细节

**待研究**:
- ❓ 任务存储机制（数据库？文件？内存？）
- ❓ 持久化策略（何时保存？事务支持？）
- ❓ 任务查询性能（索引？缓存？）
- ❓ 并发控制（多个 agent 同时操作？）

**为何重要**: 理解存储机制有助于优化性能和可靠性。

### 14.2 Checkpoint 集成

**待研究**:
- ❓ 任务操作如何创建 checkpoint
- ❓ 回滚机制的具体实现
- ❓ 如何协调文件操作与任务状态
- ❓ Checkpoint 与任务的关联方式

**为何重要**: Checkpoint 与任务的紧密集成保证了一致性。

### 14.3 LLM Prompt 设计

**待研究**:
- ❓ System prompt 如何引导 LLM 使用任务工具
- ❓ 任务拆解的 few-shot 示例
- ❓ Plan 功能的 prompt 工程最佳实践
- ❓ 如何让 LLM 更智能地决策任务粒度

**为何重要**: Prompt 设计直接影响 plan 功能的使用效果。

**相关文档**: 参考 `docs/PROMPT_SYSTEM.md` 进行分析。

### 14.4 客户端 UI 实现

**待研究**:
- ❓ Plan entries 如何渲染（组件设计？）
- ❓ 用户交互方式（点击任务？展开/折叠？）
- ❓ 任务状态可视化（进度条？颜色编码？）
- ❓ 实时更新动画效果

**为何重要**: 良好的 UI 设计提升用户体验。

---

## 15. 总结

### 核心结论

1. **Augment 没有独立的 PLAN 或 PLANNING chat mode**
   - 定义了 8 种 chat mode，不包含 PLAN

2. **Plan 是跨 mode 功能**
   - 通过 4 个任务管理工具实现
   - 在 AGENT、CLI_AGENT 等模式中可用
   - 由 `enableTaskList` feature flag 控制

3. **核心实现机制**
   - Session Update Type: "plan"
   - 任务树 → Plan Entries 递归转换
   - 实时推送给客户端

4. **设计模式**
   - 跨 Mode 功能：灵活、可扩展
   - 工具驱动：LLM 自主决策
   - 实时推送：即时反馈
   - 层级管理：任务树结构

### 优势

- ✅ **工具丰富**: 4 个任务管理工具覆盖所有场景
- ✅ **灵活设计**: 跨 mode 可用，不受限于特定模式
- ✅ **实时反馈**: Session update 机制提供即时更新
- ✅ **层级支持**: 完整的任务树结构
- ✅ **批量操作**: 高效的批量更新能力

### 劣势

- ❌ **无独立 Mode**: 不像 Claude Code 有专门的 plan mode
- ❌ **依赖 LLM**: 无法强制创建计划
- ❌ **UI 依赖**: 纯 CLI 环境体验受限

### 适用场景

- ✅ 复杂功能开发（多步骤任务）
- ✅ Bug 修复追踪（系统化修复流程）
- ✅ 代码重构（结构化重构计划）
- ✅ 项目管理（任务分配和追踪）

### 未来可能的改进方向

1. **独立 Plan Mode**: 添加专门的 PLANNING mode
2. **更智能的 Prompt**: 自动决策何时创建计划
3. **CLI 友好输出**: 纯文本环境的 plan 可视化
4. **任务模板**: 预定义的任务分解模板
5. **任务依赖**: 支持任务间的依赖关系

---

## 附录

### A. 完整代码示例

#### 示例 1: Yur 函数完整实现

```javascript
// 文件: chunks.97.mjs:915-943
// 功能: 递归转换任务树为 plan entries

function Yur(task, entries = [], depth = 0) {
    // 跳过已取消的任务
    if (task.state === "CANCELLED") {
        return entries;
    }

    // depth > 0 时才添加（跳过根任务）
    if (depth > 0) {
        entries.push({
            content: task.name,
            priority: uea(depth),      // 计算优先级
            status: dea(task.state)    // 映射状态
        });
    }

    // 递归处理子任务
    if (task.subTasksData && Array.isArray(task.subTasksData)) {
        for (let subTask of task.subTasksData) {
            Yur(subTask, entries, depth + 1);
        }
    }

    return entries;
}

// 优先级计算函数
function uea(depth) {
    return depth <= 1 ? "high"
         : depth === 2 ? "medium"
         : "low";
}

// 状态映射函数
function dea(state) {
    switch (state) {
        case "NOT_STARTED": return "pending";
        case "IN_PROGRESS": return "in_progress";
        case "COMPLETE": return "completed";
        case "CANCELLED": return "pending";
        default: return "pending";
    }
}
```

### B. 工具调用示例

#### 示例 1: add_tasks

```json
{
  "tool": "add_tasks",
  "input": {
    "tasks": [
      {
        "name": "设计数据库 schema",
        "description": "设计用户、角色和权限表的 schema"
      },
      {
        "name": "实现 CRUD API",
        "description": "实现用户管理的 CRUD 接口"
      }
    ]
  }
}
```

#### 示例 2: update_tasks

```json
{
  "tool": "update_tasks",
  "input": {
    "tasks": [
      {
        "task_id": "abc-123",
        "state": "COMPLETE"
      }
    ]
  }
}
```

#### 示例 3: reorganize_tasklist

```json
{
  "tool": "reorganize_tasklist",
  "input": {
    "markdown": "- [/] 项目实现 (root-uuid)\n  - [x] 设计阶段 (task-1)\n  - [/] 实现阶段 (task-2)\n    - [ ] 前端开发 (NEW_UUID)\n    - [ ] 后端开发 (NEW_UUID)\n  - [ ] 测试阶段 (task-3)"
  }
}
```

### C. 相关文档

- `docs/COMPACT_MECHANISM.md` - Compact 机制分析
- `docs/PROMPT_SYSTEM.md` - Prompt 系统分析
- `docs/CODE_SEARCH_ANALYSIS.md` - 代码搜索分析

---

**文档创建时间**: 2025-12-05
**分析状态**: ✅ 完成
**版本**: v1.0
