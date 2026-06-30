use codex_tools::LoadableToolSpec;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSearchDebugResult {
    pub indexed_tool_count: usize,
    pub matching_tool_count: usize,
    pub requested_limit: usize,
    pub effective_limit: usize,
    pub top_k_truncated: bool,
    pub tools: Vec<LoadableToolSpec>,
    pub results: Vec<ToolSearchDebugResultEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSearchDebugResultEntry {
    pub rank: usize,
    pub index: usize,
    pub score: Option<f32>,
    pub source_name: Option<String>,
    pub source_description: Option<String>,
    pub searchable_text: String,
    pub tools: Vec<LoadableToolSpec>,
}
