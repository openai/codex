fn inspect_document(document: &PresentationDocument, args: &InspectArgs) -> String {
    let include_kinds = args
        .include
        .as_deref()
        .or(args.kind.as_deref())
        .unwrap_or(
            "deck,slide,textbox,shape,connector,table,chart,image,notes,layoutList,textRange,comment",
        );
    let included_kinds = include_kinds
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();
    let excluded_kinds = args
        .exclude
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();
    let include = |name: &str| included_kinds.contains(name) && !excluded_kinds.contains(name);
    let mut records: Vec<(Value, Option<String>)> = Vec::new();
    if include("deck") {
        records.push((
            serde_json::json!({
                "kind": "deck",
                "id": format!("pr/{}", document.artifact_id),
                "name": document.name,
                "slides": document.slides.len(),
                "styleIds": document
                    .named_text_styles()
                    .iter()
                    .map(|style| format!("st/{}", style.name))
                    .collect::<Vec<_>>(),
                "activeSlideIndex": document.active_slide_index,
                "activeSlideId": document.active_slide_index.and_then(|index| document.slides.get(index)).map(|slide| format!("sl/{}", slide.slide_id)),
                "commentThreadIds": document
                    .comment_threads
                    .iter()
                    .map(|thread| format!("th/{}", thread.thread_id))
                    .collect::<Vec<_>>(),
            }),
            None,
        ));
    }
    if include("styleList") {
        for style in document.named_text_styles() {
            records.push((named_text_style_to_json(&style, "st"), None));
        }
    }
    if include("layoutList") {
        for layout in &document.layouts {
            let placeholders = resolved_layout_placeholders(document, &layout.layout_id, "inspect")
                .unwrap_or_default()
                .into_iter()
                .map(|placeholder| {
                    serde_json::json!({
                        "name": placeholder.definition.name,
                        "type": placeholder.definition.placeholder_type,
                        "sourceLayoutId": placeholder.source_layout_id,
                        "textPreview": placeholder.definition.text,
                    })
                })
                .collect::<Vec<_>>();
            records.push((
                serde_json::json!({
                    "kind": "layout",
                    "id": format!("ly/{}", layout.layout_id),
                    "layoutId": layout.layout_id,
                    "name": layout.name,
                    "type": match layout.kind { LayoutKind::Layout => "layout", LayoutKind::Master => "master" },
                    "parentLayoutId": layout.parent_layout_id,
                    "placeholders": placeholders,
                }),
                None,
            ));
        }
    }
    for (index, slide) in document.slides.iter().enumerate() {
        let slide_id = format!("sl/{}", slide.slide_id);
        if include("slide") {
            records.push((
                serde_json::json!({
                    "kind": "slide",
                    "id": slide_id,
                    "slide": index + 1,
                    "slideIndex": index,
                    "isActive": document.active_slide_index == Some(index),
                    "layoutId": slide.layout_id,
                    "elements": slide.elements.len(),
                }),
                Some(slide_id.clone()),
            ));
        }
        if include("notes") && !slide.notes.text.is_empty() {
            records.push((
                serde_json::json!({
                    "kind": "notes",
                    "id": format!("nt/{}", slide.slide_id),
                    "slide": index + 1,
                    "visible": slide.notes.visible,
                    "text": slide.notes.text,
                    "textPreview": slide.notes.text.replace('\n', " | "),
                    "textChars": slide.notes.text.chars().count(),
                    "textLines": slide.notes.text.lines().count(),
                    "richText": rich_text_to_proto(&slide.notes.text, &slide.notes.rich_text),
                }),
                Some(slide_id.clone()),
            ));
        }
        if include("textRange") {
            records.extend(
                slide
                    .notes
                    .rich_text
                    .ranges
                    .iter()
                    .map(|range| {
                        let mut record = text_range_to_proto(&slide.notes.text, range);
                        record["kind"] = Value::String("textRange".to_string());
                        record["slide"] = Value::from(index + 1);
                        record["slideIndex"] = Value::from(index);
                        record["hostAnchor"] = Value::String(format!("nt/{}", slide.slide_id));
                        record["hostKind"] = Value::String("notes".to_string());
                        (record, Some(slide_id.clone()))
                    }),
            );
        }
        for element in &slide.elements {
            let mut record = match element {
                PresentationElement::Text(text) => {
                    if !include("textbox") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "textbox",
                        "id": format!("sh/{}", text.element_id),
                        "slide": index + 1,
                        "text": text.text,
                        "textStyle": text_style_to_proto(&text.style),
                        "textPreview": text.text.replace('\n', " | "),
                        "textChars": text.text.chars().count(),
                        "textLines": text.text.lines().count(),
                        "richText": rich_text_to_proto(&text.text, &text.rich_text),
                        "bbox": [text.frame.left, text.frame.top, text.frame.width, text.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Shape(shape) => {
                    if !(include("shape") || include("textbox") && shape.text.is_some()) {
                        continue;
                    }
                    let kind = if shape.text.is_some() && include("textbox") {
                        "textbox"
                    } else {
                        "shape"
                    };
                    let mut record = serde_json::json!({
                        "kind": kind,
                        "id": format!("sh/{}", shape.element_id),
                        "slide": index + 1,
                        "geometry": format!("{:?}", shape.geometry),
                        "text": shape.text,
                        "textStyle": text_style_to_proto(&shape.text_style),
                        "richText": shape
                            .text
                            .as_ref()
                            .zip(shape.rich_text.as_ref())
                            .map(|(text, rich_text)| rich_text_to_proto(text, rich_text))
                            .unwrap_or(Value::Null),
                        "rotation": shape.rotation_degrees,
                        "flipHorizontal": shape.flip_horizontal,
                        "flipVertical": shape.flip_vertical,
                        "bbox": [shape.frame.left, shape.frame.top, shape.frame.width, shape.frame.height],
                        "bboxUnit": "points",
                    });
                    if let Some(text) = &shape.text {
                        record["textPreview"] = Value::String(text.replace('\n', " | "));
                        record["textChars"] = Value::from(text.chars().count());
                        record["textLines"] = Value::from(text.lines().count());
                    }
                    record
                }
                PresentationElement::Connector(connector) => {
                    if !include("shape") && !include("connector") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "connector",
                        "id": format!("cn/{}", connector.element_id),
                        "slide": index + 1,
                        "connectorType": format!("{:?}", connector.connector_type),
                        "start": [connector.start.left, connector.start.top],
                        "end": [connector.end.left, connector.end.top],
                        "lineStyle": format!("{:?}", connector.line_style),
                        "label": connector.label,
                    })
                }
                PresentationElement::Table(table) => {
                    if !include("table") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "table",
                        "id": format!("tb/{}", table.element_id),
                        "slide": index + 1,
                        "rows": table.rows.len(),
                        "cols": table.rows.iter().map(std::vec::Vec::len).max().unwrap_or(0),
                        "columnWidths": table.column_widths,
                        "rowHeights": table.row_heights,
                        "preview": table.rows.first().map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>().join(" | ")),
                        "style": table.style,
                        "styleOptions": table_style_options_to_proto(&table.style_options),
                        "borders": table.borders.as_ref().map(table_borders_to_proto),
                        "rightToLeft": table.right_to_left,
                        "cellTextStyles": table
                            .rows
                            .iter()
                            .map(|row| row.iter().map(|cell| text_style_to_proto(&cell.text_style)).collect::<Vec<_>>())
                            .collect::<Vec<_>>(),
                        "rowsData": table
                            .rows
                            .iter()
                            .map(|row| row.iter().map(table_cell_to_proto).collect::<Vec<_>>())
                            .collect::<Vec<_>>(),
                        "bbox": [table.frame.left, table.frame.top, table.frame.width, table.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Chart(chart) => {
                    if !include("chart") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "chart",
                        "id": format!("ch/{}", chart.element_id),
                        "slide": index + 1,
                        "chartType": format!("{:?}", chart.chart_type),
                        "title": chart.title,
                        "styleIndex": chart.style_index,
                        "hasLegend": chart.has_legend,
                        "legend": chart.legend.as_ref().map(chart_legend_to_proto),
                        "xAxis": chart.x_axis.as_ref().map(chart_axis_to_proto),
                        "yAxis": chart.y_axis.as_ref().map(chart_axis_to_proto),
                        "dataLabels": chart.data_labels.as_ref().map(chart_data_labels_to_proto),
                        "chartFill": chart.chart_fill,
                        "plotAreaFill": chart.plot_area_fill,
                        "series": chart
                            .series
                            .iter()
                            .map(|series| serde_json::json!({
                                "name": series.name,
                                "values": series.values,
                                "categories": series.categories,
                                "xValues": series.x_values,
                                "fill": series.fill,
                                "stroke": series.stroke.as_ref().map(stroke_to_proto),
                                "marker": series.marker.as_ref().map(chart_marker_to_proto),
                                "dataLabelOverrides": series
                                    .data_label_overrides
                                    .iter()
                                    .map(chart_data_label_override_to_proto)
                                    .collect::<Vec<_>>(),
                            }))
                            .collect::<Vec<_>>(),
                        "bbox": [chart.frame.left, chart.frame.top, chart.frame.width, chart.frame.height],
                        "bboxUnit": "points",
                    })
                }
                PresentationElement::Image(image) => {
                    if !include("image") {
                        continue;
                    }
                    serde_json::json!({
                        "kind": "image",
                        "id": format!("im/{}", image.element_id),
                        "slide": index + 1,
                        "alt": image.alt_text,
                        "prompt": image.prompt,
                        "fit": format!("{:?}", image.fit_mode),
                        "rotation": image.rotation_degrees,
                        "flipHorizontal": image.flip_horizontal,
                        "flipVertical": image.flip_vertical,
                        "crop": image.crop.map(|(left, top, right, bottom)| serde_json::json!({
                            "left": left,
                            "top": top,
                            "right": right,
                            "bottom": bottom,
                        })),
                        "isPlaceholder": image.is_placeholder,
                        "lockAspectRatio": image.lock_aspect_ratio,
                        "bbox": [image.frame.left, image.frame.top, image.frame.width, image.frame.height],
                        "bboxUnit": "points",
                    })
                }
            };
            if let Some(placeholder) = match element {
                PresentationElement::Text(text) => text.placeholder.as_ref(),
                PresentationElement::Shape(shape) => shape.placeholder.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
                PresentationElement::Image(image) => image.placeholder.as_ref(),
            } {
                record["placeholder"] = Value::String(placeholder.placeholder_type.clone());
                record["placeholderName"] = Value::String(placeholder.name.clone());
                record["placeholderIndex"] =
                    placeholder.index.map(Value::from).unwrap_or(Value::Null);
            }
            if let PresentationElement::Shape(shape) = element
                && let Some(stroke) = &shape.stroke
            {
                record["stroke"] = serde_json::json!({
                    "color": stroke.color,
                    "width": stroke.width,
                    "style": stroke.style.as_api_str(),
                });
            }
            if let Some(hyperlink) = match element {
                PresentationElement::Text(text) => text.hyperlink.as_ref(),
                PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Image(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["hyperlink"] = hyperlink.to_json();
            }
            records.push((record, Some(slide_id.clone())));
            if include("textRange") {
                match element {
                    PresentationElement::Text(text) => {
                        records.extend(text.rich_text.ranges.iter().map(|range| {
                            let mut record = text_range_to_proto(&text.text, range);
                            record["kind"] = Value::String("textRange".to_string());
                            record["slide"] = Value::from(index + 1);
                            record["slideIndex"] = Value::from(index);
                            record["hostAnchor"] = Value::String(format!("sh/{}", text.element_id));
                            record["hostKind"] = Value::String("textbox".to_string());
                            (record, Some(slide_id.clone()))
                        }));
                    }
                    PresentationElement::Shape(shape) => {
                        if let Some((text, rich_text)) = shape.text.as_ref().zip(shape.rich_text.as_ref()) {
                            records.extend(rich_text.ranges.iter().map(|range| {
                                let mut record = text_range_to_proto(text, range);
                                record["kind"] = Value::String("textRange".to_string());
                                record["slide"] = Value::from(index + 1);
                                record["slideIndex"] = Value::from(index);
                                record["hostAnchor"] = Value::String(format!("sh/{}", shape.element_id));
                                record["hostKind"] = Value::String("textbox".to_string());
                                (record, Some(slide_id.clone()))
                            }));
                        }
                    }
                    PresentationElement::Table(table) => {
                        for (row_index, row) in table.rows.iter().enumerate() {
                            for (column_index, cell) in row.iter().enumerate() {
                                records.extend(cell.rich_text.ranges.iter().map(|range| {
                                    let mut record = text_range_to_proto(&cell.text, range);
                                    record["kind"] = Value::String("textRange".to_string());
                                    record["slide"] = Value::from(index + 1);
                                    record["slideIndex"] = Value::from(index);
                                    record["hostAnchor"] = Value::String(format!(
                                        "tb/{}#cell/{row_index}/{column_index}",
                                        table.element_id
                                    ));
                                    record["hostKind"] = Value::String("tableCell".to_string());
                                    (record, Some(slide_id.clone()))
                                }));
                            }
                        }
                    }
                    PresentationElement::Connector(_)
                    | PresentationElement::Image(_)
                    | PresentationElement::Chart(_) => {}
                }
            }
        }
    }
    if include("comment") {
        records.extend(document.comment_threads.iter().map(|thread| {
            let mut record = comment_thread_to_proto(thread);
            record["id"] = Value::String(format!("th/{}", thread.thread_id));
            (record, None)
        }));
    }

    if let Some(target_id) = args.target_id.as_deref() {
        records.retain(|(record, slide_id)| {
            legacy_target_matches(target_id, record, slide_id.as_deref())
        });
        if records.is_empty() {
            records.push((
                serde_json::json!({
                    "kind": "notice",
                    "noticeType": "targetNotFound",
                    "target": { "id": target_id },
                    "message": format!("No inspect records matched target `{target_id}`."),
                }),
                None,
            ));
        }
    }

    if let Some(search) = args.search.as_deref() {
        let search_lowercase = search.to_ascii_lowercase();
        records.retain(|(record, _)| {
            record
                .to_string()
                .to_ascii_lowercase()
                .contains(&search_lowercase)
        });
        if records.is_empty() {
            records.push((
                serde_json::json!({
                    "kind": "notice",
                    "noticeType": "noMatches",
                    "search": search,
                    "message": format!("No inspect records matched search `{search}`."),
                }),
                None,
            ));
        }
    }

    if let Some(target) = args.target.as_ref() {
        if let Some(target_index) = records.iter().position(|(record, _)| {
            record.get("id").and_then(Value::as_str) == Some(target.id.as_str())
        }) {
            let start = target_index.saturating_sub(target.before_lines.unwrap_or(0));
            let end = (target_index + target.after_lines.unwrap_or(0) + 1).min(records.len());
            records = records.into_iter().skip(start).take(end - start).collect();
        } else {
            records = vec![(
                serde_json::json!({
                    "kind": "notice",
                    "noticeType": "targetNotFound",
                    "target": {
                        "id": target.id,
                        "beforeLines": target.before_lines,
                        "afterLines": target.after_lines,
                    },
                    "message": format!("No inspect records matched target `{}`.", target.id),
                }),
                None,
            )];
        }
    }

    let mut lines = Vec::new();
    let mut omitted_lines = 0usize;
    let mut omitted_chars = 0usize;
    for line in records.into_iter().map(|(record, _)| record.to_string()) {
        let separator_len = usize::from(!lines.is_empty());
        if let Some(max_chars) = args.max_chars
            && lines.iter().map(String::len).sum::<usize>() + separator_len + line.len() > max_chars
        {
            omitted_lines += 1;
            omitted_chars += line.len();
            continue;
        }
        lines.push(line);
    }
    if omitted_lines > 0 {
        lines.push(
            serde_json::json!({
                "kind": "notice",
                "noticeType": "truncation",
                "maxChars": args.max_chars,
                "omittedLines": omitted_lines,
                "omittedChars": omitted_chars,
                "message": format!(
                    "Truncated inspect output by omitting {omitted_lines} lines. Increase maxChars or narrow the filter."
                ),
            })
            .to_string(),
        );
    }
    lines.join("\n")
}

