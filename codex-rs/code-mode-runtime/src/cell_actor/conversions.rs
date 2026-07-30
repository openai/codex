use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::ToolDefinition;
use codex_protocol::ToolName;

use crate::session_runtime::CreateCellRequest as CellRequest;
use crate::session_runtime::ImageDetail as CellImageDetail;
use crate::session_runtime::OutputItem as CellOutputItem;
use crate::session_runtime::ToolKind as CellToolKind;

pub(super) fn runtime_request(request: CellRequest) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: request.tool_call_id,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(|definition| ToolDefinition {
                name: definition.name,
                tool_name: ToolName {
                    name: definition.tool_name.name,
                    namespace: definition.tool_name.namespace,
                },
                description: definition.description,
                kind: match definition.kind {
                    CellToolKind::Function => CodeModeToolKind::Function,
                    CellToolKind::Freeform => CodeModeToolKind::Freeform,
                },
                input_schema: None,
                output_schema: None,
            })
            .collect(),
        source: request.source,
        yield_time_ms: None,
        max_output_tokens: None,
    }
}

pub(super) fn cell_tool_kind(kind: CodeModeToolKind) -> CellToolKind {
    match kind {
        CodeModeToolKind::Function => CellToolKind::Function,
        CodeModeToolKind::Freeform => CellToolKind::Freeform,
    }
}

pub(super) fn output_item(item: FunctionCallOutputContentItem) -> CellOutputItem {
    match item {
        FunctionCallOutputContentItem::InputText { text } => CellOutputItem::Text { text },
        FunctionCallOutputContentItem::InputImage { image_url, detail } => CellOutputItem::Image {
            image_url,
            detail: detail.map(|detail| match detail {
                ImageDetail::Auto => CellImageDetail::Auto,
                ImageDetail::Low => CellImageDetail::Low,
                ImageDetail::High => CellImageDetail::High,
                ImageDetail::Original => CellImageDetail::Original,
            }),
        },
        FunctionCallOutputContentItem::InputAudio { audio_url } => {
            CellOutputItem::Audio { audio_url }
        }
    }
}
