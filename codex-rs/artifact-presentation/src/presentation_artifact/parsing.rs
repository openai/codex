fn parse_args<T>(action: &str, value: &Value) -> Result<T, PresentationArtifactError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(value.clone()).map_err(|error| PresentationArtifactError::InvalidArgs {
        action: action.to_string(),
        message: error.to_string(),
    })
}

fn required_artifact_id(
    request: &PresentationArtifactRequest,
) -> Result<String, PresentationArtifactError> {
    request
        .artifact_id
        .clone()
        .ok_or_else(|| PresentationArtifactError::MissingArtifactId {
            action: request.action.clone(),
        })
}

fn is_read_only_action(action: &str) -> bool {
    matches!(
        action,
        "get_summary"
            | "list_slides"
            | "list_layouts"
            | "list_layout_placeholders"
            | "list_slide_placeholders"
            | "inspect"
            | "resolve"
            | "to_proto"
            | "get_style"
            | "describe_styles"
            | "record_patch"
    )
}

fn tracks_history(action: &str) -> bool {
    !is_read_only_action(action)
        && !matches!(
            action,
            "export_pptx" | "export_preview" | "undo" | "redo" | "apply_patch"
        )
}

fn patch_operation_supported(action: &str) -> bool {
    tracks_history(action) && !matches!(action, "create" | "import_pptx" | "delete_artifact")
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn normalize_color(
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    normalize_color_with_palette(None, color, action, field)
}

fn normalize_color_with_document(
    document: &PresentationDocument,
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    normalize_color_with_palette(Some(&document.theme), color, action, field)
}

fn normalize_color_with_palette(
    theme: Option<&ThemeState>,
    color: &str,
    action: &str,
    field: &str,
) -> Result<String, PresentationArtifactError> {
    let trimmed = color.trim();
    let normalized = theme
        .and_then(|palette| palette.resolve_color(trimmed))
        .unwrap_or_else(|| trimmed.trim_start_matches('#').to_uppercase());
    if normalized.len() != 6
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("field `{field}` must be a 6-digit RGB hex color"),
        });
    }
    Ok(normalized)
}

fn parse_shape_geometry(
    geometry: &str,
    action: &str,
) -> Result<ShapeGeometry, PresentationArtifactError> {
    match geometry {
        "rectangle" | "rect" => Ok(ShapeGeometry::Rectangle),
        "rounded_rectangle" | "roundedRect" => Ok(ShapeGeometry::RoundedRectangle),
        "ellipse" | "circle" => Ok(ShapeGeometry::Ellipse),
        "triangle" => Ok(ShapeGeometry::Triangle),
        "right_triangle" => Ok(ShapeGeometry::RightTriangle),
        "diamond" => Ok(ShapeGeometry::Diamond),
        "pentagon" => Ok(ShapeGeometry::Pentagon),
        "hexagon" => Ok(ShapeGeometry::Hexagon),
        "octagon" => Ok(ShapeGeometry::Octagon),
        "star4" => Ok(ShapeGeometry::Star4),
        "star" | "star5" => Ok(ShapeGeometry::Star5),
        "star6" => Ok(ShapeGeometry::Star6),
        "star8" => Ok(ShapeGeometry::Star8),
        "right_arrow" => Ok(ShapeGeometry::RightArrow),
        "left_arrow" => Ok(ShapeGeometry::LeftArrow),
        "up_arrow" => Ok(ShapeGeometry::UpArrow),
        "down_arrow" => Ok(ShapeGeometry::DownArrow),
        "left_right_arrow" | "leftRightArrow" => Ok(ShapeGeometry::LeftRightArrow),
        "up_down_arrow" | "upDownArrow" => Ok(ShapeGeometry::UpDownArrow),
        "chevron" => Ok(ShapeGeometry::Chevron),
        "heart" => Ok(ShapeGeometry::Heart),
        "cloud" => Ok(ShapeGeometry::Cloud),
        "wave" => Ok(ShapeGeometry::Wave),
        "flowChartProcess" | "flow_chart_process" => Ok(ShapeGeometry::FlowChartProcess),
        "flowChartDecision" | "flow_chart_decision" => Ok(ShapeGeometry::FlowChartDecision),
        "flowChartConnector" | "flow_chart_connector" => Ok(ShapeGeometry::FlowChartConnector),
        "parallelogram" => Ok(ShapeGeometry::Parallelogram),
        "trapezoid" => Ok(ShapeGeometry::Trapezoid),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("geometry `{geometry}` is not supported"),
        }),
    }
}

