# qwen-code Plan 模式与 System Reminder 实现分析

> 本文档详细分析 qwen-code 的 ApprovalMode.PLAN 和 `<system-reminder>` 机制实现，供 codex 参考实现相关功能。

---

## 1. 设计目标

### 1.1 Plan 模式目标

| 目标 | 说明 |
|------|------|
| **安全执行** | 在用户确认前，阻止所有可能修改系统状态的操作 |
| **研究优先** | 允许 AI 自由探索、搜索、读取代码，但不做任何修改 |
| **用户控制** | 用户可以审查计划后决定是否执行、如何执行 |
| **渐进授权** | 用户可选择单次执行(DEFAULT)或自动编辑(AUTO_EDIT) |

### 1.2 System Reminder 目标

| 目标 | 说明 |
|------|------|
| **动态注入** | 在运行时向模型注入上下文相关的指令，无需修改系统提示词 |
| **透明传递** | 模型可见，用户不可见，实现内部状态同步 |
| **覆盖性优先** | 明确声明优先级高于其他指令 |
| **结构化标记** | 使用 XML 标签便于模型识别和遵循 |

---

## 2. 核心架构

### 2.1 ApprovalMode 枚举

**文件**: `packages/core/src/config/config.ts:104-111`

```typescript
export enum ApprovalMode {
  PLAN = 'plan',        // 最保守 - 只读操作，需要确认计划
  DEFAULT = 'default',  // 默认 - 需要用户确认每个操作
  AUTO_EDIT = 'auto-edit', // 自动编辑 - 自动执行编辑操作
  YOLO = 'yolo',        // 完全自动 - 无需任何确认
}
```

### 2.2 工具权限分类

**文件**: `packages/core/src/tools/tools.ts:585-603`

```typescript
export enum Kind {
  Read = 'read',        // ✅ Plan 模式允许
  Search = 'search',    // ✅ Plan 模式允许
  Fetch = 'fetch',      // ✅ Plan 模式允许
  Think = 'think',      // ✅ 仅 exit_plan_mode
  Edit = 'edit',        // ❌ Plan 模式阻断
  Delete = 'delete',    // ❌ Plan 模式阻断
  Move = 'move',        // ❌ Plan 模式阻断
  Execute = 'execute',  // ❌ Plan 模式阻断
  Other = 'other',      // 取决于 confirmation 配置
}

export const MUTATOR_KINDS: Kind[] = [
  Kind.Edit,
  Kind.Delete,
  Kind.Move,
  Kind.Execute,
];
```

---

## 3. Plan 模式实现细节

### 3.1 系统提示注入

**文件**: `packages/core/src/core/prompts.ts:849-855`

```typescript
export function getPlanModeSystemReminder(): string {
  return `<system-reminder>
Plan mode is active. The user indicated that they do not want you to execute yet -- you MUST NOT make any edits, run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supercedes any other instructions you have received (for example, to make edits). Instead, you should:
1. Answer the user's query comprehensively
2. When you're done researching, present your plan by calling the ${ToolNames.EXIT_PLAN_MODE} tool, which will prompt the user to confirm the plan. Do NOT make any file changes or run any tools that modify the system state in any way until the user has confirmed the plan.
</system-reminder>`;
}
```

**关键设计点**:
- 使用 `<system-reminder>` XML 标签包裹
- 明确声明 "supercedes any other instructions" 确保优先级
- 指向 `exit_plan_mode` 工具作为唯一退出路径

### 3.2 工具执行阻断

**文件**: `packages/core/src/core/coreToolScheduler.ts:780-800`

```typescript
const allowedTools = this.config.getAllowedTools() || [];
const isPlanMode = this.config.getApprovalMode() === ApprovalMode.PLAN;
const isExitPlanModeTool = reqInfo.name === 'exit_plan_mode';

if (isPlanMode && !isExitPlanModeTool) {
  if (confirmationDetails) {
    // 需要确认的工具 = 非只读工具，阻断并返回错误
    this.setStatusInternal(reqInfo.callId, 'error', {
      callId: reqInfo.callId,
      responseParts: convertToFunctionResponse(
        reqInfo.name,
        reqInfo.callId,
        getPlanModeSystemReminder(),  // 返回提醒，强化约束
      ),
      resultDisplay: 'Plan mode blocked a non-read-only tool call.',
      error: undefined,
      errorType: undefined,
    });
  } else {
    // 不需要确认的工具 = 只读工具，正常调度
    this.setStatusInternal(reqInfo.callId, 'scheduled');
  }
}
```

**阻断逻辑**:
1. 检查当前是否为 PLAN 模式
2. 检查工具是否为 `exit_plan_mode`（唯一豁免）
3. 检查工具是否有 `confirmationDetails`（需要确认 = 非只读）
4. 如需确认则阻断，返回错误并附带 system-reminder

