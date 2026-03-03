#[derive(Debug, Clone, Serialize)]
pub struct PresentationArtifactResponse {
    pub artifact_id: String,
    pub action: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_actions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exported_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_snapshot: Option<ArtifactSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_list: Option<Vec<SlideListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_list: Option<Vec<LayoutListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder_list: Option<Vec<PlaceholderListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspect_ndjson: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_record: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proto_json: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_slide_index: Option<usize>,
}

impl PresentationArtifactResponse {
    fn new(
        artifact_id: String,
        action: String,
        summary: String,
        artifact_snapshot: ArtifactSnapshot,
    ) -> Self {
        Self {
            artifact_id,
            action,
            summary,
            executed_actions: None,
            exported_paths: Vec::new(),
            artifact_snapshot: Some(artifact_snapshot),
            slide_list: None,
            layout_list: None,
            placeholder_list: None,
            theme: None,
            inspect_ndjson: None,
            resolved_record: None,
            proto_json: None,
            patch: None,
            active_slide_index: None,
        }
    }
}

fn response_for_document_state(
    artifact_id: String,
    action: String,
    summary: String,
    document: Option<&PresentationDocument>,
) -> PresentationArtifactResponse {
    PresentationArtifactResponse {
        artifact_id,
        action,
        summary,
        executed_actions: None,
        exported_paths: Vec::new(),
        artifact_snapshot: document.map(snapshot_for_document),
        slide_list: None,
        layout_list: None,
        placeholder_list: None,
        theme: document.map(PresentationDocument::theme_snapshot),
        inspect_ndjson: None,
        resolved_record: None,
        proto_json: None,
        patch: None,
        active_slide_index: document.and_then(|current| current.active_slide_index),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactSnapshot {
    pub slide_count: usize,
    pub slides: Vec<SlideSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideSnapshot {
    pub slide_id: String,
    pub index: usize,
    pub element_ids: Vec<String>,
    pub element_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideListEntry {
    pub slide_id: String,
    pub index: usize,
    pub is_active: bool,
    pub notes: Option<String>,
    pub notes_visible: bool,
    pub background_fill: Option<String>,
    pub layout_id: Option<String>,
    pub element_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutListEntry {
    pub layout_id: String,
    pub name: String,
    pub kind: String,
    pub parent_layout_id: Option<String>,
    pub placeholder_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderListEntry {
    pub scope: String,
    pub source_layout_id: Option<String>,
    pub slide_index: Option<usize>,
    pub element_id: Option<String>,
    pub name: String,
    pub placeholder_type: String,
    pub index: Option<u32>,
    pub geometry: Option<String>,
    pub text_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeSnapshot {
    pub color_scheme: HashMap<String, String>,
    pub hex_color_map: HashMap<String, String>,
    pub major_font: Option<String>,
    pub minor_font: Option<String>,
}
