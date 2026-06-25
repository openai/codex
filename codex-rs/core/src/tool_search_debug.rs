#[derive(Debug, Clone, PartialEq)]
pub struct ToolSearchInspection {
    pub indexed_tool_count: usize,
    pub matching_tool_count: usize,
    pub requested_limit: usize,
    pub effective_limit: usize,
    pub top_k_truncated: bool,
    pub results: Vec<ToolSearchInspectionResult>,
    pub output_tools: Vec<ToolSearchInspectionOutputTool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSearchInspectionResult {
    pub rank: usize,
    pub index: usize,
    pub score: Option<f32>,
    pub source: Option<ToolSearchInspectionSource>,
    pub tools: Vec<ToolSearchInspectionTool>,
    pub searchable_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearchInspectionSource {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearchInspectionTool {
    pub namespace: Option<String>,
    pub name: String,
    pub canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSearchInspectionOutputTool {
    pub namespace: Option<String>,
    pub tool_names: Vec<String>,
}