fn parse_chart_type(
    chart_type: &str,
    action: &str,
) -> Result<ChartTypeSpec, PresentationArtifactError> {
    match chart_type {
        "bar" => Ok(ChartTypeSpec::Bar),
        "bar_horizontal" => Ok(ChartTypeSpec::BarHorizontal),
        "bar_stacked" => Ok(ChartTypeSpec::BarStacked),
        "bar_stacked_100" => Ok(ChartTypeSpec::BarStacked100),
        "line" => Ok(ChartTypeSpec::Line),
        "line_markers" => Ok(ChartTypeSpec::LineMarkers),
        "line_stacked" => Ok(ChartTypeSpec::LineStacked),
        "pie" => Ok(ChartTypeSpec::Pie),
        "doughnut" => Ok(ChartTypeSpec::Doughnut),
        "area" => Ok(ChartTypeSpec::Area),
        "area_stacked" => Ok(ChartTypeSpec::AreaStacked),
        "area_stacked_100" => Ok(ChartTypeSpec::AreaStacked100),
        "scatter" => Ok(ChartTypeSpec::Scatter),
        "scatter_lines" => Ok(ChartTypeSpec::ScatterLines),
        "scatter_smooth" => Ok(ChartTypeSpec::ScatterSmooth),
        "bubble" => Ok(ChartTypeSpec::Bubble),
        "radar" => Ok(ChartTypeSpec::Radar),
        "radar_filled" => Ok(ChartTypeSpec::RadarFilled),
        "stock_hlc" => Ok(ChartTypeSpec::StockHlc),
        "stock_ohlc" => Ok(ChartTypeSpec::StockOhlc),
        "combo" => Ok(ChartTypeSpec::Combo),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("chart_type `{chart_type}` is not supported"),
        }),
    }
}

fn parse_stroke(
    document: &PresentationDocument,
    stroke: Option<StrokeArgs>,
    action: &str,
) -> Result<Option<StrokeStyle>, PresentationArtifactError> {
    stroke
        .map(|value| parse_required_stroke(document, value, action))
        .transpose()
}

fn parse_required_stroke(
    document: &PresentationDocument,
    stroke: StrokeArgs,
    action: &str,
) -> Result<StrokeStyle, PresentationArtifactError> {
    Ok(StrokeStyle {
        color: normalize_color_with_document(document, &stroke.color, action, "stroke.color")?,
        width: stroke.width,
        style: stroke
            .style
            .as_deref()
            .map(|style| parse_line_style(style, action))
            .transpose()?
            .unwrap_or(LineStyle::Solid),
    })
}

fn parse_connector_kind(
    connector_type: &str,
    action: &str,
) -> Result<ConnectorKind, PresentationArtifactError> {
    match connector_type {
        "straight" => Ok(ConnectorKind::Straight),
        "elbow" => Ok(ConnectorKind::Elbow),
        "curved" => Ok(ConnectorKind::Curved),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector_type `{connector_type}` is not supported"),
        }),
    }
}

