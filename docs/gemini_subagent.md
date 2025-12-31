# Gemini-CLI Subagent 实现分析

> 本文档深入分析 gemini-cli 的 subagent 实现，用于指导 codex 的 subagent 优化。

## 1. 设计目标

### 1.1 核心目标

1. **任务隔离**：Subagent 在独立的执行上下文中运行，拥有自己的工具白名单、消息历史和资源限制
2. **安全约束**：只允许只读工具（ls, read-file, grep, glob 等），防止 subagent 执行危险操作
3. **资源控制**：支持超时限制和回合限制，防止无限循环和资源耗尽
4. **优雅终止**：提供 Grace Period 机制，让 subagent 在达到限制时仍有机会提交结果
5. **可观察性**：通过 Activity Event 流式输出 subagent 的执行过程

### 1.2 设计原则

- **Tool-as-Agent**：将 Agent 包装成普通 Tool，父 Agent 可以像调用工具一样调用子 Agent
- **声明式配置**：通过 `AgentDefinition` 声明 Agent 的所有配置
- **强类型输出**：使用 Zod Schema 验证 subagent 输出
- **单一完成信号**：必须调用 `complete_task` 工具来结束任务

---

## 2. 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                         Parent Agent                             │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                      ToolRegistry                            ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ ││
│  │  │  read-file   │  │    grep      │  │ SubagentToolWrapper│ ││
│  │  └──────────────┘  └──────────────┘  └─────────┬──────────┘ ││
│  └──────────────────────────────────────────────────┼───────────┘│
└──────────────────────────────────────────────────────┼───────────┘
                                                       │
                                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SubagentToolWrapper                           │
│  - 将 AgentDefinition 包装成 DeclarativeTool                     │
│  - 动态生成 InputConfig → JSON Schema                            │
│  - 创建 SubagentInvocation 实例                                  │
└──────────────────────────────────────────────────────────────────┘
                                                       │
                                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SubagentInvocation                           │
│  - BaseToolInvocation<AgentInputs, ToolResult>                   │
│  - 桥接 AgentExecutor 和 Tool 输出流                              │
│  - 格式化最终结果为 ToolResult                                    │
└──────────────────────────────────────────────────────────────────┘
                                                       │
                                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                      AgentExecutor                               │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  独立执行循环                                                 ││
│  │  1. 初始化 GeminiChat + 隔离的 ToolRegistry                   ││
│  │  2. while (!terminated) {                                    ││
│  │       - 检查终止条件 (超时/回合限制)                           ││
│  │       - 调用模型                                              ││
│  │       - 处理 function calls                                   ││
│  │       - 如果 complete_task → 返回结果                         ││
│  │     }                                                        ││
│  │  3. Grace Period 恢复尝试                                     ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  工具白名单: ls, read-file, grep, glob, read-many-files,         │
│             memory, web-search                                   │
└──────────────────────────────────────────────────────────────────┘
```

---

## 3. 核心数据结构

### 3.1 AgentDefinition - Agent 完整定义

```typescript
interface AgentDefinition<TOutput extends z.ZodTypeAny> {
  // 基本信息
  name: string;                    // 唯一标识符，用于注册和调用
  displayName?: string;            // 显示名称
  description: string;             // 描述，告诉父 Agent 何时调用

  // 配置
  promptConfig: PromptConfig;      // Prompt 配置
  modelConfig: ModelConfig;        // 模型配置
  runConfig: RunConfig;            // 运行配置
  toolConfig?: ToolConfig;         // 工具配置
  inputConfig: InputConfig;        // 输入参数配置
  outputConfig?: OutputConfig<TOutput>;  // 输出配置（可选）

  // 输出处理
  processOutput?: (output: z.infer<TOutput>) => string;  // 自定义输出处理
}
```

### 3.2 PromptConfig - Prompt 配置

```typescript
interface PromptConfig {
  // 系统提示词，支持 ${input_name} 模板语法
  systemPrompt?: string;

  // 初始消息（few-shot prompting）
  initialMessages?: Content[];

