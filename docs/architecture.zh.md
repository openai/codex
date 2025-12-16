# 架构概览（Architecture）

> 本文是 `docs/architecture.md` 的中文版本，内容与英文版在信息上保持一致，但表述上略有调整。

## 顶层结构

仓库主要目录：

- `codex-rs/`：Rust 实现的主工程（Cargo workspace），当前主维护版本。
- `codex-cli/`：旧版 TypeScript CLI（legacy），保留用于兼容和参考。
- `sdk/typescript/`：TypeScript SDK，用于在 Node/TS 项目中以编程方式使用 Codex。
- `docs/`：文档（安装、配置、sandbox、FAQ 等）。
- 其他：`scripts/`、Nix 配置（`flake.nix`）、JS/TS workspace 配置（`pnpm-workspace.yaml`）等。

以下分层图主要聚焦 `codex-rs/`。

## codex-rs 分层架构

```text
┌───────────────────────────── 顶层 CLI / 入口层 ─────────────────────────────┐
│  codex-rs/cli        → 多子命令入口：codex / codex exec / codex mcp...     │
│     - clap 解析参数，选择 TUI / Exec / MCP / Sandbox 等子命令              │
│     - 装配 Config / Feature flags / 日志等                                 │
│                                                                             │
│  codex-rs/tui        → 交互式全屏终端 UI                                   │
│     - 事件循环、键盘/鼠标处理、布局、主题                                  │
│     - 通过 codex-core 驱动“智能体回合”：发请求、收事件、展示 diff          │
│                                                                             │
│  codex-rs/exec       → 非交互 “headless” 模式 (codex exec)                  │
│     - 单次任务：读 prompt → 调用 codex-core → 输出结果 → 退出               │
└─────────────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌────────────────────────────── 业务核心层 ───────────────────────────────────┐
│  codex-rs/core (codex-core)                                               │
│     - 会话/对话管理：ConversationManager, CodexConversation               │
│     - 工具与命令：shell 执行、apply-patch、文件操作、git 信息              │
│     - 安全控制：sandboxing（seatbelt/landlock/windows）、command_safety   │
│     - 配置：config & config_loader & features                             │
│     - 模型调用：ModelClient、model_provider_info、chat_completions        │
│     - MCP / 外部工具：mcp, mcp_connection_manager, mcp_tool_call          │
│     - 上下文：project_doc、environment_context、message_history 等         │
│     - 观测：otel_init、rollout 记录、事件映射 event_mapping               │
│     - 协议 re-export：codex_protocol::protocol / config_types             │
└─────────────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌───────────────────────────── 适配 & 基础设施层 ─────────────────────────────┐
│  协议 & SDK 适配                                                     │
│    - codex-rs/protocol          → CLI 内部使用的 protocol 定义            │
│    - codex-rs/app-server-protocol → 与外部 “app server” 通信模型         │
│    - codex-rs/mcp-types         → MCP 数据结构                            │
│    - sdk/typescript             → TypeScript SDK，对外暴露编程接口        │
│                                                                       │
│  后端与模型适配                                                     │
│    - codex-rs/backend-client    → 与 Codex backend / cloud 通讯           │
│    - codex-rs/codex-backend-openapi-models → 后端 OpenAPI 生成的类型      │
│    - codex-rs/chatgpt           → ChatGPT 登录 / apply 命令等             │
│    - codex-rs/ollama            → 本地 Ollama 模型支持                     │
│    - codex-rs/responses-api-proxy → responses API 的本地代理              │
│    - codex-rs/rmcp-client       → 远程 MCP 客户端                         │
│                                                                       │
│  Sandbox / 安全隔离                                                  │
│    - codex-rs/linux-sandbox     → Landlock + seccomp                     │
│    - codex-rs/windows-sandbox-rs→ Windows restricted token               │
│    - codex-rs/execpolicy        → 进程执行策略                           │
│    - codex-rs/process-hardening → 附加加固逻辑                           │
│                                                                       │
│  运行时工具 & 辅助                                                   │
│    - codex-rs/file-search       → 项目内模糊/语义搜索                     │
│    - codex-rs/ansi-escape       → 解析/处理 ANSI 转义序列                 │
│    - codex-rs/app-server        → 本地 app server, 用于 UI/编辑器集成     │
│    - codex-rs/cloud-tasks*      → Cloud 任务浏览 / 应用                   │
│    - codex-rs/login / keyring-store → 登录流程与凭据存储                  │
│    - codex-rs/feedback          → 反馈管道                                │
│    - codex-rs/otel              → OpenTelemetry 链路追踪                  │
│    - codex-rs/stdio-to-uds      → 标准 IO 与 Unix socket 中继             │
└─────────────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────── 工具 & 公共库层 ─────────────────────────────┐
│  codex-rs/common              → CLI 通用配置、错误处理、公共类型等          │
│  codex-rs/utils/*             → git、string、tokenizer、pty、image 等工具   │
│  codex-rs/async-utils         → 异步相关工具                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 设计原则

### 分层与职责边界

- 入口层（`cli` / `tui` / `exec`）只负责用户交互与参数解析。
- `codex-core` 统一承载“智能体”业务逻辑（会话、工具调用、模型调用、安全策略）。
- 适配层处理与外部世界的形式差异（协议、后端、平台 sandbox）。
- 底层 util crate 封装复用性高的技术细节。

### 协议优先

- 通过 `codex-protocol`、`app-server-protocol`、`mcp-types` 等 crate 明确组件边界：
  - CLI ↔ IDE/前端
  - CLI ↔ app server
  - CLI ↔ MCP 世界
- TypeScript SDK 和其他系统只需要遵守这些协议，不依赖内部具体实现。

### 安全优先

- 尽可能在 `codex-core` 内统一进行命令安全检查和 sandbox 策略选择。
- 沙箱实现细节拆分到专门 crate，便于跨平台演进和独立测试。

### 多前端共享同一核心

- TUI、headless CLI、未来的 IDE 插件或 Web 前端都围绕同一个 `codex-core` 工作。
- 新增前端时，无需重写业务逻辑，只需适配交互形态和展示方式。

### 可插拔的模型与后端

- 通过 `ModelClient` 抽象模型调用，底层可以是 ChatGPT、Ollama 或其他兼容服务。
- 后端访问由独立 crate 管理（如 `backend-client`、`responses-api-proxy`），可按需替换或扩展。