fn parse_connector_arrow(
    value: &str,
    action: &str,
) -> Result<ConnectorArrowKind, PresentationArtifactError> {
    match value {
        "none" => Ok(ConnectorArrowKind::None),
        "triangle" => Ok(ConnectorArrowKind::Triangle),
        "stealth" => Ok(ConnectorArrowKind::Stealth),
        "diamond" => Ok(ConnectorArrowKind::Diamond),
        "oval" => Ok(ConnectorArrowKind::Oval),
        "open" => Ok(ConnectorArrowKind::Open),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector arrow `{value}` is not supported"),
        }),
    }
}

fn parse_connector_arrow_size(
    value: &str,
    action: &str,
) -> Result<ConnectorArrowScale, PresentationArtifactError> {
    match value {
        "small" => Ok(ConnectorArrowScale::Small),
        "medium" => Ok(ConnectorArrowScale::Medium),
        "large" => Ok(ConnectorArrowScale::Large),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("connector arrow_size `{value}` is not supported"),
        }),
    }
}

fn parse_line_style(value: &str, action: &str) -> Result<LineStyle, PresentationArtifactError> {
    match value {
        "solid" => Ok(LineStyle::Solid),
        "dashed" => Ok(LineStyle::Dashed),
        "dotted" => Ok(LineStyle::Dotted),
        "dash-dot" | "dash_dot" => Ok(LineStyle::DashDot),
        "dash-dot-dot" | "dash_dot_dot" => Ok(LineStyle::DashDotDot),
        "long-dash" | "long_dash" => Ok(LineStyle::LongDash),
        "long-dash-dot" | "long_dash_dot" => Ok(LineStyle::LongDashDot),
        _ => Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("line style `{value}` is not supported"),
        }),
    }
}

fn parse_connector_line(
    document: &PresentationDocument,
    line: Option<ConnectorLineArgs>,
    action: &str,
) -> Result<ParsedConnectorLine, PresentationArtifactError> {
    let line = line.unwrap_or_default();
    Ok(ParsedConnectorLine {
        color: line
            .color
            .as_deref()
            .map(|value| normalize_color_with_document(document, value, action, "line.color"))
            .transpose()?
            .unwrap_or_else(|| "000000".to_string()),
        width: line.width.unwrap_or(1),
        style: line
            .style
            .as_deref()
            .map(|value| parse_line_style(value, action))
            .transpose()?
            .unwrap_or(LineStyle::Solid),
    })
}

struct ParsedConnectorLine {
    color: String,
    width: u32,
    style: LineStyle,
}

fn normalize_text_style_with_document(
    document: &PresentationDocument,
    styling: &TextStylingArgs,
    action: &str,
) -> Result<TextStyle, PresentationArtifactError> {
    normalize_text_style_with_palette(Some(&document.theme), styling, action, |style_name| {
        document.resolve_named_text_style(style_name, action)
    })
}

fn normalize_text_style_with_palette(
    theme: Option<&ThemeState>,
    styling: &TextStylingArgs,
    action: &str,
    resolve_style_name: impl Fn(&str) -> Result<TextStyle, PresentationArtifactError>,
) -> Result<TextStyle, PresentationArtifactError> {
    let mut style = styling
        .style
        .as_deref()
        .map(resolve_style_name)
        .transpose()?
        .unwrap_or_default();
    style.font_size = styling.font_size.or(style.font_size);
    style.font_family = styling.font_family.clone().or(style.font_family);
    style.color = styling
        .color
        .as_deref()
        .map(|value| normalize_color_with_palette(theme, value, action, "color"))
        .transpose()?
        .or(style.color);
    style.alignment = styling
        .alignment
        .as_deref()
        .map(|value| parse_alignment(value, action))
        .transpose()?
        .or(style.alignment);
    style.bold = styling.bold.unwrap_or(style.bold);
    style.italic = styling.italic.unwrap_or(style.italic);
    style.underline = styling.underline.unwrap_or(style.underline);
    if let Some(style_name) = &styling.style {
        style.style_name = Some(normalize_style_name(style_name, action)?);
    }
    Ok(style)
}

