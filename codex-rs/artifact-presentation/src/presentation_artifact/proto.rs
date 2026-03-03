fn document_to_proto(
    document: &PresentationDocument,
    action: &str,
) -> Result<Value, PresentationArtifactError> {
    let layouts = document
        .layouts
        .iter()
        .map(|layout| layout_to_proto(document, layout, action))
        .collect::<Result<Vec<_>, _>>()?;
    let slides = document
        .slides
        .iter()
        .enumerate()
        .map(|(slide_index, slide)| slide_to_proto(slide, slide_index))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "kind": "presentation",
        "artifactId": document.artifact_id,
        "anchor": format!("pr/{}", document.artifact_id),
        "name": document.name,
        "slideSize": rect_to_proto(document.slide_size),
        "activeSlideIndex": document.active_slide_index,
        "activeSlideId": document.active_slide_index.and_then(|index| document.slides.get(index)).map(|slide| slide.slide_id.clone()),
        "theme": serde_json::json!({
            "colorScheme": document.theme.color_scheme,
            "hexColorMap": document.theme.color_scheme,
            "majorFont": document.theme.major_font,
            "minorFont": document.theme.minor_font,
        }),
        "styles": document
            .named_text_styles()
            .iter()
            .map(|style| named_text_style_to_json(style, "st"))
            .collect::<Vec<_>>(),
        "masters": document.layouts.iter().filter(|layout| layout.kind == LayoutKind::Master).map(|layout| layout.layout_id.clone()).collect::<Vec<_>>(),
        "layouts": layouts,
        "slides": slides,
    }))
}

fn layout_to_proto(
    document: &PresentationDocument,
    layout: &LayoutDocument,
    action: &str,
) -> Result<Value, PresentationArtifactError> {
    let placeholders = layout
        .placeholders
        .iter()
        .map(placeholder_definition_to_proto)
        .collect::<Vec<_>>();
    let resolved_placeholders = resolved_layout_placeholders(document, &layout.layout_id, action)?
        .into_iter()
        .map(|placeholder| {
            let mut value = placeholder_definition_to_proto(&placeholder.definition);
            value["sourceLayoutId"] = Value::String(placeholder.source_layout_id);
            value
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "layoutId": layout.layout_id,
        "anchor": format!("ly/{}", layout.layout_id),
        "name": layout.name,
        "kind": match layout.kind {
            LayoutKind::Layout => "layout",
            LayoutKind::Master => "master",
        },
        "parentLayoutId": layout.parent_layout_id,
        "placeholders": placeholders,
        "resolvedPlaceholders": resolved_placeholders,
    }))
}

fn placeholder_definition_to_proto(placeholder: &PlaceholderDefinition) -> Value {
    serde_json::json!({
        "name": placeholder.name,
        "placeholderType": placeholder.placeholder_type,
        "index": placeholder.index,
        "text": placeholder.text,
        "geometry": format!("{:?}", placeholder.geometry),
        "frame": rect_to_proto(placeholder.frame),
    })
}

fn slide_to_proto(slide: &PresentationSlide, slide_index: usize) -> Value {
    serde_json::json!({
        "slideId": slide.slide_id,
        "anchor": format!("sl/{}", slide.slide_id),
        "index": slide_index,
        "layoutId": slide.layout_id,
        "backgroundFill": slide.background_fill,
        "notes": serde_json::json!({
            "anchor": format!("nt/{}", slide.slide_id),
            "text": slide.notes.text,
            "visible": slide.notes.visible,
            "textPreview": slide.notes.text.replace('\n', " | "),
            "textChars": slide.notes.text.chars().count(),
            "textLines": slide.notes.text.lines().count(),
        }),
        "elements": slide.elements.iter().map(element_to_proto).collect::<Vec<_>>(),
    })
}

