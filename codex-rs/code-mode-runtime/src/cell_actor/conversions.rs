use codex_code_mode_protocol::CodeModeToolKind;

use crate::session_runtime::ToolKind as CellToolKind;

pub(super) fn cell_tool_kind(kind: CodeModeToolKind) -> CellToolKind {
    match kind {
        CodeModeToolKind::Function => CellToolKind::Function,
        CodeModeToolKind::Freeform => CellToolKind::Freeform,
    }
}
