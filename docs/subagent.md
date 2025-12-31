# Codex Task, SubAgent, Slash Command 综合文档

> 本文档用于 LLM 快速理解 Codex 的 Task、SubAgent、Slash Command 实现机制

## 1. 架构概览

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Codex 执行架构                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  用户输入 (TUI/CLI/App-Server)                                           │
│      │                                                                  │
│      ├── Slash Command (TUI解析)                                         │
│      │   └── /review, /undo, /compact, /model, /status...              │
│      │                                                                  │
│      └── Op (Protocol层)                                                 │
│          ├── Op::UserInput        → RegularTask                         │
│          ├── Op::Undo             → UndoTask                            │
│          ├── Op::Compact          → CompactTask                         │
│          ├── Op::RunUserShellCommand → UserShellCommandTask             │
│          └── Op::Review           → ReviewTask ──┐                      │
│                                                  │                      │
│  Session::spawn_task(task)                       │                      │
│      │                                           │                      │
│      └── Task Execution                          │                      │
│          ├── 直接执行 (大多数Task)                │                      │
│          └── SubAgent委派 ◄──────────────────────┘                      │
│              └── Codex::spawn() (完整子实例)                             │
│                  └── codex_delegate.rs                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 核心概念对比

| 概念 | 定义 | 生命周期 | 配置支持 | 使用场景 |
|------|------|---------|---------|---------|
| **Slash Command** | TUI层用户命令 | 单次触发 | 硬编码 | 用户交互入口 |
| **Op** | Protocol层操作 | 单次请求 | 枚举定义 | 跨进程通信 |
| **Task** | 后台执行单元 | Turn期间 | 硬编码 | 所有用户交互 |
| **SubAgent** | 子Codex实例 | Task内 | 硬编码 | 仅Review |

---

## 3. Slash Command 实现

### 3.1 定义 (`tui/src/slash_command.rs`)

```rust
pub enum SlashCommand {
    Model,       // 选择模型
    Approvals,   // 审批策略
    Review,      // 代码审查 → ReviewTask → SubAgent
    New,         // 新对话
    Init,        // 创建 AGENTS.md
    Compact,     // 压缩历史 → CompactTask
    Undo,        // 撤销 → UndoTask
    Diff,        // 显示diff
    Mention,     // 提及文件
    Status,      // 状态信息
    Mcp,         // MCP工具列表
    Logout,      // 登出
    Quit,        // 退出
    Exit,        // 退出
    Feedback,    // 反馈
    Rollout,     // (debug) rollout路径
    TestApproval,// (debug) 测试审批
}
```

### 3.2 Command → Op → Task 映射

| Slash Command | Op 变体 | Task 实现 | SubAgent? |
|---------------|---------|-----------|-----------|
| `/review` | Op::Review | ReviewTask | ✅ 是 |
| `/undo` | Op::Undo | UndoTask | ❌ 否 |
| `/compact` | Op::Compact | CompactTask | ❌ 否 |
| `!cmd` | Op::RunUserShellCommand | UserShellCommandTask | ❌ 否 |
| (普通消息) | Op::UserInput | RegularTask | ❌ 否 |

### 3.3 关键文件

| 文件 | 作用 |
|------|------|
| `tui/src/slash_command.rs` | SlashCommand 枚举定义 |
| `protocol/src/protocol.rs` | Op 枚举定义 |
| `core/src/codex.rs` | Op → Task 路由 (submission_loop) |

---

## 4. Task 系统实现

### 4.1 SessionTask Trait (`core/src/tasks/mod.rs`)

```rust
#[async_trait]
pub(crate) trait SessionTask: Send + Sync + 'static {
    /// 任务类型标识
    fn kind(&self) -> TaskKind;

    /// 主执行方法
    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String>;  // 返回最终agent消息

    /// 取消清理钩子 (可选)
    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {}
}
```

### 4.2 Task 类型详解

| Task | 文件 | TaskKind | 功能 | 触发 |
|------|------|----------|------|------|
| `RegularTask` | `tasks/regular.rs` | Regular | 主对话循环 | 用户消息 |
| `UserShellCommandTask` | `tasks/user_shell.rs` | Regular | 执行shell命令 | `!cmd` |
| `UndoTask` | `tasks/undo.rs` | Regular | 恢复git快照 | `/undo` |
| `CompactTask` | `tasks/compact.rs` | Compact | 压缩对话历史 | `/compact`或自动 |
| `ReviewTask` | `tasks/review.rs` | Review | 代码审查 | `/review` |
| `GhostSnapshotTask` | `tasks/ghost_snapshot.rs` | Regular | 捕获git快照 | RegularTask内部 |

