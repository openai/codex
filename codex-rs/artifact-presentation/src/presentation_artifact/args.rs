#[derive(Debug, Deserialize)]
struct CreateArgs {
    name: Option<String>,
    slide_size: Option<Value>,
    theme: Option<ThemeArgs>,
}

#[derive(Debug, Deserialize)]
struct ImportPptxArgs {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ExportPptxArgs {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ExportPreviewArgs {
    path: PathBuf,
    slide_index: Option<u32>,
    format: Option<String>,
    scale: Option<f32>,
    quality: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct AddSlideArgs {
    layout: Option<String>,
    notes: Option<String>,
    background_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateLayoutArgs {
    name: String,
    kind: Option<String>,
    parent_layout_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewOutputFormat {
    Png,
    Jpeg,
    Svg,
}

impl PreviewOutputFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Svg => "svg",
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddLayoutPlaceholderArgs {
    layout_id: String,
    name: String,
    placeholder_type: String,
    index: Option<u32>,
    text: Option<String>,
    geometry: Option<String>,
    position: Option<PositionArgs>,
}

#[derive(Debug, Deserialize)]
struct LayoutIdArgs {
    layout_id: String,
}

#[derive(Debug, Deserialize)]
struct SetSlideLayoutArgs {
    slide_index: u32,
    layout_id: String,
}

#[derive(Debug, Deserialize)]
struct UpdatePlaceholderTextArgs {
    slide_index: u32,
    name: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct NotesArgs {
    slide_index: u32,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NotesVisibilityArgs {
    slide_index: u32,
    visible: bool,
}

#[derive(Debug, Deserialize)]
struct ThemeArgs {
    color_scheme: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StyleNameArgs {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AddStyleArgs {
    name: String,
    #[serde(flatten)]
    styling: TextStylingArgs,
}

#[derive(Debug, Deserialize)]
struct InspectArgs {
    kind: Option<String>,
    include: Option<String>,
    exclude: Option<String>,
    search: Option<String>,
    target_id: Option<String>,
    target: Option<InspectTargetArgs>,
    max_chars: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct InspectTargetArgs {
    id: String,
    before_lines: Option<usize>,
    after_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ResolveArgs {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchOperationInput {
    artifact_id: Option<String>,
    action: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Deserialize)]
struct RecordPatchArgs {
    operations: Vec<PatchOperationInput>,
}

#[derive(Debug, Deserialize)]
struct ApplyPatchArgs {
    operations: Option<Vec<PatchOperationInput>>,
    patch: Option<PresentationPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresentationPatch {
    version: u32,
    artifact_id: String,
    operations: Vec<PatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatchOperation {
    action: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Default, Deserialize)]
struct InsertSlideArgs {
    index: Option<u32>,
    after_slide_index: Option<u32>,
    layout: Option<String>,
    notes: Option<String>,
    background_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlideIndexArgs {
    slide_index: u32,
}

#[derive(Debug, Deserialize)]
struct MoveSlideArgs {
    from_index: u32,
    to_index: u32,
}

#[derive(Debug, Deserialize)]
struct SetActiveSlideArgs {
    slide_index: u32,
}

#[derive(Debug, Deserialize)]
struct SetSlideBackgroundArgs {
    slide_index: u32,
    fill: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PositionArgs {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PartialPositionArgs {
    left: Option<u32>,
    top: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TextStylingArgs {
    style: Option<String>,
    font_size: Option<u32>,
    font_family: Option<String>,
    color: Option<String>,
    fill: Option<String>,
    alignment: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TextLayoutArgs {
    insets: Option<TextInsetsArgs>,
    wrap: Option<String>,
    auto_fit: Option<String>,
    vertical_alignment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TextInsetsArgs {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RichTextInput {
    Plain(String),
    Paragraphs(Vec<RichParagraphInput>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RichParagraphInput {
    Plain(String),
    Runs(Vec<RichRunInput>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RichRunInput {
    Plain(String),
    Styled(RichRunObjectInput),
}

#[derive(Debug, Clone, Deserialize)]
struct RichRunObjectInput {
    run: String,
    #[serde(default)]
    text_style: TextStylingArgs,
    link: Option<RichTextLinkInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct RichTextLinkInput {
    uri: Option<String>,
    is_external: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AddTextShapeArgs {
    slide_index: u32,
    text: String,
    position: PositionArgs,
    #[serde(flatten)]
    styling: TextStylingArgs,
    #[serde(default)]
    text_layout: TextLayoutArgs,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StrokeArgs {
    color: String,
    width: u32,
    style: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddShapeArgs {
    slide_index: u32,
    geometry: String,
    position: PositionArgs,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    text: Option<String>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    #[serde(default)]
    text_style: TextStylingArgs,
    #[serde(default)]
    text_layout: TextLayoutArgs,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ConnectorLineArgs {
    color: Option<String>,
    width: Option<u32>,
    style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointArgs {
    left: u32,
    top: u32,
}

#[derive(Debug, Deserialize)]
struct AddConnectorArgs {
    slide_index: u32,
    connector_type: String,
    start: PointArgs,
    end: PointArgs,
    line: Option<ConnectorLineArgs>,
    start_arrow: Option<String>,
    end_arrow: Option<String>,
    arrow_size: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddImageArgs {
    slide_index: u32,
    path: Option<PathBuf>,
    data_url: Option<String>,
    blob: Option<String>,
    uri: Option<String>,
    position: PositionArgs,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    lock_aspect_ratio: Option<bool>,
    alt: Option<String>,
    prompt: Option<String>,
}

impl AddImageArgs {
    fn image_source(&self) -> Result<ImageInputSource, PresentationArtifactError> {
        match (&self.path, &self.data_url, &self.blob, &self.uri) {
            (Some(path), None, None, None) => Ok(ImageInputSource::Path(path.clone())),
            (None, Some(data_url), None, None) => Ok(ImageInputSource::DataUrl(data_url.clone())),
            (None, None, Some(blob), None) => Ok(ImageInputSource::Blob(blob.clone())),
            (None, None, None, Some(uri)) => Ok(ImageInputSource::Uri(uri.clone())),
            (None, None, None, None) if self.prompt.is_some() => Ok(ImageInputSource::Placeholder),
            _ => Err(PresentationArtifactError::InvalidArgs {
                action: "add_image".to_string(),
                message:
                    "provide exactly one of `path`, `data_url`, `blob`, or `uri`, or provide `prompt` for a placeholder image"
                        .to_string(),
            }),
        }
    }
}

enum ImageInputSource {
    Path(PathBuf),
    DataUrl(String),
    Blob(String),
    Uri(String),
    Placeholder,
}

#[derive(Debug, Clone, Deserialize)]
struct ImageCropArgs {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[derive(Debug, Deserialize)]
struct AddTableArgs {
    slide_index: u32,
    position: PositionArgs,
    rows: Vec<Vec<Value>>,
    column_widths: Option<Vec<u32>>,
    row_heights: Option<Vec<u32>>,
    style: Option<String>,
    style_options: Option<TableStyleOptionsArgs>,
    borders: Option<TableBordersArgs>,
    right_to_left: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AddChartArgs {
    slide_index: u32,
    position: PositionArgs,
    chart_type: String,
    categories: Vec<String>,
    series: Vec<ChartSeriesArgs>,
    title: Option<String>,
    style_index: Option<u32>,
    has_legend: Option<bool>,
    legend_position: Option<String>,
    #[serde(default)]
    legend_text_style: TextStylingArgs,
    x_axis_title: Option<String>,
    y_axis_title: Option<String>,
    data_labels: Option<ChartDataLabelsArgs>,
    chart_fill: Option<String>,
    plot_area_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartSeriesArgs {
    name: String,
    values: Vec<f64>,
    categories: Option<Vec<String>>,
    x_values: Option<Vec<f64>>,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    marker: Option<ChartMarkerArgs>,
    data_label_overrides: Option<Vec<ChartDataLabelOverrideArgs>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChartMarkerArgs {
    symbol: Option<String>,
    size: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChartDataLabelsArgs {
    show_value: Option<bool>,
    show_category_name: Option<bool>,
    show_leader_lines: Option<bool>,
    position: Option<String>,
    #[serde(default)]
    text_style: TextStylingArgs,
}

#[derive(Debug, Clone, Deserialize)]
struct ChartDataLabelOverrideArgs {
    idx: u32,
    text: Option<String>,
    position: Option<String>,
    #[serde(default)]
    text_style: TextStylingArgs,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
}

#[derive(Debug, Deserialize)]
struct UpdateTextArgs {
    element_id: String,
    text: String,
    #[serde(default)]
    styling: TextStylingArgs,
    #[serde(default)]
    text_layout: TextLayoutArgs,
}

#[derive(Debug, Deserialize)]
struct SetRichTextArgs {
    element_id: Option<String>,
    slide_index: Option<u32>,
    row: Option<u32>,
    column: Option<u32>,
    notes: Option<bool>,
    text: RichTextInput,
    #[serde(default)]
    styling: TextStylingArgs,
    #[serde(default)]
    text_layout: TextLayoutArgs,
}

#[derive(Debug, Deserialize)]
struct FormatTextRangeArgs {
    element_id: Option<String>,
    slide_index: Option<u32>,
    row: Option<u32>,
    column: Option<u32>,
    notes: Option<bool>,
    query: Option<String>,
    occurrence: Option<usize>,
    start_cp: Option<usize>,
    length: Option<usize>,
    #[serde(default)]
    styling: TextStylingArgs,
    #[serde(default)]
    text_layout: TextLayoutArgs,
    link: Option<RichTextLinkInput>,
    spacing_before: Option<u32>,
    spacing_after: Option<u32>,
    line_spacing: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ReplaceTextArgs {
    element_id: String,
    search: String,
    replace: String,
}

#[derive(Debug, Deserialize)]
struct InsertTextAfterArgs {
    element_id: String,
    after: String,
    insert: String,
}

#[derive(Debug, Deserialize)]
struct SetHyperlinkArgs {
    element_id: String,
    link_type: Option<String>,
    url: Option<String>,
    slide_index: Option<u32>,
    address: Option<String>,
    subject: Option<String>,
    path: Option<String>,
    tooltip: Option<String>,
    highlight_click: Option<bool>,
    clear: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateShapeStyleArgs {
    element_id: String,
    position: Option<PartialPositionArgs>,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    lock_aspect_ratio: Option<bool>,
    z_order: Option<u32>,
    #[serde(default)]
    text_layout: TextLayoutArgs,
}

#[derive(Debug, Deserialize)]
struct ElementIdArgs {
    element_id: String,
}

#[derive(Debug, Deserialize)]
struct ReplaceImageArgs {
    element_id: String,
    path: Option<PathBuf>,
    data_url: Option<String>,
    blob: Option<String>,
    uri: Option<String>,
    fit: Option<ImageFitMode>,
    crop: Option<ImageCropArgs>,
    rotation: Option<i32>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    lock_aspect_ratio: Option<bool>,
    alt: Option<String>,
    prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTableCellArgs {
    element_id: String,
    row: u32,
    column: u32,
    value: Value,
    #[serde(default)]
    styling: TextStylingArgs,
    background_fill: Option<String>,
    alignment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TableStyleOptionsArgs {
    header_row: Option<bool>,
    banded_rows: Option<bool>,
    banded_columns: Option<bool>,
    first_column: Option<bool>,
    last_column: Option<bool>,
    total_row: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct TableBorderArgs {
    color: String,
    width: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct TableBordersArgs {
    outside: Option<TableBorderArgs>,
    inside: Option<TableBorderArgs>,
    top: Option<TableBorderArgs>,
    bottom: Option<TableBorderArgs>,
    left: Option<TableBorderArgs>,
    right: Option<TableBorderArgs>,
}

#[derive(Debug, Deserialize)]
struct UpdateTableStyleArgs {
    element_id: String,
    style: Option<String>,
    style_options: Option<TableStyleOptionsArgs>,
    borders: Option<TableBordersArgs>,
    right_to_left: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct StyleTableBlockArgs {
    element_id: String,
    row: u32,
    column: u32,
    row_count: u32,
    column_count: u32,
    #[serde(default)]
    styling: TextStylingArgs,
    background_fill: Option<String>,
    alignment: Option<String>,
    borders: Option<TableBordersArgs>,
}

#[derive(Debug, Deserialize)]
struct MergeTableCellsArgs {
    element_id: String,
    start_row: u32,
    end_row: u32,
    start_column: u32,
    end_column: u32,
}

#[derive(Debug, Deserialize)]
struct UpdateChartArgs {
    element_id: String,
    title: Option<String>,
    categories: Option<Vec<String>>,
    style_index: Option<u32>,
    has_legend: Option<bool>,
    legend_position: Option<String>,
    #[serde(default)]
    legend_text_style: TextStylingArgs,
    x_axis_title: Option<String>,
    y_axis_title: Option<String>,
    data_labels: Option<ChartDataLabelsArgs>,
    chart_fill: Option<String>,
    plot_area_fill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddChartSeriesArgs {
    element_id: String,
    name: String,
    values: Vec<f64>,
    categories: Option<Vec<String>>,
    x_values: Option<Vec<f64>>,
    fill: Option<String>,
    stroke: Option<StrokeArgs>,
    marker: Option<ChartMarkerArgs>,
}

#[derive(Debug, Deserialize)]
struct SetCommentAuthorArgs {
    display_name: String,
    initials: String,
    email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentPositionArgs {
    x: u32,
    y: u32,
}

#[derive(Debug, Deserialize)]
struct AddCommentThreadArgs {
    slide_index: Option<u32>,
    element_id: Option<String>,
    query: Option<String>,
    occurrence: Option<usize>,
    start_cp: Option<usize>,
    length: Option<usize>,
    text: String,
    position: Option<CommentPositionArgs>,
}

#[derive(Debug, Deserialize)]
struct AddCommentReplyArgs {
    thread_id: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct ToggleCommentReactionArgs {
    thread_id: String,
    message_id: Option<String>,
    emoji: String,
}

#[derive(Debug, Deserialize)]
struct CommentThreadIdArgs {
    thread_id: String,
}
