use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCellRangeRef;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetFontFace {
    pub font_family: Option<String>,
    pub font_scheme: Option<String>,
    pub typeface: Option<String>,
}

impl SpreadsheetFontFace {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            font_family: patch
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            font_scheme: patch
                .font_scheme
                .clone()
                .or_else(|| self.font_scheme.clone()),
            typeface: patch.typeface.clone().or_else(|| self.typeface.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpreadsheetTextStyle {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub font_size: Option<f64>,
    pub font_color: Option<String>,
    pub text_alignment: Option<String>,
    pub anchor: Option<String>,
    pub vertical_text_orientation: Option<String>,
    pub text_rotation: Option<i32>,
    pub paragraph_spacing: Option<bool>,
    pub bottom_inset: Option<f64>,
    pub left_inset: Option<f64>,
    pub right_inset: Option<f64>,
    pub top_inset: Option<f64>,
    pub font_family: Option<String>,
    pub font_scheme: Option<String>,
    pub typeface: Option<String>,
    pub font_face: Option<SpreadsheetFontFace>,
}

impl SpreadsheetTextStyle {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            bold: patch.bold.or(self.bold),
            italic: patch.italic.or(self.italic),
            underline: patch.underline.or(self.underline),
            font_size: patch.font_size.or(self.font_size),
            font_color: patch.font_color.clone().or_else(|| self.font_color.clone()),
            text_alignment: patch
                .text_alignment
                .clone()
                .or_else(|| self.text_alignment.clone()),
            anchor: patch.anchor.clone().or_else(|| self.anchor.clone()),
            vertical_text_orientation: patch
                .vertical_text_orientation
                .clone()
                .or_else(|| self.vertical_text_orientation.clone()),
            text_rotation: patch.text_rotation.or(self.text_rotation),
            paragraph_spacing: patch.paragraph_spacing.or(self.paragraph_spacing),
            bottom_inset: patch.bottom_inset.or(self.bottom_inset),
            left_inset: patch.left_inset.or(self.left_inset),
            right_inset: patch.right_inset.or(self.right_inset),
            top_inset: patch.top_inset.or(self.top_inset),
            font_family: patch
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            font_scheme: patch
                .font_scheme
                .clone()
                .or_else(|| self.font_scheme.clone()),
            typeface: patch.typeface.clone().or_else(|| self.typeface.clone()),
            font_face: match (&self.font_face, &patch.font_face) {
                (Some(base), Some(update)) => Some(base.merge(update)),
                (None, Some(update)) => Some(update.clone()),
                (Some(base), None) => Some(base.clone()),
                (None, None) => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetGradientStop {
    pub position: f64,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetFillRectangle {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpreadsheetFill {
    pub solid_fill_color: Option<String>,
    pub pattern_type: Option<String>,
    pub pattern_foreground_color: Option<String>,
    pub pattern_background_color: Option<String>,
    #[serde(default)]
    pub color_transforms: Vec<String>,
    pub gradient_fill_type: Option<String>,
    #[serde(default)]
    pub gradient_stops: Vec<SpreadsheetGradientStop>,
    pub gradient_kind: Option<String>,
    pub angle: Option<f64>,
    pub scaled: Option<bool>,
    pub path_type: Option<String>,
    pub fill_rectangle: Option<SpreadsheetFillRectangle>,
    pub image_reference: Option<String>,
}

impl SpreadsheetFill {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            solid_fill_color: patch
                .solid_fill_color
                .clone()
                .or_else(|| self.solid_fill_color.clone()),
            pattern_type: patch
                .pattern_type
                .clone()
                .or_else(|| self.pattern_type.clone()),
            pattern_foreground_color: patch
                .pattern_foreground_color
                .clone()
                .or_else(|| self.pattern_foreground_color.clone()),
            pattern_background_color: patch
                .pattern_background_color
                .clone()
                .or_else(|| self.pattern_background_color.clone()),
            color_transforms: if patch.color_transforms.is_empty() {
                self.color_transforms.clone()
            } else {
                patch.color_transforms.clone()
            },
            gradient_fill_type: patch
                .gradient_fill_type
                .clone()
                .or_else(|| self.gradient_fill_type.clone()),
            gradient_stops: if patch.gradient_stops.is_empty() {
                self.gradient_stops.clone()
            } else {
                patch.gradient_stops.clone()
            },
            gradient_kind: patch
                .gradient_kind
                .clone()
                .or_else(|| self.gradient_kind.clone()),
            angle: patch.angle.or(self.angle),
            scaled: patch.scaled.or(self.scaled),
            path_type: patch.path_type.clone().or_else(|| self.path_type.clone()),
            fill_rectangle: patch
                .fill_rectangle
                .clone()
                .or_else(|| self.fill_rectangle.clone()),
            image_reference: patch
                .image_reference
                .clone()
                .or_else(|| self.image_reference.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetBorderLine {
    pub style: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetBorder {
    pub top: Option<SpreadsheetBorderLine>,
    pub right: Option<SpreadsheetBorderLine>,
    pub bottom: Option<SpreadsheetBorderLine>,
    pub left: Option<SpreadsheetBorderLine>,
}

impl SpreadsheetBorder {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            top: patch.top.clone().or_else(|| self.top.clone()),
            right: patch.right.clone().or_else(|| self.right.clone()),
            bottom: patch.bottom.clone().or_else(|| self.bottom.clone()),
            left: patch.left.clone().or_else(|| self.left.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetAlignment {
    pub horizontal: Option<String>,
    pub vertical: Option<String>,
}

impl SpreadsheetAlignment {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            horizontal: patch.horizontal.clone().or_else(|| self.horizontal.clone()),
            vertical: patch.vertical.clone().or_else(|| self.vertical.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetNumberFormat {
    pub format_id: Option<u32>,
    pub format_code: Option<String>,
}

impl SpreadsheetNumberFormat {
    fn merge(&self, patch: &Self) -> Self {
        Self {
            format_id: patch.format_id.or(self.format_id),
            format_code: patch
                .format_code
                .clone()
                .or_else(|| self.format_code.clone()),
        }
    }

    fn normalized(mut self) -> Self {
        if self.format_code.is_none() {
            self.format_code = self.format_id.and_then(builtin_number_format_code);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpreadsheetCellFormat {
    pub text_style_id: Option<u32>,
    pub fill_id: Option<u32>,
    pub border_id: Option<u32>,
    pub alignment: Option<SpreadsheetAlignment>,
    pub number_format_id: Option<u32>,
    pub wrap_text: Option<bool>,
    pub base_cell_style_format_id: Option<u32>,
}

impl SpreadsheetCellFormat {
    pub fn wrap(mut self) -> Self {
        self.wrap_text = Some(true);
        self
    }

    pub fn unwrap(mut self) -> Self {
        self.wrap_text = Some(false);
        self
    }

    fn merge(&self, patch: &Self) -> Self {
        Self {
            text_style_id: patch.text_style_id.or(self.text_style_id),
            fill_id: patch.fill_id.or(self.fill_id),
            border_id: patch.border_id.or(self.border_id),
            alignment: match (&self.alignment, &patch.alignment) {
                (Some(base), Some(update)) => Some(base.merge(update)),
                (None, Some(update)) => Some(update.clone()),
                (Some(base), None) => Some(base.clone()),
                (None, None) => None,
            },
            number_format_id: patch.number_format_id.or(self.number_format_id),
            wrap_text: patch.wrap_text.or(self.wrap_text),
            base_cell_style_format_id: patch
                .base_cell_style_format_id
                .or(self.base_cell_style_format_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SpreadsheetDifferentialFormat {
    pub text_style_id: Option<u32>,
    pub fill_id: Option<u32>,
    pub border_id: Option<u32>,
    pub alignment: Option<SpreadsheetAlignment>,
    pub number_format_id: Option<u32>,
    pub wrap_text: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetCellFormatSummary {
    pub style_index: u32,
    pub text_style: Option<SpreadsheetTextStyle>,
    pub fill: Option<SpreadsheetFill>,
    pub border: Option<SpreadsheetBorder>,
    pub alignment: Option<SpreadsheetAlignment>,
    pub number_format: Option<SpreadsheetNumberFormat>,
    pub wrap_text: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetRangeFormat {
    pub sheet_name: String,
    pub range: String,
}

impl SpreadsheetRangeFormat {
    pub fn new(sheet_name: String, range: &CellRange) -> Self {
        Self {
            sheet_name,
            range: range.to_a1(),
        }
    }

    pub fn range_ref(&self) -> Result<SpreadsheetCellRangeRef, SpreadsheetArtifactError> {
        let range = CellRange::parse(&self.range)?;
        Ok(SpreadsheetCellRangeRef::new(
            self.sheet_name.clone(),
            &range,
        ))
    }

    pub fn top_left_style_index(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<u32, SpreadsheetArtifactError> {
        self.range_ref()?.top_left_style_index(sheet)
    }

    pub fn top_left_cell_format(
        &self,
        artifact: &SpreadsheetArtifact,
        sheet: &SpreadsheetSheet,
    ) -> Result<Option<SpreadsheetCellFormatSummary>, SpreadsheetArtifactError> {
        let range = self.range_ref()?.range()?;
        Ok(artifact.cell_format_summary(sheet.top_left_style_index(&range)))
    }
}

impl SpreadsheetArtifact {
    pub fn create_text_style(
        &mut self,
        style: SpreadsheetTextStyle,
        source_style_id: Option<u32>,
        merge_with_existing_components: bool,
    ) -> Result<u32, SpreadsheetArtifactError> {
        let created = if let Some(source_style_id) = source_style_id {
            let source = self
                .text_styles
                .get(&source_style_id)
                .cloned()
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: "create_text_style".to_string(),
                    message: format!("text style `{source_style_id}` was not found"),
                })?;
            if merge_with_existing_components {
                source.merge(&style)
            } else {
                style
            }
        } else {
            style
        };
        Ok(insert_with_next_id(&mut self.text_styles, created))
    }

    pub fn get_text_style(&self, style_id: u32) -> Option<&SpreadsheetTextStyle> {
        self.text_styles.get(&style_id)
    }

    pub fn create_fill(
        &mut self,
        fill: SpreadsheetFill,
        source_fill_id: Option<u32>,
        merge_with_existing_components: bool,
    ) -> Result<u32, SpreadsheetArtifactError> {
        let created = if let Some(source_fill_id) = source_fill_id {
            let source = self.fills.get(&source_fill_id).cloned().ok_or_else(|| {
                SpreadsheetArtifactError::InvalidArgs {
                    action: "create_fill".to_string(),
                    message: format!("fill `{source_fill_id}` was not found"),
                }
            })?;
            if merge_with_existing_components {
                source.merge(&fill)
            } else {
                fill
            }
        } else {
            fill
        };
        Ok(insert_with_next_id(&mut self.fills, created))
    }

    pub fn get_fill(&self, fill_id: u32) -> Option<&SpreadsheetFill> {
        self.fills.get(&fill_id)
    }

    pub fn create_border(
        &mut self,
        border: SpreadsheetBorder,
        source_border_id: Option<u32>,
        merge_with_existing_components: bool,
    ) -> Result<u32, SpreadsheetArtifactError> {
        let created = if let Some(source_border_id) = source_border_id {
            let source = self
                .borders
                .get(&source_border_id)
                .cloned()
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: "create_border".to_string(),
                    message: format!("border `{source_border_id}` was not found"),
                })?;
            if merge_with_existing_components {
                source.merge(&border)
            } else {
                border
            }
        } else {
            border
        };
        Ok(insert_with_next_id(&mut self.borders, created))
    }

    pub fn get_border(&self, border_id: u32) -> Option<&SpreadsheetBorder> {
        self.borders.get(&border_id)
    }

    pub fn create_number_format(
        &mut self,
        format: SpreadsheetNumberFormat,
        source_number_format_id: Option<u32>,
        merge_with_existing_components: bool,
    ) -> Result<u32, SpreadsheetArtifactError> {
        let created = if let Some(source_number_format_id) = source_number_format_id {
            let source = self
                .number_formats
                .get(&source_number_format_id)
                .cloned()
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: "create_number_format".to_string(),
                    message: format!("number format `{source_number_format_id}` was not found"),
                })?;
            if merge_with_existing_components {
                source.merge(&format)
            } else {
                format
            }
        } else {
            format
        };
        Ok(insert_with_next_id(
            &mut self.number_formats,
            created.normalized(),
        ))
    }

    pub fn get_number_format(&self, number_format_id: u32) -> Option<&SpreadsheetNumberFormat> {
        self.number_formats.get(&number_format_id)
    }

    pub fn create_cell_format(
        &mut self,
        format: SpreadsheetCellFormat,
        source_format_id: Option<u32>,
        merge_with_existing_components: bool,
    ) -> Result<u32, SpreadsheetArtifactError> {
        let created = if let Some(source_format_id) = source_format_id {
            let source = self
                .cell_formats
                .get(&source_format_id)
                .cloned()
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: "create_cell_format".to_string(),
                    message: format!("cell format `{source_format_id}` was not found"),
                })?;
            if merge_with_existing_components {
                source.merge(&format)
            } else {
                format
            }
        } else {
            format
        };
        Ok(insert_with_next_id(&mut self.cell_formats, created))
    }

    pub fn get_cell_format(&self, format_id: u32) -> Option<&SpreadsheetCellFormat> {
        self.cell_formats.get(&format_id)
    }

    pub fn create_differential_format(&mut self, format: SpreadsheetDifferentialFormat) -> u32 {
        insert_with_next_id(&mut self.differential_formats, format)
    }

    pub fn get_differential_format(
        &self,
        format_id: u32,
    ) -> Option<&SpreadsheetDifferentialFormat> {
        self.differential_formats.get(&format_id)
    }

    pub fn resolve_cell_format(&self, style_index: u32) -> Option<SpreadsheetCellFormat> {
        let format = self.cell_formats.get(&style_index)?.clone();
        resolve_cell_format_recursive(&self.cell_formats, &format, 0)
    }

    pub fn cell_format_summary(&self, style_index: u32) -> Option<SpreadsheetCellFormatSummary> {
        let resolved = self.resolve_cell_format(style_index)?;
        Some(SpreadsheetCellFormatSummary {
            style_index,
            text_style: resolved
                .text_style_id
                .and_then(|id| self.text_styles.get(&id).cloned()),
            fill: resolved.fill_id.and_then(|id| self.fills.get(&id).cloned()),
            border: resolved
                .border_id
                .and_then(|id| self.borders.get(&id).cloned()),
            alignment: resolved.alignment,
            number_format: resolved
                .number_format_id
                .and_then(|id| self.number_formats.get(&id).cloned()),
            wrap_text: resolved.wrap_text,
        })
    }
}

impl SpreadsheetSheet {
    pub fn range_format(&self, range: &CellRange) -> SpreadsheetRangeFormat {
        SpreadsheetRangeFormat::new(self.name.clone(), range)
    }
}

fn insert_with_next_id<T>(map: &mut BTreeMap<u32, T>, value: T) -> u32 {
    let next_id = map.last_key_value().map(|(key, _)| key + 1).unwrap_or(1);
    map.insert(next_id, value);
    next_id
}

fn resolve_cell_format_recursive(
    cell_formats: &BTreeMap<u32, SpreadsheetCellFormat>,
    format: &SpreadsheetCellFormat,
    depth: usize,
) -> Option<SpreadsheetCellFormat> {
    if depth > 32 {
        return None;
    }
    let base = format
        .base_cell_style_format_id
        .and_then(|id| cell_formats.get(&id))
        .and_then(|base| resolve_cell_format_recursive(cell_formats, base, depth + 1));
    Some(match base {
        Some(base) => base.merge(format),
        None => format.clone(),
    })
}

fn builtin_number_format_code(format_id: u32) -> Option<String> {
    match format_id {
        0 => Some("General".to_string()),
        1 => Some("0".to_string()),
        2 => Some("0.00".to_string()),
        3 => Some("#,##0".to_string()),
        4 => Some("#,##0.00".to_string()),
        9 => Some("0%".to_string()),
        10 => Some("0.00%".to_string()),
        _ => None,
    }
}