fn parse_hyperlink_state(
    document: &PresentationDocument,
    args: &SetHyperlinkArgs,
    action: &str,
) -> Result<HyperlinkState, PresentationArtifactError> {
    let link_type =
        args.link_type
            .as_deref()
            .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "`link_type` is required unless `clear` is true".to_string(),
            })?;
    let target = match link_type {
        "url" => HyperlinkTarget::Url(required_hyperlink_field(&args.url, action, "url")?.clone()),
        "slide" => {
            let slide_index =
                args.slide_index
                    .ok_or_else(|| PresentationArtifactError::InvalidArgs {
                        action: action.to_string(),
                        message: "`slide_index` is required for slide hyperlinks".to_string(),
                    })?;
            if slide_index as usize >= document.slides.len() {
                return Err(index_out_of_range(
                    action,
                    slide_index as usize,
                    document.slides.len(),
                ));
            }
            HyperlinkTarget::Slide(slide_index)
        }
        "first_slide" => HyperlinkTarget::FirstSlide,
        "last_slide" => HyperlinkTarget::LastSlide,
        "next_slide" => HyperlinkTarget::NextSlide,
        "previous_slide" => HyperlinkTarget::PreviousSlide,
        "end_show" => HyperlinkTarget::EndShow,
        "email" => HyperlinkTarget::Email {
            address: required_hyperlink_field(&args.address, action, "address")?.clone(),
            subject: args.subject.clone(),
        },
        "file" => {
            HyperlinkTarget::File(required_hyperlink_field(&args.path, action, "path")?.clone())
        }
        other => {
            return Err(PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("hyperlink type `{other}` is not supported"),
            });
        }
    };
    Ok(HyperlinkState {
        target,
        tooltip: args.tooltip.clone(),
        highlight_click: args.highlight_click.unwrap_or(true),
    })
}

fn required_hyperlink_field<'a>(
    value: &'a Option<String>,
    action: &str,
    field: &str,
) -> Result<&'a String, PresentationArtifactError> {
    value
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("`{field}` is required for this hyperlink type"),
        })
}

fn coerce_table_rows(
    rows: Vec<Vec<Value>>,
    action: &str,
) -> Result<Vec<Vec<TableCellSpec>>, PresentationArtifactError> {
    if rows.is_empty() {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`rows` must contain at least one row".to_string(),
        });
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| TableCellSpec {
                    text: cell_value_to_string(value),
                    text_style: TextStyle::default(),
                    background_fill: None,
                    alignment: None,
                })
                .collect()
        })
        .collect())
}

fn normalize_table_dimensions(
    rows: &[Vec<TableCellSpec>],
    frame: Rect,
    column_widths: Option<Vec<u32>>,
    row_heights: Option<Vec<u32>>,
    action: &str,
) -> Result<(Vec<u32>, Vec<u32>), PresentationArtifactError> {
    let column_count = rows.iter().map(std::vec::Vec::len).max().unwrap_or(1);
    let normalized_column_widths = match column_widths {
        Some(widths) => {
            if widths.len() != column_count {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!(
                        "`column_widths` must contain {column_count} entries for this table"
                    ),
                });
            }
            widths
        }
        None => split_points(frame.width, column_count),
    };
    let normalized_row_heights = match row_heights {
        Some(heights) => {
            if heights.len() != rows.len() {
                return Err(PresentationArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!(
                        "`row_heights` must contain {} entries for this table",
                        rows.len()
                    ),
                });
            }
            heights
        }
        None => split_points(frame.height, rows.len()),
    };
    Ok((normalized_column_widths, normalized_row_heights))
}

fn split_points(total: u32, count: usize) -> Vec<u32> {
    if count == 0 {
        return Vec::new();
    }
    let base = total / count as u32;
    let remainder = total % count as u32;
    (0..count)
        .map(|index| base + u32::from(index < remainder as usize))
        .collect()
}