### 4.3 Task 生命周期

```
Session::spawn_task(turn_context, input, task)
    │
    ├── 1. abort_all_tasks() // 取消现有任务
    │
    ├── 2. Arc::new(task) // 包装为 Arc<dyn SessionTask>
    │
    ├── 3. tokio::spawn() // 启动后台任务
    │       └── task.run(session_ctx, ctx, input, cancellation_token)
    │               │
    │               ├── [执行任务逻辑...]
    │               │
    │               └── 完成/取消
    │
    ├── 4. register_new_active_task() // 注册到 active_turn
    │
    └── 5. on_task_finished() → EventMsg::TaskComplete
```

### 4.4 关键文件

| 文件 | 作用 |
|------|------|
| `core/src/tasks/mod.rs` | SessionTask trait + spawn_task() |
| `core/src/tasks/regular.rs` | 主对话任务 |
| `core/src/tasks/review.rs` | 代码审查任务 (使用SubAgent) |
| `core/src/tasks/compact.rs` | 压缩任务 |
| `core/src/tasks/undo.rs` | 撤销任务 |
| `core/src/tasks/user_shell.rs` | Shell命令任务 |
| `core/src/tasks/ghost_snapshot.rs` | Git快照任务 |
| `core/src/state/turn.rs` | TaskKind, RunningTask |

---

## 5. SubAgent (Delegate) 实现

### 5.1 核心文件: `core/src/codex_delegate.rs`

SubAgent 是一个**完整的子 Codex 实例**，用于隔离执行特定任务。

### 5.2 SubAgentSource 枚举

```rust
// protocol/src/protocol.rs
pub enum SubAgentSource {
    Review,        // ✅ 实际使用
    Compact,       // ❌ 定义但未使用
    Other(String), // 预留扩展
}
```

### 5.3 SubAgent 创建流程

```rust
// codex_delegate.rs
pub async fn run_codex_conversation_interactive(
    config: Config,              // 子agent配置 (可定制)
    auth_manager: Arc<AuthManager>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
    initial_history: Option<InitialHistory>,
) -> Result<Codex, CodexErr> {
    // 创建完整的子Codex实例
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        auth_manager,
        initial_history,
        SessionSource::SubAgent(SubAgentSource::Review),
    ).await?;

    // 启动事件转发 (子agent → 父session)
    tokio::spawn(forward_events(...));

    // 启动操作转发 (调用者 → 子agent)
    tokio::spawn(forward_ops(...));

    Ok(codex)
}
```

### 5.4 SubAgent 事件处理

```rust
async fn forward_events(...) {
    match event.msg {
        // 审批请求 → 路由到父Session处理
        EventMsg::ExecApprovalRequest(e) => {
            handle_exec_approval(&codex, &parent_session, e).await;
        }
        EventMsg::ApplyPatchApprovalRequest(e) => {
            handle_patch_approval(&codex, &parent_session, e).await;
        }
        // 其他事件 → 转发给消费者
        other => tx_sub.send(other).await,
    }
}
```

### 5.5 ReviewTask 使用 SubAgent

```rust
// tasks/review.rs
async fn start_review_conversation(...) {
    let mut sub_agent_config = config.clone();

    // 定制子agent配置
    sub_agent_config.user_instructions = None;     // 移除用户指令
    sub_agent_config.project_doc_max_bytes = 0;    // 不加载项目文档
    sub_agent_config.features
        .disable(Feature::WebSearchRequest)         // 禁用web搜索
        .disable(Feature::ViewImageTool);           // 禁用图片查看
    sub_agent_config.base_instructions = Some(REVIEW_PROMPT.to_string());

    // 启动子agent
    run_codex_conversation_one_shot(sub_agent_config, ...).await
}
```

### 5.6 关键文件

| 文件 | 作用 |
|------|------|
| `core/src/codex_delegate.rs` | SubAgent 核心实现 |
| `protocol/src/protocol.rs` | SessionSource, SubAgentSource 定义 |
| `core/src/tasks/review.rs` | ReviewTask (唯一使用SubAgent的Task) |

---

## 6. 数据流对比

### 6.1 普通消息流 (RegularTask)

