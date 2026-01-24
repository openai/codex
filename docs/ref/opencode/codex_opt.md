# Codex 优化建议：OpenCode 特性对比

本文档基于对 OpenCode 源码的深入分析，提出 Codex 可参考的功能特性和实现细节，按优先级排序。

---

## 1. 执行摘要

### 1.1 对比结论

| 领域 | OpenCode 优势 | Codex 优势 | 建议 |
|------|--------------|-----------|------|
| Provider 适配 | 30+ 内置 | 1 个 (Gemini) | **借鉴** |
| Plugin 系统 | 完整钩子 | 无 | **借鉴** |
| Doom Loop | 自动检测 | 无 | **借鉴** |
| 上下文压缩 | 自动+手动 | CompactV2 更强 | 保持 |
| System Reminder | TextPart 注入 | 7 个 Generator | 保持 |
| LSP 集成 | 40+ Server | 功能类似 | 保持 |

### 1.2 优先级分布

```
P0 (Critical)  ████████  3 项 - Doom Loop, Adapters, Plugin
P1 (High)      ██████    4 项 - EventBus, MaxSteps, ModeSwitch, Agents
P2 (Medium)    ████      3 项 - ConfigVar, ProviderPrompt, Retry
P3 (Low)       ███       3 项 - Color, MarkdownConfig, Keybinds
```

---

## 2. 模块对比表

### 2.1 Provider/Adapter 系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 内置适配器数量 | 30+ | 1 (Gemini) | **Critical** | P0 |
| 环境变量发现 | 自动检测 `ANTHROPIC_API_KEY` 等 | 手动配置 | Medium | P1 |
| 自定义 Header | per-provider 注入 | 有限 | Low | P2 |
| Response ID | 会话连续性 | ✓ 已实现 | - | - |
| 模型成本追踪 | input/output/cache 价格 | 无 | Medium | P2 |

**OpenCode 内置 Provider 列表:**
```typescript
const BUNDLED_PROVIDERS = {
  "@ai-sdk/amazon-bedrock": createAmazonBedrock,
  "@ai-sdk/anthropic": createAnthropic,
  "@ai-sdk/azure": createAzure,
  "@ai-sdk/google": createGoogleGenerativeAI,
  "@ai-sdk/google-vertex": createVertex,
  "@ai-sdk/google-vertex/anthropic": createVertexAnthropic,
  "@ai-sdk/openai": createOpenAI,
  "@ai-sdk/openai-compatible": createOpenAICompatible,
  "@openrouter/ai-sdk-provider": createOpenRouter,
  "@ai-sdk/xai": createXai,
  "@ai-sdk/mistral": createMistral,
  "@ai-sdk/groq": createGroq,
  "@ai-sdk/deepinfra": createDeepInfra,
  "@ai-sdk/cerebras": createCerebras,
  "@ai-sdk/cohere": createCohere,
  "@ai-sdk/gateway": createGateway,
  "@ai-sdk/togetherai": createTogetherAI,
  "@ai-sdk/perplexity": createPerplexity,
  "@ai-sdk/github-copilot": createGitHubCopilotOpenAICompatible,
}
```

### 2.2 Subagent 系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 内置 Agent | 7 个 | 2 个 | Medium | P1 |
| Max Steps 限制 | per-agent 迭代上限 | 无 | Medium | P1 |
| Agent 颜色 | 视觉标识 | 无 | Low | P3 |
| 模式切换提醒 | Plan→Build 自动注入 | 无 | Medium | P1 |
| Agent 描述 | description 字段 | 有 | - | - |

**OpenCode 内置 Agent:**
| Agent | 模式 | 职责 |
|-------|------|------|
| build | primary | 主要构建代理 |
| plan | primary | 规划代理 |
| general | subagent | 通用研究 |
| explore | subagent | 代码探索 |
| compaction | subagent | 上下文压缩 |
| title | subagent | 会话标题生成 |
| summary | subagent | 内容摘要 |