### 3.3 ExitPlanMode 工具

**文件**: `packages/core/src/tools/exitPlanMode.ts`

```typescript
export class ExitPlanModeToolInvocation extends BaseToolInvocation<
  ExitPlanModeParams,
  ToolResult
> {
  // 工具属性
  static Name = 'exit_plan_mode';

  // 参数定义
  params: {
    plan: string;  // 必需，AI 提交的计划内容
  }

  // 确认类型：特殊的 'plan' 类型
  override async shouldConfirmExecute(
    _abortSignal: AbortSignal,
  ): Promise<ToolPlanConfirmationDetails> {
    const details: ToolPlanConfirmationDetails = {
      type: 'plan',  // 特殊确认类型
      title: 'Would you like to proceed?',
      plan: this.params.plan,
      onConfirm: async (outcome: ToolConfirmationOutcome) => {
        switch (outcome) {
          case ToolConfirmationOutcome.ProceedAlways:
            this.wasApproved = true;
            this.setApprovalModeSafely(ApprovalMode.AUTO_EDIT);
            break;
          case ToolConfirmationOutcome.ProceedOnce:
            this.wasApproved = true;
            this.setApprovalModeSafely(ApprovalMode.DEFAULT);
            break;
          case ToolConfirmationOutcome.Cancel:
            this.wasApproved = false;
            this.setApprovalModeSafely(ApprovalMode.PLAN);
            break;
          case ToolConfirmationOutcome.ProceedOnceNoFeedback:
            this.wasApproved = true;
            this.setApprovalModeSafely(ApprovalMode.DEFAULT);
            break;
        }
      },
    };
    return details;
  }

  // 工具类型：Think（认知/规划类）
  get kind(): Kind {
    return Kind.Think;
  }
}
```

### 3.4 状态转换流程

```
                    ┌─────────────┐
                    │   PLAN      │ (只读研究阶段)
                    │  - 读取文件  │
                    │  - 搜索代码  │
                    │  - 网络查询  │
                    └──────┬──────┘
                           │
              exit_plan_mode(plan: "...")
                           │
                    ┌──────▼──────┐
                    │  用户确认弹窗 │
                    │  显示 plan   │
                    └──────┬──────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
    ProceedAlways     ProceedOnce         Cancel
    (总是继续)         (单次继续)          (取消)
         │                 │                 │
    ┌────▼────┐       ┌────▼────┐       ┌────▼────┐
    │AUTO_EDIT│       │ DEFAULT │       │  PLAN   │
    │ 自动编辑 │       │ 需确认   │       │ 保持只读│
    └─────────┘       └─────────┘       └─────────┘
```

---

## 4. System Reminder 实现细节

### 4.1 注入时机与位置

**文件**: `packages/core/src/core/client.ts:511-532`

```typescript
// sendMessageStream 方法中
let requestToSent = await flatMapTextParts(request, async (text) => [text]);

if (isNewPrompt) {  // 关键：仅在新提示时注入
  const systemReminders = [];

  // 条件1: Subagent 提醒
  const hasTaskTool = this.config.getToolRegistry().getTool(TaskTool.Name);
  const subagents = (await this.config.getSubagentManager().listSubagents())
    .filter((subagent) => subagent.level !== 'builtin')
    .map((subagent) => subagent.name);

  if (hasTaskTool && subagents.length > 0) {
    systemReminders.push(getSubagentSystemReminder(subagents));
  }

  // 条件2: Plan 模式提醒
  if (this.config.getApprovalMode() === ApprovalMode.PLAN) {
    systemReminders.push(getPlanModeSystemReminder());
  }

  // 注入方式：前置到用户消息
  requestToSent = [...systemReminders, ...requestToSent];
}

const resultStream = turn.run(
  this.config.getModel(),
  requestToSent,
  signal,
);
```

### 4.2 消息格式结构

#### A. 用户消息前缀注入

```typescript
// requestToSent 数组结构
[
  "<system-reminder>You have powerful specialized agents...</system-reminder>",
  "<system-reminder>Plan mode is active...</system-reminder>",
  "用户的实际输入内容"
]
```

#### B. 最终消息角色

```
┌─────────────────────────────────────────────────────────┐
│ role: "user"                                            │
│ content: [                                              │
│   { type: "text", text: "<system-reminder>..." },       │
│   { type: "text", text: "<system-reminder>..." },       │
│   { type: "text", text: "用户原始输入" }                 │
│ ]                                                       │
└─────────────────────────────────────────────────────────┘
```

### 4.3 System Reminder 类型一览

