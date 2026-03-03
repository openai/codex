use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use regex::Regex;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCell;
use crate::SpreadsheetCellValue;
use crate::SpreadsheetSheet;

pub(crate) fn write_xlsx(
    artifact: &mut SpreadsheetArtifact,
    path: &Path,
) -> Result<PathBuf, SpreadsheetArtifactError> {
    if artifact.auto_recalculate {
        artifact.recalculate();
    }
    for sheet in &mut artifact.sheets {
        sheet.cleanup_and_validate_sheet()?;
    }

    let file = File::create(path).map_err(|error| SpreadsheetArtifactError::ExportFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    let sheet_count = artifact.sheets.len().max(1);
    zip.start_file("[Content_Types].xml", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.write_all(content_types_xml(sheet_count).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.add_directory("_rels/", options).map_err(|error| {
        SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    zip.start_file("_rels/.rels", options).map_err(|error| {
        SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    zip.write_all(root_relationships_xml().as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.add_directory("docProps/", options).map_err(|error| {
        SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    zip.start_file("docProps/app.xml", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.write_all(app_xml(artifact).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.start_file("docProps/core.xml", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.write_all(core_xml(artifact).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.add_directory("xl/", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.start_file("xl/workbook.xml", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.write_all(workbook_xml(artifact).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.add_directory("xl/_rels/", options).map_err(|error| {
        SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    zip.write_all(workbook_relationships_xml(artifact).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.start_file("xl/styles.xml", options).map_err(|error| {
        SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    zip.write_all(styles_xml(artifact).as_bytes())
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    zip.add_directory("xl/worksheets/", options)
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if artifact.sheets.is_empty() {
        let empty = SpreadsheetSheet::new("Sheet1".to_string());
        zip.start_file("xl/worksheets/sheet1.xml", options)
            .map_err(|error| SpreadsheetArtifactError::ExportFailed {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        zip.write_all(sheet_xml(&empty).as_bytes())
            .map_err(|error| SpreadsheetArtifactError::ExportFailed {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    } else {
        for (index, sheet) in artifact.sheets.iter().enumerate() {
            zip.start_file(format!("xl/worksheets/sheet{}.xml", index + 1), options)
                .map_err(|error| SpreadsheetArtifactError::ExportFailed {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
            zip.write_all(sheet_xml(sheet).as_bytes())
                .map_err(|error| SpreadsheetArtifactError::ExportFailed {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        }
    }

    zip.finish()
        .map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    Ok(path.to_path_buf())
}

pub(crate) fn import_xlsx(
    path: &Path,
    artifact_id: Option<String>,
) -> Result<SpreadsheetArtifact, SpreadsheetArtifactError> {
    let file = File::open(path).map_err(|error| SpreadsheetArtifactError::ImportFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| SpreadsheetArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

    let workbook_xml = read_zip_entry(&mut archive, "xl/workbook.xml", path)?;
    let workbook_rels = read_zip_entry(&mut archive, "xl/_rels/workbook.xml.rels", path)?;
    let workbook_name = if archive.by_name("docProps/core.xml").is_ok() {
        let title =
            extract_workbook_title(&read_zip_entry(&mut archive, "docProps/core.xml", path)?);
        (!title.trim().is_empty()).then_some(title)
    } else {
        None
    };
    let shared_strings = if archive.by_name("xl/sharedStrings.xml").is_ok() {
        Some(parse_shared_strings(&read_zip_entry(
            &mut archive,
            "xl/sharedStrings.xml",
            path,
        )?)?)
    } else {
        None
    };

    let relationships = parse_relationships(&workbook_rels)?;
    let sheets = parse_sheet_definitions(&workbook_xml)?
        .into_iter()
        .map(|(name, relation)| {
            let target = relationships.get(&relation).ok_or_else(|| {
                SpreadsheetArtifactError::ImportFailed {
                    path: path.to_path_buf(),
                    message: format!("missing relationship `{relation}` for sheet `{name}`"),
                }
            })?;
            let normalized = if target.starts_with('/') {
                target.trim_start_matches('/').to_string()
            } else if target.starts_with("xl/") {
                target.clone()
            } else {
                format!("xl/{target}")
            };
            Ok((name, normalized))
        })
        .collect::<Result<Vec<_>, SpreadsheetArtifactError>>()?;

    let mut artifact = SpreadsheetArtifact::new(workbook_name.or_else(|| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_string)
    }));
    if let Some(artifact_id) = artifact_id {
        artifact.artifact_id = artifact_id;
    }
    artifact.sheets.clear();

    for (name, target) in sheets {
        let xml = read_zip_entry(&mut archive, &target, path)?;
        let sheet = parse_sheet(&name, &xml, shared_strings.as_deref())?;
        artifact.sheets.push(sheet);
    }

    Ok(artifact)
}

fn read_zip_entry(
    archive: &mut ZipArchive<File>,
    entry: &str,
    path: &Path,
) -> Result<String, SpreadsheetArtifactError> {
    let mut file =
        archive
            .by_name(entry)
            .map_err(|error| SpreadsheetArtifactError::ImportFailed {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| SpreadsheetArtifactError::ImportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    Ok(text)
}

fn parse_sheet_definitions(
    workbook_xml: &str,
) -> Result<Vec<(String, String)>, SpreadsheetArtifactError> {
    let regex = Regex::new(r#"<sheet\b([^>]*)/?>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    let mut sheets = Vec::new();
    for captures in regex.captures_iter(workbook_xml) {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(name) = extract_attribute(attributes, "name") else {
            continue;
        };
        let relation = extract_attribute(attributes, "r:id")
            .or_else(|| extract_attribute(attributes, "id"))
            .unwrap_or_default();
        sheets.push((xml_unescape(&name), relation));
    }
    Ok(sheets)
}

fn parse_relationships(xml: &str) -> Result<BTreeMap<String, String>, SpreadsheetArtifactError> {
    let regex = Regex::new(r#"<Relationship\b([^>]*)/?>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    Ok(regex
        .captures_iter(xml)
        .filter_map(|captures| {
            let attributes = captures.get(1)?.as_str();
            let id = extract_attribute(attributes, "Id")?;
            let target = extract_attribute(attributes, "Target")?;
            Some((id, target))
        })
        .collect())
}

fn parse_shared_strings(xml: &str) -> Result<Vec<String>, SpreadsheetArtifactError> {
    let regex = Regex::new(r#"(?s)<si\b[^>]*>(.*?)</si>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    regex
        .captures_iter(xml)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str()))
        .map(all_text_nodes)
        .collect()
}

fn parse_sheet(
    name: &str,
    xml: &str,
    shared_strings: Option<&[String]>,
) -> Result<SpreadsheetSheet, SpreadsheetArtifactError> {
    let mut sheet = SpreadsheetSheet::new(name.to_string());

    if let Some(sheet_view) = first_tag_attributes(xml, "sheetView")
        && let Some(show_grid_lines) = extract_attribute(&sheet_view, "showGridLines")
    {
        sheet.show_grid_lines = show_grid_lines != "0";
    }
    if let Some(format_pr) = first_tag_attributes(xml, "sheetFormatPr") {
        sheet.default_row_height = extract_attribute(&format_pr, "defaultRowHeight")
            .and_then(|value| value.parse::<f64>().ok());
        sheet.default_column_width = extract_attribute(&format_pr, "defaultColWidth")
            .and_then(|value| value.parse::<f64>().ok());
    }

    let col_regex = Regex::new(r#"<col\b([^>]*)/?>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    for captures in col_regex.captures_iter(xml) {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(min) =
            extract_attribute(attributes, "min").and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(max) =
            extract_attribute(attributes, "max").and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(width) =
            extract_attribute(attributes, "width").and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        for column in min..=max {
            sheet.column_widths.insert(column, width);
        }
    }

    let row_regex = Regex::new(r#"(?s)<row\b([^>]*)>(.*?)</row>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    let cell_regex = Regex::new(r#"(?s)<c\b([^>]*)>(.*?)</c>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    for row_captures in row_regex.captures_iter(xml) {
        let row_attributes = row_captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        if let Some(row_index) =
            extract_attribute(row_attributes, "r").and_then(|value| value.parse::<u32>().ok())
            && let Some(height) =
                extract_attribute(row_attributes, "ht").and_then(|value| value.parse::<f64>().ok())
            && row_index > 0
            && height > 0.0
        {
            sheet.row_heights.insert(row_index, height);
        }
        let Some(row_body) = row_captures.get(2).map(|value| value.as_str()) else {
            continue;
        };
        for cell_captures in cell_regex.captures_iter(row_body) {
            let Some(attributes) = cell_captures.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(body) = cell_captures.get(2).map(|value| value.as_str()) else {
                continue;
            };
            let Some(address) = extract_attribute(attributes, "r") else {
                continue;
            };
            let address = CellAddress::parse(&address)?;
            let style_index = extract_attribute(attributes, "s")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let cell_type = extract_attribute(attributes, "t").unwrap_or_default();
            let formula = first_tag_text(body, "f").map(|value| format!("={value}"));
            let value = parse_cell_value(body, &cell_type, shared_strings)?;

            let cell = SpreadsheetCell {
                value,
                formula,
                style_index,
                citations: Vec::new(),
            };
            if !cell.is_empty() {
                sheet.cells.insert(address, cell);
            }
        }
    }

    let merge_regex = Regex::new(r#"<mergeCell\b([^>]*)/?>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    for captures in merge_regex.captures_iter(xml) {
        let Some(attributes) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if let Some(reference) = extract_attribute(attributes, "ref") {
            sheet.merged_ranges.push(CellRange::parse(&reference)?);
        }
    }

    Ok(sheet)
}

fn parse_cell_value(
    body: &str,
    cell_type: &str,
    shared_strings: Option<&[String]>,
) -> Result<Option<SpreadsheetCellValue>, SpreadsheetArtifactError> {
    let inline_text = first_tag_text(body, "t").map(|value| xml_unescape(&value));
    let raw_value = first_tag_text(body, "v").map(|value| xml_unescape(&value));

    let parsed = match cell_type {
        "inlineStr" => inline_text.map(SpreadsheetCellValue::String),
        "s" => raw_value
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|index| shared_strings.and_then(|entries| entries.get(index).cloned()))
            .map(SpreadsheetCellValue::String),
        "b" => raw_value.map(|value| SpreadsheetCellValue::Bool(value == "1")),
        "str" => raw_value.map(SpreadsheetCellValue::String),
        "e" => raw_value.map(SpreadsheetCellValue::Error),
        _ => match raw_value {
            Some(value) => {
                if let Ok(integer) = value.parse::<i64>() {
                    Some(SpreadsheetCellValue::Integer(integer))
                } else if let Ok(float) = value.parse::<f64>() {
                    Some(SpreadsheetCellValue::Float(float))
                } else {
                    Some(SpreadsheetCellValue::String(value))
                }
            }
            None => None,
        },
    };
    Ok(parsed)
}

fn content_types_xml(sheet_count: usize) -> String {
    let mut overrides = String::new();
    for index in 1..=sheet_count {
        overrides.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
    }
    format!(
        "{}{}{}{}{}{}{}{}{}{}",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
        r#"<Default Extension="xml" ContentType="application/xml"/>"#,
        r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
        r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
        r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#,
        r#"<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
        overrides,
        r#"</Types>"#
    )
}

fn root_relationships_xml() -> &'static str {
    concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>"#,
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#,
        r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>"#,
        r#"</Relationships>"#
    )
}

fn app_xml(artifact: &SpreadsheetArtifact) -> String {
    let title = artifact
        .name
        .clone()
        .unwrap_or_else(|| "Spreadsheet".to_string());
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">"#,
            r#"<Application>Codex</Application>"#,
            r#"<DocSecurity>0</DocSecurity>"#,
            r#"<ScaleCrop>false</ScaleCrop>"#,
            r#"<HeadingPairs><vt:vector size="2" baseType="variant"><vt:variant><vt:lpstr>Worksheets</vt:lpstr></vt:variant><vt:variant><vt:i4>{}</vt:i4></vt:variant></vt:vector></HeadingPairs>"#,
            r#"<TitlesOfParts><vt:vector size="{}" baseType="lpstr">{}</vt:vector></TitlesOfParts>"#,
            r#"<Company>OpenAI</Company>"#,
            r#"<Manager>{}</Manager>"#,
            r#"</Properties>"#
        ),
        artifact.sheets.len(),
        artifact.sheets.len(),
        artifact
            .sheets
            .iter()
            .map(|sheet| format!(r#"<vt:lpstr>{}</vt:lpstr>"#, xml_escape(&sheet.name)))
            .collect::<Vec<_>>()
            .join(""),
        xml_escape(&title),
    )
}

fn core_xml(artifact: &SpreadsheetArtifact) -> String {
    let title = artifact
        .name
        .clone()
        .unwrap_or_else(|| artifact.artifact_id.clone());
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
            r#"<dc:title>{}</dc:title>"#,
            r#"<dc:creator>Codex</dc:creator>"#,
            r#"<cp:lastModifiedBy>Codex</cp:lastModifiedBy>"#,
            r#"</cp:coreProperties>"#
        ),
        xml_escape(&title),
    )
}

fn workbook_xml(artifact: &SpreadsheetArtifact) -> String {
    let sheets = if artifact.sheets.is_empty() {
        r#"<sheet name="Sheet1" sheetId="1" r:id="rId1"/>"#.to_string()
    } else {
        artifact
            .sheets
            .iter()
            .enumerate()
            .map(|(index, sheet)| {
                format!(
                    r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
                    xml_escape(&sheet.name),
                    index + 1,
                    index + 1
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    format!(
        "{}{}{}<sheets>{}</sheets>{}",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        r#"<bookViews><workbookView/></bookViews>"#,
        sheets,
        r#"</workbook>"#
    )
}

fn workbook_relationships_xml(artifact: &SpreadsheetArtifact) -> String {
    let sheet_relationships = if artifact.sheets.is_empty() {
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>"#.to_string()
    } else {
        artifact
            .sheets
            .iter()
            .enumerate()
            .map(|(index, _)| {
                format!(
                    r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#,
                    index + 1,
                    index + 1
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let style_relation_id = artifact.sheets.len().max(1) + 1;
    format!(
        "{}{}{}<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>{}",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        sheet_relationships,
        style_relation_id,
        r#"</Relationships>"#
    )
}

fn styles_xml(artifact: &SpreadsheetArtifact) -> String {
    let max_style_index = artifact
        .sheets
        .iter()
        .flat_map(|sheet| sheet.cells.values().map(|cell| cell.style_index))
        .max()
        .unwrap_or(0);
    let cell_xfs = (0..=max_style_index)
        .map(|_| r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>"#)
        .collect::<Vec<_>>()
        .join("");
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
            r#"<fonts count="1"><font/></fonts>"#,
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
            r#"<borders count="1"><border/></borders>"#,
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
            r#"<cellXfs count="{}">{}</cellXfs>"#,
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
            r#"</styleSheet>"#
        ),
        max_style_index + 1,
        cell_xfs,
    )
}

fn sheet_xml(sheet: &SpreadsheetSheet) -> String {
    let mut rows = BTreeMap::<u32, Vec<(CellAddress, &SpreadsheetCell)>>::new();
    for row_index in sheet.row_heights.keys() {
        rows.entry(*row_index).or_default();
    }
    for (address, cell) in &sheet.cells {
        rows.entry(address.row).or_default().push((*address, cell));
    }

    let sheet_data = rows
        .into_iter()
        .map(|(row_index, mut entries)| {
            entries.sort_by_key(|(address, _)| address.column);
            let cells = entries
                .into_iter()
                .map(|(address, cell)| cell_xml(address, cell))
                .collect::<Vec<_>>()
                .join("");
            let height = sheet
                .row_heights
                .get(&row_index)
                .map(|value| format!(r#" ht="{value}" customHeight="1""#))
                .unwrap_or_default();
            format!(r#"<row r="{row_index}"{height}>{cells}</row>"#)
        })
        .collect::<Vec<_>>()
        .join("");

    let cols = if sheet.column_widths.is_empty() {
        String::new()
    } else {
        let mut groups = Vec::new();
        let mut iter = sheet.column_widths.iter().peekable();
        while let Some((&start, &width)) = iter.next() {
            let mut end = start;
            while let Some((next_column, next_width)) =
                iter.peek().map(|(column, width)| (**column, **width))
            {
                if next_column == end + 1 && (next_width - width).abs() < f64::EPSILON {
                    end = next_column;
                    iter.next();
                } else {
                    break;
                }
            }
            groups.push(format!(
                r#"<col min="{start}" max="{end}" width="{width}" customWidth="1"/>"#
            ));
        }
        format!("<cols>{}</cols>", groups.join(""))
    };

    let merge_cells = if sheet.merged_ranges.is_empty() {
        String::new()
    } else {
        format!(
            r#"<mergeCells count="{}">{}</mergeCells>"#,
            sheet.merged_ranges.len(),
            sheet
                .merged_ranges
                .iter()
                .map(|range| format!(r#"<mergeCell ref="{}"/>"#, range.to_a1()))
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let default_row_height = sheet.default_row_height.unwrap_or(15.0);
    let default_column_width = sheet.default_column_width.unwrap_or(8.43);
    let grid_lines = if sheet.show_grid_lines { "1" } else { "0" };

    format!(
        "{}{}<sheetViews><sheetView workbookViewId=\"0\" showGridLines=\"{}\"/></sheetViews><sheetFormatPr defaultRowHeight=\"{}\" defaultColWidth=\"{}\"/>{}<sheetData>{}</sheetData>{}{}",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        grid_lines,
        default_row_height,
        default_column_width,
        cols,
        sheet_data,
        merge_cells,
        r#"</worksheet>"#
    )
}

fn cell_xml(address: CellAddress, cell: &SpreadsheetCell) -> String {
    let style = if cell.style_index == 0 {
        String::new()
    } else {
        format!(r#" s="{}""#, cell.style_index)
    };

    if let Some(formula) = &cell.formula {
        let formula = xml_escape(formula.trim_start_matches('='));
        let value_xml = match &cell.value {
            Some(SpreadsheetCellValue::Bool(value)) => {
                format!(
                    r#" t="b"><f>{formula}</f><v>{}</v></c>"#,
                    usize::from(*value)
                )
            }
            Some(SpreadsheetCellValue::Integer(value)) => {
                format!(r#"><f>{formula}</f><v>{value}</v></c>"#)
            }
            Some(SpreadsheetCellValue::Float(value)) => {
                format!(r#"><f>{formula}</f><v>{value}</v></c>"#)
            }
            Some(SpreadsheetCellValue::String(value))
            | Some(SpreadsheetCellValue::DateTime(value)) => format!(
                r#" t="str"><f>{formula}</f><v>{}</v></c>"#,
                xml_escape(value)
            ),
            Some(SpreadsheetCellValue::Error(value)) => {
                format!(r#" t="e"><f>{formula}</f><v>{}</v></c>"#, xml_escape(value))
            }
            None => format!(r#"><f>{formula}</f></c>"#),
        };
        return format!(r#"<c r="{}"{style}{value_xml}"#, address.to_a1());
    }

    match &cell.value {
        Some(SpreadsheetCellValue::Bool(value)) => format!(
            r#"<c r="{}"{style} t="b"><v>{}</v></c>"#,
            address.to_a1(),
            usize::from(*value)
        ),
        Some(SpreadsheetCellValue::Integer(value)) => {
            format!(r#"<c r="{}"{style}><v>{value}</v></c>"#, address.to_a1())
        }
        Some(SpreadsheetCellValue::Float(value)) => {
            format!(r#"<c r="{}"{style}><v>{value}</v></c>"#, address.to_a1())
        }
        Some(SpreadsheetCellValue::String(value)) | Some(SpreadsheetCellValue::DateTime(value)) => {
            format!(
                r#"<c r="{}"{style} t="inlineStr"><is><t>{}</t></is></c>"#,
                address.to_a1(),
                xml_escape(value)
            )
        }
        Some(SpreadsheetCellValue::Error(value)) => format!(
            r#"<c r="{}"{style} t="e"><v>{}</v></c>"#,
            address.to_a1(),
            xml_escape(value)
        ),
        None => format!(r#"<c r="{}"{style}/>"#, address.to_a1()),
    }
}

fn first_tag_attributes(xml: &str, tag: &str) -> Option<String> {
    let regex = Regex::new(&format!(r#"<{tag}\b([^>]*)/?>"#)).ok()?;
    let captures = regex.captures(xml)?;
    captures.get(1).map(|value| value.as_str().to_string())
}

fn first_tag_text(xml: &str, tag: &str) -> Option<String> {
    let regex = Regex::new(&format!(r#"(?s)<{tag}\b[^>]*>(.*?)</{tag}>"#)).ok()?;
    let captures = regex.captures(xml)?;
    captures.get(1).map(|value| value.as_str().to_string())
}

fn extract_workbook_title(xml: &str) -> String {
    let Ok(regex) =
        Regex::new(r#"(?s)<(?:[A-Za-z0-9_]+:)?title\b[^>]*>(.*?)</(?:[A-Za-z0-9_]+:)?title>"#)
    else {
        return String::new();
    };
    regex
        .captures(xml)
        .and_then(|captures| captures.get(1).map(|value| xml_unescape(value.as_str())))
        .unwrap_or_default()
}

fn all_text_nodes(xml: &str) -> Result<String, SpreadsheetArtifactError> {
    let regex = Regex::new(r#"(?s)<t\b[^>]*>(.*?)</t>"#).map_err(|error| {
        SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        }
    })?;
    Ok(regex
        .captures_iter(xml)
        .filter_map(|captures| captures.get(1).map(|value| xml_unescape(value.as_str())))
        .collect::<Vec<_>>()
        .join(""))
}

fn extract_attribute(attributes: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"{name}="([^"]*)""#);
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(attributes)?;
    captures.get(1).map(|value| xml_unescape(value.as_str()))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}