fn element_to_proto(element: &PresentationElement) -> Value {
    match element {
        PresentationElement::Text(text) => {
            let mut record = serde_json::json!({
                "kind": "text",
                "elementId": text.element_id,
                "anchor": format!("sh/{}", text.element_id),
                "frame": rect_to_proto(text.frame),
                "text": text.text,
                "textPreview": text.text.replace('\n', " | "),
                "textChars": text.text.chars().count(),
                "textLines": text.text.lines().count(),
                "fill": text.fill,
                "style": text_style_to_proto(&text.style),
                "zOrder": text.z_order,
            });
            if let Some(placeholder) = &text.placeholder {
                record["placeholder"] = placeholder_ref_to_proto(placeholder);
            }
            if let Some(hyperlink) = &text.hyperlink {
                record["hyperlink"] = hyperlink.to_json();
            }
            record
        }
        PresentationElement::Shape(shape) => {
            let mut record = serde_json::json!({
                "kind": "shape",
                "elementId": shape.element_id,
                "anchor": format!("sh/{}", shape.element_id),
                "geometry": format!("{:?}", shape.geometry),
                "frame": rect_to_proto(shape.frame),
                "fill": shape.fill,
                "stroke": shape.stroke.as_ref().map(stroke_to_proto),
                "text": shape.text,
                "textStyle": text_style_to_proto(&shape.text_style),
                "rotation": shape.rotation_degrees,
                "flipHorizontal": shape.flip_horizontal,
                "flipVertical": shape.flip_vertical,
                "zOrder": shape.z_order,
            });
            if let Some(text) = &shape.text {
                record["textPreview"] = Value::String(text.replace('\n', " | "));
                record["textChars"] = Value::from(text.chars().count());
                record["textLines"] = Value::from(text.lines().count());
            }
            if let Some(placeholder) = &shape.placeholder {
                record["placeholder"] = placeholder_ref_to_proto(placeholder);
            }
            if let Some(hyperlink) = &shape.hyperlink {
                record["hyperlink"] = hyperlink.to_json();
            }
            record
        }
        PresentationElement::Connector(connector) => serde_json::json!({
            "kind": "connector",
            "elementId": connector.element_id,
            "anchor": format!("cn/{}", connector.element_id),
            "connectorType": format!("{:?}", connector.connector_type),
            "start": serde_json::json!({
                "left": connector.start.left,
                "top": connector.start.top,
                "unit": "points",
            }),
            "end": serde_json::json!({
                "left": connector.end.left,
                "top": connector.end.top,
                "unit": "points",
            }),
            "line": stroke_to_proto(&connector.line),
            "lineStyle": connector.line_style.as_api_str(),
            "startArrow": format!("{:?}", connector.start_arrow),
            "endArrow": format!("{:?}", connector.end_arrow),
            "arrowSize": format!("{:?}", connector.arrow_size),
            "label": connector.label,
            "zOrder": connector.z_order,
        }),
        PresentationElement::Image(image) => {
            let mut record = serde_json::json!({
                "kind": "image",
                "elementId": image.element_id,
                "anchor": format!("im/{}", image.element_id),
                "frame": rect_to_proto(image.frame),
                "fit": format!("{:?}", image.fit_mode),
                "crop": image.crop.map(|(left, top, right, bottom)| serde_json::json!({
                    "left": left,
                    "top": top,
                    "right": right,
                    "bottom": bottom,
                })),
                "rotation": image.rotation_degrees,
                "flipHorizontal": image.flip_horizontal,
                "flipVertical": image.flip_vertical,
                "lockAspectRatio": image.lock_aspect_ratio,
                "alt": image.alt_text,
                "prompt": image.prompt,
                "isPlaceholder": image.is_placeholder,
                "payload": image.payload.as_ref().map(image_payload_to_proto),
                "zOrder": image.z_order,
            });
            if let Some(placeholder) = &image.placeholder {
                record["placeholder"] = placeholder_ref_to_proto(placeholder);
            }
            record
        }
        PresentationElement::Table(table) => serde_json::json!({
            "kind": "table",
            "elementId": table.element_id,
            "anchor": format!("tb/{}", table.element_id),
            "frame": rect_to_proto(table.frame),
            "rows": table.rows.iter().map(|row| {
                row.iter().map(table_cell_to_proto).collect::<Vec<_>>()
            }).collect::<Vec<_>>(),
            "columnWidths": table.column_widths,
            "rowHeights": table.row_heights,
            "style": table.style,
            "merges": table.merges.iter().map(|merge| serde_json::json!({
                "startRow": merge.start_row,
                "endRow": merge.end_row,
                "startColumn": merge.start_column,
                "endColumn": merge.end_column,
            })).collect::<Vec<_>>(),
            "zOrder": table.z_order,
        }),
        PresentationElement::Chart(chart) => serde_json::json!({
            "kind": "chart",
            "elementId": chart.element_id,
            "anchor": format!("ch/{}", chart.element_id),
            "frame": rect_to_proto(chart.frame),
            "chartType": format!("{:?}", chart.chart_type),
            "title": chart.title,
            "categories": chart.categories,
            "series": chart.series.iter().map(|series| serde_json::json!({
                "name": series.name,
                "values": series.values,
            })).collect::<Vec<_>>(),
            "zOrder": chart.z_order,
        }),
    }
}