| 类型 | 函数/来源 | 触发条件 | 注入位置 |
|------|----------|---------|---------|
| **Subagent 提醒** | `getSubagentSystemReminder()` | isNewPrompt + TaskTool存在 + 有subagent | 用户消息前缀 |
| **Plan 模式提醒** | `getPlanModeSystemReminder()` | isNewPrompt + ApprovalMode.PLAN | 用户消息前缀 |
| **Todo 状态提醒** | TodoWriteTool 响应 | TodoWrite 工具执行后 | 工具结果 llmContent |
| **工具阻断提醒** | coreToolScheduler | Plan模式下调用非只读工具 | 工具错误响应 |

### 4.4 工具结果中的 System Reminder

**文件**: `packages/core/src/tools/todoWrite.ts:350-363`

```typescript
// ToolResult 结构
interface ToolResult {
  llmContent: string;      // 模型可见，可含 system-reminder
  returnDisplay: string;   // 用户可见，不含 system-reminder
}

// 示例：Todo 更新成功
return {
  llmContent: `Todos have been modified successfully.

<system-reminder>
Your todo list has changed. DO NOT mention this explicitly to the user.
Here are the latest contents of your todo list:
${todosJson}. Continue on with the tasks at hand if applicable.
</system-reminder>`,

  returnDisplay: todoResultDisplay  // 用户看到的简洁输出
};
```

### 4.5 系统提示词中的声明

**文件**: `packages/core/src/core/prompts.ts:213`

```typescript
// 在核心系统提示词中告知模型
`- Tool results and user messages may include <system-reminder> tags.
<system-reminder> tags contain useful information and reminders.
They are NOT part of the user's provided input or the tool result.`
```

---

## 5. 关键设计决策

### 5.1 为什么使用 XML 标签？

1. **结构化**: 模型容易识别和解析
2. **隔离性**: 与普通文本明确区分
3. **可嵌套**: 可以在任意位置注入
4. **自描述**: 标签名即说明用途

### 5.2 为什么前置到用户消息？

1. **最近优先**: 模型对最近的上下文更敏感
2. **无角色冲突**: 不需要额外的 system 角色消息
3. **动态性**: 每轮可以注入不同内容
4. **简单实现**: 只需数组拼接

### 5.3 为什么不使用 Subagent？

qwen-code 的 Plan 模式设计为**单 agent 模式约束**，不使用 subagent：

| 特性 | 设计选择 | 原因 |
|------|---------|------|
| 执行隔离 | 工具调度层阻断 | 简单直接，无需复杂的 agent 协调 |
| 研究能力 | 复用当前 agent | 避免上下文切换开销 |
| 计划输出 | exit_plan_mode 工具 | 统一的退出路径，便于用户审查 |

### 5.4 为什么不启用扩展思考？

Plan 模式与 thinking/reasoning 配置是**独立维度**：

- **Plan 模式**: 权限控制（能做什么）
- **Thinking 模式**: 推理深度（如何思考）

两者可以组合使用，但不互相依赖。

---

## 6. Codex 实现建议

### 6.1 核心组件

```rust
// 1. ApprovalMode 枚举
pub enum ApprovalMode {
    Plan,       // 只读研究
    Default,    // 需要确认
    AutoEdit,   // 自动编辑
    Yolo,       // 完全自动
}

// 2. 工具权限分类
pub enum ToolKind {
    Read,       // Plan 模式允许
    Search,     // Plan 模式允许
    Fetch,      // Plan 模式允许
    Think,      // 仅 exit_plan_mode
    Edit,       // Plan 模式阻断
    Execute,    // Plan 模式阻断
    Delete,     // Plan 模式阻断
}