  // 触发 Agent 执行的查询语句，支持模板
  // 如果不提供，默认使用 "Get Started!"
  query?: string;
}
```

### 3.3 ModelConfig - 模型配置

```typescript
interface ModelConfig {
  model: string;           // 模型名称
  temp: number;            // 温度参数
  top_p: number;           // Top-P 采样
  thinkingBudget?: number; // 思考预算 (-1 表示无限制)
}
```

### 3.4 RunConfig - 运行限制

```typescript
interface RunConfig {
  max_time_minutes: number;  // 最大执行时间（分钟）
  max_turns?: number;        // 最大对话回合数
}
```

### 3.5 InputConfig - 输入参数定义

```typescript
interface InputConfig {
  inputs: Record<string, {
    description: string;
    type: 'string' | 'number' | 'boolean' | 'integer' | 'string[]' | 'number[]';
    required: boolean;
  }>;
}
```

### 3.6 OutputConfig - 输出定义

```typescript
interface OutputConfig<T extends z.ZodTypeAny> {
  outputName: string;      // 输出参数名称（用于 complete_task 工具）
  description: string;     // 输出描述
  schema: T;               // Zod Schema，用于验证输出
}
```

### 3.7 AgentTerminateMode - 终止模式枚举

```typescript
enum AgentTerminateMode {
  GOAL = 'GOAL',                              // 成功完成
  TIMEOUT = 'TIMEOUT',                        // 超时
  MAX_TURNS = 'MAX_TURNS',                    // 达到回合限制
  ERROR = 'ERROR',                            // 执行错误
  ABORTED = 'ABORTED',                        // 用户取消
  ERROR_NO_COMPLETE_TASK_CALL = 'ERROR_NO_COMPLETE_TASK_CALL',  // 未调用 complete_task
}
```

### 3.8 SubagentActivityEvent - 活动事件

```typescript
interface SubagentActivityEvent {
  isSubagentActivityEvent: true;  // 类型标识
  agentName: string;              // Agent 名称
  type: 'TOOL_CALL_START' | 'TOOL_CALL_END' | 'THOUGHT_CHUNK' | 'ERROR';
  data: Record<string, unknown>;  // 事件数据
}
```

---

## 4. 核心组件详解

### 4.1 AgentRegistry - Agent 注册表

**文件**: `packages/core/src/agents/registry.ts`

**职责**:
- 管理 AgentDefinition 的注册和查询
- 加载内置 Agent（如 CodebaseInvestigatorAgent）
- 为每个 Agent 注册独立的 ModelConfig

**关键代码**:

```typescript
class AgentRegistry {
  private readonly agents = new Map<string, AgentDefinition<any>>();

  async initialize(): Promise<void> {
    this.loadBuiltInAgents();
  }

  private loadBuiltInAgents(): void {
    const settings = this.config.getCodebaseInvestigatorSettings();
    if (settings?.enabled) {
      // 合并用户配置和默认配置
      const agentDef = {
        ...CodebaseInvestigatorAgent,
        modelConfig: { ...CodebaseInvestigatorAgent.modelConfig, ...userOverrides },
        runConfig: { ...CodebaseInvestigatorAgent.runConfig, ...userOverrides },
      };
      this.registerAgent(agentDef);
    }
  }

  protected registerAgent<TOutput>(definition: AgentDefinition<TOutput>): void {
    this.agents.set(definition.name, definition);
    // 同时注册模型配置
    this.config.modelConfigService.registerRuntimeModelConfig(
      `${definition.name}-config`,
      runtimeAlias,
    );
  }
}
```

### 4.2 SubagentToolWrapper - Agent 到 Tool 的包装器

**文件**: `packages/core/src/agents/subagent-tool-wrapper.ts`

**职责**:
- 将 `AgentDefinition` 包装成标准的 `DeclarativeTool`
- 动态生成 InputConfig → JSON Schema
- 创建 `SubagentInvocation` 实例

**关键设计**:

```typescript
class SubagentToolWrapper extends BaseDeclarativeTool<AgentInputs, ToolResult> {
  constructor(
    private readonly definition: AgentDefinition,
    private readonly config: Config,
    messageBus?: MessageBus,
  ) {
    // 动态生成 JSON Schema
    const parameterSchema = convertInputConfigToJsonSchema(definition.inputConfig);

    super(
      definition.name,
      definition.displayName ?? definition.name,
      definition.description,
      Kind.Think,           // 工具类型：思考型
      parameterSchema,
      true,                 // isOutputMarkdown
      true,                 // canUpdateOutput (支持流式输出)
      messageBus,
    );
  }