fn parse_alignment(value: &str, action: &str) -> Result<TextAlignment, PresentationArtifactError> {
    match value {
        "left" => Ok(TextAlignment::Left),
        "center" | "middle" => Ok(TextAlignment::Center),
        "right" => Ok(TextAlignment::Right),
        "justify" => Ok(TextAlignment::Justify),
        _ => Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("unsupported alignment `{value}`"),
        }),
    }
}

fn normalize_theme(args: ThemeArgs, action: &str) -> Result<ThemeState, PresentationArtifactError> {
    let color_scheme = args
        .color_scheme
        .into_iter()
        .map(|(key, value)| {
            normalize_color(&value, action, &key)
                .map(|normalized| (key.to_ascii_lowercase(), normalized))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(ThemeState {
        color_scheme,
        major_font: args.major_font,
        minor_font: args.minor_font,
    })
}

fn normalize_style_name(
    style_name: &str,
    action: &str,
) -> Result<String, PresentationArtifactError> {
    let normalized_style_name = style_name.trim().to_ascii_lowercase();
    if normalized_style_name.is_empty() {
        return Err(PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "`name` must not be empty".to_string(),
        });
    }
    Ok(normalized_style_name)
}

fn built_in_text_styles(theme: &ThemeState) -> HashMap<String, TextStyle> {
    ["title", "heading1", "body", "list", "numberedlist"]
        .into_iter()
        .filter_map(|name| built_in_text_style(theme, name).map(|style| (name.to_string(), style)))
        .collect()
}

fn built_in_text_style(theme: &ThemeState, style_name: &str) -> Option<TextStyle> {
    let default_color = theme.resolve_color("tx1");
    let default_font = theme
        .major_font
        .clone()
        .or_else(|| theme.minor_font.clone());
    let body_font = theme
        .minor_font
        .clone()
        .or_else(|| theme.major_font.clone());
    let style = match style_name {
        "title" => TextStyle {
            style_name: Some("title".to_string()),
            font_size: Some(28),
            font_family: default_font,
            color: default_color,
            alignment: Some(TextAlignment::Left),
            bold: true,
            italic: false,
            underline: false,
        },
        "heading1" => TextStyle {
            style_name: Some("heading1".to_string()),
            font_size: Some(22),
            font_family: default_font,
            color: default_color,
            alignment: Some(TextAlignment::Left),
            bold: true,
            italic: false,
            underline: false,
        },
        "body" => TextStyle {
            style_name: Some("body".to_string()),
            font_size: Some(14),
            font_family: body_font,
            color: default_color,
            alignment: Some(TextAlignment::Left),
            bold: false,
            italic: false,
            underline: false,
        },
        "list" => TextStyle {
            style_name: Some("list".to_string()),
            font_size: Some(14),
            font_family: body_font,
            color: default_color,
            alignment: Some(TextAlignment::Left),
            bold: false,
            italic: false,
            underline: false,
        },
        "numberedlist" => TextStyle {
            style_name: Some("numberedlist".to_string()),
            font_size: Some(14),
            font_family: body_font,
            color: default_color,
            alignment: Some(TextAlignment::Left),
            bold: false,
            italic: false,
            underline: false,
        },
        _ => return None,
    };
    Some(style)
}

fn named_text_style_to_json(style: &NamedTextStyle, id_prefix: &str) -> Value {
    serde_json::json!({
        "kind": "textStyle",
        "id": format!("{id_prefix}/{}", style.name),
        "name": style.name,
        "builtIn": style.built_in,
        "style": text_style_to_proto(&style.style),
    })
}

fn parse_slide_size(value: &Value, action: &str) -> Result<Rect, PresentationArtifactError> {
    #[derive(Deserialize)]
    struct SlideSizeArgs {
        width: u32,
        height: u32,
    }

    let slide_size: SlideSizeArgs = serde_json::from_value(value.clone()).map_err(|error| {
        PresentationArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("invalid slide_size: {error}"),
        }
    })?;
    Ok(Rect {
        left: 0,
        top: 0,
        width: slide_size.width,
        height: slide_size.height,
    })
}

