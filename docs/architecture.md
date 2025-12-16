# Codex CLI 架构概览

本文档概述 Codex 仓库的整体结构，并重点说明 Rust 实现（`codex-rs/`）的分层架构和设计思路。

## 顶层结构

仓库的主要目录结构如下：

- `codex-rs/`：Rust 实现的主工程（Cargo workspace），当前主维护版本。
- `codex-cli/`：旧版 TypeScript CLI（legacy）。
- `sdk/typescript/`：TypeScript SDK，对外暴露编程接口。
- `docs/`：各类文档（安装、配置、sandbox、FAQ 等）。
- 其他：`scripts/`、Nix 配置（`flake.nix`）、JS/TS workspace 配置（`pnpm-workspace.yaml`）等。

下面的架构图主要聚焦 `codex-rs/`。

## codex-rs 分层架构图

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

## 设计思路

### 1. 分层清晰，UI 与核心解耦

- 最上层是“入口层”（`cli` / `tui` / `exec`），负责：
  - 解析 CLI 参数、选择子命令。
  - 管理配置、feature flags、日志与 sandbox 选项。
- 中间层是 `codex-core`，承载所有与“智能体回合”相关的业务逻辑：
  - 会话管理、工具调用、模型调用、安全策略、上下文加载等。
- 底层是适配 & 基础设施层和通用工具库：
  - 与远端服务、MCP、sandbox、Cloud 等的集成。
  - 常用 util、异步工具、协议类型。

这样，TUI、headless CLI，甚至未来 IDE/Web 前端，都只需要依赖 `codex-core`，而不直接耦合具体后端或 UI 技术。

### 2. 强协议边界

- 使用专门的协议 crate 明确边界：
  - `codex-protocol`：Codex CLI 与前端/嵌入方之间的协议。
  - `app-server-protocol`：与本地/远端 app server 的通信模型。
  - `mcp-types`：MCP 类型定义。
- TypeScript SDK（`sdk/typescript`）和其他外部系统只关心这些协议，而不依赖内部实现细节。

### 3. 安全优先与 sandbox 抽象

- Sandbox 能力拆分为独立 crate（`linux-sandbox`、`windows-sandbox-rs` 等），并通过 `codex-core` 的 `sandboxing` / `command_safety` 模块统一对外。
- 所有 shell/文件操作都必须通过这些统一入口，便于：
  - 实现跨平台 sandbox。
  - 在需要时快速审计和强化安全策略。

### 4. 多前端、多模式复用相同核心

- `tui` 专注于 ratatui 布局、输入事件处理、diff/markdown 渲染等。
- `exec` 专注于非交互调用：从 stdin/参数接收 prompt，调用 `codex-core`，按事件流输出结果。
- 两者都把“决策”和“步骤编排”交给 `codex-core`，因此可以在不改核心逻辑的前提下扩展新的 front-end（例如 IDE 面板、远程服务）。

### 5. 模型与后端可插拔

- `model_provider_info`、`backend-client`、`chatgpt`、`ollama` 等 crate 把不同 provider 的细节封装起来。
- `codex-core` 仅通过 `ModelClient`/`ResponseStream` 抽象与模型交互：
  - 可以同时支持 ChatGPT、Ollama 或其他兼容 API 的服务。
  - 在配置层面可以切换或扩展 provider。

### 6. 基于“回合/事件流”的编排

- 核心采用“事件流”模型：
  - `ResponseEvent`、`ResponseStream` 表达模型回答、工具调用、文件 diff、终端输出等各种事件。
  - `event_mapping` 把底层协议事件映射为上层可消费的统一结构。
- 前端（TUI、exec）只需消费事件流并选择如何展示：
  - TUI 做增量渲染和交互。
  - Exec 简单地线性输出。

这套架构使得 Codex 在保持安全和可控性的前提下，能够以相同的核心逻辑服务不同前端形态和运行模式。

