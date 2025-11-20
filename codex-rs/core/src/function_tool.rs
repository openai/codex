// 该文件定义函数调用错误类型及相关错误信息。
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FunctionCallError {
    #[error("{0}")]
    RespondToModel(String),
    #[error("{0}")]
    #[allow(dead_code)] // TODO(jif) fix in a follow-up PR
    // 计划在后续的 PR 中修复
    Denied(String),
    #[error("LocalShellCall without call_id or id")]
    MissingLocalShellCallId,
    #[error("Fatal error: {0}")]
    Fatal(String),
}
