# Codex Plan Mode Implementation Specification

> 基于 Claude Code v2.0.59 分析和 Codex-rs 现有架构设计
> 版本: 1.0
> 最后更新: 2025-01-28

## 目录

1. [概述](#1-概述)
2. [设计决策](#2-设计决策)
3. [现有基础设施](#3-现有基础设施)
4. [核心数据结构](#4-核心数据结构)
5. [Plan Mode 模块实现](#5-plan-mode-模块实现)
6. [Tool 实现](#6-tool-实现)
7. [TUI 集成](#7-tui-集成)
8. [Protocol 事件](#8-protocol-事件)
9. [工具过滤机制](#9-工具过滤机制)
10. [System Reminder 集成](#10-system-reminder-集成)
11. [完整数据流](#11-完整数据流)
12. [文件修改清单](#12-文件修改清单)
13. [实现步骤](#13-实现步骤)
14. [测试策略](#14-测试策略)
15. [附录](#15-附录)

---

## 1. 概述

### 1.1 什么是 Plan Mode

Plan Mode 是一个结构化的工作流程，在执行复杂任务前强制进行探索和规划。在此模式下：

- **只读限制**: 只允许使用读取类工具（读文件、搜索、grep等）
- **计划文件例外**: 唯一可写的文件是计划文件本身
- **子代理探索**: 可以启动 Explore/Plan 子代理进行代码库探索
- **用户审批**: 退出 Plan Mode 需要用户审批计划

### 1.2 工作流程

```
用户输入 /plan
    │
    ▼
进入 Plan Mode
    │
    ├─► 探索代码库（只读工具）
    ├─► 启动 Explore 子代理
    ├─► 设计实现方案
    └─► 写入计划文件
    │
    ▼
调用 exit_plan_mode 工具
    │
    ▼
用户审批计划
    │
    ├─► 批准 → 退出 Plan Mode，开始实现
    └─► 拒绝 → 继续规划
```

---

## 2. 设计决策

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 进入机制 | 仅 `/plan` 命令 | 用户控制，不允许 LLM 自动进入 |
| 退出机制 | 简单批准/拒绝 | 简化 UX，避免复杂的多模式选择 |
| 功能开关 | 无，始终启用 | 功能稳定后直接可用 |
| 文件命名 | `{conv_id}_{timestamp}.md` | 简化实现，保证唯一性 |
| 工具名称 | 使用 `names::*` 常量 | 避免硬编码字符串，便于维护 |

---

## 3. 现有基础设施

Codex 已经实现了大部分 Plan Mode 所需的基础设施：

| 组件 | 位置 | 状态 |
|------|------|------|
| `PlanModeGenerator` | `core/src/system_reminder/attachments/plan_mode.rs` | ✅ 已实现 - 生成 plan mode 指令 |
| `PlanState` | `core/src/system_reminder/generator.rs:96` | ✅ 已实现 - 跟踪计划状态 |
| `GeneratorContext` | `core/src/system_reminder/generator.rs` | ✅ 已实现 - 包含 `is_plan_mode`, `plan_file_path`, `is_plan_reentry` |
| `SubagentStores.plan_state` | `core/src/subagent/stores.rs:48` | ✅ 已实现 |
| `EXPLORE_AGENT` | `core/src/subagent/definition/builtin.rs` | ✅ 已实现 - 只读工具集 |
| `PLAN_AGENT` | `core/src/subagent/definition/builtin.rs` | ✅ 已实现 - 只读工具集 |
| `ToolFilter` | `core/src/tools/spec_ext.rs` | ✅ 已实现 - 需要扩展 |

### 3.1 现有 PlanModeGenerator 代码参考

```rust
// 位置: core/src/system_reminder/attachments/plan_mode.rs
pub struct PlanModeGenerator;

impl AttachmentGenerator for PlanModeGenerator {
    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.is_plan_mode {
            return Ok(None);
        }
        // 已经实现了 plan mode 提示生成
        // 包括 re-entry 检测逻辑
    }
}
```

### 3.2 现有 GeneratorContext 结构

```rust
// 位置: core/src/system_reminder/generator.rs
pub struct GeneratorContext<'a> {
    pub turn_number: i32,
    pub is_main_agent: bool,
    pub has_user_input: bool,
    pub cwd: &'a Path,
    pub agent_id: &'a str,
    pub file_tracker: &'a FileTracker,
    pub is_plan_mode: bool,           // 已存在
    pub plan_file_path: Option<&'a str>, // 已存在
    pub is_plan_reentry: bool,        // 已存在
    pub plan_state: &'a PlanState,    // 已存在
    pub background_tasks: &'a [BackgroundTaskInfo],
    pub critical_instruction: Option<&'a str>,
    pub diagnostics_store: Option<&'a DiagnosticsStore>,
    pub lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity,
}
```

---

## 4. 核心数据结构

### 4.1 PlanModeState

**文件**: `core/src/plan_mode/mod.rs` (新建)

```rust
use std::path::PathBuf;
use codex_protocol::ConversationId;

/// Plan Mode 会话状态
///
/// 跟踪当前会话的 Plan Mode 状态，包括是否激活、计划文件路径等。
/// 存储在 SubagentStores 中，生命周期与会话相同。
#[derive(Debug, Clone, Default)]
pub struct PlanModeState {
    /// Plan Mode 是否激活
    pub is_active: bool,

    /// 计划文件路径（例如 ~/.codex/plans/{conv_id}_20250101_143022.md）
    pub plan_file_path: Option<PathBuf>,

    /// 是否已经退出过 Plan Mode（用于 re-entry 检测）
    /// 当用户批准计划退出 Plan Mode 后设为 true
    /// 如果再次进入 Plan Mode 且旧计划文件存在，触发 re-entry 逻辑
    pub has_exited: bool,

    /// 会话 ID（用于生成文件名）
    pub conversation_id: Option<ConversationId>,
}

impl PlanModeState {
    /// 创建新的 Plan Mode 状态
    pub fn new() -> Self {
        Self::default()
    }

    /// 进入 Plan Mode
    ///
    /// # Arguments
    /// * `conversation_id` - 当前会话 ID
    ///
    /// # Returns
    /// 计划文件路径
    pub fn enter(&mut self, conversation_id: ConversationId) -> PathBuf {
        let plan_file_path = get_plan_file_path(&conversation_id);

        self.is_active = true;
        self.plan_file_path = Some(plan_file_path.clone());
        self.conversation_id = Some(conversation_id);
        // has_exited 保持原值，用于 re-entry 检测

        plan_file_path
    }

    /// 退出 Plan Mode
    ///
    /// # Arguments
    /// * `approved` - 用户是否批准了计划
    pub fn exit(&mut self, approved: bool) {
        self.is_active = false;
        if approved {
            self.has_exited = true;
        }
        // plan_file_path 保留，用于 re-entry 时读取旧计划
    }

    /// 检查是否是 re-entry 情况
    ///
    /// re-entry 条件：
    /// 1. 之前退出过 Plan Mode (has_exited == true)
    /// 2. 计划文件仍然存在
    pub fn is_reentry(&self) -> bool {
        if !self.has_exited {
            return false;
        }

        match &self.plan_file_path {
            Some(path) => path.exists(),
            None => false,
        }
    }

    /// 重置 re-entry 标志
    /// 在 re-entry 提示发送后调用
    pub fn clear_reentry(&mut self) {
        self.has_exited = false;
    }
}
```

### 4.2 计划文件路径生成

```rust
use std::path::PathBuf;
use chrono::Local;
use codex_protocol::ConversationId;

/// 获取计划文件存储目录
///
/// 返回 ~/.codex/plans/，如果目录不存在则创建
pub fn get_plans_directory() -> PathBuf {
    let plans_dir = dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".codex")
        .join("plans");

    if !plans_dir.exists() {
        std::fs::create_dir_all(&plans_dir)
            .expect("无法创建 plans 目录");
    }

    plans_dir
}

/// 生成计划文件名
///
/// 格式: {conversation_id}_{YYYYMMDD_HHMMSS}.md
/// 例如: conv_abc123_20250101_143022.md
///
/// # Arguments
/// * `conversation_id` - 会话 ID
pub fn generate_plan_filename(conversation_id: &ConversationId) -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    format!("{}_{}.md", conversation_id, timestamp)
}

/// 获取计划文件完整路径
///
/// # Arguments
/// * `conversation_id` - 会话 ID
///
/// # Returns
/// 完整路径，例如 ~/.codex/plans/conv_abc123_20250101_143022.md
pub fn get_plan_file_path(conversation_id: &ConversationId) -> PathBuf {
    get_plans_directory().join(generate_plan_filename(conversation_id))
}

/// 读取计划文件内容
///
/// # Arguments
/// * `path` - 计划文件路径
///
/// # Returns
/// 文件内容，如果文件不存在或读取失败返回 None
pub fn read_plan_file(path: &std::path::Path) -> Option<String> {
    if !path.exists() {
        return None;
    }

    std::fs::read_to_string(path).ok()
}

/// 检查计划文件是否存在
pub fn plan_file_exists(path: &std::path::Path) -> bool {
    path.exists()
}
```

---

## 5. Plan Mode 模块实现

### 5.1 模块结构

**文件**: `core/src/plan_mode/mod.rs` (新建)

```rust
//! Plan Mode 模块
//!
//! 提供 Plan Mode 的核心功能：
//! - PlanModeState: 会话级别的 Plan Mode 状态
//! - 计划文件路径生成和管理
//! - 进入/退出 Plan Mode 的逻辑

mod state;
mod file_management;

pub use state::PlanModeState;
pub use file_management::{
    get_plans_directory,
    generate_plan_filename,
    get_plan_file_path,
    read_plan_file,
    plan_file_exists,
};

#[cfg(test)]
mod tests;
```

### 5.2 集成到 SubagentStores

**文件**: `core/src/subagent/stores.rs` (修改)

```rust
// 添加导入
use crate::plan_mode::PlanModeState;

/// Session-scoped subagent stores.
#[derive(Debug)]
pub struct SubagentStores {
    pub registry: Arc<AgentRegistry>,
    pub background_store: Arc<BackgroundTaskStore>,
    pub transcript_store: Arc<TranscriptStore>,
    pub reminder_orchestrator: Arc<SystemReminderOrchestrator>,
    pub file_tracker: Arc<FileTracker>,
    pub plan_state: Arc<RwLock<PlanState>>,

    // 新增: Plan Mode 状态
    pub plan_mode: Arc<RwLock<PlanModeState>>,

    inject_call_count: AtomicI32,
}

impl SubagentStores {
    pub fn new() -> Self {
        let search_paths = build_default_search_paths();
        Self {
            registry: Arc::new(AgentRegistry::with_search_paths(search_paths)),
            background_store: Arc::new(BackgroundTaskStore::new()),
            transcript_store: Arc::new(TranscriptStore::new()),
            reminder_orchestrator: Arc::new(SystemReminderOrchestrator::new(
                SystemReminderConfig::default(),
            )),
            file_tracker: Arc::new(FileTracker::new()),
            plan_state: Arc::new(RwLock::new(PlanState::default())),

            // 新增
            plan_mode: Arc::new(RwLock::new(PlanModeState::new())),

            inject_call_count: AtomicI32::new(0),
        }
    }

    // 新增: Plan Mode 辅助方法

    /// 进入 Plan Mode
    pub fn enter_plan_mode(&self, conversation_id: ConversationId) -> PathBuf {
        let mut state = self.plan_mode.write().expect("plan_mode lock poisoned");
        state.enter(conversation_id)
    }

    /// 退出 Plan Mode
    pub fn exit_plan_mode(&self, approved: bool) {
        let mut state = self.plan_mode.write().expect("plan_mode lock poisoned");
        state.exit(approved);
    }

    /// 获取 Plan Mode 状态快照
    pub fn get_plan_mode_state(&self) -> PlanModeState {
        self.plan_mode.read().expect("plan_mode lock poisoned").clone()
    }

    /// 检查是否在 Plan Mode 中
    pub fn is_plan_mode_active(&self) -> bool {
        self.plan_mode.read().expect("plan_mode lock poisoned").is_active
    }
}
```

### 5.3 导出模块

**文件**: `core/src/lib.rs` (修改)

```rust
// 添加模块导出
pub mod plan_mode;

// 在 pub use 部分添加
pub use plan_mode::{PlanModeState, get_plan_file_path, read_plan_file};
```

---

## 6. Tool 实现

### 6.1 Tool 名称常量

**文件**: `core/src/tools/names.rs` (修改)

```rust
// 在文件末尾添加

// Plan Mode Tools
pub const EXIT_PLAN_MODE: &str = "exit_plan_mode";

// 如果不存在，也添加
pub const ASK_USER_QUESTION: &str = "ask_user_question";
```

### 6.2 ExitPlanMode Tool Handler

**文件**: `core/src/tools/ext/plan_mode.rs` (新建)

```rust
//! Plan Mode 工具实现
//!
//! 提供 exit_plan_mode 工具，用于退出 Plan Mode 并请求用户审批计划。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::error::CodexErr;
use crate::plan_mode::{read_plan_file, plan_file_exists};
use crate::subagent::get_or_create_stores;
use crate::tools::FunctionCallError;
use crate::tools::ToolHandler;
use crate::tools::ToolInvocation;
use crate::tools::ToolKind;
use crate::tools::ToolOutput;
use crate::tools::ToolPayload;

/// exit_plan_mode 工具参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitPlanModeArgs {
    // 无参数 - 工具从 plan_mode_state 读取计划文件路径
}

/// exit_plan_mode 工具处理器
///
/// 功能：
/// 1. 读取计划文件内容
/// 2. 发送审批请求事件到 TUI
/// 3. 等待用户批准/拒绝
/// 4. 更新 Plan Mode 状态
#[derive(Debug, Clone)]
pub struct ExitPlanModeHandler;

impl ExitPlanModeHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExitPlanModeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for ExitPlanModeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        // exit_plan_mode 会修改 Plan Mode 状态，但不修改文件系统
        false
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. 获取会话 stores
        let session = invocation.session;
        let stores = get_or_create_stores(session.conversation_id);

        // 2. 检查是否在 Plan Mode 中
        let plan_mode_state = stores.get_plan_mode_state();
        if !plan_mode_state.is_active {
            return Err(FunctionCallError::InvalidArguments(
                "Not in plan mode. Cannot exit.".to_string()
            ));
        }

        // 3. 获取计划文件路径
        let plan_file_path = match &plan_mode_state.plan_file_path {
            Some(path) => path.clone(),
            None => {
                return Err(FunctionCallError::InvalidArguments(
                    "No plan file path set. Enter plan mode first.".to_string()
                ));
            }
        };

        // 4. 检查计划文件是否存在
        if !plan_file_exists(&plan_file_path) {
            return Err(FunctionCallError::InvalidArguments(
                format!(
                    "Plan file not found at {}. Please write your plan to this file before exiting.",
                    plan_file_path.display()
                )
            ));
        }

        // 5. 读取计划文件内容
        let plan_content = read_plan_file(&plan_file_path).ok_or_else(|| {
            FunctionCallError::InvalidArguments(
                format!("Failed to read plan file at {}", plan_file_path.display())
            )
        })?;

        // 6. 发送审批请求事件
        // 注意：这里需要通过 session 发送事件到 TUI
        // TUI 收到事件后显示计划内容，等待用户批准/拒绝
        session.send_event(
            invocation.turn.as_ref(),
            codex_protocol::protocol::EventMsg::PlanModeExitRequest(
                codex_protocol::protocol::PlanModeExitRequestEvent {
                    plan_content: plan_content.clone(),
                    plan_file_path: plan_file_path.to_string_lossy().to_string(),
                }
            )
        ).await;

        // 7. 返回成功消息
        // 实际的状态更新在用户批准后由 TUI 触发
        Ok(ToolOutput::Function {
            content: format!(
                "Exit plan mode requested. Waiting for user approval.\n\n\
                 Plan file: {}\n\n\
                 ## Plan Content:\n\n{}",
                plan_file_path.display(),
                plan_content
            ),
            content_items: None,
            success: Some(true),
        })
    }
}

/// 创建 exit_plan_mode 工具规格
pub fn create_exit_plan_mode_tool() -> crate::tools::ToolSpec {
    use crate::tools::ToolSpec;
    use crate::tools::ResponsesApiTool;
    use codex_protocol::json_schema::JsonSchema;
    use std::collections::BTreeMap;

    ToolSpec::Function(ResponsesApiTool {
        name: crate::tools::names::EXIT_PLAN_MODE.to_string(),
        description: "Exit plan mode and request user approval for the plan. \
            Call this tool when you have finished writing your plan to the plan file \
            and are ready for user review. The plan file must exist before calling this tool."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),  // 无参数
            required: None,
            additional_properties: Some(Box::new(false.into())),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_plan_mode_handler_kind() {
        let handler = ExitPlanModeHandler::new();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_create_exit_plan_mode_tool() {
        let tool = create_exit_plan_mode_tool();
        match tool {
            crate::tools::ToolSpec::Function(func) => {
                assert_eq!(func.name, "exit_plan_mode");
            }
            _ => panic!("Expected Function tool spec"),
        }
    }
}
```

### 6.3 注册 Tool

**文件**: `core/src/tools/spec_ext.rs` (修改)

```rust
// 在文件顶部添加导入
use crate::tools::ext::plan_mode::{ExitPlanModeHandler, create_exit_plan_mode_tool};

// 在 register_ext_tools 函数中添加
pub fn register_ext_tools(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    // ... 现有注册代码 ...

    // 注册 exit_plan_mode 工具
    register_exit_plan_mode(builder);
}

/// 注册 exit_plan_mode 工具
fn register_exit_plan_mode(builder: &mut ToolRegistryBuilder) {
    builder.push_spec(create_exit_plan_mode_tool());
    builder.register_handler(
        crate::tools::names::EXIT_PLAN_MODE,
        Arc::new(ExitPlanModeHandler::new())
    );
}
```

### 6.4 导出 Tool Handler 模块

**文件**: `core/src/tools/ext/mod.rs` (修改)

```rust
// 添加模块导出
pub mod plan_mode;
```

---

## 7. TUI 集成

### 7.1 Slash Command 定义

**文件**: `tui/src/slash_command.rs` (修改)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // ... 现有命令 ...

    /// 进入 Plan Mode
    Plan,
}

impl SlashCommand {
    pub fn description(self) -> &'static str {
        match self {
            // ... 现有描述 ...
            Self::Plan => "Enter plan mode for complex task planning",
        }
    }

    pub fn available_during_task(self) -> bool {
        match self {
            // ... 现有逻辑 ...
            Self::Plan => false,  // 任务执行中不能进入 Plan Mode
        }
    }

    pub fn is_visible(self) -> bool {
        match self {
            // ... 现有逻辑 ...
            Self::Plan => true,
        }
    }
}
```

**文件**: `tui2/src/slash_command.rs` (同样修改)

### 7.2 Slash Command Handler

**文件**: `tui/src/app.rs` 或相应的命令处理文件 (修改)

```rust
// 在处理 slash command 的地方添加

async fn handle_slash_command(&mut self, cmd: SlashCommand) -> Result<(), Error> {
    match cmd {
        // ... 现有命令处理 ...

        SlashCommand::Plan => {
            self.enter_plan_mode().await?;
        }
    }
    Ok(())
}

/// 进入 Plan Mode
async fn enter_plan_mode(&mut self) -> Result<(), Error> {
    // 1. 获取 stores
    let stores = get_or_create_stores(self.session.conversation_id);

    // 2. 检查是否已在 Plan Mode
    if stores.is_plan_mode_active() {
        self.show_message("Already in plan mode.")?;
        return Ok(());
    }

    // 3. 进入 Plan Mode
    let plan_file_path = stores.enter_plan_mode(self.session.conversation_id);

    // 4. 发送事件通知 Session
    self.session.submit(Op::SetPlanMode {
        active: true,
        plan_file_path: Some(plan_file_path.to_string_lossy().to_string()),
    }).await?;

    // 5. 显示确认消息
    self.show_message(&format!(
        "Entered plan mode.\n\
         Plan file: {}\n\n\
         In plan mode, you can:\n\
         - Explore the codebase using read-only tools\n\
         - Launch Explore/Plan agents for thorough analysis\n\
         - Write your plan to the plan file\n\
         - Call exit_plan_mode when ready for review",
        plan_file_path.display()
    ))?;

    Ok(())
}
```

### 7.3 Plan Mode 退出审批 UI

**文件**: `tui/src/plan_mode_approval.rs` (新建)

```rust
//! Plan Mode 退出审批 UI 组件

use ratatui::prelude::*;
use ratatui::widgets::*;

/// Plan Mode 审批状态
pub enum PlanApprovalState {
    /// 等待用户操作
    Pending { plan_content: String, plan_file_path: String },
    /// 用户已批准
    Approved,
    /// 用户已拒绝
    Rejected,
}

/// Plan Mode 审批组件
pub struct PlanApprovalWidget {
    state: PlanApprovalState,
    scroll_offset: u16,
}

impl PlanApprovalWidget {
    pub fn new(plan_content: String, plan_file_path: String) -> Self {
        Self {
            state: PlanApprovalState::Pending { plan_content, plan_file_path },
            scroll_offset: 0,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if let PlanApprovalState::Pending { plan_content, plan_file_path } = &self.state {
            // 渲染标题
            let title = format!("Plan Review - {}", plan_file_path);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            // 渲染计划内容
            let inner = block.inner(area);
            block.render(area, buf);

            let paragraph = Paragraph::new(plan_content.as_str())
                .wrap(Wrap { trim: false })
                .scroll((self.scroll_offset, 0));
            paragraph.render(inner, buf);

            // 渲染底部操作提示
            let help = "[Enter] Approve  [Esc] Reject  [↑↓] Scroll";
            let help_area = Rect::new(
                area.x + 2,
                area.y + area.height - 2,
                area.width - 4,
                1,
            );
            Paragraph::new(help)
                .style(Style::default().fg(Color::DarkGray))
                .render(help_area, buf);
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> Option<bool> {
        match key {
            crossterm::event::KeyCode::Enter => {
                self.state = PlanApprovalState::Approved;
                Some(true)  // 批准
            }
            crossterm::event::KeyCode::Esc => {
                self.state = PlanApprovalState::Rejected;
                Some(false)  // 拒绝
            }
            crossterm::event::KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                None
            }
            crossterm::event::KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                None
            }
            _ => None,
        }
    }
}
```

### 7.4 处理审批结果

```rust
// 在 TUI 事件处理中

async fn handle_plan_approval(&mut self, approved: bool) -> Result<(), Error> {
    // 1. 获取 stores
    let stores = get_or_create_stores(self.session.conversation_id);

    // 2. 更新 Plan Mode 状态
    stores.exit_plan_mode(approved);

    // 3. 发送事件到 Session
    self.session.send_event(
        None,
        EventMsg::PlanModeExited(PlanModeExitedEvent { approved })
    ).await?;

    // 4. 显示结果
    if approved {
        self.show_message("Plan approved. Exiting plan mode. You can now start implementation.")?;
    } else {
        self.show_message("Plan rejected. Continuing in plan mode.")?;
    }

    Ok(())
}
```

---

## 8. Protocol 事件

### 8.1 事件类型定义

**文件**: `protocol/src/protocol.rs` (修改)

```rust
// 在 EventMsg 枚举中添加

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMsg {
    // ... 现有事件 ...

    /// Plan Mode 已进入
    PlanModeEntered(PlanModeEnteredEvent),

    /// Plan Mode 退出请求（等待用户审批）
    PlanModeExitRequest(PlanModeExitRequestEvent),

    /// Plan Mode 已退出（用户已做出决定）
    PlanModeExited(PlanModeExitedEvent),
}

/// Plan Mode 进入事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeEnteredEvent {
    /// 计划文件路径
    pub plan_file_path: String,
}

/// Plan Mode 退出请求事件
///
/// 当 LLM 调用 exit_plan_mode 工具时发送此事件
/// TUI 收到后显示计划内容，等待用户批准/拒绝
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeExitRequestEvent {
    /// 计划文件内容
    pub plan_content: String,

    /// 计划文件路径
    pub plan_file_path: String,
}

/// Plan Mode 退出完成事件
///
/// 用户做出审批决定后发送
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeExitedEvent {
    /// 用户是否批准了计划
    pub approved: bool,
}
```

### 8.2 Op 类型定义

**文件**: `protocol/src/protocol.rs` (修改)

```rust
// 在 Op 枚举中添加

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Op {
    // ... 现有操作 ...

    /// 设置 Plan Mode 状态
    SetPlanMode {
        /// 是否激活
        active: bool,
        /// 计划文件路径（进入时设置）
        plan_file_path: Option<String>,
    },

    /// Plan Mode 审批响应
    PlanModeApproval {
        /// 是否批准
        approved: bool,
    },
}
```

### 8.3 更新事件处理

所有接收 `EventMsg` 的地方需要处理新事件类型：

**文件**: `mcp-server/src/codex_tool_runner.rs` (修改)
**文件**: `exec/src/event_processor_with_human_output.rs` (修改)
**文件**: `tui/src/chatwidget.rs` (修改)

```rust
// 示例：在 match EventMsg 中添加
match event.msg {
    // ... 现有事件处理 ...

    EventMsg::PlanModeEntered(e) => {
        // 处理 Plan Mode 进入
        tracing::info!(plan_file = %e.plan_file_path, "Entered plan mode");
    }

    EventMsg::PlanModeExitRequest(e) => {
        // 显示审批 UI
        self.show_plan_approval_ui(e.plan_content, e.plan_file_path);
    }

    EventMsg::PlanModeExited(e) => {
        // 处理退出结果
        if e.approved {
            tracing::info!("Plan approved, exiting plan mode");
        } else {
            tracing::info!("Plan rejected, continuing in plan mode");
        }
    }
}
```

---

## 9. 工具过滤机制

### 9.1 Plan Mode Tool Filter

**文件**: `core/src/tools/spec_ext.rs` (修改)

```rust
use crate::tools::names;
use std::collections::HashSet;
use std::path::Path;

impl ToolFilter {
    /// 为 Plan Mode 创建工具过滤器
    ///
    /// Plan Mode 只允许：
    /// - 读取类工具（read_file, glob_files, grep_files 等）
    /// - 子代理工具（task）
    /// - 交互工具（ask_user_question）
    /// - Plan Mode 退出工具（exit_plan_mode）
    ///
    /// 例外：
    /// - write_file/smart_edit 只允许写入计划文件
    pub fn for_plan_mode(plan_file_path: Option<&Path>) -> Self {
        // 允许的工具列表（使用常量，不硬编码）
        let allowed: HashSet<String> = [
            names::THINK,
            names::READ_FILE,
            names::LIST_DIR,
            names::GLOB_FILES,
            names::GREP_FILES,
            names::WEB_FETCH,
            names::WEB_SEARCH,
            names::TASK,
            names::ASK_USER_QUESTION,
            names::EXIT_PLAN_MODE,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        // 阻止的工具列表
        let blocked: HashSet<String> = [
            names::SHELL,
            names::SHELL_COMMAND,
            names::APPLY_PATCH,
            names::SMART_EDIT,
            names::WRITE_FILE,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            allowed_tools: Some(allowed),
            blocked_tools: blocked,
            plan_file_path: plan_file_path.map(|p| p.to_path_buf()),
        }
    }

    /// 检查工具是否在 Plan Mode 中允许
    ///
    /// 特殊处理：write_file 和 smart_edit 只允许操作计划文件
    pub fn is_allowed_in_plan_mode(
        &self,
        tool_name: &str,
        target_path: Option<&Path>,
    ) -> bool {
        // 检查是否是写入工具
        if tool_name == names::WRITE_FILE || tool_name == names::SMART_EDIT {
            // 只允许写入计划文件
            return match (&self.plan_file_path, target_path) {
                (Some(plan_path), Some(target)) => plan_path == target,
                _ => false,
            };
        }

        // 其他工具使用标准过滤逻辑
        self.is_allowed(tool_name)
    }
}

// 扩展 ToolFilter 结构
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolFilter {
    pub allowed_tools: Option<HashSet<String>>,
    pub blocked_tools: HashSet<String>,

    // 新增：Plan Mode 计划文件路径
    pub plan_file_path: Option<PathBuf>,
}
```

### 9.2 工具执行时的过滤集成

**文件**: `core/src/tools/registry.rs` (修改)

```rust
impl ToolRegistry {
    /// 执行工具调用
    pub async fn execute(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let tool_name = &invocation.name;

        // 获取 Plan Mode 状态
        let stores = get_or_create_stores(invocation.session.conversation_id);
        let plan_mode_state = stores.get_plan_mode_state();

        // 如果在 Plan Mode 中，应用过滤
        if plan_mode_state.is_active {
            let filter = ToolFilter::for_plan_mode(
                plan_mode_state.plan_file_path.as_deref()
            );

            // 获取目标路径（如果是文件操作工具）
            let target_path = self.extract_target_path(&invocation);

            if !filter.is_allowed_in_plan_mode(tool_name, target_path.as_deref()) {
                return Err(FunctionCallError::ToolNotAllowed(format!(
                    "Tool '{}' is not allowed in plan mode. \
                     Only read-only tools and the plan file can be used.",
                    tool_name
                )));
            }
        }

        // 正常执行工具
        // ... 现有执行逻辑 ...
    }

    /// 从调用中提取目标文件路径
    fn extract_target_path(&self, invocation: &ToolInvocation) -> Option<PathBuf> {
        // 解析工具参数，提取 file_path 或 path 字段
        match &invocation.payload {
            ToolPayload::Function { arguments } => {
                let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
                args.get("file_path")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
            }
            _ => None,
        }
    }
}
```

---

## 10. System Reminder 集成

### 10.1 传递 Plan Mode 状态到 GeneratorContext

**文件**: `core/src/codex_ext.rs` (修改)

```rust
use crate::plan_mode::{PlanModeState, plan_file_exists};

/// 注入 System Reminder
pub async fn inject_system_reminders(
    // ... 现有参数 ...
) -> Result<Vec<SystemReminder>, CodexErr> {
    // 获取 stores
    let stores = get_or_create_stores(session.conversation_id);

    // 获取 Plan Mode 状态
    let plan_mode_state = stores.get_plan_mode_state();

    // 检查是否是 re-entry
    let is_plan_reentry = plan_mode_state.is_active && plan_mode_state.is_reentry();

    // 如果是 re-entry，发送后清除标志
    if is_plan_reentry {
        stores.plan_mode.write().unwrap().clear_reentry();
    }

    // 获取计划文件路径字符串
    let plan_file_path_str = plan_mode_state.plan_file_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    // 构建 GeneratorContext
    let ctx = GeneratorContext {
        turn_number,
        is_main_agent,
        has_user_input,
        cwd,
        agent_id,
        file_tracker,

        // Plan Mode 相关字段
        is_plan_mode: plan_mode_state.is_active,
        plan_file_path: plan_file_path_str.as_deref(),
        is_plan_reentry,

        plan_state,
        background_tasks,
        critical_instruction,
        diagnostics_store,
        lsp_diagnostics_min_severity,
    };

    // 生成 reminders
    orchestrator.generate(&ctx).await
}
```

### 10.2 现有 PlanModeGenerator 无需修改

`PlanModeGenerator`（位于 `core/src/system_reminder/attachments/plan_mode.rs`）已经实现了所有必要的逻辑：

- 当 `ctx.is_plan_mode == true` 时生成 Plan Mode 指令
- 当 `ctx.is_plan_reentry == true` 时包含 re-entry 指导
- 根据 `ctx.plan_file_path` 是否存在生成不同的提示

---

## 11. 完整数据流

### 11.1 进入 Plan Mode

```
┌──────────────────────────────────────────────────────────────────────┐
│                         进入 Plan Mode 流程                           │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  用户输入: /plan                                                      │
│      │                                                               │
│      ▼                                                               │
│  TUI: 解析 SlashCommand::Plan                                         │
│      │                                                               │
│      ▼                                                               │
│  TUI: enter_plan_mode()                                              │
│      │                                                               │
│      ├─► 获取 SubagentStores                                          │
│      │                                                               │
│      ├─► 调用 stores.enter_plan_mode(conversation_id)                 │
│      │      │                                                        │
│      │      ├─► 生成文件名: {conv_id}_{timestamp}.md                   │
│      │      ├─► 设置 is_active = true                                 │
│      │      ├─► 设置 plan_file_path                                   │
│      │      └─► 返回 plan_file_path                                   │
│      │                                                               │
│      ├─► 发送 Op::SetPlanMode { active: true, plan_file_path }        │
│      │                                                               │
│      └─► 显示确认消息                                                  │
│                                                                      │
│  Session: 处理 Op::SetPlanMode                                        │
│      │                                                               │
│      └─► 发送 EventMsg::PlanModeEntered                               │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 11.2 Plan Mode 中的工具调用

```
┌──────────────────────────────────────────────────────────────────────┐
│                      Plan Mode 工具调用流程                           │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LLM: 调用工具 (例如 read_file, shell)                                 │
│      │                                                               │
│      ▼                                                               │
│  ToolRegistry::execute()                                             │
│      │                                                               │
│      ├─► 获取 PlanModeState                                           │
│      │                                                               │
│      ├─► 检查 is_active?                                              │
│      │      │                                                        │
│      │      ├─► false: 正常执行                                       │
│      │      │                                                        │
│      │      └─► true: 应用 Plan Mode 过滤                             │
│      │             │                                                 │
│      │             ├─► ToolFilter::for_plan_mode()                   │
│      │             │                                                 │
│      │             ├─► 提取 target_path（如果是文件操作）               │
│      │             │                                                 │
│      │             └─► is_allowed_in_plan_mode()?                    │
│      │                    │                                          │
│      │                    ├─► true: 继续执行                          │
│      │                    │                                          │
│      │                    └─► false: 返回错误                         │
│      │                          "Tool not allowed in plan mode"      │
│      │                                                               │
│      └─► 执行工具并返回结果                                            │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 11.3 退出 Plan Mode

```
┌──────────────────────────────────────────────────────────────────────┐
│                         退出 Plan Mode 流程                           │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LLM: 调用 exit_plan_mode 工具                                        │
│      │                                                               │
│      ▼                                                               │
│  ExitPlanModeHandler::handle()                                       │
│      │                                                               │
│      ├─► 检查 is_plan_mode_active?                                    │
│      │      └─► false: 返回错误                                       │
│      │                                                               │
│      ├─► 获取 plan_file_path                                          │
│      │      └─► None: 返回错误                                        │
│      │                                                               │
│      ├─► 检查计划文件是否存在                                          │
│      │      └─► false: 返回错误 "Please write plan first"             │
│      │                                                               │
│      ├─► 读取计划文件内容                                              │
│      │                                                               │
│      ├─► 发送 EventMsg::PlanModeExitRequest                           │
│      │      │                                                        │
│      │      └─► { plan_content, plan_file_path }                     │
│      │                                                               │
│      └─► 返回成功（等待审批）                                          │
│                                                                      │
│  TUI: 处理 PlanModeExitRequest                                        │
│      │                                                               │
│      ├─► 显示 PlanApprovalWidget                                      │
│      │      │                                                        │
│      │      ├─► 显示计划内容                                          │
│      │      └─► [Enter] 批准  [Esc] 拒绝                              │
│      │                                                               │
│      └─► 用户操作                                                     │
│             │                                                        │
│             ▼                                                        │
│  TUI: handle_plan_approval(approved)                                 │
│      │                                                               │
│      ├─► stores.exit_plan_mode(approved)                             │
│      │      │                                                        │
│      │      ├─► is_active = false                                    │
│      │      └─► if approved: has_exited = true                       │
│      │                                                               │
│      ├─► 发送 EventMsg::PlanModeExited { approved }                   │
│      │                                                               │
│      └─► 显示结果消息                                                  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 11.4 Re-entry 检测

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Re-entry 检测流程                            │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  前提条件:                                                            │
│  - 之前退出过 Plan Mode (has_exited = true)                           │
│  - 旧的计划文件仍然存在                                                │
│                                                                      │
│  用户再次输入: /plan                                                   │
│      │                                                               │
│      ▼                                                               │
│  TUI: enter_plan_mode()                                              │
│      │                                                               │
│      ├─► stores.enter_plan_mode()                                    │
│      │      └─► 保留 has_exited = true（不清除）                       │
│      │                                                               │
│      └─► ...                                                         │
│                                                                      │
│  inject_system_reminders():                                          │
│      │                                                               │
│      ├─► plan_mode_state.is_reentry()?                               │
│      │      │                                                        │
│      │      ├─► has_exited == true?                                  │
│      │      └─► plan_file_path.exists()?                             │
│      │             │                                                 │
│      │             └─► true: is_plan_reentry = true                  │
│      │                                                               │
│      ├─► 如果 is_plan_reentry:                                        │
│      │      └─► stores.plan_mode.clear_reentry()                     │
│      │                                                               │
│      └─► GeneratorContext { is_plan_reentry: true, ... }             │
│                                                                      │
│  PlanModeGenerator::generate():                                      │
│      │                                                               │
│      └─► 如果 is_plan_reentry:                                        │
│             └─► 包含 re-entry 指导:                                   │
│                   "You are returning to plan mode..."                │
│                   "1. Read existing plan file"                       │
│                   "2. Evaluate user's request"                       │
│                   "3. Decide: new task or continuation"              │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 12. 文件修改清单

### 12.1 新建文件

| 文件路径 | 描述 |
|----------|------|
| `core/src/plan_mode/mod.rs` | Plan Mode 模块主文件 |
| `core/src/plan_mode/state.rs` | PlanModeState 结构定义 |
| `core/src/plan_mode/file_management.rs` | 计划文件管理函数 |
| `core/src/plan_mode/tests.rs` | 单元测试 |
| `core/src/tools/ext/plan_mode.rs` | exit_plan_mode 工具实现 |
| `tui/src/plan_mode_approval.rs` | Plan Mode 审批 UI 组件 |

### 12.2 修改文件

| 文件路径 | 修改内容 |
|----------|----------|
| `core/src/lib.rs` | 导出 `plan_mode` 模块 |
| `core/src/subagent/stores.rs` | 添加 `plan_mode: Arc<RwLock<PlanModeState>>` 和辅助方法 |
| `core/src/tools/names.rs` | 添加 `EXIT_PLAN_MODE`, `ASK_USER_QUESTION` 常量 |
| `core/src/tools/spec_ext.rs` | 注册 exit_plan_mode 工具，添加 `for_plan_mode()` |
| `core/src/tools/ext/mod.rs` | 导出 `plan_mode` 模块 |
| `core/src/tools/registry.rs` | 添加 Plan Mode 工具过滤逻辑 |
| `core/src/codex_ext.rs` | 传递 Plan Mode 状态到 GeneratorContext |
| `protocol/src/protocol.rs` | 添加 PlanMode 相关事件和操作类型 |
| `tui/src/slash_command.rs` | 添加 `Plan` 命令 |
| `tui2/src/slash_command.rs` | 添加 `Plan` 命令 |
| `tui/src/app.rs` | 添加 `/plan` 命令处理和审批处理 |
| `mcp-server/src/codex_tool_runner.rs` | 处理新事件类型 |
| `exec/src/event_processor_with_human_output.rs` | 处理新事件类型 |
| `tui/src/chatwidget.rs` | 处理新事件类型 |

---

## 13. 实现步骤

### Phase 1: 核心状态和存储 (预计 3 个任务)

1. **创建 plan_mode 模块**
   - 文件: `core/src/plan_mode/mod.rs`, `state.rs`, `file_management.rs`
   - 内容: PlanModeState, 文件名生成, 文件读写
   - 测试: 单元测试

2. **集成到 SubagentStores**
   - 文件: `core/src/subagent/stores.rs`
   - 内容: 添加 `plan_mode` 字段和辅助方法
   - 测试: 验证状态管理

3. **导出模块**
   - 文件: `core/src/lib.rs`
   - 内容: `pub mod plan_mode`

### Phase 2: 进入机制 - /plan 命令 (预计 4 个任务)

4. **添加 Slash Command**
   - 文件: `tui/src/slash_command.rs`, `tui2/src/slash_command.rs`
   - 内容: `Plan` 变体

5. **添加 Protocol 事件**
   - 文件: `protocol/src/protocol.rs`
   - 内容: `PlanModeEntered`, `PlanModeExitRequest`, `PlanModeExited`
   - 内容: `Op::SetPlanMode`, `Op::PlanModeApproval`

6. **实现命令处理**
   - 文件: `tui/src/app.rs`
   - 内容: `enter_plan_mode()` 函数

7. **更新事件处理**
   - 文件: `mcp-server/...`, `exec/...`, `tui/...`
   - 内容: 处理新事件类型的 match arms

### Phase 3: 工具过滤 (预计 3 个任务)

8. **扩展 ToolFilter**
   - 文件: `core/src/tools/spec_ext.rs`
   - 内容: `for_plan_mode()`, `is_allowed_in_plan_mode()`

9. **集成过滤逻辑**
   - 文件: `core/src/tools/registry.rs`
   - 内容: 在 execute() 中检查 Plan Mode

10. **测试过滤**
    - 验证读取工具允许
    - 验证写入工具阻止（除计划文件外）

### Phase 4: 退出机制 (预计 4 个任务)

11. **添加工具名称常量**
    - 文件: `core/src/tools/names.rs`
    - 内容: `EXIT_PLAN_MODE`, `ASK_USER_QUESTION`

12. **实现 exit_plan_mode 工具**
    - 文件: `core/src/tools/ext/plan_mode.rs`
    - 内容: `ExitPlanModeHandler`, `create_exit_plan_mode_tool()`

13. **注册工具**
    - 文件: `core/src/tools/spec_ext.rs`
    - 内容: `register_exit_plan_mode()`

14. **实现审批 UI**
    - 文件: `tui/src/plan_mode_approval.rs`
    - 内容: `PlanApprovalWidget`
    - 文件: `tui/src/app.rs`
    - 内容: `handle_plan_approval()`

### Phase 5: System Reminder 集成 (预计 2 个任务)

15. **传递状态到 GeneratorContext**
    - 文件: `core/src/codex_ext.rs`
    - 内容: 读取 PlanModeState，设置 `is_plan_mode`, `plan_file_path`, `is_plan_reentry`

16. **测试 re-entry 检测**
    - 流程: 进入 → 退出 → 再次进入
    - 验证: re-entry 提示正确生成

---

## 14. 测试策略

### 14.1 单元测试

**文件**: `core/src/plan_mode/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plan_mode_state_default() {
        let state = PlanModeState::new();
        assert!(!state.is_active);
        assert!(state.plan_file_path.is_none());
        assert!(!state.has_exited);
    }

    #[test]
    fn test_plan_mode_enter() {
        let mut state = PlanModeState::new();
        let conv_id = ConversationId::new();

        let path = state.enter(conv_id);

        assert!(state.is_active);
        assert!(state.plan_file_path.is_some());
        assert!(path.to_string_lossy().contains(&conv_id.to_string()));
    }

    #[test]
    fn test_plan_mode_exit_approved() {
        let mut state = PlanModeState::new();
        let conv_id = ConversationId::new();
        state.enter(conv_id);

        state.exit(true);

        assert!(!state.is_active);
        assert!(state.has_exited);
    }

    #[test]
    fn test_plan_mode_exit_rejected() {
        let mut state = PlanModeState::new();
        let conv_id = ConversationId::new();
        state.enter(conv_id);

        state.exit(false);

        assert!(!state.is_active);
        assert!(!state.has_exited);
    }

    #[test]
    fn test_is_reentry() {
        let temp = tempdir().unwrap();
        let mut state = PlanModeState::new();

        // 首次进入 - 不是 re-entry
        assert!(!state.is_reentry());

        // 模拟进入、退出、文件存在
        state.has_exited = true;
        let plan_path = temp.path().join("test_plan.md");
        std::fs::write(&plan_path, "test").unwrap();
        state.plan_file_path = Some(plan_path);

        // 现在是 re-entry
        assert!(state.is_reentry());
    }

    #[test]
    fn test_generate_plan_filename() {
        let conv_id = ConversationId::new();
        let filename = generate_plan_filename(&conv_id);

        assert!(filename.contains(&conv_id.to_string()));
        assert!(filename.ends_with(".md"));
        // 格式: {conv_id}_{YYYYMMDD_HHMMSS}.md
        assert!(filename.contains("_"));
    }
}
```

### 14.2 工具测试

**文件**: `core/src/tools/ext/plan_mode.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_plan_mode_tool_spec() {
        let tool = create_exit_plan_mode_tool();
        match tool {
            ToolSpec::Function(func) => {
                assert_eq!(func.name, "exit_plan_mode");
                assert!(!func.strict);
            }
            _ => panic!("Expected Function tool spec"),
        }
    }

    #[tokio::test]
    async fn test_exit_plan_mode_not_in_plan_mode() {
        // 测试不在 Plan Mode 中调用 exit_plan_mode 的错误处理
        // ...
    }

    #[tokio::test]
    async fn test_exit_plan_mode_no_plan_file() {
        // 测试计划文件不存在时的错误处理
        // ...
    }
}
```

### 14.3 工具过滤测试

```rust
#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn test_plan_mode_filter_allows_read_tools() {
        let filter = ToolFilter::for_plan_mode(None);

        assert!(filter.is_allowed_in_plan_mode(names::READ_FILE, None));
        assert!(filter.is_allowed_in_plan_mode(names::GLOB_FILES, None));
        assert!(filter.is_allowed_in_plan_mode(names::GREP_FILES, None));
        assert!(filter.is_allowed_in_plan_mode(names::THINK, None));
    }

    #[test]
    fn test_plan_mode_filter_blocks_write_tools() {
        let filter = ToolFilter::for_plan_mode(None);

        assert!(!filter.is_allowed_in_plan_mode(names::SHELL, None));
        assert!(!filter.is_allowed_in_plan_mode(names::WRITE_FILE, None));
        assert!(!filter.is_allowed_in_plan_mode(names::APPLY_PATCH, None));
    }

    #[test]
    fn test_plan_mode_filter_allows_plan_file_write() {
        let plan_path = PathBuf::from("/tmp/test_plan.md");
        let filter = ToolFilter::for_plan_mode(Some(&plan_path));

        // 允许写入计划文件
        assert!(filter.is_allowed_in_plan_mode(
            names::WRITE_FILE,
            Some(&plan_path)
        ));

        // 不允许写入其他文件
        let other_path = PathBuf::from("/tmp/other.txt");
        assert!(!filter.is_allowed_in_plan_mode(
            names::WRITE_FILE,
            Some(&other_path)
        ));
    }
}
```

### 14.4 集成测试

```rust
// 文件: core/tests/plan_mode_integration.rs

#[tokio::test]
async fn test_plan_mode_full_flow() {
    // 1. 创建测试会话
    // 2. 进入 Plan Mode
    // 3. 验证只读工具可用
    // 4. 验证写入工具被阻止（除计划文件）
    // 5. 写入计划文件
    // 6. 调用 exit_plan_mode
    // 7. 模拟用户批准
    // 8. 验证退出 Plan Mode
}

#[tokio::test]
async fn test_plan_mode_reentry() {
    // 1. 进入 Plan Mode
    // 2. 写入计划
    // 3. 退出（批准）
    // 4. 再次进入 Plan Mode
    // 5. 验证 re-entry 提示生成
}
```

---

## 15. 附录

### 15.1 与 Claude Code 的差异

| 方面 | Claude Code | Codex |
|------|-------------|-------|
| 文件命名 | `{adjective}-{action}-{noun}.md` | `{conv_id}_{timestamp}.md` |
| 进入方式 | EnterPlanMode 工具 + 用户确认 | 仅 `/plan` 命令 |
| 退出模式 | 三种模式选择 | 简单批准/拒绝 |
| 工具名称 | 硬编码字符串 | `names::*` 常量 |
| Feature Flag | 无 | 无 |

### 15.2 常用命令

```bash
# 进入 Plan Mode
/plan

# Plan Mode 中可用的工具
- read_file      # 读取文件
- glob_files     # 搜索文件
- grep_files     # 搜索内容
- list_dir       # 列出目录
- think          # 思考
- task           # 启动子代理
- web_fetch      # 获取网页
- web_search     # 搜索网络
- write_file     # 只能写入计划文件
- exit_plan_mode # 退出 Plan Mode

# 退出 Plan Mode
# LLM 调用 exit_plan_mode 工具后，用户在 UI 中批准/拒绝
```

### 15.3 依赖项

**文件**: `core/Cargo.toml` (修改)

```toml
[dependencies]
# ... 现有依赖 ...
chrono = { version = "0.4", features = ["serde"] }  # 用于时间戳生成
```

### 15.4 错误类型扩展

**文件**: `core/src/tools/error.rs` 或相关文件 (修改)

```rust
/// 工具调用错误
#[derive(Debug, thiserror::Error)]
pub enum FunctionCallError {
    // ... 现有变体 ...

    /// 工具在当前模式下不允许使用
    #[error("Tool not allowed: {0}")]
    ToolNotAllowed(String),
}
```

### 15.5 Session 处理 Op::SetPlanMode

**文件**: `core/src/codex.rs` 或操作处理文件 (修改)

```rust
// 在处理 Op 的地方添加

async fn handle_op(&mut self, op: Op) -> Result<(), CodexErr> {
    match op {
        // ... 现有处理 ...

        Op::SetPlanMode { active, plan_file_path } => {
            // 获取 stores
            let stores = get_or_create_stores(self.conversation_id);

            if active {
                // 进入 Plan Mode
                if let Some(path) = plan_file_path {
                    // TUI 已经设置了状态，这里只需发送事件确认
                    self.send_event(EventMsg::PlanModeEntered(PlanModeEnteredEvent {
                        plan_file_path: path,
                    })).await?;
                }
            } else {
                // 退出 Plan Mode（通常由 TUI 在审批后直接调用 stores.exit_plan_mode）
            }
        }

        Op::PlanModeApproval { approved } => {
            // 处理审批响应
            let stores = get_or_create_stores(self.conversation_id);
            stores.exit_plan_mode(approved);

            self.send_event(EventMsg::PlanModeExited(PlanModeExitedEvent {
                approved,
            })).await?;

            // 如果批准，通知 LLM 可以开始实现
            if approved {
                // 可选：注入一条系统消息告知 LLM 计划已批准
            }
        }
    }
    Ok(())
}
```

### 15.6 审批流程详细设计

**问题**: `exit_plan_mode` 工具是异步的，发送审批请求后直接返回，LLM 不会等待。

**解决方案**: 使用"请求-响应"模式

```
┌──────────────────────────────────────────────────────────────────────┐
│                        审批流程详细设计                               │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  1. LLM 调用 exit_plan_mode                                          │
│      │                                                               │
│      ▼                                                               │
│  2. ExitPlanModeHandler:                                             │
│      ├─► 读取计划文件                                                 │
│      ├─► 发送 PlanModeExitRequest 事件                               │
│      └─► 返回 "Waiting for user approval" 消息                       │
│                                                                      │
│  3. TUI 收到事件:                                                     │
│      ├─► 显示审批 UI                                                  │
│      └─► **暂停接收新的 LLM 输出**（重要！）                          │
│                                                                      │
│  4. 用户操作:                                                         │
│      ├─► [Enter] 批准                                                 │
│      └─► [Esc] 拒绝                                                   │
│                                                                      │
│  5. TUI 处理审批结果:                                                 │
│      ├─► 调用 stores.exit_plan_mode(approved)                        │
│      ├─► 发送 Op::PlanModeApproval { approved }                      │
│      └─► **恢复接收 LLM 输出**                                        │
│                                                                      │
│  6. Session 处理 Op::PlanModeApproval:                               │
│      ├─► 发送 PlanModeExited 事件                                     │
│      └─► 如果批准：发送用户消息触发新一轮对话                          │
│             "Plan approved. You can now start implementation."       │
│                                                                      │
│  7. LLM 收到消息后开始实现                                            │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**关键实现**: 审批后自动发送用户消息

```rust
// 在 Session 或 TUI 中
async fn handle_plan_approval(&mut self, approved: bool) -> Result<(), Error> {
    let stores = get_or_create_stores(self.session.conversation_id);
    stores.exit_plan_mode(approved);

    if approved {
        // 自动发送用户消息触发新一轮对话
        self.session.submit(Op::UserInput {
            items: vec![UserInput::Text {
                content: "Plan approved. You can now start implementing the plan.".to_string(),
            }],
        }).await?;
    } else {
        // 拒绝时发送消息让 LLM 继续规划
        self.session.submit(Op::UserInput {
            items: vec![UserInput::Text {
                content: "Plan rejected. Please continue refining the plan.".to_string(),
            }],
        }).await?;
    }

    Ok(())
}
```

### 15.7 TUI Plan Mode 状态指示器

**文件**: `tui/src/status_bar.rs` 或相应文件 (修改)

```rust
/// 渲染状态栏
fn render_status_bar(&self, area: Rect, buf: &mut Buffer) {
    let stores = get_or_create_stores(self.session.conversation_id);
    let plan_mode_active = stores.is_plan_mode_active();

    let status = if plan_mode_active {
        " PLAN MODE ".on_yellow().black()
    } else {
        " NORMAL ".on_green().black()
    };

    // 渲染状态
    Paragraph::new(status).render(area, buf);
}
```

**在输入框上方显示 Plan Mode 提示**:

```rust
fn render_input_area(&self, area: Rect, buf: &mut Buffer) {
    let stores = get_or_create_stores(self.session.conversation_id);

    if stores.is_plan_mode_active() {
        let plan_state = stores.get_plan_mode_state();
        let hint = format!(
            "📋 Plan Mode | File: {} | Use read-only tools, then call exit_plan_mode",
            plan_state.plan_file_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
                .unwrap_or_default()
        );
        Paragraph::new(hint)
            .style(Style::default().fg(Color::Yellow))
            .render(hint_area, buf);
    }
}
```

### 15.8 参考资料

- Claude Code Plan Mode 分析: `docs/claudecode_plan_mode.md`
- 现有 PlanModeGenerator: `core/src/system_reminder/attachments/plan_mode.rs`
- 现有子代理定义: `core/src/subagent/definition/builtin.rs`
- 工具过滤: `core/src/tools/spec_ext.rs`
