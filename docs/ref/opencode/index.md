# OpenCode 深度分析文档

本目录包含对 OpenCode 代码库的全面深度分析，供 codex 研发团队参考。

---

## 文档索引

| 文档 | 描述 | 关键组件 |
|------|------|----------|
| [architecture.md](./architecture.md) | 整体架构概览 | 包结构、入口点、核心依赖 |
| [subagent-system.md](./subagent-system.md) | 子代理系统 | Agent、Task Tool、会话隔离 |
| [context-compaction.md](./context-compaction.md) | 上下文压缩 | 溢出检测、修剪、摘要生成 |
| [prompt-system.md](./prompt-system.md) | 提示词系统 | 三层结构、环境注入、自定义指令 |
| [core-tools.md](./core-tools.md) | 核心工具 | 工具定义、注册表、权限过滤 |
| [reminder-system.md](./reminder-system.md) | 提醒系统 | Plan/Build 提醒、合成文本 |
| [interaction-flow.md](./interaction-flow.md) | 交互流程 | 会话循环、消息处理、流式响应 |
| [extensibility.md](./extensibility.md) | 扩展性设计 | 插件、Provider、Skill、MCP |
| [configuration.md](./configuration.md) | 配置系统 | 加载层级、Schema、合并策略 |
| [lsp-design.md](./lsp-design.md) | LSP 集成 | 语言服务器、客户端通信 |
| [codex_opt.md](./codex_opt.md) | **Codex 优化建议** | 优先级排序、实现路线图 |

---

## 快速参考

### 核心技术栈

| 技术 | 用途 | 版本 |
|------|------|------|
| TypeScript | 主要开发语言 | 5.x |
| Bun | 运行时和包管理 | 1.x |
| Hono.js | HTTP 服务器 | 4.x |
| Vercel AI SDK | LLM 集成 | 4.x |
| Zod | Schema 验证 | 3.x |
| vscode-jsonrpc | LSP 通信 | 8.x |

### 包结构

```
opencode/
├── packages/
│   ├── opencode/          # 主 CLI 应用 (19 模块)
│   │   ├── src/agent/     # 代理系统
│   │   ├── src/session/   # 会话管理
│   │   ├── src/tool/      # 工具实现
│   │   ├── src/config/    # 配置系统
│   │   ├── src/plugin/    # 插件系统
│   │   ├── src/lsp/       # LSP 集成
│   │   ├── src/provider/  # LLM Provider
│   │   ├── src/mcp/       # MCP 集成
│   │   └── src/bus/       # 事件总线
│   ├── app/               # Web/TUI 前端
│   ├── sdk/               # JavaScript SDK
│   ├── plugin/            # 插件 API
│   ├── util/              # 共享工具
│   └── enterprise/        # 企业版功能
└── sdks/                  # 多语言 SDK
```

### 关键文件路径

| 功能 | 路径 |
|------|------|
| CLI 入口 | `packages/opencode/src/index.ts` |
| Agent 定义 | `packages/opencode/src/agent/agent.ts` |
| 会话循环 | `packages/opencode/src/session/prompt.ts` |
| 消息处理 | `packages/opencode/src/session/processor.ts` |
| 工具注册 | `packages/opencode/src/tool/registry.ts` |
| 配置加载 | `packages/opencode/src/config/config.ts` |
| 插件系统 | `packages/opencode/src/plugin/index.ts` |
| LSP 协调 | `packages/opencode/src/lsp/index.ts` |

---

## 与 codex 对比

| 特性 | opencode | codex |
|------|----------|-------|
| 语言 | TypeScript/Bun | Rust |
| Agent 系统 | 7 个原生 Agent | subagent delegate 模式 |
| 工具数量 | 15+ 内置 | 10+ 内置 |
| 上下文压缩 | 自动 + 手动 | 手动触发 |
| 提示词注入 | 三层结构 + 插件 Hook | system_reminder 附件 |
| 插件系统 | npm 包 + 本地文件 | (待实现) |
| LSP 支持 | 40+ 内置 Server | (待实现) |
| Provider | 30+ 内置适配器 | adapter registry |

---

## 分析方法

本分析基于以下方法:

1. **源码阅读**: 逐文件分析关键模块实现
2. **数据流追踪**: 从用户输入到响应输出的完整路径
3. **架构对比**: 与 codex 实现的差异与可借鉴点
4. **扩展点识别**: 插件、Hook、配置等扩展机制

---

*文档生成时间: 2025-12-28*
*基于 opencode 源码分析*