// 3. System Reminder 生成
pub fn get_plan_mode_system_reminder() -> String {
    format!(r#"<system-reminder>
Plan mode is active. You MUST NOT make any edits or run non-readonly tools.
Instead, research and call {} when ready to present your plan.
</system-reminder>"#, EXIT_PLAN_MODE_TOOL)
}

// 4. 消息注入
pub fn inject_system_reminders(
    request: &mut Vec<Content>,
    config: &Config,
    is_new_prompt: bool,
) {
    if !is_new_prompt { return; }

    let mut reminders = Vec::new();

    if config.approval_mode() == ApprovalMode::Plan {
        reminders.push(get_plan_mode_system_reminder());
    }

    // 前置注入
    request.splice(0..0, reminders.into_iter().map(Content::Text));
}
```

### 6.2 工具阻断逻辑

```rust
// 在工具调度器中
pub fn schedule_tool(&mut self, tool_call: ToolCall) -> Result<()> {
    let is_plan_mode = self.config.approval_mode() == ApprovalMode::Plan;
    let is_exit_plan = tool_call.name == "exit_plan_mode";

    if is_plan_mode && !is_exit_plan {
        if tool_call.requires_confirmation() {
            // 阻断并返回错误 + system-reminder
            return Err(ToolError::PlanModeBlocked {
                message: "Plan mode blocked this tool call.",
                reminder: get_plan_mode_system_reminder(),
            });
        }
    }

    // 正常调度
    self.execute(tool_call)
}
```

### 6.3 ExitPlanMode 工具

```rust
pub struct ExitPlanModeTool;

impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str { "exit_plan_mode" }
    fn kind(&self) -> ToolKind { ToolKind::Think }

    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter {
            name: "plan",
            type_: "string",
            required: true,
            description: "The plan to present to the user",
        }]
    }

    async fn execute(&self, params: Value, ctx: &mut Context) -> Result<ToolResult> {
        let plan = params["plan"].as_str().unwrap();

        // 请求用户确认
        let outcome = ctx.request_plan_confirmation(plan).await?;

        match outcome {
            Outcome::ProceedAlways => {
                ctx.set_approval_mode(ApprovalMode::AutoEdit);
                Ok(ToolResult::success("Proceeding with auto-edit mode."))
            }
            Outcome::ProceedOnce => {
                ctx.set_approval_mode(ApprovalMode::Default);
                Ok(ToolResult::success("Proceeding with default mode."))
            }
            Outcome::Cancel => {
                Ok(ToolResult::success("Staying in plan mode."))
            }
        }
    }
}
```

### 6.4 ToolResult 双通道设计

```rust
pub struct ToolResult {
    /// 模型可见的完整输出（可含 system-reminder）
    pub llm_content: String,

    /// 用户可见的简洁输出（不含 system-reminder）
    pub display_content: String,
}

// 使用示例
impl TodoWriteTool {
    fn execute(&self, ...) -> ToolResult {
        ToolResult {
            llm_content: format!(
                "Todos updated.\n\n<system-reminder>\n{}\n</system-reminder>",
                serde_json::to_string(&todos)?
            ),
            display_content: "Todos updated successfully.".to_string(),
        }
    }
}
```

---

## 7. 关键文件索引

| 文件 | 行号 | 功能 |
|------|------|------|
| `config/config.ts` | 104-111 | ApprovalMode 枚举定义 |
| `config/config.ts` | 799-810 | setApprovalMode() 验证 |
| `core/prompts.ts` | 823-825 | getSubagentSystemReminder() |
| `core/prompts.ts` | 849-855 | getPlanModeSystemReminder() |
| `core/prompts.ts` | 213 | 系统提示词中的 reminder 声明 |
| `core/client.ts` | 511-532 | **注入逻辑入口** |
| `core/coreToolScheduler.ts` | 780-800 | **工具阻断逻辑** |
| `tools/exitPlanMode.ts` | 49-150 | ExitPlanMode 工具实现 |
| `tools/todoWrite.ts` | 350-363 | 工具结果内嵌 reminder |
| `tools/tools.ts` | 585-603 | Kind 枚举和 MUTATOR_KINDS |

---

## 8. 总结

qwen-code 的 Plan 模式和 System Reminder 机制是一套**轻量级的行为控制系统**：

1. **Plan 模式**: 通过工具调度层的阻断逻辑实现只读约束，不使用 subagent，不启用特殊推理模式
2. **System Reminder**: 通过 XML 标签在用户消息前注入动态指令，实现运行时行为控制
3. **双通道输出**: ToolResult 区分 llmContent 和 displayContent，实现内部状态同步
4. **状态机转换**: 通过 exit_plan_mode 工具的用户确认结果决定后续模式

这套设计的核心优势是**简单直接**，在不增加架构复杂度的前提下实现了有效的权限控制和动态指令注入。

---

## 9. 附录：与其他实现的对比

### 9.1 Claude Code Plan 模式对比

| 特性 | qwen-code | Claude Code |
|------|-----------|-------------|
| Plan 文件 | ❌ 无，计划通过工具参数传递 | ✅ 写入指定 .md 文件 |
| Subagent | ❌ 不使用 | ✅ 使用 Explore/Plan subagent |
| 退出机制 | `exit_plan_mode` 工具 | `ExitPlanMode` 工具 |
| 扩展思考 | ❌ 独立功能 | ❌ 独立功能 |
| 工具阻断 | 调度层阻断 | 系统提示约束 |
| Reminder 注入 | 用户消息前缀 | 用户消息前缀 |

### 9.2 设计权衡

**qwen-code 优势**：
- 实现简单，代码量少
- 无需额外的 agent 管理
- 阻断逻辑在调度层，强制性高

**Claude Code 优势**：
- Plan 文件便于用户审查和编辑
- Subagent 可以并行探索
- 更灵活的工作流控制
