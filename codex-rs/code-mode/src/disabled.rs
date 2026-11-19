//! Unavailable provider used when the code-mode host is disabled.

use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;

/// Rejects code-mode sessions when the standalone host is disabled.
#[derive(Default)]
pub struct DisabledCodeModeSessionProvider;

impl CodeModeSessionProvider for DisabledCodeModeSessionProvider {
    fn availability(&self) -> Result<(), String> {
        Err("code-mode host is disabled".to_string())
    }

    fn create_session(&self) -> CodeModeSessionProviderFuture<'_> {
        Box::pin(async { Err("code-mode host is disabled".to_string()) })
    }

    fn create_session_with_limits<'a>(
        &'a self,
        _limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        self.create_session()
    }
}
