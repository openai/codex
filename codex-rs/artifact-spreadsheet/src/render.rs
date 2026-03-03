use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetRenderOptions {
    pub output_path: Option<PathBuf>,
    pub center_address: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub include_headers: bool,
    pub scale: f64,
    pub performance_mode: bool,
}

impl Default for SpreadsheetRenderOptions {
    fn default() -> Self {
        Self {
            output_path: None,
            center_address: None,
            width: None,
            height: None,
            include_headers: true,
            scale: 1.0,
            performance_mode: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpreadsheetRenderedOutput {
    pub path: PathBuf,
    pub html: String,
}

impl SpreadsheetSheet {
    pub fn render_html(
        &self,
        range: Option<&CellRange>,
        options: &SpreadsheetRenderOptions,
    ) -> Result<String, SpreadsheetArtifactError> {
        let center = options
            .center_address
            .as_deref()
            .map(CellAddress::parse)
            .transpose()?;
        let viewport = render_viewport(self, range, center, options)?;
        let title = range
            .map(CellRange::to_a1)
            .unwrap_or_else(|| self.name.clone());
        Ok(format!(
            concat!(
                "<!doctype html><html><head><meta charset=\"utf-8\">",
                "<title>{}</title>",
                "<style>{}</style>",
                "</head><body>",
                "<section class=\"spreadsheet-preview\" data-sheet=\"{}\" data-performance-mode=\"{}\">",
                "<header><h1>{}</h1><p>{}</p></header>",
                "<div class=\"viewport\" style=\"{}\">",
                "<table>{}</table>",
                "</div></section></body></html>"
            ),
            html_escape(&title),
            preview_css(),
            html_escape(&self.name),
            options.performance_mode,
            html_escape(&title),
            html_escape(&viewport.to_a1()),
            viewport_style(options),
            render_table(self, &viewport, options),
        ))
    }
}

impl SpreadsheetArtifact {
    pub fn render_workbook_previews(
        &self,
        cwd: &Path,
        options: &SpreadsheetRenderOptions,
    ) -> Result<Vec<SpreadsheetRenderedOutput>, SpreadsheetArtifactError> {
        let sheets = if self.sheets.is_empty() {
            vec![SpreadsheetSheet::new("Sheet1".to_string())]
        } else {
            self.sheets.clone()
        };
        let output_paths = workbook_output_paths(self, cwd, options, &sheets);
        sheets
            .iter()
            .zip(output_paths)
            .map(|(sheet, path)| {
                let html = sheet.render_html(None, options)?;
                write_rendered_output(&path, &html)?;
                Ok(SpreadsheetRenderedOutput { path, html })
            })
            .collect()
    }

    pub fn render_sheet_preview(
        &self,
        cwd: &Path,
        sheet: &SpreadsheetSheet,
        options: &SpreadsheetRenderOptions,
    ) -> Result<SpreadsheetRenderedOutput, SpreadsheetArtifactError> {
        let path = single_output_path(
            cwd,
            self,
            options.output_path.as_deref(),
            &format!("render_{}", sanitize_file_component(&sheet.name)),
        );
        let html = sheet.render_html(None, options)?;
        write_rendered_output(&path, &html)?;
        Ok(SpreadsheetRenderedOutput { path, html })
    }

    pub fn render_range_preview(
        &self,
        cwd: &Path,
        sheet: &SpreadsheetSheet,
        range: &CellRange,
        options: &SpreadsheetRenderOptions,
    ) -> Result<SpreadsheetRenderedOutput, SpreadsheetArtifactError> {
        let path = single_output_path(
            cwd,
            self,
            options.output_path.as_deref(),
            &format!(
                "render_{}_{}",
                sanitize_file_component(&sheet.name),
                sanitize_file_component(&range.to_a1())
            ),
        );
        let html = sheet.render_html(Some(range), options)?;
        write_rendered_output(&path, &html)?;
        Ok(SpreadsheetRenderedOutput { path, html })
    }
}

fn render_viewport(
    sheet: &SpreadsheetSheet,
    range: Option<&CellRange>,
    center: Option<CellAddress>,
    options: &SpreadsheetRenderOptions,
) -> Result<CellRange, SpreadsheetArtifactError> {
    let base = range
        .cloned()
        .or_else(|| sheet.minimum_range())
        .unwrap_or_else(|| {
            CellRange::from_start_end(
                CellAddress { column: 1, row: 1 },
                CellAddress { column: 1, row: 1 },
            )
        });
    let Some(center) = center else {
        return Ok(base);
    };
    let visible_columns = options
        .width
        .map(|width| estimated_visible_count(width, 96.0, options.scale))
        .unwrap_or(base.width() as u32);
    let visible_rows = options
        .height
        .map(|height| estimated_visible_count(height, 28.0, options.scale))
        .unwrap_or(base.height() as u32);

    let half_columns = visible_columns / 2;
    let half_rows = visible_rows / 2;
    let start_column = center
        .column
        .saturating_sub(half_columns)
        .max(base.start.column);
    let start_row = center.row.saturating_sub(half_rows).max(base.start.row);
    let end_column = (start_column + visible_columns.saturating_sub(1)).min(base.end.column);
    let end_row = (start_row + visible_rows.saturating_sub(1)).min(base.end.row);
    Ok(CellRange::from_start_end(
        CellAddress {
            column: start_column,
            row: start_row,
        },
        CellAddress {
            column: end_column.max(start_column),
            row: end_row.max(start_row),
        },
    ))
}

fn estimated_visible_count(dimension: u32, cell_size: f64, scale: f64) -> u32 {
    ((dimension as f64 / (cell_size * scale.max(0.1))).floor() as u32).max(1)
}

fn render_table(
    sheet: &SpreadsheetSheet,
    range: &CellRange,
    options: &SpreadsheetRenderOptions,
) -> String {
    let mut rows = Vec::new();
    if options.include_headers {
        let mut header = vec!["<tr><th class=\"corner\"></th>".to_string()];
        for column in range.start.column..=range.end.column {
            header.push(format!(
                "<th>{}</th>",
                crate::column_index_to_letters(column)
            ));
        }
        header.push("</tr>".to_string());
        rows.push(header.join(""));
    }
    for row in range.start.row..=range.end.row {
        let mut cells = Vec::new();
        if options.include_headers {
            cells.push(format!("<th>{row}</th>"));
        }
        for column in range.start.column..=range.end.column {
            let address = CellAddress { column, row };
            let view = sheet.get_cell_view(address);
            let value = view
                .data
                .as_ref()
                .map(render_data_value)
                .unwrap_or_default();
            cells.push(format!(
                "<td data-address=\"{}\" data-style-index=\"{}\">{}</td>",
                address.to_a1(),
                view.style_index,
                html_escape(&value)
            ));
        }
        rows.push(format!("<tr>{}</tr>", cells.join("")));
    }
    rows.join("")
}

fn render_data_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn viewport_style(options: &SpreadsheetRenderOptions) -> String {
    let mut style = vec![
        format!("--scale: {}", options.scale.max(0.1)),
        format!(
            "--headers: {}",
            if options.include_headers { "1" } else { "0" }
        ),
    ];
    if let Some(width) = options.width {
        style.push(format!("width: {width}px"));
    }
    if let Some(height) = options.height {
        style.push(format!("height: {height}px"));
    }
    style.push("overflow: auto".to_string());
    style.join("; ")
}

fn preview_css() -> &'static str {
    concat!(
        "body{margin:0;padding:24px;background:#f5f3ee;color:#1e1e1e;font-family:Georgia,serif;}",
        ".spreadsheet-preview{display:flex;flex-direction:column;gap:16px;}",
        "header h1{margin:0;font-size:24px;}header p{margin:0;color:#6b6257;font-size:13px;}",
        ".viewport{border:1px solid #d6d0c7;background:#fff;box-shadow:0 12px 30px rgba(0,0,0,.08);}",
        "table{border-collapse:collapse;transform:scale(var(--scale));transform-origin:top left;}",
        "th,td{border:1px solid #ddd3c6;padding:6px 10px;min-width:72px;max-width:240px;font-size:13px;text-align:left;vertical-align:top;}",
        "th{background:#f0ebe3;font-weight:600;position:sticky;top:0;z-index:1;}",
        ".corner{background:#e7e0d6;left:0;z-index:2;}",
        "td{white-space:pre-wrap;}"
    )
}

fn write_rendered_output(path: &Path, html: &str) -> Result<(), SpreadsheetArtifactError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SpreadsheetArtifactError::ExportFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    fs::write(path, html).map_err(|error| SpreadsheetArtifactError::ExportFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn workbook_output_paths(
    artifact: &SpreadsheetArtifact,
    cwd: &Path,
    options: &SpreadsheetRenderOptions,
    sheets: &[SpreadsheetSheet],
) -> Vec<PathBuf> {
    if let Some(output_path) = options.output_path.as_deref() {
        if output_path.extension().is_some_and(|ext| ext == "html") {
            let stem = output_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("render");
            let parent = output_path.parent().unwrap_or(cwd);
            return sheets
                .iter()
                .map(|sheet| {
                    parent.join(format!(
                        "{}_{}.html",
                        stem,
                        sanitize_file_component(&sheet.name)
                    ))
                })
                .collect();
        }
        return sheets
            .iter()
            .map(|sheet| output_path.join(format!("{}.html", sanitize_file_component(&sheet.name))))
            .collect();
    }
    sheets
        .iter()
        .map(|sheet| {
            cwd.join(format!(
                "{}_render_{}.html",
                artifact.artifact_id,
                sanitize_file_component(&sheet.name)
            ))
        })
        .collect()
}

fn single_output_path(
    cwd: &Path,
    artifact: &SpreadsheetArtifact,
    output_path: Option<&Path>,
    suffix: &str,
) -> PathBuf {
    if let Some(output_path) = output_path {
        return if output_path.extension().is_some_and(|ext| ext == "html") {
            output_path.to_path_buf()
        } else {
            output_path.join(format!("{suffix}.html"))
        };
    }
    cwd.join(format!("{}_{}.html", artifact.artifact_id, suffix))
}

fn sanitize_file_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
