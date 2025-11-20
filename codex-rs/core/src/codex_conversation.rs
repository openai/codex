// 该文件封装 Codex 会话的双向通信通道与提交接口。
use crate::codex::Codex;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use std::path::PathBuf;

pub struct CodexConversation {
    codex: Codex,
    rollout_path: PathBuf,
}

/// Conduit for the bidirectional stream of messages that compose a conversation
/// in Codex.
/// 为 Codex 中构成对话的双向消息流提供通道
impl CodexConversation {
    pub(crate) fn new(codex: Codex, rollout_path: PathBuf) -> Self {
        Self {
            codex,
            rollout_path,
        }
    }

    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.codex.submit(op).await
    }

    /// Use sparingly: this is intended to be removed soon.
    /// 请谨慎使用：该接口计划尽快移除
    pub async fn submit_with_id(&self, sub: Submission) -> CodexResult<()> {
        self.codex.submit_with_id(sub).await
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        self.codex.next_event().await
    }

    pub fn rollout_path(&self) -> PathBuf {
        self.rollout_path.clone()
    }
}
