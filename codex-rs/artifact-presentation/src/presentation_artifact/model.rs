#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ThemeState {
    color_scheme: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

impl ThemeState {
    fn resolve_color(&self, color: &str) -> Option<String> {
        let key = color.trim().to_ascii_lowercase();
        let alias = match key.as_str() {
            "background1" => "bg1",
            "background2" => "bg2",
            "text1" => "tx1",
            "text2" => "tx2",
            "dark1" => "dk1",
            "dark2" => "dk2",
            "light1" => "lt1",
            "light2" => "lt2",
            other => other,
        };
        self.color_scheme
            .get(alias)
            .or_else(|| self.color_scheme.get(&key))
            .cloned()
            .map(|value| value.trim_start_matches('#').to_uppercase())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LayoutKind {
    Layout,
    Master,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayoutDocument {
    layout_id: String,
    name: String,
    kind: LayoutKind,
    parent_layout_id: Option<String>,
    placeholders: Vec<PlaceholderDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlaceholderDefinition {
    name: String,
    placeholder_type: String,
    index: Option<u32>,
    text: Option<String>,
    geometry: ShapeGeometry,
    frame: Rect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResolvedPlaceholder {
    source_layout_id: String,
    definition: PlaceholderDefinition,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NotesState {
    text: String,
    visible: bool,
    #[serde(default)]
    rich_text: RichTextState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TextStyle {
    style_name: Option<String>,
    font_size: Option<u32>,
    font_family: Option<String>,
    color: Option<String>,
    alignment: Option<TextAlignment>,
    bold: bool,
    italic: bool,
    underline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NamedTextStyle {
    name: String,
    style: TextStyle,
    built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HyperlinkState {
    target: HyperlinkTarget,
    tooltip: Option<String>,
    highlight_click: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum HyperlinkTarget {
    Url(String),
    Slide(u32),
    FirstSlide,
    LastSlide,
    NextSlide,
    PreviousSlide,
    EndShow,
    Email {
        address: String,
        subject: Option<String>,
    },
    File(String),
}

impl HyperlinkTarget {
    fn relationship_target(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Slide(slide_index) => format!("slide{}.xml", slide_index + 1),
            Self::FirstSlide => "ppaction://hlinkshowjump?jump=firstslide".to_string(),
            Self::LastSlide => "ppaction://hlinkshowjump?jump=lastslide".to_string(),
            Self::NextSlide => "ppaction://hlinkshowjump?jump=nextslide".to_string(),
            Self::PreviousSlide => "ppaction://hlinkshowjump?jump=previousslide".to_string(),
            Self::EndShow => "ppaction://hlinkshowjump?jump=endshow".to_string(),
            Self::Email { address, subject } => {
                let mut mailto = format!("mailto:{address}");
                if let Some(subject) = subject {
                    mailto.push_str(&format!("?subject={subject}"));
                }
                mailto
            }
            Self::File(path) => format!("file:///{}", path.replace('\\', "/")),
        }
    }

    fn is_external(&self) -> bool {
        matches!(self, Self::Url(_) | Self::Email { .. } | Self::File(_))
    }
}

impl HyperlinkState {
    fn to_ppt_rs(&self, relationship_id: &str) -> PptHyperlink {
        let hyperlink = match &self.target {
            HyperlinkTarget::Url(url) => PptHyperlink::new(PptHyperlinkAction::url(url)),
            HyperlinkTarget::Slide(slide_index) => {
                PptHyperlink::new(PptHyperlinkAction::slide(slide_index + 1))
            }
            HyperlinkTarget::FirstSlide => PptHyperlink::new(PptHyperlinkAction::FirstSlide),
            HyperlinkTarget::LastSlide => PptHyperlink::new(PptHyperlinkAction::LastSlide),
            HyperlinkTarget::NextSlide => PptHyperlink::new(PptHyperlinkAction::NextSlide),
            HyperlinkTarget::PreviousSlide => PptHyperlink::new(PptHyperlinkAction::PreviousSlide),
            HyperlinkTarget::EndShow => PptHyperlink::new(PptHyperlinkAction::EndShow),
            HyperlinkTarget::Email { address, subject } => PptHyperlink::new(match subject {
                Some(subject) => PptHyperlinkAction::email_with_subject(address, subject),
                None => PptHyperlinkAction::email(address),
            }),
            HyperlinkTarget::File(path) => PptHyperlink::new(PptHyperlinkAction::file(path)),
        };
        let hyperlink = if let Some(tooltip) = &self.tooltip {
            hyperlink.with_tooltip(tooltip)
        } else {
            hyperlink
        };
        hyperlink
            .with_highlight_click(self.highlight_click)
            .with_r_id(relationship_id)
    }

    fn to_json(&self) -> Value {
        let mut record = match &self.target {
            HyperlinkTarget::Url(url) => serde_json::json!({
                "type": "url",
                "url": url,
            }),
            HyperlinkTarget::Slide(slide_index) => serde_json::json!({
                "type": "slide",
                "slideIndex": slide_index,
            }),
            HyperlinkTarget::FirstSlide => serde_json::json!({
                "type": "firstSlide",
            }),
            HyperlinkTarget::LastSlide => serde_json::json!({
                "type": "lastSlide",
            }),
            HyperlinkTarget::NextSlide => serde_json::json!({
                "type": "nextSlide",
            }),
            HyperlinkTarget::PreviousSlide => serde_json::json!({
                "type": "previousSlide",
            }),
            HyperlinkTarget::EndShow => serde_json::json!({
                "type": "endShow",
            }),
            HyperlinkTarget::Email { address, subject } => serde_json::json!({
                "type": "email",
                "address": address,
                "subject": subject,
            }),
            HyperlinkTarget::File(path) => serde_json::json!({
                "type": "file",
                "path": path,
            }),
        };
        record["tooltip"] = self
            .tooltip
            .as_ref()
            .map(|tooltip| Value::String(tooltip.clone()))
            .unwrap_or(Value::Null);
        record["highlightClick"] = Value::Bool(self.highlight_click);
        record
    }

    fn relationship_xml(&self, relationship_id: &str) -> String {
        let target_mode = if self.target.is_external() {
            r#" TargetMode="External""#
        } else {
            ""
        };
        format!(
            r#"<Relationship Id="{relationship_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}"{target_mode}/>"#,
            ppt_rs::escape_xml(&self.target.relationship_target()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RichTextState {
    #[serde(default)]
    ranges: Vec<TextRangeAnnotation>,
    #[serde(default)]
    layout: TextLayoutState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextRangeAnnotation {
    range_id: String,
    start_cp: usize,
    length: usize,
    style: TextStyle,
    hyperlink: Option<HyperlinkState>,
    spacing_before: Option<u32>,
    spacing_after: Option<u32>,
    line_spacing: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TextLayoutState {
    insets: Option<TextInsets>,
    wrap: Option<TextWrapMode>,
    auto_fit: Option<TextAutoFitMode>,
    vertical_alignment: Option<TextVerticalAlignment>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct TextInsets {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum TextWrapMode {
    Square,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum TextAutoFitMode {
    None,
    ShrinkText,
    ResizeShapeToFitText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum TextVerticalAlignment {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentAuthorProfile {
    display_name: String,
    initials: String,
    email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentPosition {
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum CommentThreadStatus {
    Active,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentMessage {
    message_id: String,
    author: CommentAuthorProfile,
    text: String,
    created_at: String,
    #[serde(default)]
    reactions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CommentTarget {
    Slide {
        slide_id: String,
    },
    Element {
        slide_id: String,
        element_id: String,
    },
    TextRange {
        slide_id: String,
        element_id: String,
        start_cp: usize,
        length: usize,
        context: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentThread {
    thread_id: String,
    target: CommentTarget,
    position: Option<CommentPosition>,
    status: CommentThreadStatus,
    messages: Vec<CommentMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableBorder {
    color: String,
    width: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TableBorders {
    outside: Option<TableBorder>,
    inside: Option<TableBorder>,
    top: Option<TableBorder>,
    bottom: Option<TableBorder>,
    left: Option<TableBorder>,
    right: Option<TableBorder>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TableStyleOptions {
    header_row: bool,
    banded_rows: bool,
    banded_columns: bool,
    first_column: bool,
    last_column: bool,
    total_row: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChartMarkerStyle {
    symbol: Option<String>,
    size: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChartDataLabels {
    show_value: bool,
    show_category_name: bool,
    show_leader_lines: bool,
    position: Option<String>,
    text_style: TextStyle,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChartLegend {
    position: Option<String>,
    text_style: TextStyle,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChartAxisSpec {
    title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ChartDataLabelOverride {
    idx: usize,
    text: Option<String>,
    position: Option<String>,
    text_style: TextStyle,
    fill: Option<String>,
    stroke: Option<StrokeStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaceholderRef {
    name: String,
    placeholder_type: String,
    index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableMergeRegion {
    start_row: usize,
    end_row: usize,
    start_column: usize,
    end_column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableCellSpec {
    text: String,
    text_style: TextStyle,
    background_fill: Option<String>,
    alignment: Option<TextAlignment>,
    #[serde(default)]
    rich_text: RichTextState,
    borders: Option<TableBorders>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PresentationDocument {
    artifact_id: String,
    name: Option<String>,
    slide_size: Rect,
    theme: ThemeState,
    custom_text_styles: HashMap<String, TextStyle>,
    layouts: Vec<LayoutDocument>,
    slides: Vec<PresentationSlide>,
    active_slide_index: Option<usize>,
    #[serde(default)]
    comment_self: Option<CommentAuthorProfile>,
    #[serde(default)]
    comment_threads: Vec<CommentThread>,
    next_slide_seq: u32,
    next_element_seq: u32,
    next_layout_seq: u32,
    next_text_range_seq: u32,
    next_comment_thread_seq: u32,
    next_comment_message_seq: u32,
}

impl PresentationDocument {
    fn new(name: Option<String>) -> Self {
        Self {
            artifact_id: format!("presentation_{}", Uuid::new_v4().simple()),
            name,
            slide_size: Rect {
                left: 0,
                top: 0,
                width: DEFAULT_SLIDE_WIDTH_POINTS,
                height: DEFAULT_SLIDE_HEIGHT_POINTS,
            },
            theme: ThemeState::default(),
            custom_text_styles: HashMap::new(),
            layouts: Vec::new(),
            slides: Vec::new(),
            active_slide_index: None,
            comment_self: None,
            comment_threads: Vec::new(),
            next_slide_seq: 1,
            next_element_seq: 1,
            next_layout_seq: 1,
            next_text_range_seq: 1,
            next_comment_thread_seq: 1,
            next_comment_message_seq: 1,
        }
    }

    fn from_ppt_rs(presentation: Presentation) -> Self {
        let mut document = Self::new(
            (!presentation.get_title().is_empty()).then(|| presentation.get_title().to_string()),
        );
        for imported_slide in presentation.slides() {
            let mut slide = PresentationSlide {
                slide_id: format!("slide_{}", document.next_slide_seq),
                notes: NotesState {
                    text: imported_slide.notes.clone().unwrap_or_default(),
                    visible: true,
                    rich_text: RichTextState::default(),
                },
                background_fill: None,
                layout_id: None,
                elements: Vec::new(),
            };
            document.next_slide_seq += 1;

            if !imported_slide.title.is_empty() {
                slide.elements.push(PresentationElement::Text(TextElement {
                    element_id: document.next_element_id(),
                    text: imported_slide.title.clone(),
                    frame: Rect {
                        left: DEFAULT_IMPORTED_TITLE_LEFT,
                        top: DEFAULT_IMPORTED_TITLE_TOP,
                        width: DEFAULT_IMPORTED_TITLE_WIDTH,
                        height: DEFAULT_IMPORTED_TITLE_HEIGHT,
                    },
                    fill: None,
                    style: TextStyle::default(),
                    hyperlink: None,
                    rich_text: RichTextState::default(),
                    placeholder: None,
                    z_order: slide.elements.len(),
                }));
            }

            if !imported_slide.content.is_empty() {
                slide.elements.push(PresentationElement::Text(TextElement {
                    element_id: document.next_element_id(),
                    text: imported_slide.content.join("\n"),
                    frame: Rect {
                        left: DEFAULT_IMPORTED_CONTENT_LEFT,
                        top: DEFAULT_IMPORTED_CONTENT_TOP,
                        width: DEFAULT_IMPORTED_CONTENT_WIDTH,
                        height: DEFAULT_IMPORTED_CONTENT_HEIGHT,
                    },
                    fill: None,
                    style: TextStyle::default(),
                    hyperlink: None,
                    rich_text: RichTextState::default(),
                    placeholder: None,
                    z_order: slide.elements.len(),
                }));
            }

            for imported_shape in &imported_slide.shapes {
                slide
                    .elements
                    .push(PresentationElement::Shape(ShapeElement {
                        element_id: document.next_element_id(),
                        geometry: ShapeGeometry::from_shape_type(imported_shape.shape_type),
                        frame: Rect::from_emu(
                            imported_shape.x,
                            imported_shape.y,
                            imported_shape.width,
                            imported_shape.height,
                        ),
                        fill: imported_shape.fill.as_ref().map(|fill| fill.color.clone()),
                        stroke: imported_shape.line.as_ref().map(|line| StrokeStyle {
                            color: line.color.clone(),
                            width: emu_to_points(line.width),
                            style: LineStyle::Solid,
                        }),
                        text: imported_shape.text.clone(),
                        text_style: TextStyle::default(),
                        hyperlink: None,
                        rich_text: imported_shape
                            .text
                            .as_ref()
                            .map(|_| RichTextState::default()),
                        placeholder: None,
                        rotation_degrees: imported_shape.rotation,
                        flip_horizontal: false,
                        flip_vertical: false,
                        z_order: slide.elements.len(),
                    }));
            }

            if let Some(imported_table) = &imported_slide.table {
                slide
                    .elements
                    .push(PresentationElement::Table(TableElement {
                        element_id: document.next_element_id(),
                        frame: Rect::from_emu(
                            imported_table.x,
                            imported_table.y,
                            imported_table.width(),
                            imported_table.height(),
                        ),
                        rows: imported_table
                            .rows
                            .iter()
                            .map(|row| {
                                row.cells
                                    .iter()
                                    .map(|text| TableCellSpec {
                                        text: text.text.clone(),
                                        text_style: TextStyle::default(),
                                        background_fill: None,
                                        alignment: None,
                                        rich_text: RichTextState::default(),
                                        borders: None,
                                    })
                                    .collect()
                            })
                            .collect(),
                        column_widths: imported_table
                            .column_widths
                            .iter()
                            .copied()
                            .map(emu_to_points)
                            .collect(),
                        row_heights: imported_table
                            .rows
                            .iter()
                            .map(|row| row.height.map_or(400_000, |height| height))
                            .map(emu_to_points)
                            .collect(),
                        style: None,
                        style_options: TableStyleOptions::default(),
                        borders: None,
                        right_to_left: false,
                        merges: Vec::new(),
                        z_order: slide.elements.len(),
                    }));
            }

            document.slides.push(slide);
        }
        document.active_slide_index = (!document.slides.is_empty()).then_some(0);
        document
    }

    fn new_slide(
        &mut self,
        notes: Option<String>,
        background_fill: Option<String>,
        action: &str,
    ) -> Result<PresentationSlide, PresentationArtifactError> {
        let normalized_fill = background_fill
            .map(|value| {
                normalize_color_with_palette(Some(&self.theme), &value, action, "background_fill")
            })
            .transpose()?;
        let slide = PresentationSlide {
            slide_id: format!("slide_{}", self.next_slide_seq),
            notes: NotesState {
                text: notes.unwrap_or_default(),
                visible: true,
                rich_text: RichTextState::default(),
            },
            background_fill: normalized_fill,
            layout_id: None,
            elements: Vec::new(),
        };
        self.next_slide_seq += 1;
        Ok(slide)
    }

    fn append_slide(&mut self, slide: PresentationSlide) -> usize {
        let index = self.slides.len();
        self.slides.push(slide);
        if self.active_slide_index.is_none() {
            self.active_slide_index = Some(index);
        }
        index
    }

    fn clone_slide(&mut self, slide: PresentationSlide) -> PresentationSlide {
        let mut clone = slide;
        clone.slide_id = format!("slide_{}", self.next_slide_seq);
        self.next_slide_seq += 1;
        for element in &mut clone.elements {
            element.set_element_id(self.next_element_id());
        }
        clone
    }

    fn next_element_id(&mut self) -> String {
        let element_id = format!("element_{}", self.next_element_seq);
        self.next_element_seq += 1;
        element_id
    }

    fn next_text_range_id(&mut self) -> String {
        let range_id = format!("range_{}", self.next_text_range_seq);
        self.next_text_range_seq += 1;
        range_id
    }

    fn next_comment_thread_id(&mut self) -> String {
        let thread_id = format!("thread_{}", self.next_comment_thread_seq);
        self.next_comment_thread_seq += 1;
        thread_id
    }

    fn next_comment_message_id(&mut self) -> String {
        let message_id = format!("message_{}", self.next_comment_message_seq);
        self.next_comment_message_seq += 1;
        message_id
    }

    fn total_element_count(&self) -> usize {
        self.slides.iter().map(|slide| slide.elements.len()).sum()
    }

    fn set_active_slide_index(
        &mut self,
        slide_index: usize,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        if slide_index >= self.slides.len() {
            return Err(index_out_of_range(action, slide_index, self.slides.len()));
        }
        self.active_slide_index = Some(slide_index);
        Ok(())
    }

    fn adjust_active_slide_for_insert(&mut self, inserted_index: usize) {
        match self.active_slide_index {
            None => self.active_slide_index = Some(inserted_index),
            Some(active_index) if inserted_index <= active_index => {
                self.active_slide_index = Some(active_index + 1);
            }
            Some(_) => {}
        }
    }

    fn adjust_active_slide_for_move(&mut self, from_index: usize, to_index: usize) {
        if let Some(active_index) = self.active_slide_index {
            self.active_slide_index = Some(if active_index == from_index {
                to_index
            } else if from_index < active_index && active_index <= to_index {
                active_index - 1
            } else if to_index <= active_index && active_index < from_index {
                active_index + 1
            } else {
                active_index
            });
        }
    }

    fn adjust_active_slide_for_delete(&mut self, deleted_index: usize) {
        self.active_slide_index = match self.active_slide_index {
            None => None,
            Some(_) if self.slides.is_empty() => None,
            Some(active_index) if active_index == deleted_index => {
                Some(deleted_index.min(self.slides.len() - 1))
            }
            Some(active_index) if deleted_index < active_index => Some(active_index - 1),
            Some(active_index) => Some(active_index),
        };
    }

    fn next_layout_id(&mut self) -> String {
        let layout_id = format!("layout_{}", self.next_layout_seq);
        self.next_layout_seq += 1;
        layout_id
    }

    fn get_layout(
        &self,
        layout_ref: &str,
        action: &str,
    ) -> Result<&LayoutDocument, PresentationArtifactError> {
        if let Some(layout) = self
            .layouts
            .iter()
            .find(|layout| layout.layout_id == layout_ref)
        {
            return Ok(layout);
        }

        let exact_name_matches = self
            .layouts
            .iter()
            .filter(|layout| layout.name == layout_ref)
            .collect::<Vec<_>>();
        if exact_name_matches.len() > 1 {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("layout name `{layout_ref}` is ambiguous"),
            });
        }
        if let Some(layout) = exact_name_matches.into_iter().next() {
            return Ok(layout);
        }

        let case_insensitive_name_matches = self
            .layouts
            .iter()
            .filter(|layout| layout.name.eq_ignore_ascii_case(layout_ref))
            .collect::<Vec<_>>();
        if case_insensitive_name_matches.len() > 1 {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("layout name `{layout_ref}` is ambiguous"),
            });
        }
        case_insensitive_name_matches
            .into_iter()
            .next()
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("unknown layout id or name `{layout_ref}`"),
            })
    }

    fn theme_snapshot(&self) -> ThemeSnapshot {
        ThemeSnapshot {
            color_scheme: self.theme.color_scheme.clone(),
            hex_color_map: self.theme.color_scheme.clone(),
            major_font: self.theme.major_font.clone(),
            minor_font: self.theme.minor_font.clone(),
        }
    }

    fn named_text_styles(&self) -> Vec<NamedTextStyle> {
        let mut styles = built_in_text_styles(&self.theme)
            .into_iter()
            .map(|(name, style)| NamedTextStyle {
                name,
                style,
                built_in: true,
            })
            .collect::<Vec<_>>();
        styles.extend(
            self.custom_text_styles
                .iter()
                .map(|(name, style)| NamedTextStyle {
                    name: name.clone(),
                    style: style.clone(),
                    built_in: false,
                }),
        );
        styles.sort_by_cached_key(|style| style.name.to_ascii_lowercase());
        styles
    }

    fn resolve_named_text_style(
        &self,
        style_name: &str,
        action: &str,
    ) -> Result<TextStyle, PresentationArtifactError> {
        let normalized_style_name = style_name.trim().to_ascii_lowercase();
        if let Some(style) = self.custom_text_styles.get(&normalized_style_name) {
            return Ok(style.clone());
        }
        built_in_text_style(&self.theme, &normalized_style_name).ok_or_else(|| {
            PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("unknown text style `{style_name}`"),
            }
        })
    }

    fn find_element_mut(
        &mut self,
        element_id: &str,
        action: &str,
    ) -> Result<&mut PresentationElement, PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(element) = slide
                .elements
                .iter_mut()
                .find(|element| element.element_id() == element_id)
            {
                return Ok(element);
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn get_slide_mut(
        &mut self,
        slide_index: u32,
        action: &str,
    ) -> Result<&mut PresentationSlide, PresentationArtifactError> {
        let index = slide_index as usize;
        if index >= self.slides.len() {
            return Err(index_out_of_range(action, index, self.slides.len()));
        }
        Ok(&mut self.slides[index])
    }

    fn remove_element(
        &mut self,
        element_id: &str,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(index) = slide
                .elements
                .iter()
                .position(|element| element.element_id() == element_id)
            {
                slide.elements.remove(index);
                resequence_z_order(slide);
                return Ok(());
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn set_z_order(
        &mut self,
        element_id: &str,
        target_index: usize,
        action: &str,
    ) -> Result<(), PresentationArtifactError> {
        let element_id = normalize_element_lookup_id(element_id);
        for slide in &mut self.slides {
            if let Some(current_index) = slide
                .elements
                .iter()
                .position(|element| element.element_id() == element_id)
            {
                let destination = target_index.min(slide.elements.len().saturating_sub(1));
                let element = slide.elements.remove(current_index);
                slide.elements.insert(destination, element);
                resequence_z_order(slide);
                return Ok(());
            }
        }
        Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("unknown element id `{element_id}`"),
        })
    }

    fn to_ppt_rs(&self) -> Presentation {
        let mut presentation = self
            .name
            .as_deref()
            .map(Presentation::with_title)
            .unwrap_or_default();
        for slide in &self.slides {
            presentation = presentation.add_slide(slide.to_ppt_rs(self.slide_size));
        }
        presentation
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PresentationSlide {
    slide_id: String,
    notes: NotesState,
    background_fill: Option<String>,
    layout_id: Option<String>,
    elements: Vec<PresentationElement>,
}

struct ImportedPicture {
    relationship_id: String,
    frame: Rect,
    crop: Option<ImageCrop>,
    alt_text: Option<String>,
    rotation_degrees: Option<i32>,
    flip_horizontal: bool,
    flip_vertical: bool,
    lock_aspect_ratio: bool,
}

fn import_pptx_images(
    path: &Path,
    document: &mut PresentationDocument,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let file =
        std::fs::File::open(path).map_err(|error| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    for slide_index in 0..document.slides.len() {
        let slide_number = slide_index + 1;
        let slide_xml_path = format!("ppt/slides/slide{slide_number}.xml");
        let Some(slide_xml) =
            zip_entry_string_if_exists(&mut archive, &slide_xml_path).map_err(|message| {
                PresentationArtifactError::ImportFailed {
                    path: path.to_path_buf(),
                    message,
                }
            })?
        else {
            continue;
        };
        let pictures = parse_imported_pictures(&slide_xml);
        if pictures.is_empty() {
            continue;
        }
        let relationships = zip_entry_string_if_exists(
            &mut archive,
            &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
        )
        .map_err(|message| PresentationArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message,
        })?
        .map(|xml| parse_slide_image_relationship_targets(&xml))
        .unwrap_or_default();
        let mut imported_images = Vec::new();
        for picture in pictures {
            let Some(target) = relationships.get(&picture.relationship_id) else {
                continue;
            };
            let media_path = resolve_zip_relative_path(&slide_xml_path, target);
            let Some(bytes) =
                zip_entry_bytes_if_exists(&mut archive, &media_path).map_err(|message| {
                    PresentationArtifactError::ImportFailed {
                        path: path.to_path_buf(),
                        message,
                    }
                })?
            else {
                continue;
            };
            let Some(filename) = Path::new(&media_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            let Ok(payload) = build_image_payload(bytes, filename, action) else {
                continue;
            };
            imported_images.push(ImageElement {
                element_id: document.next_element_id(),
                frame: picture.frame,
                payload: Some(payload),
                fit_mode: ImageFitMode::Stretch,
                crop: picture.crop,
                rotation_degrees: picture.rotation_degrees,
                flip_horizontal: picture.flip_horizontal,
                flip_vertical: picture.flip_vertical,
                lock_aspect_ratio: picture.lock_aspect_ratio,
                alt_text: picture.alt_text,
                prompt: None,
                is_placeholder: false,
                placeholder: None,
                z_order: 0,
            });
        }
        let slide = &mut document.slides[slide_index];
        for mut image in imported_images {
            image.z_order = slide.elements.len();
            slide.elements.push(PresentationElement::Image(image));
        }
    }
    Ok(())
}

fn zip_entry_string_if_exists<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Option<String>, String> {
    let Some(bytes) = zip_entry_bytes_if_exists(archive, path)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("zip entry `{path}` is not valid UTF-8: {error}"))
}

fn zip_entry_bytes_if_exists<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Option<Vec<u8>>, String> {
    match archive.by_name(path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| format!("failed to read zip entry `{path}`: {error}"))?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(format!("failed to open zip entry `{path}`: {error}")),
    }
}

fn parse_imported_pictures(slide_xml: &str) -> Vec<ImportedPicture> {
    let mut pictures = Vec::new();
    let mut remaining = slide_xml;
    while let Some(start) = remaining.find("<p:pic>") {
        let block_start = start;
        let Some(block_end_offset) = remaining[block_start..].find("</p:pic>") else {
            break;
        };
        let block_end = block_start + block_end_offset + "</p:pic>".len();
        let block = &remaining[block_start..block_end];
        remaining = &remaining[block_end..];

        let Some(relationship_id) = xml_tag_attribute(block, "<a:blip", "r:embed") else {
            continue;
        };
        let Some(x) = xml_tag_attribute(block, "<a:off", "x").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(y) = xml_tag_attribute(block, "<a:off", "y").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(width) =
            xml_tag_attribute(block, "<a:ext", "cx").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let Some(height) =
            xml_tag_attribute(block, "<a:ext", "cy").and_then(|value| value.parse().ok())
        else {
            continue;
        };
        pictures.push(ImportedPicture {
            relationship_id,
            frame: Rect::from_emu(x, y, width, height),
            crop: xml_tag_attribute(block, "<a:srcRect", "l").map(|left| {
                (
                    left.parse::<f64>().unwrap_or(0.0) / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "t")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "r")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                    xml_tag_attribute(block, "<a:srcRect", "b")
                        .and_then(|value| value.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / 100_000.0,
                )
            }),
            alt_text: xml_tag_attribute(block, "<p:cNvPr", "descr"),
            rotation_degrees: xml_tag_attribute(block, "<a:xfrm", "rot")
                .and_then(|value| value.parse::<i64>().ok())
                .map(|rotation| (rotation as f64 / 60_000.0).round() as i32),
            flip_horizontal: xml_tag_attribute(block, "<a:xfrm", "flipH").as_deref() == Some("1"),
            flip_vertical: xml_tag_attribute(block, "<a:xfrm", "flipV").as_deref() == Some("1"),
            lock_aspect_ratio: xml_tag_attribute(block, "<a:picLocks", "noChangeAspect").as_deref()
                != Some("0"),
        });
    }
    pictures
}

fn parse_slide_image_relationship_targets(rels_xml: &str) -> HashMap<String, String> {
    let mut relationships = HashMap::new();
    let mut remaining = rels_xml;
    while let Some(start) = remaining.find("<Relationship ") {
        let tag_start = start;
        let Some(tag_end_offset) = remaining[tag_start..].find("/>") else {
            break;
        };
        let tag_end = tag_start + tag_end_offset + 2;
        let tag = &remaining[tag_start..tag_end];
        remaining = &remaining[tag_end..];
        if xml_attribute(tag, "Type").as_deref()
            != Some("http://schemas.openxmlformats.org/officeDocument/2006/relationships/image")
        {
            continue;
        }
        let (Some(id), Some(target)) = (xml_attribute(tag, "Id"), xml_attribute(tag, "Target"))
        else {
            continue;
        };
        relationships.insert(id, target);
    }
    relationships
}

fn resolve_zip_relative_path(base_path: &str, target: &str) -> String {
    let mut components = Path::new(base_path)
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            std::path::Component::CurDir => None,
            std::path::Component::ParentDir => None,
            std::path::Component::RootDir | std::path::Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>();
    for component in Path::new(target).components() {
        match component {
            std::path::Component::Normal(value) => {
                components.push(value.to_string_lossy().to_string())
            }
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                components.clear();
            }
        }
    }
    components.join("/")
}

fn xml_tag_attribute(xml: &str, tag_start: &str, attribute: &str) -> Option<String> {
    let start = xml.find(tag_start)?;
    let tag = &xml[start..start + xml[start..].find('>')?];
    xml_attribute(tag, attribute)
}

fn xml_attribute(tag: &str, attribute: &str) -> Option<String> {
    let pattern = format!(r#"{attribute}=""#);
    let start = tag.find(&pattern)? + pattern.len();
    let end = start + tag[start..].find('"')?;
    Some(
        tag[start..end]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

impl PresentationSlide {
    fn to_ppt_rs(&self, slide_size: Rect) -> SlideContent {
        let mut content = SlideContent::new("").layout(SlideLayout::Blank);
        if self.notes.visible && !self.notes.text.is_empty() {
            content = content.notes(&self.notes.text);
        }

        if let Some(background_fill) = &self.background_fill {
            content = content.add_shape(
                Shape::new(
                    ShapeType::Rectangle,
                    0,
                    0,
                    points_to_emu(slide_size.width),
                    points_to_emu(slide_size.height),
                )
                .with_fill(ShapeFill::new(background_fill)),
            );
        }

        let mut ordered = self.elements.clone();
        ordered.sort_by_key(PresentationElement::z_order);
        let mut hyperlink_seq = 1_u32;
        for element in ordered {
            match element {
                PresentationElement::Text(text) => {
                    let mut shape = Shape::new(
                        ShapeType::Rectangle,
                        points_to_emu(text.frame.left),
                        points_to_emu(text.frame.top),
                        points_to_emu(text.frame.width),
                        points_to_emu(text.frame.height),
                    )
                    .with_text(&text.text);
                    if let Some(fill) = text.fill {
                        shape = shape.with_fill(ShapeFill::new(&fill));
                    }
                    if let Some(hyperlink) = &text.hyperlink {
                        let relationship_id = format!("rIdHyperlink{hyperlink_seq}");
                        hyperlink_seq += 1;
                        shape = shape.with_hyperlink(hyperlink.to_ppt_rs(&relationship_id));
                    }
                    content = content.add_shape(shape);
                }
                PresentationElement::Shape(shape) => {
                    let mut ppt_shape = Shape::new(
                        shape.geometry.to_ppt_rs(),
                        points_to_emu(shape.frame.left),
                        points_to_emu(shape.frame.top),
                        points_to_emu(shape.frame.width),
                        points_to_emu(shape.frame.height),
                    );
                    if let Some(text) = shape.text {
                        ppt_shape = ppt_shape.with_text(&text);
                    }
                    if let Some(fill) = shape.fill {
                        ppt_shape = ppt_shape.with_fill(ShapeFill::new(&fill));
                    }
                    if let Some(stroke) = shape.stroke {
                        ppt_shape = ppt_shape
                            .with_line(ShapeLine::new(&stroke.color, points_to_emu(stroke.width)));
                    }
                    if let Some(rotation) = shape.rotation_degrees {
                        ppt_shape = ppt_shape.with_rotation(rotation);
                    }
                    if let Some(hyperlink) = &shape.hyperlink {
                        let relationship_id = format!("rIdHyperlink{hyperlink_seq}");
                        hyperlink_seq += 1;
                        ppt_shape = ppt_shape.with_hyperlink(hyperlink.to_ppt_rs(&relationship_id));
                    }
                    content = content.add_shape(ppt_shape);
                }
                PresentationElement::Connector(connector) => {
                    let mut ppt_connector = Connector::new(
                        connector.connector_type.to_ppt_rs(),
                        points_to_emu(connector.start.left),
                        points_to_emu(connector.start.top),
                        points_to_emu(connector.end.left),
                        points_to_emu(connector.end.top),
                    )
                    .with_line(
                        ConnectorLine::new(
                            &connector.line.color,
                            points_to_emu(connector.line.width),
                        )
                        .with_dash(connector.line_style.to_ppt_rs()),
                    )
                    .with_arrow_size(connector.arrow_size.to_ppt_rs())
                    .with_start_arrow(connector.start_arrow.to_ppt_rs())
                    .with_end_arrow(connector.end_arrow.to_ppt_rs());
                    if let Some(label) = connector.label {
                        ppt_connector = ppt_connector.with_label(&label);
                    }
                    content = content.add_connector(ppt_connector);
                }
                PresentationElement::Image(image) => {
                    if let Some(ref payload) = image.payload {
                        let mut ppt_image = Image::from_bytes(
                            payload.bytes.clone(),
                            points_to_emu(image.frame.width),
                            points_to_emu(image.frame.height),
                            &payload.format,
                        )
                        .position(
                            points_to_emu(image.frame.left),
                            points_to_emu(image.frame.top),
                        );
                        if image.fit_mode != ImageFitMode::Stretch {
                            let (x, y, width, height, crop) = fit_image(&image);
                            ppt_image = Image::from_bytes(
                                payload.bytes.clone(),
                                points_to_emu(width),
                                points_to_emu(height),
                                &payload.format,
                            )
                            .position(points_to_emu(x), points_to_emu(y));
                            if let Some((left, top, right, bottom)) = crop {
                                ppt_image = ppt_image.with_crop(left, top, right, bottom);
                            }
                        }
                        if let Some((left, top, right, bottom)) = image.crop {
                            ppt_image = ppt_image.with_crop(left, top, right, bottom);
                        }
                        content = content.add_image(ppt_image);
                    } else {
                        let mut placeholder = Shape::new(
                            ShapeType::Rectangle,
                            points_to_emu(image.frame.left),
                            points_to_emu(image.frame.top),
                            points_to_emu(image.frame.width),
                            points_to_emu(image.frame.height),
                        )
                        .with_text(image.prompt.as_deref().unwrap_or("Image placeholder"));
                        if let Some(rotation) = image.rotation_degrees {
                            placeholder = placeholder.with_rotation(rotation);
                        }
                        content = content.add_shape(placeholder);
                    }
                }
                PresentationElement::Table(table) => {
                    let mut builder = TableBuilder::new(
                        table
                            .column_widths
                            .iter()
                            .copied()
                            .map(points_to_emu)
                            .collect(),
                    )
                    .position(
                        points_to_emu(table.frame.left),
                        points_to_emu(table.frame.top),
                    );
                    for (row_index, row) in table.rows.into_iter().enumerate() {
                        let cells = row
                            .into_iter()
                            .enumerate()
                            .map(|(column_index, cell)| {
                                build_table_cell(cell, &table.merges, row_index, column_index)
                            })
                            .collect::<Vec<_>>();
                        let mut table_row = TableRow::new(cells);
                        if let Some(height) = table.row_heights.get(row_index) {
                            table_row = table_row.with_height(points_to_emu(*height));
                        }
                        builder = builder.add_row(table_row);
                    }
                    content = content.table(builder.build());
                }
                PresentationElement::Chart(chart) => {
                    let mut ppt_chart = Chart::new(
                        chart.title.as_deref().unwrap_or("Chart"),
                        chart.chart_type.to_ppt_rs(),
                        chart.categories,
                        points_to_emu(chart.frame.left),
                        points_to_emu(chart.frame.top),
                        points_to_emu(chart.frame.width),
                        points_to_emu(chart.frame.height),
                    );
                    for series in chart.series {
                        ppt_chart =
                            ppt_chart.add_series(ChartSeries::new(&series.name, series.values));
                    }
                    content = content.add_chart(ppt_chart);
                }
            }
        }
        content
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum PresentationElement {
    Text(TextElement),
    Shape(ShapeElement),
    Connector(ConnectorElement),
    Image(ImageElement),
    Table(TableElement),
    Chart(ChartElement),
}

impl PresentationElement {
    fn element_id(&self) -> &str {
        match self {
            Self::Text(element) => &element.element_id,
            Self::Shape(element) => &element.element_id,
            Self::Connector(element) => &element.element_id,
            Self::Image(element) => &element.element_id,
            Self::Table(element) => &element.element_id,
            Self::Chart(element) => &element.element_id,
        }
    }

    fn set_element_id(&mut self, new_id: String) {
        match self {
            Self::Text(element) => element.element_id = new_id,
            Self::Shape(element) => element.element_id = new_id,
            Self::Connector(element) => element.element_id = new_id,
            Self::Image(element) => element.element_id = new_id,
            Self::Table(element) => element.element_id = new_id,
            Self::Chart(element) => element.element_id = new_id,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Shape(_) => "shape",
            Self::Connector(_) => "connector",
            Self::Image(_) => "image",
            Self::Table(_) => "table",
            Self::Chart(_) => "chart",
        }
    }

    fn z_order(&self) -> usize {
        match self {
            Self::Text(element) => element.z_order,
            Self::Shape(element) => element.z_order,
            Self::Connector(element) => element.z_order,
            Self::Image(element) => element.z_order,
            Self::Table(element) => element.z_order,
            Self::Chart(element) => element.z_order,
        }
    }

    fn set_z_order(&mut self, z_order: usize) {
        match self {
            Self::Text(element) => element.z_order = z_order,
            Self::Shape(element) => element.z_order = z_order,
            Self::Connector(element) => element.z_order = z_order,
            Self::Image(element) => element.z_order = z_order,
            Self::Table(element) => element.z_order = z_order,
            Self::Chart(element) => element.z_order = z_order,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextElement {
    element_id: String,
    text: String,
    frame: Rect,
    fill: Option<String>,
    style: TextStyle,
    hyperlink: Option<HyperlinkState>,
    #[serde(default)]
    rich_text: RichTextState,
    placeholder: Option<PlaceholderRef>,
    z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShapeElement {
    element_id: String,
    geometry: ShapeGeometry,
    frame: Rect,
    fill: Option<String>,
    stroke: Option<StrokeStyle>,
    text: Option<String>,
    text_style: TextStyle,
    hyperlink: Option<HyperlinkState>,
    rich_text: Option<RichTextState>,
    placeholder: Option<PlaceholderRef>,
    rotation_degrees: Option<i32>,
    flip_horizontal: bool,
    flip_vertical: bool,
    z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectorElement {
    element_id: String,
    connector_type: ConnectorKind,
    start: PointArgs,
    end: PointArgs,
    line: StrokeStyle,
    line_style: LineStyle,
    start_arrow: ConnectorArrowKind,
    end_arrow: ConnectorArrowKind,
    arrow_size: ConnectorArrowScale,
    label: Option<String>,
    z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImageElement {
    pub(crate) element_id: String,
    pub(crate) frame: Rect,
    pub(crate) payload: Option<ImagePayload>,
    pub(crate) fit_mode: ImageFitMode,
    pub(crate) crop: Option<ImageCrop>,
    pub(crate) rotation_degrees: Option<i32>,
    pub(crate) flip_horizontal: bool,
    pub(crate) flip_vertical: bool,
    pub(crate) lock_aspect_ratio: bool,
    pub(crate) alt_text: Option<String>,
    pub(crate) prompt: Option<String>,
    pub(crate) is_placeholder: bool,
    pub(crate) placeholder: Option<PlaceholderRef>,
    pub(crate) z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableElement {
    element_id: String,
    frame: Rect,
    rows: Vec<Vec<TableCellSpec>>,
    column_widths: Vec<u32>,
    row_heights: Vec<u32>,
    style: Option<String>,
    style_options: TableStyleOptions,
    borders: Option<TableBorders>,
    right_to_left: bool,
    merges: Vec<TableMergeRegion>,
    z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChartElement {
    element_id: String,
    frame: Rect,
    chart_type: ChartTypeSpec,
    categories: Vec<String>,
    series: Vec<ChartSeriesSpec>,
    title: Option<String>,
    style_index: Option<u32>,
    has_legend: bool,
    legend: Option<ChartLegend>,
    x_axis: Option<ChartAxisSpec>,
    y_axis: Option<ChartAxisSpec>,
    data_labels: Option<ChartDataLabels>,
    chart_fill: Option<String>,
    plot_area_fill: Option<String>,
    z_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImagePayload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) format: String,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChartSeriesSpec {
    name: String,
    values: Vec<f64>,
    categories: Option<Vec<String>>,
    x_values: Option<Vec<f64>>,
    fill: Option<String>,
    stroke: Option<StrokeStyle>,
    marker: Option<ChartMarkerStyle>,
    #[serde(default)]
    data_label_overrides: Vec<ChartDataLabelOverride>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ShapeGeometry {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Star4,
    Star5,
    Star6,
    Star8,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    LeftRightArrow,
    UpDownArrow,
    Chevron,
    Heart,
    Cloud,
    Wave,
    FlowChartProcess,
    FlowChartDecision,
    FlowChartConnector,
    Parallelogram,
    Trapezoid,
}

impl ShapeGeometry {
    fn from_shape_type(shape_type: ShapeType) -> Self {
        match shape_type {
            ShapeType::RoundedRectangle => Self::RoundedRectangle,
            ShapeType::Ellipse | ShapeType::Circle => Self::Ellipse,
            ShapeType::Triangle => Self::Triangle,
            ShapeType::RightTriangle => Self::RightTriangle,
            ShapeType::Diamond => Self::Diamond,
            ShapeType::Pentagon => Self::Pentagon,
            ShapeType::Hexagon => Self::Hexagon,
            ShapeType::Octagon => Self::Octagon,
            ShapeType::Star4 => Self::Star4,
            ShapeType::Star5 => Self::Star5,
            ShapeType::Star6 => Self::Star6,
            ShapeType::Star8 => Self::Star8,
            ShapeType::RightArrow => Self::RightArrow,
            ShapeType::LeftArrow => Self::LeftArrow,
            ShapeType::UpArrow => Self::UpArrow,
            ShapeType::DownArrow => Self::DownArrow,
            ShapeType::LeftRightArrow => Self::LeftRightArrow,
            ShapeType::UpDownArrow => Self::UpDownArrow,
            ShapeType::ChevronArrow => Self::Chevron,
            ShapeType::Heart => Self::Heart,
            ShapeType::Cloud => Self::Cloud,
            ShapeType::Wave => Self::Wave,
            ShapeType::FlowChartProcess => Self::FlowChartProcess,
            ShapeType::FlowChartDecision => Self::FlowChartDecision,
            ShapeType::FlowChartConnector => Self::FlowChartConnector,
            ShapeType::Parallelogram => Self::Parallelogram,
            ShapeType::Trapezoid => Self::Trapezoid,
            _ => Self::Rectangle,
        }
    }

    fn to_ppt_rs(self) -> ShapeType {
        match self {
            Self::Rectangle => ShapeType::Rectangle,
            Self::RoundedRectangle => ShapeType::RoundedRectangle,
            Self::Ellipse => ShapeType::Ellipse,
            Self::Triangle => ShapeType::Triangle,
            Self::RightTriangle => ShapeType::RightTriangle,
            Self::Diamond => ShapeType::Diamond,
            Self::Pentagon => ShapeType::Pentagon,
            Self::Hexagon => ShapeType::Hexagon,
            Self::Octagon => ShapeType::Octagon,
            Self::Star4 => ShapeType::Star4,
            Self::Star5 => ShapeType::Star5,
            Self::Star6 => ShapeType::Star6,
            Self::Star8 => ShapeType::Star8,
            Self::RightArrow => ShapeType::RightArrow,
            Self::LeftArrow => ShapeType::LeftArrow,
            Self::UpArrow => ShapeType::UpArrow,
            Self::DownArrow => ShapeType::DownArrow,
            Self::LeftRightArrow => ShapeType::LeftRightArrow,
            Self::UpDownArrow => ShapeType::UpDownArrow,
            Self::Chevron => ShapeType::ChevronArrow,
            Self::Heart => ShapeType::Heart,
            Self::Cloud => ShapeType::Cloud,
            Self::Wave => ShapeType::Wave,
            Self::FlowChartProcess => ShapeType::FlowChartProcess,
            Self::FlowChartDecision => ShapeType::FlowChartDecision,
            Self::FlowChartConnector => ShapeType::FlowChartConnector,
            Self::Parallelogram => ShapeType::Parallelogram,
            Self::Trapezoid => ShapeType::Trapezoid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ChartTypeSpec {
    Bar,
    BarHorizontal,
    BarStacked,
    BarStacked100,
    Line,
    LineMarkers,
    LineStacked,
    Pie,
    Doughnut,
    Area,
    AreaStacked,
    AreaStacked100,
    Scatter,
    ScatterLines,
    ScatterSmooth,
    Bubble,
    Radar,
    RadarFilled,
    StockHlc,
    StockOhlc,
    Combo,
}

impl ChartTypeSpec {
    fn to_ppt_rs(self) -> ChartType {
        match self {
            Self::Bar => ChartType::Bar,
            Self::BarHorizontal => ChartType::BarHorizontal,
            Self::BarStacked => ChartType::BarStacked,
            Self::BarStacked100 => ChartType::BarStacked100,
            Self::Line => ChartType::Line,
            Self::LineMarkers => ChartType::LineMarkers,
            Self::LineStacked => ChartType::LineStacked,
            Self::Pie => ChartType::Pie,
            Self::Doughnut => ChartType::Doughnut,
            Self::Area => ChartType::Area,
            Self::AreaStacked => ChartType::AreaStacked,
            Self::AreaStacked100 => ChartType::AreaStacked100,
            Self::Scatter => ChartType::Scatter,
            Self::ScatterLines => ChartType::ScatterLines,
            Self::ScatterSmooth => ChartType::ScatterSmooth,
            Self::Bubble => ChartType::Bubble,
            Self::Radar => ChartType::Radar,
            Self::RadarFilled => ChartType::RadarFilled,
            Self::StockHlc => ChartType::StockHLC,
            Self::StockOhlc => ChartType::StockOHLC,
            Self::Combo => ChartType::Combo,
        }
    }
}

impl ConnectorKind {
    fn to_ppt_rs(self) -> ConnectorType {
        match self {
            Self::Straight => ConnectorType::Straight,
            Self::Elbow => ConnectorType::Elbow,
            Self::Curved => ConnectorType::Curved,
        }
    }
}

impl ConnectorArrowKind {
    fn to_ppt_rs(self) -> ArrowType {
        match self {
            Self::None => ArrowType::None,
            Self::Triangle => ArrowType::Triangle,
            Self::Stealth => ArrowType::Stealth,
            Self::Diamond => ArrowType::Diamond,
            Self::Oval => ArrowType::Oval,
            Self::Open => ArrowType::Open,
        }
    }
}

impl ConnectorArrowScale {
    fn to_ppt_rs(self) -> ArrowSize {
        match self {
            Self::Small => ArrowSize::Small,
            Self::Medium => ArrowSize::Medium,
            Self::Large => ArrowSize::Large,
        }
    }
}

impl LineStyle {
    fn to_ppt_rs(self) -> LineDash {
        match self {
            Self::Solid => LineDash::Solid,
            Self::Dashed => LineDash::Dash,
            Self::Dotted => LineDash::Dot,
            Self::DashDot => LineDash::DashDot,
            Self::DashDotDot => LineDash::DashDotDot,
            Self::LongDash => LineDash::LongDash,
            Self::LongDashDot => LineDash::LongDashDot,
        }
    }

    fn to_ppt_xml(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dash",
            Self::Dotted => "dot",
            Self::DashDot => "dashDot",
            Self::DashDotDot => "dashDotDot",
            Self::LongDash => "lgDash",
            Self::LongDashDot => "lgDashDot",
        }
    }

    fn as_api_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
            Self::Dotted => "dotted",
            Self::DashDot => "dash-dot",
            Self::DashDotDot => "dash-dot-dot",
            Self::LongDash => "long-dash",
            Self::LongDashDot => "long-dash-dot",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ImageFitMode {
    Stretch,
    Contain,
    Cover,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StrokeStyle {
    color: String,
    width: u32,
    style: LineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ConnectorKind {
    Straight,
    Elbow,
    Curved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ConnectorArrowKind {
    None,
    Triangle,
    Stealth,
    Diamond,
    Oval,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ConnectorArrowScale {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    DashDot,
    DashDotDot,
    LongDash,
    LongDashDot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Rect {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rect {
    fn from_emu(left: u32, top: u32, width: u32, height: u32) -> Self {
        Self {
            left: emu_to_points(left),
            top: emu_to_points(top),
            width: emu_to_points(width),
            height: emu_to_points(height),
        }
    }
}

impl From<PositionArgs> for Rect {
    fn from(value: PositionArgs) -> Self {
        Self {
            left: value.left,
            top: value.top,
            width: value.width,
            height: value.height,
        }
    }
}

fn apply_partial_position(rect: Rect, position: PartialPositionArgs) -> Rect {
    Rect {
        left: position.left.unwrap_or(rect.left),
        top: position.top.unwrap_or(rect.top),
        width: position.width.unwrap_or(rect.width),
        height: position.height.unwrap_or(rect.height),
    }
}

fn apply_partial_position_to_image(image: &ImageElement, position: PartialPositionArgs) -> Rect {
    let mut frame = apply_partial_position(image.frame, position.clone());
    if image.lock_aspect_ratio {
        let base_ratio = image
            .payload
            .as_ref()
            .map(|payload| payload.width_px as f64 / payload.height_px as f64)
            .unwrap_or_else(|| image.frame.width as f64 / image.frame.height as f64);
        if let Some(width) = position.width
            && position.height.is_none()
        {
            frame.height = (width as f64 / base_ratio).round() as u32;
        } else if let Some(height) = position.height
            && position.width.is_none()
        {
            frame.width = (height as f64 * base_ratio).round() as u32;
        }
    }
    frame
}