  // 当父 Agent 调用此工具时，创建执行实例
  protected createInvocation(params: AgentInputs): ToolInvocation {
    return new SubagentInvocation(params, this.definition, this.config, this.messageBus);
  }
}
```

### 4.3 SubagentInvocation - 单次执行实例

**文件**: `packages/core/src/agents/invocation.ts`

**职责**:
- 代表一次 subagent 调用
- 初始化 `AgentExecutor`
- 桥接执行器事件到工具输出流
- 格式化最终结果

**关键代码**:

```typescript
class SubagentInvocation extends BaseToolInvocation<AgentInputs, ToolResult> {
  async execute(
    signal: AbortSignal,
    updateOutput?: (output: string | AnsiOutput) => void,
  ): Promise<ToolResult> {
    // 活动回调：将执行器事件转发到 UI
    const onActivity = (activity: SubagentActivityEvent): void => {
      if (activity.type === 'THOUGHT_CHUNK' && typeof activity.data['text'] === 'string') {
        updateOutput?.(`🤖💭 ${activity.data['text']}`);
      }
    };

    const executor = await AgentExecutor.create(this.definition, this.config, onActivity);
    const output = await executor.run(this.params, signal);

    return {
      llmContent: [{ text: `Subagent '${this.definition.name}' finished.\nResult:\n${output.result}` }],
      returnDisplay: `Termination Reason: ${output.terminate_reason}\n\n${output.result}`,
    };
  }
}
```

### 4.4 AgentExecutor - 核心执行引擎

**文件**: `packages/core/src/agents/executor.ts`

**职责**:
- 管理 Agent 的完整执行生命周期
- 隔离工具注册表
- 实现超时和回合限制
- 提供 Grace Period 恢复机制
- 发送活动事件

#### 4.4.1 工具白名单

```typescript
const allowlist = new Set([
  LS_TOOL_NAME,
  READ_FILE_TOOL_NAME,
  GREP_TOOL_NAME,
  GLOB_TOOL_NAME,
  READ_MANY_FILES_TOOL_NAME,
  MEMORY_TOOL_NAME,
  WEB_SEARCH_TOOL_NAME,
]);
```

#### 4.4.2 执行循环核心算法

```typescript
async run(inputs: AgentInputs, signal: AbortSignal): Promise<OutputObject> {
  const { max_time_minutes } = this.definition.runConfig;

  // 1. 设置超时控制器
  const timeoutController = new AbortController();
  setTimeout(() => timeoutController.abort(), max_time_minutes * 60 * 1000);
  const combinedSignal = AbortSignal.any([signal, timeoutController.signal]);

  // 2. 初始化 Chat 和工具列表
  const tools = this.prepareToolsList();  // 包含 complete_task
  const chat = await this.createChatObject(inputs, tools);
  const query = templateString(this.definition.promptConfig.query ?? 'Get Started!', inputs);
  let currentMessage = { role: 'user', parts: [{ text: query }] };

  // 3. 执行循环
  while (true) {
    // 检查终止条件
    const reason = this.checkTermination(startTime, turnCounter);
    if (reason || combinedSignal.aborted) break;

    // 执行单个回合
    const turnResult = await this.executeTurn(chat, currentMessage, turnCounter++, combinedSignal, timeoutController.signal);

    if (turnResult.status === 'stop') {
      terminateReason = turnResult.terminateReason;
      finalResult = turnResult.finalResult;
      break;
    }

    currentMessage = turnResult.nextMessage;
  }

  // 4. Grace Period 恢复尝试
  if (terminateReason !== GOAL && terminateReason !== ABORTED && terminateReason !== ERROR) {
    const recoveryResult = await this.executeFinalWarningTurn(chat, turnCounter, terminateReason, signal);
    if (recoveryResult !== null) {
      terminateReason = GOAL;
      finalResult = recoveryResult;
    }
  }

  return { result: finalResult, terminate_reason: terminateReason };
}
```

#### 4.4.3 Grace Period 恢复机制

```typescript
private async executeFinalWarningTurn(
  chat: GeminiChat,
  turnCounter: number,
  reason: TIMEOUT | MAX_TURNS | ERROR_NO_COMPLETE_TASK_CALL,
  externalSignal: AbortSignal,
): Promise<string | null> {
  const GRACE_PERIOD_MS = 60 * 1000;  // 60 秒

  // 发送警告消息
  const warningMessage = `${explanation} You have one final chance to complete the task.
    You MUST call \`complete_task\` immediately with your best answer.
    Do not call any other tools.`;

  const graceTimeoutController = new AbortController();
  setTimeout(() => graceTimeoutController.abort(), GRACE_PERIOD_MS);

  const combinedSignal = AbortSignal.any([externalSignal, graceTimeoutController.signal]);
  const turnResult = await this.executeTurn(chat, recoveryMessage, turnCounter, combinedSignal, graceTimeoutController.signal);

  if (turnResult.status === 'stop' && turnResult.terminateReason === GOAL) {
    return turnResult.finalResult;  // 恢复成功
  }

  return null;  // 恢复失败
}
```

#### 4.4.4 complete_task 工具处理

```typescript
// 动态生成 complete_task 工具定义
private prepareToolsList(): FunctionDeclaration[] {
  const completeTool: FunctionDeclaration = {
    name: 'complete_task',
    description: outputConfig
      ? 'Call this tool to submit your final answer and complete the task.'
      : 'Call this tool to signal that you have completed your task.',
    parameters: {
      type: 'object',
      properties: {},
      required: [],
    },
  };

  // 如果有 outputConfig，添加输出参数
  if (outputConfig) {
    const jsonSchema = zodToJsonSchema(outputConfig.schema);
    completeTool.parameters.properties[outputConfig.outputName] = jsonSchema;
    completeTool.parameters.required.push(outputConfig.outputName);
  }

  return [...registeredTools, completeTool];
}
```

---

## 5. 模板系统

**文件**: `packages/core/src/agents/utils.ts`

支持在 systemPrompt 和 query 中使用 `${input_name}` 占位符：

```typescript
function templateString(template: string, inputs: AgentInputs): string {
  const placeholderRegex = /\$\{(\w+)\}/g;

  // 验证所有占位符都有对应的输入
  const requiredKeys = new Set(Array.from(template.matchAll(placeholderRegex), (m) => m[1]));
  const missingKeys = Array.from(requiredKeys).filter((key) => !(key in inputs));
  if (missingKeys.length > 0) {
    throw new Error(`Missing required input parameters: ${missingKeys.join(', ')}`);
  }

  return template.replace(placeholderRegex, (_match, key) => String(inputs[key]));
}
```

---

## 6. Schema 工具

**文件**: `packages/core/src/agents/schema-utils.ts`

将 `InputConfig` 转换为标准 JSON Schema：

```typescript
function convertInputConfigToJsonSchema(inputConfig: InputConfig): JsonSchemaObject {
  const properties: Record<string, JsonSchemaProperty> = {};
  const required: string[] = [];

  for (const [name, definition] of Object.entries(inputConfig.inputs)) {
    switch (definition.type) {
      case 'string':
      case 'number':
      case 'integer':
      case 'boolean':
        properties[name] = { type: definition.type, description: definition.description };
        break;
      case 'string[]':
        properties[name] = { type: 'array', items: { type: 'string' }, description: definition.description };
        break;
      case 'number[]':
        properties[name] = { type: 'array', items: { type: 'number' }, description: definition.description };
        break;
    }

    if (definition.required) {
      required.push(name);
    }
  }

  return { type: 'object', properties, required };
}
```

---

## 7. 完整示例：CodebaseInvestigatorAgent

**文件**: `packages/core/src/agents/codebase-investigator.ts`

```typescript
const CodebaseInvestigationReportSchema = z.object({
  SummaryOfFindings: z.string().describe("Investigation conclusions"),
  ExplorationTrace: z.array(z.string()).describe("Step-by-step actions"),
  RelevantLocations: z.array(z.object({
    FilePath: z.string(),
    Reasoning: z.string(),
    KeySymbols: z.array(z.string()),
  })).describe("Relevant files"),
});