fn rect_to_proto(rect: Rect) -> Value {
    serde_json::json!({
        "left": rect.left,
        "top": rect.top,
        "width": rect.width,
        "height": rect.height,
        "unit": "points",
    })
}

fn stroke_to_proto(stroke: &StrokeStyle) -> Value {
    serde_json::json!({
        "color": stroke.color,
        "width": stroke.width,
        "style": stroke.style.as_api_str(),
        "unit": "points",
    })
}

fn text_style_to_proto(style: &TextStyle) -> Value {
    serde_json::json!({
        "styleName": style.style_name,
        "fontSize": style.font_size,
        "fontFamily": style.font_family,
        "color": style.color,
        "alignment": style.alignment,
        "bold": style.bold,
        "italic": style.italic,
        "underline": style.underline,
    })
}

fn placeholder_ref_to_proto(placeholder: &PlaceholderRef) -> Value {
    serde_json::json!({
        "name": placeholder.name,
        "placeholderType": placeholder.placeholder_type,
        "index": placeholder.index,
    })
}

fn image_payload_to_proto(payload: &ImagePayload) -> Value {
    serde_json::json!({
        "format": payload.format,
        "widthPx": payload.width_px,
        "heightPx": payload.height_px,
        "bytesBase64": BASE64_STANDARD.encode(&payload.bytes),
    })
}

fn table_cell_to_proto(cell: &TableCellSpec) -> Value {
    serde_json::json!({
        "text": cell.text,
        "textStyle": text_style_to_proto(&cell.text_style),
        "backgroundFill": cell.background_fill,
        "alignment": cell.alignment,
    })
}

fn build_table_cell(
    cell: TableCellSpec,
    merges: &[TableMergeRegion],
    row_index: usize,
    column_index: usize,
) -> TableCell {
    let mut table_cell = TableCell::new(&cell.text);
    if cell.text_style.bold {
        table_cell = table_cell.bold();
    }
    if cell.text_style.italic {
        table_cell = table_cell.italic();
    }
    if cell.text_style.underline {
        table_cell = table_cell.underline();
    }
    if let Some(color) = cell.text_style.color {
        table_cell = table_cell.text_color(&color);
    }
    if let Some(fill) = cell.background_fill {
        table_cell = table_cell.background_color(&fill);
    }
    if let Some(size) = cell.text_style.font_size {
        table_cell = table_cell.font_size(size);
    }
    if let Some(font_family) = cell.text_style.font_family {
        table_cell = table_cell.font_family(&font_family);
    }
    if let Some(alignment) = cell.alignment.or(cell.text_style.alignment) {
        table_cell = match alignment {
            TextAlignment::Left => table_cell.align_left(),
            TextAlignment::Center => table_cell.align_center(),
            TextAlignment::Right => table_cell.align_right(),
            TextAlignment::Justify => table_cell.align(CellAlign::Justify),
        };
    }
    for merge in merges {
        if row_index == merge.start_row && column_index == merge.start_column {
            table_cell = table_cell
                .grid_span((merge.end_column - merge.start_column + 1) as u32)
                .row_span((merge.end_row - merge.start_row + 1) as u32);
        } else if row_index >= merge.start_row
            && row_index <= merge.end_row
            && column_index >= merge.start_column
            && column_index <= merge.end_column
        {
            if row_index == merge.start_row {
                table_cell = table_cell.h_merge();
            } else {
                table_cell = table_cell.v_merge();
            }
        }
    }
    table_cell
}