fn apply_layout_to_slide(
    document: &mut PresentationDocument,
    slide: &mut PresentationSlide,
    layout_ref: &str,
    action: &str,
) -> Result<(), PresentationArtifactError> {
    let layout = document.get_layout(layout_ref, action)?.clone();
    let placeholders = resolved_layout_placeholders(document, &layout.layout_id, action)?;
    slide.layout_id = Some(layout.layout_id);
    for resolved in placeholders {
        slide.elements.push(materialize_placeholder_element(
            document.next_element_id(),
            resolved.definition,
            slide.elements.len(),
        ));
    }
    Ok(())
}

fn materialize_placeholder_element(
    element_id: String,
    placeholder: PlaceholderDefinition,
    z_order: usize,
) -> PresentationElement {
    let placeholder_ref = Some(PlaceholderRef {
        name: placeholder.name.clone(),
        placeholder_type: placeholder.placeholder_type.clone(),
        index: placeholder.index,
    });
    if placeholder_is_image(&placeholder.placeholder_type) {
        return PresentationElement::Image(ImageElement {
            element_id,
            frame: placeholder.frame,
            payload: None,
            fit_mode: ImageFitMode::Stretch,
            crop: None,
            rotation_degrees: None,
            flip_horizontal: false,
            flip_vertical: false,
            lock_aspect_ratio: true,
            alt_text: Some(placeholder.name.clone()),
            prompt: placeholder
                .text
                .clone()
                .or_else(|| Some(format!("Image placeholder: {}", placeholder.name))),
            is_placeholder: true,
            placeholder: placeholder_ref,
            z_order,
        });
    }
    if placeholder.geometry == ShapeGeometry::Rectangle {
        PresentationElement::Text(TextElement {
            element_id,
            text: placeholder.text.unwrap_or_default(),
            frame: placeholder.frame,
            fill: None,
            style: TextStyle::default(),
            hyperlink: None,
            placeholder: placeholder_ref,
            z_order,
        })
    } else {
        PresentationElement::Shape(ShapeElement {
            element_id,
            geometry: placeholder.geometry,
            frame: placeholder.frame,
            fill: None,
            stroke: None,
            text: placeholder.text,
            text_style: TextStyle::default(),
            hyperlink: None,
            placeholder: placeholder_ref,
            rotation_degrees: None,
            flip_horizontal: false,
            flip_vertical: false,
            z_order,
        })
    }
}

fn resolved_layout_placeholders(
    document: &PresentationDocument,
    layout_id: &str,
    action: &str,
) -> Result<Vec<ResolvedPlaceholder>, PresentationArtifactError> {
    let mut lineage = Vec::new();
    collect_layout_lineage(
        document,
        layout_id,
        action,
        &mut HashSet::new(),
        &mut lineage,
    )?;
    let mut resolved: Vec<ResolvedPlaceholder> = Vec::new();
    for layout in lineage {
        for placeholder in &layout.placeholders {
            if let Some(index) = resolved.iter().position(|entry| {
                placeholder_key(&entry.definition) == placeholder_key(placeholder)
            }) {
                resolved[index] = ResolvedPlaceholder {
                    source_layout_id: layout.layout_id.clone(),
                    definition: placeholder.clone(),
                };
            } else {
                resolved.push(ResolvedPlaceholder {
                    source_layout_id: layout.layout_id.clone(),
                    definition: placeholder.clone(),
                });
            }
        }
    }
    Ok(resolved)
}

