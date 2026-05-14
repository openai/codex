use codex_tools::LoadableToolSpec;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::default_namespace_description;

#[derive(Clone)]
pub(crate) struct ToolSearchEntry {
    pub(crate) search_text: String,
    pub(crate) output: LoadableToolSpec,
}

#[derive(Clone)]
pub(crate) struct ToolSearchInfo {
    pub(crate) entry: ToolSearchEntry,
    pub(crate) source_info: Option<ToolSearchSourceInfo>,
}

impl ToolSearchInfo {
    pub(crate) fn from_spec(
        search_text: String,
        spec: ToolSpec,
        source_info: Option<ToolSearchSourceInfo>,
    ) -> Option<Self> {
        let output = match spec {
            ToolSpec::Function(mut tool) => {
                tool.defer_loading = Some(true);
                tool.output_schema = None;
                LoadableToolSpec::Function(tool)
            }
            ToolSpec::Namespace(mut namespace) => {
                if namespace.description.trim().is_empty() {
                    namespace.description = default_namespace_description(&namespace.name);
                }
                for tool in &mut namespace.tools {
                    let ResponsesApiNamespaceTool::Function(tool) = tool;
                    tool.defer_loading = Some(true);
                    tool.output_schema = None;
                }
                LoadableToolSpec::Namespace(namespace)
            }
            ToolSpec::ToolSearch { .. }
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Freeform(_) => return None,
        };

        Some(Self {
            entry: ToolSearchEntry {
                search_text,
                output,
            },
            source_info,
        })
    }
}