export const CodebaseInvestigatorAgent: AgentDefinition<typeof CodebaseInvestigationReportSchema> = {
  name: 'codebase_investigator',
  displayName: 'Codebase Investigator Agent',
  description: `The specialized tool for codebase analysis...`,

  inputConfig: {
    inputs: {
      objective: {
        description: `Comprehensive description of the user's goal...`,
        type: 'string',
        required: true,
      },
    },
  },

  outputConfig: {
    outputName: 'report',
    description: 'The final investigation report as a JSON object.',
    schema: CodebaseInvestigationReportSchema,
  },

  processOutput: (output) => JSON.stringify(output, null, 2),

  modelConfig: {
    model: DEFAULT_GEMINI_MODEL,
    temp: 0.1,        // 低温度确保准确性
    top_p: 0.95,
    thinkingBudget: -1,  // 无限思考
  },

  runConfig: {
    max_time_minutes: 5,
    max_turns: 15,
  },

  toolConfig: {
    tools: [LS_TOOL_NAME, READ_FILE_TOOL_NAME, GLOB_TOOL_NAME, GREP_TOOL_NAME],
  },

  promptConfig: {
    query: `Your task is to do a deep investigation for the following objective:
<objective>
\${objective}
</objective>`,

    systemPrompt: `You are **Codebase Investigator**, a hyper-specialized AI agent...
## Core Directives
1. DEEP ANALYSIS, NOT JUST FILE FINDING
2. SYSTEMATIC & CURIOUS EXPLORATION
3. HOLISTIC & PRECISE

## Scratchpad Management
[详细的 scratchpad 规则]

## Termination
Your mission is complete ONLY when your Questions to Resolve list is empty.
You MUST call the complete_task tool with a valid JSON report.
`,
  },
};
```

---

## 8. 文件清单

| 文件路径 | 职责 | 行数 |
|---------|------|------|
| `agents/types.ts` | 核心类型定义（AgentDefinition, AgentTerminateMode 等） | 170 |
| `agents/executor.ts` | AgentExecutor 执行引擎（含 Grace Period 恢复） | 1080 |
| `agents/invocation.ts` | SubagentInvocation 执行实例 | 138 |
| `agents/subagent-tool-wrapper.ts` | 将 Agent 包装为 Tool | 79 |
| `agents/registry.ts` | AgentRegistry 注册表 | 136 |
| `agents/codebase-investigator.ts` | 内置 Agent 示例 | 154 |
| `agents/schema-utils.ts` | InputConfig → JSON Schema 转换 | 91 |
| `agents/utils.ts` | 模板字符串处理 | 44 |

---

## 9. 关键设计决策总结

| 决策 | 说明 | 优点 |
|-----|------|------|
| Tool-as-Agent | Agent 被包装成 Tool | 统一的调用接口，父 Agent 无需特殊处理 |
| 只读工具白名单 | 只允许 ls/grep/read 等 | 安全隔离，防止破坏性操作 |
| complete_task 强制 | 必须调用此工具结束 | 明确的完成信号，避免悬空执行 |
| Grace Period | 超时后给 60 秒补救 | 避免工作白费，提高成功率 |
| Zod Schema 验证 | 输出类型强校验 | 确保结构化输出符合预期 |
| 模板语法 | ${input_name} | 灵活的 Prompt 定制 |
| 隔离 ToolRegistry | 每个 Agent 独立工具集 | 安全边界，避免权限泄露 |
| Activity Event 流 | 实时事件通知 | 可观察性，便于调试和 UI 展示 |

---

## 10. 对 Codex 优化的启示

1. **引入 AgentDefinition 声明式配置**：将 Agent 配置从代码中抽离，支持运行时加载
2. **实现 Tool-as-Agent 模式**：统一 Tool 和 Agent 的调用接口
3. **添加 Grace Period 机制**：避免超时导致的结果丢失
4. **使用 Zod/JSON Schema 验证输出**：确保 subagent 返回结构化、可验证的结果
5. **实现 Activity Event 流**：提供 subagent 执行的可观察性
6. **严格的工具白名单**：确保 subagent 不能执行危险操作
7. **模板系统**：支持在 Prompt 中使用输入参数