```
用户输入 "fix the bug"
    │
    └── TUI/CLI 解析
            │
            └── Op::UserInput { items: [...] }
                    │
                    └── codex.rs: submission_loop
                            │
                            └── spawn_task(RegularTask)
                                    │
                                    └── run_task() [直接执行]
                                            │
                                            └── LLM调用 + 工具执行
```

### 6.2 Slash Command 流 (ReviewTask + SubAgent)

```
用户输入 "/review"
    │
    └── TUI: SlashCommand::Review
            │
            └── Op::Review { request: ReviewRequest }
                    │
                    └── codex.rs: submission_loop
                            │
                            └── spawn_task(ReviewTask)
                                    │
                                    └── run_codex_conversation_one_shot() [SubAgent]
                                            │
                                            └── Codex::spawn() [子实例]
                                                    │
                                                    └── 独立LLM调用
```

---

## 7. 配置化扩展分析

### 7.1 当前状态

| 组件 | 配置支持 | 扩展方式 |
|------|---------|---------|
| Slash Command | ❌ 硬编码枚举 | 修改 `slash_command.rs` |
| Op | ❌ 硬编码枚举 | 修改 `protocol.rs` |
| Task | ❌ 硬编码实现 | 新增 `tasks/*.rs` |
| SubAgent | ❌ 硬编码 | 修改 `codex_delegate.rs` |

### 7.2 社区讨论的扩展方向

参考 GitHub Issues:
- [#2604](https://github.com/openai/codex/issues/2604): Subagent Support
- [#2770](https://github.com/openai/codex/issues/2770): First-Class Subagents
- [#2771](https://github.com/openai/codex/issues/2771): Orchestrator Agent
- [#3655](https://github.com/openai/codex/pull/3655): Multi-Agent Orchestration

计划支持:
- 配置化 agent 定义 (`~/.codex/agents.toml`)
- 动态 SubAgent 注册
- Orchestrator 模式
- 并行 SubAgent 执行

---

## 8. 与 Claude Code 对比

| 特性 | Codex | Claude Code |
|------|-------|-------------|
| Slash Command | TUI层枚举 | 系统内置 + 可配置 |
| Task | SessionTask trait | 未暴露 |
| SubAgent | 仅Review使用 | 多种专用类型 |
| 配置方式 | 代码硬编码 | 部分可配置 |
| 并行执行 | 单任务 | 支持 |

---

## 9. 关键代码位置索引

```
codex-rs/
├── protocol/src/
│   └── protocol.rs              # Op, SessionSource, SubAgentSource
│
├── tui/src/
│   └── slash_command.rs         # SlashCommand 枚举
│
└── core/src/
    ├── codex.rs                 # submission_loop (Op→Task路由)
    ├── codex_delegate.rs        # SubAgent 核心实现
    │
    ├── tasks/
    │   ├── mod.rs               # SessionTask trait
    │   ├── regular.rs           # RegularTask
    │   ├── review.rs            # ReviewTask (SubAgent)
    │   ├── compact.rs           # CompactTask
    │   ├── undo.rs              # UndoTask
    │   ├── user_shell.rs        # UserShellCommandTask
    │   └── ghost_snapshot.rs    # GhostSnapshotTask
    │
    └── state/
        └── turn.rs              # TaskKind, RunningTask
```

---

## 10. 总结

1. **Slash Command** 是 TUI 层的用户入口，映射到 **Op** 变体
2. **Op** 是跨进程通信协议，在 `codex.rs` 中路由到对应 **Task**
3. **Task** 是后台执行单元，实现 `SessionTask` trait
4. **SubAgent** 是完整子 Codex 实例，仅 **ReviewTask** 使用
5. 当前所有组件**不支持配置化**，社区正在讨论扩展方案

---

## 附录: 快速查阅

### A. 添加新 Slash Command

1. `tui/src/slash_command.rs` - 添加枚举变体
2. 实现 `description()` 和 `available_during_task()`
3. TUI 中处理命令逻辑

### B. 添加新 Task

1. `core/src/tasks/new_task.rs` - 实现 `SessionTask` trait
2. `core/src/tasks/mod.rs` - 导出
3. `core/src/codex.rs` - 添加 `spawn_task()` 调用

### C. 添加新 SubAgent 类型

1. `protocol/src/protocol.rs` - 扩展 `SubAgentSource`
2. `core/src/codex_delegate.rs` - 复用现有基础设施
3. 新 Task 中调用 `run_codex_conversation_one_shot()`
