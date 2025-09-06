use codex_core::protocol::ConversationHistoryResponseEvent;
use codex_core::protocol::Event;
use codex_file_search::FileMatch;

use crate::history_cell::HistoryCell;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    CodexEvent(Event),

    /// Start a new session.
    NewSession,

    /// Request to exit the application gracefully.
    ExitRequest,

    /// Forward an `Op` to the Agent. Using an `AppEvent` for this avoids
    /// bubbling channels through layers of widgets.
    CodexOp(codex_core::protocol::Op),

    /// Kick off an asynchronous file search for the given query (text after
    /// the `@`). Previous searches may be cancelled by the app layer so there
    /// is at most one in-flight search.
    StartFileSearch(String),

    /// Result of a completed asynchronous file search. The `query` echoes the
    /// original search term so the UI can decide whether the results are
    /// still relevant.
    FileSearchResult {
        query: String,
        matches: Vec<FileMatch>,
    },

    /// Result of computing a `/diff` command.
    DiffResult(String),

    InsertHistoryCell(Box<dyn HistoryCell>),

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(ReasoningEffort),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),

    /// Forwarded conversation history snapshot from the current conversation.
    ConversationHistory(ConversationHistoryResponseEvent),
    // ===== !Modify Start: 用户提问节点选择器 事件扩展 =====
    // !Modify[owner=yoyooyooo]: 自定义事件，用于从“用户提问节点选择器”中确认回退目标。
    // !Purpose: 允许用户回到较早的某条“用户提问”节点并丢弃其后的上下文（不传给 API）。
    // !Semantics: 回退到“从最近起第 N 条用户消息”（N=1 表示最近的用户提问）。
    // !Trigger: 选择器弹窗中按 Enter 后发送。
    // !AI_GUIDANCE: 若与上游实现冲突，优先考虑共存；若无法共存，请先询问用户再变更该自定义逻辑。
    BacktrackTo(usize),
    // ===== !Modify End: 用户提问节点选择器 事件扩展 =====
}