fn legacy_target_matches(target_id: &str, record: &Value, slide_id: Option<&str>) -> bool {
    record.get("id").and_then(Value::as_str) == Some(target_id) || slide_id == Some(target_id)
}

fn add_text_metadata(record: &mut Value, text: &str) {
    record["textPreview"] = Value::String(text.replace('\n', " | "));
    record["textChars"] = Value::from(text.chars().count());
    record["textLines"] = Value::from(text.lines().count());
}

fn normalize_element_lookup_id(element_id: &str) -> &str {
    element_id
        .split_once('/')
        .map(|(_, normalized)| normalized)
        .unwrap_or(element_id)
}

fn resolve_anchor(
    document: &PresentationDocument,
    id: &str,
    action: &str,
) -> Result<Value, PresentationArtifactError> {
    if id == format!("pr/{}", document.artifact_id) {
        return Ok(serde_json::json!({
            "kind": "deck",
            "id": id,
            "artifactId": document.artifact_id,
            "name": document.name,
            "slideCount": document.slides.len(),
            "styleIds": document
                .named_text_styles()
                .iter()
                .map(|style| format!("st/{}", style.name))
                .collect::<Vec<_>>(),
            "activeSlideIndex": document.active_slide_index,
            "activeSlideId": document.active_slide_index.and_then(|index| document.slides.get(index)).map(|slide| format!("sl/{}", slide.slide_id)),
        }));
    }
    if let Some(style_name) = id.strip_prefix("st/") {
        let named_style = document
            .named_text_styles()
            .into_iter()
            .find(|style| style.name == style_name)
            .ok_or_else(|| PresentationArtifactError::UnsupportedFeature {
                action: action.to_string(),
                message: format!("unknown style id `{id}`"),
            })?;
        return Ok(named_text_style_to_json(&named_style, "st"));
    }

    for (slide_index, slide) in document.slides.iter().enumerate() {
        let slide_id = format!("sl/{}", slide.slide_id);
        if id == slide_id {
            return Ok(serde_json::json!({
                "kind": "slide",
                "id": slide_id,
                "slide": slide_index + 1,
                "slideIndex": slide_index,
                "isActive": document.active_slide_index == Some(slide_index),
                "layoutId": slide.layout_id,
                "notesId": (!slide.notes.text.is_empty()).then(|| format!("nt/{}", slide.slide_id)),
                "elementIds": slide.elements.iter().map(|element| {
                    let prefix = match element {
                        PresentationElement::Text(_) | PresentationElement::Shape(_) => "sh",
                        PresentationElement::Connector(_) => "cn",
                        PresentationElement::Image(_) => "im",
                        PresentationElement::Table(_) => "tb",
                        PresentationElement::Chart(_) => "ch",
                    };
                    format!("{prefix}/{}", element.element_id())
                }).collect::<Vec<_>>(),
            }));
        }
        let notes_id = format!("nt/{}", slide.slide_id);
        if id == notes_id {
            let mut record = serde_json::json!({
                "kind": "notes",
                "id": notes_id,
                "slide": slide_index + 1,
                "slideIndex": slide_index,
                "visible": slide.notes.visible,
                "text": slide.notes.text,
            });
            add_text_metadata(&mut record, &slide.notes.text);
            record["richText"] = rich_text_to_proto(&slide.notes.text, &slide.notes.rich_text);
            return Ok(record);
        }
        if let Some(range_id) = id.strip_prefix("tr/")
            && let Some(record) = slide
                .notes
                .rich_text
                .ranges
                .iter()
                .find(|range| range.range_id == range_id)
                .map(|range| {
                    let mut record = text_range_to_proto(&slide.notes.text, range);
                    record["kind"] = Value::String("textRange".to_string());
                    record["id"] = Value::String(id.to_string());
                    record["slide"] = Value::from(slide_index + 1);
                    record["slideIndex"] = Value::from(slide_index);
                    record["hostAnchor"] = Value::String(notes_id.clone());
                    record["hostKind"] = Value::String("notes".to_string());
                    record
                })
        {
            return Ok(record);
        }
        for element in &slide.elements {
            let mut record = match element {
                PresentationElement::Text(text) => {
                    let mut record = serde_json::json!({
                        "kind": "textbox",
                        "id": format!("sh/{}", text.element_id),
                        "elementId": text.element_id,
                        "slide": slide_index + 1,
                        "slideIndex": slide_index,
                        "text": text.text,
                        "textStyle": text_style_to_proto(&text.style),
                        "richText": rich_text_to_proto(&text.text, &text.rich_text),
                        "bbox": [text.frame.left, text.frame.top, text.frame.width, text.frame.height],
                        "bboxUnit": "points",
                    });
                    add_text_metadata(&mut record, &text.text);
                    record
                }
                PresentationElement::Shape(shape) => {
                    let mut record = serde_json::json!({
                        "kind": if shape.text.is_some() { "textbox" } else { "shape" },
                        "id": format!("sh/{}", shape.element_id),
                        "elementId": shape.element_id,
                        "slide": slide_index + 1,
                        "slideIndex": slide_index,
                        "geometry": format!("{:?}", shape.geometry),
                        "text": shape.text,
                        "textStyle": text_style_to_proto(&shape.text_style),
                        "richText": shape
                            .text
                            .as_ref()
                            .zip(shape.rich_text.as_ref())
                            .map(|(text, rich_text)| rich_text_to_proto(text, rich_text))
                            .unwrap_or(Value::Null),
                        "rotation": shape.rotation_degrees,
                        "flipHorizontal": shape.flip_horizontal,
                        "flipVertical": shape.flip_vertical,
                        "bbox": [shape.frame.left, shape.frame.top, shape.frame.width, shape.frame.height],
                        "bboxUnit": "points",
                    });
                    if let Some(text) = &shape.text {
                        add_text_metadata(&mut record, text);
                    }
                    record
                }
                PresentationElement::Connector(connector) => serde_json::json!({
                    "kind": "connector",
                    "id": format!("cn/{}", connector.element_id),
                    "elementId": connector.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "connectorType": format!("{:?}", connector.connector_type),
                    "start": [connector.start.left, connector.start.top],
                    "end": [connector.end.left, connector.end.top],
                    "lineStyle": format!("{:?}", connector.line_style),
                    "label": connector.label,
                }),
                PresentationElement::Image(image) => serde_json::json!({
                    "kind": "image",
                    "id": format!("im/{}", image.element_id),
                    "elementId": image.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "alt": image.alt_text,
                    "prompt": image.prompt,
                    "fit": format!("{:?}", image.fit_mode),
                    "rotation": image.rotation_degrees,
                    "flipHorizontal": image.flip_horizontal,
                    "flipVertical": image.flip_vertical,
                    "crop": image.crop.map(|(left, top, right, bottom)| serde_json::json!({
                        "left": left,
                        "top": top,
                        "right": right,
                        "bottom": bottom,
                    })),
                    "isPlaceholder": image.is_placeholder,
                    "lockAspectRatio": image.lock_aspect_ratio,
                    "bbox": [image.frame.left, image.frame.top, image.frame.width, image.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Table(table) => serde_json::json!({
                    "kind": "table",
                    "id": format!("tb/{}", table.element_id),
                    "elementId": table.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "rows": table.rows.len(),
                    "cols": table.rows.iter().map(std::vec::Vec::len).max().unwrap_or(0),
                    "columnWidths": table.column_widths,
                    "rowHeights": table.row_heights,
                    "style": table.style,
                    "styleOptions": table_style_options_to_proto(&table.style_options),
                    "borders": table.borders.as_ref().map(table_borders_to_proto),
                    "rightToLeft": table.right_to_left,
                    "cellTextStyles": table
                        .rows
                        .iter()
                        .map(|row| row.iter().map(|cell| text_style_to_proto(&cell.text_style)).collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                    "rowsData": table
                        .rows
                        .iter()
                        .map(|row| row.iter().map(table_cell_to_proto).collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                    "bbox": [table.frame.left, table.frame.top, table.frame.width, table.frame.height],
                    "bboxUnit": "points",
                }),
                PresentationElement::Chart(chart) => serde_json::json!({
                    "kind": "chart",
                    "id": format!("ch/{}", chart.element_id),
                    "elementId": chart.element_id,
                    "slide": slide_index + 1,
                    "slideIndex": slide_index,
                    "chartType": format!("{:?}", chart.chart_type),
                    "title": chart.title,
                    "styleIndex": chart.style_index,
                    "hasLegend": chart.has_legend,
                    "legend": chart.legend.as_ref().map(chart_legend_to_proto),
                    "xAxis": chart.x_axis.as_ref().map(chart_axis_to_proto),
                    "yAxis": chart.y_axis.as_ref().map(chart_axis_to_proto),
                    "dataLabels": chart.data_labels.as_ref().map(chart_data_labels_to_proto),
                    "chartFill": chart.chart_fill,
                    "plotAreaFill": chart.plot_area_fill,
                    "series": chart
                        .series
                        .iter()
                        .map(|series| serde_json::json!({
                            "name": series.name,
                            "values": series.values,
                            "categories": series.categories,
                            "xValues": series.x_values,
                            "fill": series.fill,
                            "stroke": series.stroke.as_ref().map(stroke_to_proto),
                            "marker": series.marker.as_ref().map(chart_marker_to_proto),
                            "dataLabelOverrides": series
                                .data_label_overrides
                                .iter()
                                .map(chart_data_label_override_to_proto)
                                .collect::<Vec<_>>(),
                        }))
                        .collect::<Vec<_>>(),
                    "bbox": [chart.frame.left, chart.frame.top, chart.frame.width, chart.frame.height],
                    "bboxUnit": "points",
                }),
            };
            if let Some(hyperlink) = match element {
                PresentationElement::Text(text) => text.hyperlink.as_ref(),
                PresentationElement::Shape(shape) => shape.hyperlink.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Image(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["hyperlink"] = hyperlink.to_json();
            }
            if let PresentationElement::Shape(shape) = element
                && let Some(stroke) = &shape.stroke
            {
                record["stroke"] = serde_json::json!({
                    "color": stroke.color,
                    "width": stroke.width,
                    "style": stroke.style.as_api_str(),
                });
            }
            if let Some(placeholder) = match element {
                PresentationElement::Text(text) => text.placeholder.as_ref(),
                PresentationElement::Shape(shape) => shape.placeholder.as_ref(),
                PresentationElement::Image(image) => image.placeholder.as_ref(),
                PresentationElement::Connector(_)
                | PresentationElement::Table(_)
                | PresentationElement::Chart(_) => None,
            } {
                record["placeholder"] = Value::String(placeholder.placeholder_type.clone());
                record["placeholderName"] = Value::String(placeholder.name.clone());
                record["placeholderIndex"] =
                    placeholder.index.map(Value::from).unwrap_or(Value::Null);
            }
            if record.get("id").and_then(Value::as_str) == Some(id) {
                return Ok(record);
            }
            if let Some(range_id) = id.strip_prefix("tr/") {
                match element {
                    PresentationElement::Text(text) => {
                        if let Some(range) =
                            text.rich_text.ranges.iter().find(|range| range.range_id == range_id)
                        {
                            let mut range_record = text_range_to_proto(&text.text, range);
                            range_record["kind"] = Value::String("textRange".to_string());
                            range_record["id"] = Value::String(id.to_string());
                            range_record["slide"] = Value::from(slide_index + 1);
                            range_record["slideIndex"] = Value::from(slide_index);
                            range_record["hostAnchor"] =
                                Value::String(format!("sh/{}", text.element_id));
                            range_record["hostKind"] = Value::String("textbox".to_string());
                            return Ok(range_record);
                        }
                    }
                    PresentationElement::Shape(shape) => {
                        if let Some((text, rich_text)) =
                            shape.text.as_ref().zip(shape.rich_text.as_ref())
                            && let Some(range) =
                                rich_text.ranges.iter().find(|range| range.range_id == range_id)
                        {
                            let mut range_record = text_range_to_proto(text, range);
                            range_record["kind"] = Value::String("textRange".to_string());
                            range_record["id"] = Value::String(id.to_string());
                            range_record["slide"] = Value::from(slide_index + 1);
                            range_record["slideIndex"] = Value::from(slide_index);
                            range_record["hostAnchor"] =
                                Value::String(format!("sh/{}", shape.element_id));
                            range_record["hostKind"] = Value::String("textbox".to_string());
                            return Ok(range_record);
                        }
                    }
                    PresentationElement::Table(table) => {
                        for (row_index, row) in table.rows.iter().enumerate() {
                            for (column_index, cell) in row.iter().enumerate() {
                                if let Some(range) = cell
                                    .rich_text
                                    .ranges
                                    .iter()
                                    .find(|range| range.range_id == range_id)
                                {
                                    let mut range_record = text_range_to_proto(&cell.text, range);
                                    range_record["kind"] = Value::String("textRange".to_string());
                                    range_record["id"] = Value::String(id.to_string());
                                    range_record["slide"] = Value::from(slide_index + 1);
                                    range_record["slideIndex"] = Value::from(slide_index);
                                    range_record["hostAnchor"] = Value::String(format!(
                                        "tb/{}#cell/{row_index}/{column_index}",
                                        table.element_id
                                    ));
                                    range_record["hostKind"] =
                                        Value::String("tableCell".to_string());
                                    return Ok(range_record);
                                }
                            }
                        }
                    }
                    PresentationElement::Connector(_)
                    | PresentationElement::Image(_)
                    | PresentationElement::Chart(_) => {}
                }
            }
        }
    }

    if let Some(thread_id) = id.strip_prefix("th/")
        && let Some(thread) = document
            .comment_threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
    {
        let mut record = comment_thread_to_proto(thread);
        record["id"] = Value::String(id.to_string());
        return Ok(record);
    }

    for layout in &document.layouts {
        let layout_id = format!("ly/{}", layout.layout_id);
        if id == layout_id {
            return Ok(serde_json::json!({
                "kind": "layout",
                "id": layout_id,
                "layoutId": layout.layout_id,
                "name": layout.name,
                "type": match layout.kind {
                    LayoutKind::Layout => "layout",
                    LayoutKind::Master => "master",
                },
                "parentLayoutId": layout.parent_layout_id,
                "placeholders": layout_placeholder_list(document, &layout.layout_id, action)?,
            }));
        }
    }

    Err(PresentationArtifactError::UnsupportedFeature {
        action: action.to_string(),
        message: format!("unknown resolve id `{id}`"),
    })
}