### 2.3 System Reminder/Attachment

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 附件系统 | Synthetic TextPart | 7 个 Generator | Codex 领先 | - |
| Plan Mode | 专用提醒 | ✓ PlanReminderGenerator | - | - |
| Doom Loop | 3+ 相同调用 → 警告 | 无 | **High** | P0 |
| Throttle | 基于 turn 的节流 | ✓ ThrottleConfig | - | - |

### 2.4 Tools 系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 核心工具 | 15+ | 15+ | - | - |
| 权限过滤 | ask/allow/deny | Tool filtering | 类似 | - |
| Doom Loop 防护 | 自动检测重复 | 无 | **High** | P0 |
| 工具重试 | 失败自动重试 | 无 | Medium | P2 |

### 2.5 Plugin/Hook 系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| Plugin 接口 | 完整钩子系统 | **无** | **Critical** | P0 |
| Event Bus | 全事件发布/订阅 | 最小 event bridge | **High** | P1 |
| Plugin 发现 | npm + 本地文件 | **无** | **High** | P1 |
| Hook 触发 | before/after tool | **无** | **High** | P1 |

### 2.6 配置系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 多层配置 | Global → Project → Dir → Env | Profile-based | 类似 | - |
| 变量替换 | `{env:VAR}`, `{file:path}` | 有限 env 支持 | Medium | P2 |
| 快捷键 | 70+ 可配置 | TUI keybinds 存在 | Low | P3 |
| Markdown 配置 | agent/*.md, command/*.md | 仅 YAML | Low | P3 |

### 2.7 Prompt 系统

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 三层结构 | Header + Provider + Context | 系统提示词组装 | 类似 | - |
| Provider 专属提示 | Anthropic/GPT/Gemini 变体 | 无 | Medium | P2 |
| 自定义指令 | CLAUDE.md, AGENTS.md | CODEX.md 存在 | 类似 | - |

### 2.8 Context Compaction

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 自动压缩 | overflow → 自动压缩 | ✓ CompactV2 | Codex 领先 | - |
| Prune 阈值 | 20K/40K tokens | 可配置 | 类似 | - |
| 保护工具 | skill tool 保护 | Tool filtering | 类似 | - |

### 2.9 LSP 集成

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 内置 Server | 40+ 语言服务器 | Feature-gated LSP | 类似 | - |
| 诊断注入 | Error/warning 注入上下文 | ✓ LspDiagnosticsGenerator | - | - |

### 2.10 会话管理 (新增)

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 会话状态跟踪 | idle/busy/retry 状态 | 无 | Medium | P2 |
| 会话恢复 | Task Tool 的 session_id 参数 | 有 InitialHistory | 类似 | - |
| 步骤快照 | 每步记录文件 diff | 无 | Medium | P2 |
| 并行子代理 | 多 Task 并发调用 | 顺序执行 | Medium | P2 |
| 会话分享 | share URL + 过期时间 | 无 | Low | P3 |

### 2.11 Skill 系统 (新增)

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| Skill 发现 | SKILL.md 自动扫描 | ✓ Skills 模块 | 类似 | - |
| Skill 路径 | 多层 (admin/system/user/repo) | 多层支持 | 类似 | - |
| Skill 调用 | Skill Tool 执行 | SkillInjections | 类似 | - |

### 2.12 MCP 系统 (新增)

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 传输类型 | Stdio + HTTP + SSE | Stdio + StreamableHttp | 类似 | - |
| OAuth 认证 | 完整 OAuth 流程 | 无 | Low | P3 |
| 工具过滤 | 配置级别 | ✓ enabled/disabled tools | 类似 | - |
| 动态连接 | connect/disconnect | 无 | Low | P3 |

### 2.13 实验性功能 (新增)

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 文件编辑钩子 | file_edited hook | 无 | Medium | P2 |
| 会话完成钩子 | session_completed hook | 无 | Medium | P2 |
| OpenTelemetry | experimental.openTelemetry | ✓ OTEL 配置 | 类似 | - |
| 批量工具 | batch_tool | 无 | Low | P3 |
| 小模型配置 | small_model (用于 title/summary) | 无 | Medium | P2 |

### 2.14 Bash 权限模式 (新增详情)

| 特性 | OpenCode | Codex | 差距 | 优先级 |
|------|----------|-------|------|--------|
| 模式匹配 | minimatch 通配符 | 无精细控制 | Medium | P2 |
| 预设模式 | plan agent 只读命令白名单 | 无 | Medium | P2 |
| 动态匹配 | `git diff*`, `find * -delete*` | 静态配置 | Medium | P2 |

**OpenCode Plan Agent Bash 白名单示例:**
```typescript
bash: {
  "git diff*": "allow",
  "git log*": "allow",
  "git show*": "allow",
  "git status*": "allow",
  "git branch": "allow",
  "grep*": "allow",
  "ls*": "allow",
  "find *": "allow",
  "find * -delete*": "ask",  // 删除需确认
  "find * -exec*": "ask",    // exec 需确认
  "*": "ask",                // 其他询问
}
```

---

## 3. P0 优先级功能 (Critical)

### 3.1 Doom Loop 检测

**问题:** LLM 可能陷入无限重复相同工具调用的循环。

**OpenCode 实现:**

```typescript
// packages/opencode/src/session/processor.ts
const DOOM_LOOP_THRESHOLD = 3

// 检测逻辑
const toolCallHistory: string[] = []

function detectDoomLoop(toolName: string, args: any): boolean {
  const signature = JSON.stringify({ tool: toolName, args })
  toolCallHistory.push(signature)

  // 检查最近 N 次调用是否相同
  if (toolCallHistory.length >= DOOM_LOOP_THRESHOLD) {
    const recent = toolCallHistory.slice(-DOOM_LOOP_THRESHOLD)
    if (recent.every(s => s === recent[0])) {
      return true // 触发 doom loop
    }
  }
  return false
}

// 触发时的处理
if (detectDoomLoop(call.name, call.args)) {
  // 1. 注入警告到上下文
  // 2. 拒绝执行并提示用户
  // 3. 可配置为 ask/deny
}
```

**Codex 实现建议:**

```rust
// core/src/doom_loop.rs
pub struct DoomLoopDetector {
    history: VecDeque<String>,
    threshold: usize,
}

impl DoomLoopDetector {
    pub fn new(threshold: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(threshold + 1),
            threshold,
        }
    }

    pub fn check(&mut self, tool_name: &str, args: &serde_json::Value) -> bool {
        let signature = format!("{}:{}", tool_name, args);
        self.history.push_back(signature.clone());

        if self.history.len() > self.threshold {
            self.history.pop_front();
        }

        if self.history.len() >= self.threshold {
            self.history.iter().all(|s| s == &signature)
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
    }
}
```

**关键文件:**
- `core/src/codex.rs` - 集成检测逻辑
- `core/src/system_reminder/` - 添加 DoomLoopGenerator

---

### 3.2 Adapter 扩展

**问题:** Codex 仅有 1 个内置 Adapter (Gemini)，限制了多 Provider 支持。

**OpenCode 架构:**

```typescript
// 自定义 Loader 模式
const CUSTOM_LOADERS: Record<string, CustomLoader> = {
  async anthropic() {
    return {
      autoload: false,
      options: {
        headers: {
          "anthropic-beta": "claude-code-20250219,interleaved-thinking-2025-05-14",
        },
      },
    }
  },

  async openai() {
    return {
      autoload: false,
      async getModel(sdk, modelID) {
        return sdk.responses(modelID)  // 使用 responses API
      },
    }
  },

  async "amazon-bedrock"() {
    const region = Env.get("AWS_REGION") ?? "us-east-1"
    return {
      autoload: true,
      options: {
        region,
        credentialProvider: fromNodeProviderChain(),
      },
      async getModel(sdk, modelID) {
        // 自动添加区域前缀
        if (modelID.includes("claude")) {
          modelID = `${regionPrefix}.${modelID}`
        }
        return sdk.languageModel(modelID)
      },
    }
  },
}
```

**Codex 实现建议:**

```rust
// codex-api/src/adapters/anthropic.rs
pub struct AnthropicAdapter {
    client: reqwest::Client,
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn name(&self) -> &str { "anthropic" }

    fn validate_provider(&self, provider: &ModelProviderInfo) -> Result<()> {
        if provider.wire_api != Some("messages") {
            return Err(CodexErr::Fatal("Anthropic requires wire_api=messages".into()));
        }
        Ok(())
    }

    async fn generate(
        &self,
        prompt: Prompt,
        config: &AdapterConfig,
        provider: &ModelProviderInfo,
    ) -> Result<Vec<ResponseEvent>> {
        // Transform to Anthropic format
        let request = self.transform_request(&prompt, config, provider)?;

        // Send request with Anthropic-specific headers
        let response = self.client
            .post(&format!("{}/v1/messages", provider.base_url()))
            .header("x-api-key", &config.api_key.unwrap_or_default())
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "claude-code-20250219")
            .json(&request)
            .send()
            .await?;

        self.transform_response(response).await
    }
}

// codex-api/src/adapters/openai.rs
pub struct OpenAIAdapter { ... }

// codex-api/src/adapters/azure.rs
pub struct AzureAdapter { ... }
```

**注册表扩展:**

```rust
// codex-api/src/adapters/registry.rs
fn initialize_registry(registry: &mut AdapterRegistry) {
    registry.register(Arc::new(GeminiAdapter::new()));
    registry.register(Arc::new(AnthropicAdapter::new()));
    registry.register(Arc::new(OpenAIAdapter::new()));
    registry.register(Arc::new(AzureAdapter::new()));
    registry.register(Arc::new(BedrockAdapter::new()));
}
```

---

### 3.3 Plugin/Hook 系统

**问题:** Codex 缺乏统一的扩展机制，限制了第三方集成。

**OpenCode 架构:**

```typescript
// packages/opencode/src/plugin/index.ts
interface Hooks {
  auth?: (provider: string) => Promise<Credentials>
  event?: (event: BusEvent) => void
  tool?: (name: string, args: any) => void
  config?: (config: Config) => void
  system?: (prompt: string) => string  // transform system prompt
  message?: (msg: Message) => Message  // transform message
}

// 触发钩子
export async function trigger<Name extends keyof Hooks>(
  name: Name,
  input: any,
  output: any
): Promise<any> {
  for (const hook of await state().then(x => x.hooks)) {
    const fn = hook[name]
    if (!fn) continue
    await fn(input, output)  // 链式调用
  }
  return output
}

// 事件订阅
Bus.subscribeAll(async (event) => {
  for (const hook of hooks) {
    hook.event?.({ event })
  }
})
```

**Codex 实现建议:**

```rust
// core/src/plugin/mod.rs
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    // 生命周期钩子
    fn on_init(&self, config: &Config) -> Result<()> { Ok(()) }
    fn on_shutdown(&self) -> Result<()> { Ok(()) }

    // 事件钩子
    fn on_event(&self, event: &Event) -> Result<()> { Ok(()) }

    // 转换钩子
    fn transform_system(&self, prompt: &mut String) -> Result<()> { Ok(()) }
    fn transform_message(&self, msg: &mut Message) -> Result<()> { Ok(()) }

    // 工具钩子
    fn before_tool(&self, name: &str, args: &Value) -> Result<()> { Ok(()) }
    fn after_tool(&self, name: &str, result: &Value) -> Result<()> { Ok(()) }
}

// core/src/plugin/registry.rs
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub async fn trigger_event(&self, event: &Event) -> Result<()> {
        for plugin in &self.plugins {
            plugin.on_event(event)?;
        }
        Ok(())
    }

    pub fn transform_system(&self, prompt: &mut String) -> Result<()> {
        for plugin in &self.plugins {
            plugin.transform_system(prompt)?;
        }
        Ok(())
    }
}
```

---

## 4. P1 优先级功能 (High)

### 4.1 Event Bus 增强

**OpenCode 实现:**

```typescript
// packages/opencode/src/bus/index.ts
export namespace Bus {
  const subscribers = new Map<string, Set<Callback>>()

  export function subscribe<T>(event: BusEvent<T>, callback: (data: T) => void) {
    if (!subscribers.has(event.type)) {
      subscribers.set(event.type, new Set())
    }
    subscribers.get(event.type)!.add(callback)
    return () => subscribers.get(event.type)?.delete(callback)
  }

  export function publish<T>(event: BusEvent<T>, data: T) {
    subscribers.get(event.type)?.forEach(cb => cb(data))
  }

  export function subscribeAll(callback: (event: any) => void) {
    // 订阅所有事件
  }
}

// 事件定义
export const SessionCreated = BusEvent.define("session.created", z.object({...}))
export const MessageUpdated = BusEvent.define("message.updated", z.object({...}))
export const ToolExecuted = BusEvent.define("tool.executed", z.object({...}))
```

**Codex 实现建议:**

```rust
// core/src/event/bus.rs
pub struct EventBus {
    subscribers: RwLock<HashMap<String, Vec<Box<dyn Fn(&Event) + Send + Sync>>>>,
}

impl EventBus {
    pub fn subscribe<F>(&self, event_type: &str, callback: F)
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.subscribers.write().unwrap()
            .entry(event_type.to_string())
            .or_default()
            .push(Box::new(callback));
    }

    pub fn publish(&self, event: &Event) {
        if let Some(callbacks) = self.subscribers.read().unwrap().get(&event.event_type) {
            for callback in callbacks {
                callback(event);
            }
        }
    }
}
```

### 4.2 Max Steps 限制

**OpenCode 实现:**

```typescript
// packages/opencode/src/agent/agent.ts
export const Agent = z.object({
  maxSteps: z.number().int().positive().optional(),
  // ...
})

// packages/opencode/src/session/prompt.ts
const maxSteps = agent.maxSteps ?? Infinity
const isLastStep = step >= maxSteps

if (isLastStep) {
  // 1. 注入 max-steps.txt 警告
  // 2. 禁用所有工具
  messages.push({
    role: "assistant",
    content: MAX_STEPS_WARNING,
  })
  tools = {}  // 禁用工具
}
```

**Codex 实现建议:**

```rust
// core/src/subagent/definition/mod.rs
pub struct AgentDefinition {
    pub max_steps: Option<u32>,
    // ...
}

// core/src/codex.rs
impl Codex {
    fn check_max_steps(&self, step: u32) -> bool {
        let max = self.agent.max_steps.unwrap_or(u32::MAX);
        step >= max
    }

    async fn run_loop(&mut self) {
        for step in 0.. {
            if self.check_max_steps(step) {
                self.inject_max_steps_warning().await;
                self.disable_tools();
                break;
            }
            // ... normal processing
        }
    }
}
```

### 4.3 Mode Switch Reminder

**OpenCode 实现:**

```typescript
// packages/opencode/src/session/prompt.ts
function insertReminders(input: { messages, agent }) {
  // 检查是否从 plan 切换到 build
  const wasPlan = input.messages.some(
    msg => msg.info.role === "assistant" && msg.info.agent === "plan"
  )

  if (wasPlan && input.agent.name === "build") {
    userMessage.parts.push({
      type: "text",
      text: BUILD_SWITCH_REMINDER,  // 从 build-switch.txt 加载
      synthetic: true,
    })
  }
}
```

**build-switch.txt 内容:**
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
</system-reminder>
```

**Codex 实现建议:**

```rust
// core/src/system_reminder/attachments/mode_switch.rs
pub struct ModeSwitchGenerator;

impl AttachmentGenerator for ModeSwitchGenerator {
    fn generate(&self, context: &AttachmentContext) -> Option<Attachment> {
        let was_plan = context.messages.iter().any(|m| {
            m.role == Role::Assistant && m.agent == Some("plan")
        });

        if was_plan && context.current_agent == "build" {
            Some(Attachment::SystemReminder(MODE_SWITCH_REMINDER.into()))
        } else {
            None
        }
    }
}
```

### 4.4 Additional Built-in Agents

**OpenCode 额外 Agent:**

| Agent | 职责 | 实现要点 |
|-------|------|----------|
| title | 生成会话标题 | 使用小模型, 简短输出 |
| summary | 生成摘要 | 用于 compaction |
| general | 通用研究 | 更宽泛的工具访问 |
| compaction | 压缩上下文 | 专用压缩提示词 |

**Codex 实现建议:**

```yaml
# core/src/subagent/definition/builtin/title.yaml
name: title
description: Generate concise session titles
model_config:
  model: gpt-4o-mini
tools: []  # 无工具访问
prompt: |
  Generate a concise title (max 50 chars) for this conversation.
  Focus on the main topic or task discussed.
```

---

## 5. P2-P3 功能列表

### 5.1 P2 - Medium Priority

#### 5.1.1 Config 变量替换

```typescript
// OpenCode 实现
text = text.replace(/\{env:([^}]+)\}/g, (_, varName) => {
  return process.env[varName] || ""
})

// 文件引用
text = text.replace(/\{file:([^}]+)\}/g, (_, path) => {
  return fs.readFileSync(path, 'utf8')
})
```

**使用示例:**
```json
{
  "provider": {
    "anthropic": {
      "options": {
        "apiKey": "{env:ANTHROPIC_API_KEY}"
      }
    }
  },
  "agent": {
    "custom": {
      "prompt": "{file:./prompts/custom.txt}"
    }
  }
}
```

#### 5.1.2 Provider 专属提示词

```typescript
// OpenCode: packages/opencode/src/session/system.ts
export function provider(model: string): string {
  if (model.includes("claude")) {
    return ANTHROPIC_PROMPT  // Claude 专用提示
  }
  if (model.includes("gpt")) {
    return GPT_PROMPT  // GPT 专用提示
  }
  if (model.includes("gemini")) {
    return GEMINI_PROMPT  // Gemini 专用提示
  }
  return DEFAULT_PROMPT
}
```

#### 5.1.3 工具重试逻辑

```typescript
// OpenCode 工具执行带重试
async function executeWithRetry(tool, args, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await tool.execute(args)
    } catch (e) {
      if (i === maxRetries - 1) throw e
      await sleep(1000 * (i + 1))  // 指数退避
    }
  }
}
```

### 5.2 P3 - Low Priority

#### 5.2.1 Agent 颜色/主题

```typescript
// OpenCode Agent 定义
export const Agent = z.object({
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/).optional(),
  // 用于 TUI 中区分不同 Agent
})
```

#### 5.2.2 Markdown 配置支持

```
.opencode/
├── agent/
│   ├── code-reviewer.md
│   └── security-auditor.md
└── command/
    ├── deploy.md
    └── test.md
```

#### 5.2.3 扩展快捷键

```typescript
// OpenCode 70+ 快捷键配置
export const Keybinds = z.object({
  leader: z.string().default("ctrl+x"),
  session_new: z.string().default("<leader>n"),
  model_list: z.string().default("<leader>m"),
  agent_cycle: z.string().default("tab"),
  // ... 70+ 更多
})
```

---

## 6. 实现路线图

### Phase 1: Core Safety & Extensibility (P0) - 2-3 周

```
Week 1-2:
├── Doom Loop Detection
│   ├── DoomLoopDetector struct
│   ├── 集成到 codex.rs conversation loop
│   └── 添加 DoomLoopGenerator 到 system_reminder
│
└── Adapter Expansion
    ├── AnthropicAdapter
    ├── OpenAIAdapter
    └── AzureAdapter

Week 3:
└── Plugin System Foundation
    ├── Plugin trait 定义
    ├── PluginRegistry
    └── 基础钩子点集成
```

### Phase 2: Agent Enhancement (P1) - 2 周

```
Week 4:
├── Event Bus
│   ├── EventBus struct
│   └── 关键事件定义 (Session, Message, Tool)
│
└── Max Steps Limit
    ├── AgentDefinition.max_steps
    └── MAX_STEPS_WARNING attachment

Week 5:
├── Mode Switch Reminder
│   └── ModeSwitchGenerator
│
└── Built-in Agents
    ├── title agent
    ├── summary agent
    └── general agent
```

### Phase 3: Configuration & Polish (P2-P3) - 1-2 周

```
Week 6:
├── Config Variable Substitution
├── Provider-specific Prompts
└── Tool Retry Logic

Week 7 (Optional):
├── Agent Color/Theming
├── Markdown Config
└── Extended Keybinds
```

---

## 7. 代码示例

### 7.1 Doom Loop 完整实现

```rust
// core/src/doom_loop.rs
use std::collections::VecDeque;
use serde_json::Value;

pub struct DoomLoopDetector {
    history: VecDeque<String>,
    threshold: usize,
    warned: bool,
}

impl DoomLoopDetector {
    pub fn new(threshold: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(threshold + 1),
            threshold,
            warned: false,
        }
    }

    pub fn record(&mut self, tool_name: &str, args: &Value) -> DoomLoopResult {
        let signature = format!("{}:{}", tool_name, serde_json::to_string(args).unwrap_or_default());
        self.history.push_back(signature.clone());

        if self.history.len() > self.threshold {
            self.history.pop_front();
        }

        if self.history.len() >= self.threshold && self.history.iter().all(|s| s == &signature) {
            if self.warned {
                DoomLoopResult::Deny
            } else {
                self.warned = true;
                DoomLoopResult::Warn
            }
        } else {
            self.warned = false;
            DoomLoopResult::Ok
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.warned = false;
    }
}

pub enum DoomLoopResult {
    Ok,
    Warn,  // 首次检测到，注入警告
    Deny,  // 重复检测，拒绝执行
}
```

### 7.2 Plugin Trait 完整定义

```rust
// core/src/plugin/mod.rs
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// 插件唯一标识
    fn name(&self) -> &str;

    /// 插件初始化
    async fn on_init(&self, config: &Config) -> Result<(), PluginError> {
        Ok(())
    }

    /// 插件关闭
    async fn on_shutdown(&self) -> Result<(), PluginError> {
        Ok(())
    }

    /// 配置变更
    async fn on_config_change(&self, config: &Config) -> Result<(), PluginError> {
        Ok(())
    }

    /// 事件通知
    fn on_event(&self, event: &Event) -> Result<(), PluginError> {
        Ok(())
    }

    /// 系统提示词转换
    fn transform_system_prompt(&self, prompt: &mut String) -> Result<(), PluginError> {
        Ok(())
    }

    /// 消息转换
    fn transform_message(&self, message: &mut Message) -> Result<(), PluginError> {
        Ok(())
    }

    /// 工具执行前
    fn before_tool_call(&self, name: &str, args: &Value) -> Result<(), PluginError> {
        Ok(())
    }

    /// 工具执行后
    fn after_tool_call(&self, name: &str, result: &Value) -> Result<(), PluginError> {
        Ok(())
    }

    /// 认证提供
    async fn authenticate(&self, provider: &str) -> Option<Credentials> {
        None
    }
}

#[derive(Debug)]
pub enum PluginError {
    InitFailed(String),
    HookFailed(String),
    ConfigError(String),
}
```

---

## 8. 参考文件索引

### OpenCode 关键文件

| 功能 | 路径 | 行数 |
|------|------|------|
| Doom Loop | `session/processor.ts` | ~50 |
| Provider | `provider/provider.ts` | 1057 |
| Plugin | `plugin/index.ts` | 92 |
| Event Bus | `bus/index.ts` | ~200 |
| Agent | `agent/agent.ts` | 399 |
| Max Steps | `session/prompt.ts` | 474-549 |
| Mode Switch | `session/prompt.ts` | 1004-1030 |
| Config | `config/config.ts` | 1021 |

### Codex 待修改文件

| 功能 | 路径 | 类型 |
|------|------|------|
| Doom Loop | `core/src/codex.rs` | 修改 |
| Doom Loop | `core/src/doom_loop.rs` | 新建 |
| Adapters | `codex-api/src/adapters/*.rs` | 新建 |
| Plugin | `core/src/plugin/mod.rs` | 新建 |
| Event Bus | `core/src/event/bus.rs` | 新建 |
| Max Steps | `core/src/subagent/definition/mod.rs` | 修改 |
| Mode Switch | `core/src/system_reminder/attachments/mode_switch.rs` | 新建 |

---

*文档生成时间: 2025-12-28*
*基于 OpenCode 源码分析与 Codex 对比*