fn collect_layout_lineage<'a>(
    document: &'a PresentationDocument,
    layout_id: &str,
    action: &str,
    seen: &mut HashSet<String>,
    lineage: &mut Vec<&'a LayoutDocument>,
) -> Result<(), PresentationArtifactError> {
    if !seen.insert(layout_id.to_string()) {
        return Err(PresentationArtifactError::UnsupportedFeature {
            action: action.to_string(),
            message: format!("layout inheritance cycle detected at `{layout_id}`"),
        });
    }
    let layout = document.get_layout(layout_id, action)?;
    if let Some(parent_layout_id) = &layout.parent_layout_id {
        collect_layout_lineage(document, parent_layout_id, action, seen, lineage)?;
    }
    lineage.push(layout);
    Ok(())
}

fn placeholder_key(placeholder: &PlaceholderDefinition) -> (String, String, Option<u32>) {
    (
        placeholder.name.to_ascii_lowercase(),
        placeholder.placeholder_type.to_ascii_lowercase(),
        placeholder.index,
    )
}

fn layout_placeholder_list(
    document: &PresentationDocument,
    layout_id: &str,
    action: &str,
) -> Result<Vec<PlaceholderListEntry>, PresentationArtifactError> {
    resolved_layout_placeholders(document, layout_id, action).map(|placeholders| {
        placeholders
            .into_iter()
            .map(|placeholder| PlaceholderListEntry {
                scope: "layout".to_string(),
                source_layout_id: Some(placeholder.source_layout_id),
                slide_index: None,
                element_id: None,
                name: placeholder.definition.name,
                placeholder_type: placeholder.definition.placeholder_type,
                index: placeholder.definition.index,
                geometry: Some(format!("{:?}", placeholder.definition.geometry)),
                text_preview: placeholder.definition.text,
            })
            .collect()
    })
}

fn placeholder_is_image(placeholder_type: &str) -> bool {
    matches!(
        placeholder_type.to_ascii_lowercase().as_str(),
        "image" | "picture" | "pic" | "photo"
    )
}

fn slide_placeholder_list(
    slide: &PresentationSlide,
    slide_index: usize,
) -> Vec<PlaceholderListEntry> {
    slide
        .elements
        .iter()
        .filter_map(|element| match element {
            PresentationElement::Text(text) => {
                text.placeholder
                    .as_ref()
                    .map(|placeholder| PlaceholderListEntry {
                        scope: "slide".to_string(),
                        source_layout_id: slide.layout_id.clone(),
                        slide_index: Some(slide_index),
                        element_id: Some(text.element_id.clone()),
                        name: placeholder.name.clone(),
                        placeholder_type: placeholder.placeholder_type.clone(),
                        index: placeholder.index,
                        geometry: Some("Rectangle".to_string()),
                        text_preview: Some(text.text.clone()),
                    })
            }
            PresentationElement::Shape(shape) => {
                shape
                    .placeholder
                    .as_ref()
                    .map(|placeholder| PlaceholderListEntry {
                        scope: "slide".to_string(),
                        source_layout_id: slide.layout_id.clone(),
                        slide_index: Some(slide_index),
                        element_id: Some(shape.element_id.clone()),
                        name: placeholder.name.clone(),
                        placeholder_type: placeholder.placeholder_type.clone(),
                        index: placeholder.index,
                        geometry: Some(format!("{:?}", shape.geometry)),
                        text_preview: shape.text.clone(),
                    })
            }
            PresentationElement::Image(image) => {
                image
                    .placeholder
                    .as_ref()
                    .map(|placeholder| PlaceholderListEntry {
                        scope: "slide".to_string(),
                        source_layout_id: slide.layout_id.clone(),
                        slide_index: Some(slide_index),
                        element_id: Some(image.element_id.clone()),
                        name: placeholder.name.clone(),
                        placeholder_type: placeholder.placeholder_type.clone(),
                        index: placeholder.index,
                        geometry: Some("Image".to_string()),
                        text_preview: image.prompt.clone(),
                    })
            }
            PresentationElement::Connector(_)
            | PresentationElement::Table(_)
            | PresentationElement::Chart(_) => None,
        })
        .collect()
}

