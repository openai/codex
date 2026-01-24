# OpenCode 扩展性设计

本文档详细分析 OpenCode 的扩展性和灵活性设计，包括插件系统、Provider 适配器、Skill 系统和 MCP 集成。

---

## 目录

1. [扩展性概览](#1-扩展性概览)
2. [插件系统](#2-插件系统)
3. [Provider 适配器](#3-provider-适配器)
4. [Skill 系统](#4-skill-系统)
5. [MCP 集成](#5-mcp-集成)
6. [自定义工具](#6-自定义工具)
7. [与 codex 对比](#7-与-codex-对比)

---

## 1. 扩展性概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       扩展性架构                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Plugin System                              │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │  │
│  │  │ Auth Hooks  │ │ Tool Hooks  │ │ System Transform Hooks │ │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│  ┌──────────────────────────┼──────────────────────────────────┐  │
│  │          Provider Adapters (30+ bundled)                     │  │
│  │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────────┐│  │
│  │  │ Anthropic │ │  OpenAI   │ │  Gemini   │ │ OpenRouter   ││  │
│  │  │   Azure   │ │  Bedrock  │ │  Vertex   │ │ Copilot      ││  │
│  │  └───────────┘ └───────────┘ └───────────┘ └───────────────┘│  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│  ┌──────────────────────────┼──────────────────────────────────┐  │
│  │           Skill System (SKILL.md discovery)                  │  │
│  │  ┌─────────────────────────────────────────────────────────┐│  │
│  │  │  skill/*/SKILL.md → name, description, location        ││  │
│  │  └─────────────────────────────────────────────────────────┘│  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│  ┌──────────────────────────┼──────────────────────────────────┐  │
│  │           MCP Integration (local + remote)                   │  │
│  │  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐  │  │
│  │  │ StdioTransport │ │ SSETransport   │ │ HTTPTransport  │  │  │
│  │  │   (local)      │ │   (remote)     │ │   (remote)     │  │  │
│  │  └────────────────┘ └────────────────┘ └────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心文件

| 文件 | 职责 |
|------|------|
| `src/plugin/index.ts` | 插件加载和钩子触发 |
| `src/provider/provider.ts` | Provider 适配器注册 |
| `src/skill/skill.ts` | Skill 发现和加载 |
| `src/mcp/index.ts` | MCP 客户端管理 |

---

## 2. 插件系统

### 2.1 插件接口定义

**文件:** `packages/opencode/src/plugin/index.ts`

```typescript
import type { Hooks, PluginInput, Plugin as PluginInstance } from "@opencode-ai/plugin"

interface PluginInput {
  client: OpencodeClient     // SDK 客户端
  project: string            // 项目路径
  worktree: string           // 工作树路径
  directory: string          // 当前目录
  $: BunShell                // Bun shell
}

interface Hooks {
  auth?: AuthHook           // 认证钩子
  event?: EventHook         // 事件钩子
  tool?: ToolHook           // 工具钩子
  config?: ConfigHook       // 配置钩子
  // ... 其他钩子
}
```

### 2.2 插件加载流程

```
┌─────────────────────────────────────────────────────────────────┐
│                    插件加载流程                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. 读取配置                                                     │
│     config.plugin = ["plugin1@0.0.1", "file://local/plugin"]    │
│                              │                                   │
│  2. 默认插件 (可禁用)                                            │
│     ├─ opencode-copilot-auth@0.0.9                              │
│     └─ opencode-anthropic-auth@0.0.5                            │
│                              │                                   │
│  3. 加载插件                 ▼                                   │
│     ┌─────────────────────────────────────────────────────────┐ │
│     │  for (let plugin of plugins) {                          │ │
│     │    if (!plugin.startsWith("file://")) {                 │ │
│     │      plugin = await BunProc.install(pkg, version)       │ │
│     │    }                                                    │ │
│     │    const mod = await import(plugin)                     │ │
│     │    for (const fn of Object.values(mod)) {               │ │
│     │      const init = await fn(input)                       │ │
│     │      hooks.push(init)                                   │ │
│     │    }                                                    │ │
│     │  }                                                      │ │
│     └─────────────────────────────────────────────────────────┘ │
│                              │                                   │
│  4. 初始化钩子               ▼                                   │
│     hook.config?.(config)                                        │
│                              │                                   │
│  5. 事件订阅                 ▼                                   │
│     Bus.subscribeAll(event => hook.event?.({event}))            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 钩子触发机制

**文件:** `packages/opencode/src/plugin/index.ts:55-70`

```typescript
export async function trigger<
  Name extends Exclude<keyof Required<Hooks>, "auth" | "event" | "tool">,
  Input = Parameters<Required<Hooks>[Name]>[0],
  Output = Parameters<Required<Hooks>[Name]>[1],
>(name: Name, input: Input, output: Output): Promise<Output> {
  if (!name) return output
  for (const hook of await state().then((x) => x.hooks)) {
    const fn = hook[name]
    if (!fn) continue
    await fn(input, output)  // 链式调用
  }
  return output
}
```

### 2.4 内置插件

| 插件 | 版本 | 职责 |
|------|------|------|
| `opencode-copilot-auth` | 0.0.9 | GitHub Copilot 认证 |
| `opencode-anthropic-auth` | 0.0.5 | Anthropic API 认证 |

### 2.5 插件目录结构

```
.opencode/
└── plugin/
    ├── my-plugin.ts      # 自定义插件
    └── another-plugin.js # JavaScript 插件
```

---

## 3. Provider 适配器

### 3.1 内置 Provider (30+)

**文件:** `packages/opencode/src/provider/provider.ts:41-62`

```typescript
const BUNDLED_PROVIDERS: Record<string, (options: any) => SDK> = {
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

### 3.2 自定义 Loader

```typescript
const CUSTOM_LOADERS: Record<string, CustomLoader> = {
  // Anthropic 特殊头部
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

  // OpenAI Responses API
  openai: async () => {
    return {
      autoload: false,
      async getModel(sdk, modelID) {
        return sdk.responses(modelID)  // 使用 responses API
      },
    }
  },

  // AWS Bedrock 区域前缀
  "amazon-bedrock": async () => {
    const region = awsRegion ?? "us-east-1"
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

  // Google Vertex
  "google-vertex": async () => {
    const project = Env.get("GOOGLE_CLOUD_PROJECT")
    const location = Env.get("VERTEX_LOCATION") ?? "us-east5"
    return {
      autoload: Boolean(project),
      options: { project, location },
    }
  },
}
```

### 3.3 Provider 加载优先级

```
┌─────────────────────────────────────────────────────────────────┐
│                    Provider 加载优先级                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. 环境变量 (source: "env")                                     │
│     └─ 检查 provider.env 定义的环境变量                          │
│                                                                  │
│  2. Auth 存储 (source: "api")                                    │
│     └─ 从 Auth.all() 加载 API 密钥                               │
│                                                                  │
│  3. 插件认证 (source: "custom")                                  │
│     └─ 调用 plugin.auth.loader()                                 │
│                                                                  │
│  4. 自定义 Loader (source: "custom")                             │
│     └─ CUSTOM_LOADERS[providerID]                                │
│                                                                  │
│  5. 配置文件 (source: "config")                                  │
│     └─ config.provider[providerID]                               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.4 Model 定义

```typescript
export const Model = z.object({
  id: z.string(),
  providerID: z.string(),
  api: z.object({
    id: z.string(),
    url: z.string(),
    npm: z.string(),
  }),
  name: z.string(),
  family: z.string().optional(),
  capabilities: z.object({
    temperature: z.boolean(),
    reasoning: z.boolean(),
    attachment: z.boolean(),
    toolcall: z.boolean(),
    input: z.object({
      text: z.boolean(),
      audio: z.boolean(),
      image: z.boolean(),
      video: z.boolean(),
      pdf: z.boolean(),
    }),
    output: z.object({
      text: z.boolean(),
      audio: z.boolean(),
      image: z.boolean(),
      video: z.boolean(),
      pdf: z.boolean(),
    }),
    interleaved: z.union([z.boolean(), z.object({
      field: z.enum(["reasoning_content", "reasoning_details"]),
    })]),
  }),
  cost: z.object({
    input: z.number(),
    output: z.number(),
    cache: z.object({
      read: z.number(),
      write: z.number(),
    }),
  }),
  limit: z.object({
    context: z.number(),
    output: z.number(),
  }),
  status: z.enum(["alpha", "beta", "deprecated", "active"]),
})
```

---

## 4. Skill 系统

### 4.1 Skill 发现机制

**文件:** `packages/opencode/src/skill/skill.ts`

```typescript
const SKILL_GLOB = new Bun.Glob("skill/**/SKILL.md")

export const state = Instance.state(async () => {
  const directories = await Config.directories()
  const skills: Record<string, Info> = {}

  for (const dir of directories) {
    for await (const match of SKILL_GLOB.scan({
      cwd: dir,
      absolute: true,
      onlyFiles: true,
      followSymlinks: true,
    })) {
      const md = await ConfigMarkdown.parse(match)
      if (!md) continue

      const parsed = Info.pick({ name: true, description: true }).safeParse(md.data)
      if (!parsed.success) continue

      skills[parsed.data.name] = {
        name: parsed.data.name,
        description: parsed.data.description,
        location: match,
      }
    }
  }
  return skills
})
```

### 4.2 Skill 定义结构

```
.opencode/
└── skill/
    └── my-skill/
        └── SKILL.md
```

**SKILL.md 格式:**

```markdown
---
name: my-skill
description: A custom skill for doing X
---

## Instructions

When this skill is invoked...
```

### 4.3 Skill 信息模式

```typescript
export const Info = z.object({
  name: z.string(),        // Skill 名称
  description: z.string(), // 描述
  location: z.string(),    // 文件路径
})
```

---

## 5. MCP 集成

### 5.1 MCP 服务器类型

**文件:** `packages/opencode/src/mcp/index.ts`

```typescript
// 本地 MCP (Stdio)
export const McpLocal = z.object({
  type: z.literal("local"),
  command: z.string().array(),
  environment: z.record(z.string(), z.string()).optional(),
  enabled: z.boolean().optional(),
  timeout: z.number().optional(),
})

// 远程 MCP (HTTP/SSE)
export const McpRemote = z.object({
  type: z.literal("remote"),
  url: z.string(),
  enabled: z.boolean().optional(),
  headers: z.record(z.string(), z.string()).optional(),
  oauth: z.union([McpOAuth, z.literal(false)]).optional(),
  timeout: z.number().optional(),
})
```

### 5.2 MCP 连接流程

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCP 连接流程                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                      Local MCP                             │ │
│  │                                                            │ │
│  │  1. StdioClientTransport({                                 │ │
│  │       command: cmd,                                        │ │
│  │       args,                                                │ │
│  │       env: { ...process.env, ...mcp.environment }         │ │
│  │     })                                                     │ │
│  │  2. client.connect(transport)                              │ │
│  │  3. registerNotificationHandlers(client, key)              │ │
│  │  4. client.listTools()                                     │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                      Remote MCP                            │ │
│  │                                                            │ │
│  │  1. 尝试 StreamableHTTPClientTransport                     │ │
│  │  2. 失败则尝试 SSEClientTransport                          │ │
│  │  3. OAuth 认证 (如果需要)                                   │ │
│  │     ├─ McpOAuthProvider                                    │ │
│  │     ├─ UnauthorizedError → startAuth()                     │ │
│  │     └─ finishAuth(authorizationCode)                       │ │
│  │  4. client.connect(transport)                              │ │
│  │  5. client.listTools()                                     │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.3 MCP 状态管理

```typescript
export const Status = z.discriminatedUnion("status", [
  z.object({ status: z.literal("connected") }),
  z.object({ status: z.literal("disabled") }),
  z.object({ status: z.literal("failed"), error: z.string() }),
  z.object({ status: z.literal("needs_auth") }),
  z.object({ status: z.literal("needs_client_registration"), error: z.string() }),
])
```

### 5.4 MCP 工具转换

**文件:** `packages/opencode/src/mcp/index.ts:96-117`

```typescript
function convertMcpTool(mcpTool: MCPToolDef, client: MCPClient): Tool {
  const schema: JSONSchema7 = {
    ...(mcpTool.inputSchema as JSONSchema7),
    type: "object",
    properties: mcpTool.inputSchema.properties ?? {},
    additionalProperties: false,
  }

  return dynamicTool({
    description: mcpTool.description ?? "",
    inputSchema: jsonSchema(schema),
    execute: async (args: unknown) => {
      return client.callTool({
        name: mcpTool.name,
        arguments: args as Record<string, unknown>,
      })
    },
  })
}
```

### 5.5 MCP OAuth 流程

```
用户请求 → 检测需要认证 → startAuth()
                │
                ▼
         生成 OAuth state
                │
                ▼
         打开浏览器授权
                │
                ▼
         McpOAuthCallback 等待
                │
                ▼
         finishAuth(code)
                │
                ▼
         存储 tokens → 重新连接
```

---

## 6. 自定义工具

### 6.1 工具加载位置

```
.opencode/
├── command/           # 命令定义 (*.md)
├── agent/             # 代理定义 (*.md)
├── plugin/            # 插件 (*.ts, *.js)
└── skill/             # Skill 定义 (SKILL.md)
```

### 6.2 配置中禁用/启用工具

```json
{
  "tools": {
    "bash": true,
    "edit": true,
    "webfetch": false
  },
  "agent": {
    "plan": {
      "tools": {
        "edit": false,
        "write": false
      }
    }
  }
}
```

---

## 7. 与 codex 对比

### 7.1 扩展性对比

| 方面 | opencode | codex |
|------|----------|-------|
| **插件系统** | 完整的钩子系统 | 有限 |
| **Provider 数量** | 30+ 内置 | OpenAI 为主 |
| **Skill 系统** | SKILL.md 发现 | 无 |
| **MCP 集成** | 完整 OAuth | 基础支持 |
| **自定义加载** | 动态 npm 安装 | 静态编译 |

### 7.2 codex 借鉴建议

1. **插件钩子系统**

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn on_config(&self, config: &Config) -> Result<()>;
    fn on_event(&self, event: &Event) -> Result<()>;
    fn transform_system(&self, prompt: &mut String) -> Result<()>;
}

pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub async fn trigger<T>(&self, name: &str, input: T) -> Result<T> {
        for plugin in &self.plugins {
            plugin.on_event(...)?;
        }
        Ok(input)
    }
}
```

2. **Provider 注册模式**

```rust
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn autoload(&self, config: &Config) -> bool;
    fn get_model(&self, model_id: &str) -> Result<Box<dyn Model>>;
    fn options(&self) -> ProviderOptions;
}

// 注册
registry.register(Box::new(AnthropicAdapter::new()));
registry.register(Box::new(OpenAIAdapter::new()));
```

3. **Skill 发现**

```rust
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub location: PathBuf,
}

pub async fn discover_skills(directories: &[PathBuf]) -> Result<HashMap<String, SkillInfo>> {
    let mut skills = HashMap::new();
    for dir in directories {
        for entry in glob::glob(&format!("{}/skill/**/SKILL.md", dir.display()))? {
            let md = parse_markdown(&entry?)?;
            if let Some(info) = parse_skill_info(&md) {
                skills.insert(info.name.clone(), info);
            }
        }
    }
    Ok(skills)
}
```

### 7.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/plugin/index.ts` | - (待实现) |
| `src/provider/provider.ts` | `core/src/adapters/registry.rs` |
| `src/skill/skill.ts` | - (待实现) |
| `src/mcp/index.ts` | `mcp-server/src/*.rs` |

---

## 8. 配置示例

### 8.1 完整配置示例

```json
{
  "$schema": "https://opencode.ai/config.json",

  "plugin": [
    "my-custom-plugin@1.0.0",
    "file://.opencode/plugin/local-plugin.ts"
  ],

  "provider": {
    "anthropic": {
      "options": {
        "apiKey": "{env:ANTHROPIC_API_KEY}"
      }
    },
    "custom-provider": {
      "name": "My Custom Provider",
      "api": "https://api.custom.com/v1",
      "npm": "@ai-sdk/openai-compatible",
      "models": {
        "custom-model": {
          "name": "Custom Model",
          "limit": {
            "context": 128000,
            "output": 4096
          }
        }
      }
    }
  },

  "mcp": {
    "local-server": {
      "type": "local",
      "command": ["node", "mcp-server.js"],
      "environment": {
        "DEBUG": "true"
      }
    },
    "remote-server": {
      "type": "remote",
      "url": "https://mcp.example.com",
      "oauth": {
        "clientId": "my-client-id"
      }
    }
  },

  "tools": {
    "bash": true,
    "webfetch": true
  }
}
```

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
