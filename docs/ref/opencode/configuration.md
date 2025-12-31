# OpenCode 配置系统

本文档详细分析 OpenCode 的配置系统设计，包括配置加载层级、Schema 定义、代理配置和实验性功能。

---

## 目录

1. [配置概览](#1-配置概览)
2. [配置加载层级](#2-配置加载层级)
3. [Schema 定义](#3-schema-定义)
4. [代理配置](#4-代理配置)
5. [Provider 配置](#5-provider-配置)
6. [快捷键配置](#6-快捷键配置)
7. [实验性功能](#7-实验性功能)
8. [与 codex 对比](#8-与-codex-对比)

---

## 1. 配置概览

### 1.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                       配置系统架构                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    配置加载优先级 (低→高)                     │  │
│  │                                                               │  │
│  │  1. ~/.config/opencode/opencode.json (全局)                  │  │
│  │  2. .opencode/opencode.json (项目层级)                       │  │
│  │  3. opencode.json (工作目录)                                 │  │
│  │  4. OPENCODE_CONFIG 环境变量                                 │  │
│  │  5. OPENCODE_CONFIG_CONTENT 环境变量                         │  │
│  │                                                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    配置目录扫描                               │  │
│  │                                                               │  │
│  │  directories = [                                              │  │
│  │    Global.Path.config,           // ~/.config/opencode        │  │
│  │    .opencode (向上查找),         // 项目 .opencode 目录       │  │
│  │    ~/.opencode,                   // 用户主目录                │  │
│  │    OPENCODE_CONFIG_DIR            // 环境变量指定              │  │
│  │  ]                                                            │  │
│  │                                                               │  │
│  │  每个目录扫描:                                                │  │
│  │    ├─ command/*.md               // 命令定义                  │  │
│  │    ├─ agent/*.md                 // 代理定义                  │  │
│  │    ├─ mode/*.md                  // 模式定义 (已废弃)         │  │
│  │    ├─ plugin/*.{ts,js}           // 插件文件                  │  │
│  │    └─ skill/**/SKILL.md          // Skill 定义                │  │
│  │                                                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Config.Info (最终合并)                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 核心文件

| 文件 | 职责 |
|------|------|
| `src/config/config.ts` | 配置加载和解析 |
| `src/config/markdown.ts` | Markdown 配置解析 |
| `src/global.ts` | 全局路径定义 |

---

## 2. 配置加载层级

### 2.1 加载流程

**文件:** `packages/opencode/src/config/config.ts:36-156`

```typescript
export const state = Instance.state(async () => {
  const auth = await Auth.all()
  let result = await global()  // 加载全局配置

  // 1. 环境变量覆盖
  if (Flag.OPENCODE_CONFIG) {
    result = mergeConfigWithPlugins(result, await loadFile(Flag.OPENCODE_CONFIG))
  }

  // 2. 项目配置 (向上查找)
  for (const file of ["opencode.jsonc", "opencode.json"]) {
    const found = await Filesystem.findUp(file, Instance.directory, Instance.worktree)
    for (const resolved of found.toReversed()) {
      result = mergeConfigWithPlugins(result, await loadFile(resolved))
    }
  }

  // 3. 内容环境变量
  if (Flag.OPENCODE_CONFIG_CONTENT) {
    result = mergeConfigWithPlugins(result, JSON.parse(Flag.OPENCODE_CONFIG_CONTENT))
  }

  // 4. Well-known 配置
  for (const [key, value] of Object.entries(auth)) {
    if (value.type === "wellknown") {
      const wellknown = await fetch(`${key}/.well-known/opencode`)
        .then(x => x.json())
      result = mergeConfigWithPlugins(result, wellknown.config ?? {})
    }
  }

  // 5. 目录配置扫描
  const directories = [
    Global.Path.config,
    ...Filesystem.up({ targets: [".opencode"], start: Instance.directory }),
    ...Filesystem.up({ targets: [".opencode"], start: Global.Path.home }),
  ]

  for (const dir of unique(directories)) {
    result.command = mergeDeep(result.command ?? {}, await loadCommand(dir))
    result.agent = mergeDeep(result.agent, await loadAgent(dir))
    result.plugin.push(...(await loadPlugin(dir)))
  }

  return { config: result, directories }
})
```

### 2.2 配置文件格式

支持两种格式:
- `opencode.json` - 标准 JSON
- `opencode.jsonc` - JSON with Comments

### 2.3 变量替换

```typescript
// 环境变量替换
text = text.replace(/\{env:([^}]+)\}/g, (_, varName) => {
  return process.env[varName] || ""
})

// 文件引用替换
// {file:./path/to/file.txt} → 文件内容
const fileMatches = text.match(/\{file:[^}]+\}/g)
```

**示例:**

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

---

## 3. Schema 定义

### 3.1 顶层配置 Schema

**文件:** `packages/opencode/src/config/config.ts:650-851`

```typescript
export const Info = z.object({
  $schema: z.string().optional(),
  theme: z.string().optional(),
  keybinds: Keybinds.optional(),
  logLevel: Log.Level.optional(),
  tui: TUI.optional(),
  server: Server.optional(),
  command: z.record(z.string(), Command).optional(),
  watcher: z.object({
    ignore: z.array(z.string()).optional(),
  }).optional(),
  plugin: z.string().array().optional(),
  snapshot: z.boolean().optional(),
  share: z.enum(["manual", "auto", "disabled"]).optional(),
  autoupdate: z.union([z.boolean(), z.literal("notify")]).optional(),
  disabled_providers: z.array(z.string()).optional(),
  enabled_providers: z.array(z.string()).optional(),
  model: z.string().optional(),
  small_model: z.string().optional(),
  default_agent: z.string().optional(),
  username: z.string().optional(),
  agent: z.record(z.string(), Agent).optional(),
  provider: z.record(z.string(), Provider).optional(),
  mcp: z.record(z.string(), Mcp).optional(),
  formatter: z.union([z.literal(false), z.record(...)]).optional(),
  lsp: z.union([z.literal(false), z.record(...)]).optional(),
  instructions: z.array(z.string()).optional(),
  permission: z.object({
    edit: Permission.optional(),
    bash: z.union([Permission, z.record(...)]).optional(),
    skill: z.union([Permission, z.record(...)]).optional(),
    webfetch: Permission.optional(),
    doom_loop: Permission.optional(),
    external_directory: Permission.optional(),
  }).optional(),
  tools: z.record(z.string(), z.boolean()).optional(),
  enterprise: z.object({
    url: z.string().optional(),
  }).optional(),
  compaction: z.object({
    auto: z.boolean().optional(),
    prune: z.boolean().optional(),
  }).optional(),
  experimental: z.object({...}).optional(),
})
```

### 3.2 权限配置

```typescript
export const Permission = z.enum(["ask", "allow", "deny"])

// 使用示例
{
  "permission": {
    "edit": "ask",           // 编辑文件需要询问
    "bash": "allow",         // 允许所有 bash 命令
    "skill": {
      "commit": "allow",     // 允许 commit skill
      "deploy": "deny"       // 禁止 deploy skill
    },
    "webfetch": "ask",
    "doom_loop": "deny",
    "external_directory": "ask"
  }
}
```

---

## 4. 代理配置

### 4.1 Agent Schema

**文件:** `packages/opencode/src/config/config.ts:400-436`

```typescript
export const Agent = z.object({
  model: z.string().optional(),
  temperature: z.number().optional(),
  top_p: z.number().optional(),
  prompt: z.string().optional(),
  tools: z.record(z.string(), z.boolean()).optional(),
  disable: z.boolean().optional(),
  description: z.string().optional(),
  mode: z.enum(["subagent", "primary", "all"]).optional(),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/).optional(),
  maxSteps: z.number().int().positive().optional(),
  permission: z.object({
    edit: Permission.optional(),
    bash: z.union([Permission, z.record(...)]).optional(),
    skill: z.union([Permission, z.record(...)]).optional(),
    webfetch: Permission.optional(),
    doom_loop: Permission.optional(),
    external_directory: Permission.optional(),
  }).optional(),
}).catchall(z.any())
```

### 4.2 内置代理配置

```json
{
  "agent": {
    "build": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "temperature": 0.7
    },
    "plan": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "tools": {
        "edit": false,
        "write": false,
        "bash": false
      }
    },
    "explore": {
      "mode": "subagent",
      "maxSteps": 10
    },
    "general": {
      "mode": "subagent"
    },
    "title": {
      "model": "anthropic/claude-haiku-3.5"
    },
    "summary": {
      "model": "anthropic/claude-haiku-3.5"
    },
    "compaction": {
      "model": "anthropic/claude-haiku-3.5"
    }
  }
}
```

### 4.3 Markdown 代理定义

**目录结构:**

```
.opencode/
└── agent/
    ├── code-reviewer.md
    └── nested/
        └── special-agent.md
```

**agent/code-reviewer.md:**

```markdown
---
mode: subagent
description: Reviews code changes and suggests improvements
maxSteps: 5
tools:
  read: true
  grep: true
  glob: true
  edit: false
---

You are a code reviewer. Analyze the provided code and suggest improvements.

Focus on:
- Code quality
- Best practices
- Potential bugs
- Performance issues
```

---

## 5. Provider 配置

### 5.1 Provider Schema

```typescript
export const Provider = ModelsDev.Provider.partial().extend({
  whitelist: z.array(z.string()).optional(),
  blacklist: z.array(z.string()).optional(),
  models: z.record(z.string(), ModelsDev.Model.partial()).optional(),
  options: z.object({
    apiKey: z.string().optional(),
    baseURL: z.string().optional(),
    enterpriseUrl: z.string().optional(),
    setCacheKey: z.boolean().optional(),
    timeout: z.union([
      z.number().int().positive(),
      z.literal(false),
    ]).optional(),
  }).catchall(z.any()).optional(),
})
```

### 5.2 Provider 配置示例

```json
{
  "provider": {
    "anthropic": {
      "options": {
        "apiKey": "{env:ANTHROPIC_API_KEY}"
      }
    },
    "openai": {
      "options": {
        "apiKey": "{env:OPENAI_API_KEY}",
        "timeout": 60000
      }
    },
    "custom-provider": {
      "name": "My Custom Provider",
      "api": "https://api.custom.com/v1",
      "npm": "@ai-sdk/openai-compatible",
      "env": ["CUSTOM_API_KEY"],
      "models": {
        "custom-model-v1": {
          "name": "Custom Model v1",
          "limit": {
            "context": 128000,
            "output": 4096
          },
          "cost": {
            "input": 0.001,
            "output": 0.002
          }
        }
      },
      "whitelist": ["custom-model-v1"],
      "blacklist": ["deprecated-model"]
    }
  },
  "disabled_providers": ["azure"],
  "enabled_providers": ["anthropic", "openai", "custom-provider"]
}
```

---

## 6. 快捷键配置

### 6.1 Keybinds Schema

**文件:** `packages/opencode/src/config/config.ts:438-582`

```typescript
export const Keybinds = z.object({
  leader: z.string().optional().default("ctrl+x"),

  // 应用操作
  app_exit: z.string().optional().default("ctrl+c,ctrl+d,<leader>q"),
  editor_open: z.string().optional().default("<leader>e"),
  theme_list: z.string().optional().default("<leader>t"),
  sidebar_toggle: z.string().optional().default("<leader>b"),

  // 会话操作
  session_new: z.string().optional().default("<leader>n"),
  session_list: z.string().optional().default("<leader>l"),
  session_timeline: z.string().optional().default("<leader>g"),
  session_interrupt: z.string().optional().default("escape"),
  session_compact: z.string().optional().default("<leader>c"),

  // 消息导航
  messages_page_up: z.string().optional().default("pageup"),
  messages_page_down: z.string().optional().default("pagedown"),
  messages_first: z.string().optional().default("ctrl+g,home"),
  messages_last: z.string().optional().default("ctrl+alt+g,end"),
  messages_copy: z.string().optional().default("<leader>y"),
  messages_undo: z.string().optional().default("<leader>u"),
  messages_redo: z.string().optional().default("<leader>r"),

  // 模型操作
  model_list: z.string().optional().default("<leader>m"),
  model_cycle_recent: z.string().optional().default("f2"),

  // 代理操作
  agent_list: z.string().optional().default("<leader>a"),
  agent_cycle: z.string().optional().default("tab"),
  agent_cycle_reverse: z.string().optional().default("shift+tab"),

  // 输入操作
  input_submit: z.string().optional().default("return"),
  input_newline: z.string().optional().default("shift+return,ctrl+return"),
  input_clear: z.string().optional().default("ctrl+c"),
  input_paste: z.string().optional().default("ctrl+v"),
  // ... 更多输入操作
})
```

### 6.2 快捷键配置示例

```json
{
  "keybinds": {
    "leader": "ctrl+space",
    "session_new": "<leader>n",
    "session_list": "<leader>l",
    "model_list": "<leader>m",
    "input_submit": "ctrl+return",
    "input_newline": "return"
  }
}
```

### 6.3 快捷键语法

| 语法 | 描述 |
|------|------|
| `ctrl+x` | Ctrl + X |
| `alt+x` | Alt + X |
| `shift+x` | Shift + X |
| `super+x` | Command/Windows + X |
| `<leader>x` | Leader 键 + X |
| `key1,key2` | 多个快捷键 |
| `none` | 禁用快捷键 |

---

## 7. 实验性功能

### 7.1 Experimental Schema

```typescript
experimental: z.object({
  hook: z.object({
    file_edited: z.record(z.string(), z.object({
      command: z.string().array(),
      environment: z.record(z.string(), z.string()).optional(),
    }).array()).optional(),
    session_completed: z.object({
      command: z.string().array(),
      environment: z.record(z.string(), z.string()).optional(),
    }).array().optional(),
  }).optional(),
  chatMaxRetries: z.number().optional(),
  disable_paste_summary: z.boolean().optional(),
  batch_tool: z.boolean().optional(),
  openTelemetry: z.boolean().optional(),
  primary_tools: z.array(z.string()).optional(),
  continue_loop_on_deny: z.boolean().optional(),
}).optional()
```

### 7.2 实验性功能示例

```json
{
  "experimental": {
    "hook": {
      "file_edited": {
        "*.ts": [
          {
            "command": ["prettier", "--write"],
            "environment": {}
          }
        ],
        "*.rs": [
          {
            "command": ["rustfmt"],
            "environment": {}
          }
        ]
      },
      "session_completed": [
        {
          "command": ["notify-send", "Session completed!"],
          "environment": {}
        }
      ]
    },
    "chatMaxRetries": 3,
    "batch_tool": true,
    "openTelemetry": true,
    "primary_tools": ["edit", "write", "bash"],
    "continue_loop_on_deny": false
  }
}
```

---

## 8. 与 codex 对比

### 8.1 配置系统对比

| 方面 | opencode | codex |
|------|----------|-------|
| **配置格式** | JSON/JSONC | TOML/JSON |
| **Schema 验证** | Zod | Serde |
| **配置层级** | 多层合并 | 单层 |
| **变量替换** | 环境变量/文件 | 环境变量 |
| **Markdown 定义** | agent/*.md | - |
| **快捷键数量** | 70+ | 有限 |
| **实验性功能** | 完整钩子系统 | 有限 |

### 8.2 codex 借鉴建议

1. **多层配置合并**

```rust
pub struct ConfigLoader {
    global_path: PathBuf,
    project_paths: Vec<PathBuf>,
}

impl ConfigLoader {
    pub async fn load(&self) -> Result<Config> {
        let mut result = Config::default();

        // 1. 全局配置
        result.merge(self.load_file(&self.global_path).await?);

        // 2. 项目配置 (向上查找)
        for path in &self.project_paths {
            result.merge(self.load_file(path).await?);
        }

        // 3. 环境变量覆盖
        result.merge(self.load_from_env()?);

        Ok(result)
    }
}
```

2. **变量替换**

```rust
fn replace_variables(text: &str) -> Result<String> {
    let re_env = Regex::new(r"\{env:([^}]+)\}")?;
    let re_file = Regex::new(r"\{file:([^}]+)\}")?;

    let text = re_env.replace_all(text, |caps: &Captures| {
        std::env::var(&caps[1]).unwrap_or_default()
    });

    let text = re_file.replace_all(&text, |caps: &Captures| {
        std::fs::read_to_string(&caps[1]).unwrap_or_default()
    });

    Ok(text.to_string())
}
```

3. **Markdown 代理定义**

```rust
pub async fn load_agents_from_dir(dir: &Path) -> Result<HashMap<String, AgentConfig>> {
    let mut agents = HashMap::new();

    for entry in glob::glob(&format!("{}/agent/**/*.md", dir.display()))? {
        let path = entry?;
        let md = parse_markdown(&path)?;

        let name = extract_agent_name(&path);
        let config = AgentConfig {
            name: name.clone(),
            prompt: md.content,
            mode: md.frontmatter.get("mode").cloned(),
            tools: md.frontmatter.get("tools").cloned(),
            max_steps: md.frontmatter.get("maxSteps").and_then(|v| v.as_i64()),
        };

        agents.insert(name, config);
    }

    Ok(agents)
}
```

### 8.3 关键文件对照

| opencode 文件 | codex 对应 |
|--------------|-----------|
| `src/config/config.ts` | `core/src/config/mod.rs` |
| `src/config/markdown.ts` | - (待实现) |
| Schema (Zod) | Serde + 手动验证 |

---

## 9. 完整配置示例

```json
{
  "$schema": "https://opencode.ai/config.json",

  "theme": "catppuccin-mocha",
  "logLevel": "info",
  "username": "developer",

  "model": "anthropic/claude-sonnet-4-20250514",
  "small_model": "anthropic/claude-haiku-3.5",
  "default_agent": "build",

  "keybinds": {
    "leader": "ctrl+x",
    "session_new": "<leader>n",
    "model_list": "<leader>m"
  },

  "tui": {
    "scroll_speed": 3,
    "diff_style": "auto"
  },

  "server": {
    "port": 4096,
    "hostname": "localhost"
  },

  "provider": {
    "anthropic": {
      "options": {
        "apiKey": "{env:ANTHROPIC_API_KEY}"
      }
    }
  },

  "agent": {
    "build": {
      "temperature": 0.7
    },
    "plan": {
      "tools": {
        "edit": false,
        "write": false
      }
    }
  },

  "mcp": {
    "filesystem": {
      "type": "local",
      "command": ["npx", "@modelcontextprotocol/server-filesystem"]
    }
  },

  "permission": {
    "edit": "ask",
    "bash": "allow",
    "webfetch": "ask"
  },

  "tools": {
    "bash": true,
    "edit": true,
    "webfetch": true
  },

  "compaction": {
    "auto": true,
    "prune": true
  },

  "experimental": {
    "hook": {
      "file_edited": {
        "*.ts": [{ "command": ["prettier", "--write"] }]
      }
    }
  }
}
```

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
