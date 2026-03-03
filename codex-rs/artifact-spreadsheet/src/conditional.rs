use serde::Deserialize;
use serde::Serialize;

use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpreadsheetConditionalFormatType {
    Expression,
    CellIs,
    ColorScale,
    DataBar,
    IconSet,
    Top10,
    UniqueValues,
    DuplicateValues,
    ContainsText,
    NotContainsText,
    BeginsWith,
    EndsWith,
    ContainsBlanks,
    NotContainsBlanks,
    ContainsErrors,
    NotContainsErrors,
    TimePeriod,
    AboveAverage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetColorScale {
    pub min_type: Option<String>,
    pub mid_type: Option<String>,
    pub max_type: Option<String>,
    pub min_value: Option<String>,
    pub mid_value: Option<String>,
    pub max_value: Option<String>,
    pub min_color: String,
    pub mid_color: Option<String>,
    pub max_color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetDataBar {
    pub color: String,
    pub min_length: Option<u8>,
    pub max_length: Option<u8>,
    pub show_value: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetIconSet {
    pub style: String,
    pub show_value: Option<bool>,
    pub reverse_order: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetConditionalFormat {
    pub id: u32,
    pub range: String,
    pub rule_type: SpreadsheetConditionalFormatType,
    pub operator: Option<String>,
    #[serde(default)]
    pub formulas: Vec<String>,
    pub text: Option<String>,
    pub dxf_id: Option<u32>,
    pub stop_if_true: bool,
    pub priority: u32,
    pub rank: Option<u32>,
    pub percent: Option<bool>,
    pub time_period: Option<String>,
    pub above_average: Option<bool>,
    pub equal_average: Option<bool>,
    pub color_scale: Option<SpreadsheetColorScale>,
    pub data_bar: Option<SpreadsheetDataBar>,
    pub icon_set: Option<SpreadsheetIconSet>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetConditionalFormatCollection {
    pub sheet_name: String,
    pub range: String,
}

impl SpreadsheetConditionalFormatCollection {
    pub fn new(sheet_name: String, range: &CellRange) -> Self {
        Self {
            sheet_name,
            range: range.to_a1(),
        }
    }

    pub fn range(&self) -> Result<CellRange, SpreadsheetArtifactError> {
        CellRange::parse(&self.range)
    }

    pub fn list(
        &self,
        artifact: &SpreadsheetArtifact,
    ) -> Result<Vec<SpreadsheetConditionalFormat>, SpreadsheetArtifactError> {
        let sheet = artifact.sheet_lookup(
            "conditional_format_collection",
            Some(&self.sheet_name),
            None,
        )?;
        Ok(sheet.list_conditional_formats(Some(&self.range()?)))
    }

    pub fn add(
        &self,
        artifact: &mut SpreadsheetArtifact,
        mut format: SpreadsheetConditionalFormat,
    ) -> Result<u32, SpreadsheetArtifactError> {
        format.range = self.range.clone();
        artifact.add_conditional_format("conditional_format_collection", &self.sheet_name, format)
    }

    pub fn delete(
        &self,
        artifact: &mut SpreadsheetArtifact,
        id: u32,
    ) -> Result<(), SpreadsheetArtifactError> {
        artifact.delete_conditional_format("conditional_format_collection", &self.sheet_name, id)
    }
}

impl SpreadsheetArtifact {
    pub fn validate_conditional_formats(
        &self,
        action: &str,
        sheet_name: &str,
    ) -> Result<(), SpreadsheetArtifactError> {
        let sheet = self.sheet_lookup(action, Some(sheet_name), None)?;
        for format in &sheet.conditional_formats {
            validate_conditional_format(self, format, action)?;
        }
        Ok(())
    }

    pub fn add_conditional_format(
        &mut self,
        action: &str,
        sheet_name: &str,
        mut format: SpreadsheetConditionalFormat,
    ) -> Result<u32, SpreadsheetArtifactError> {
        validate_conditional_format(self, &format, action)?;
        let sheet = self.sheet_lookup_mut(action, Some(sheet_name), None)?;
        let next_id = sheet
            .conditional_formats
            .iter()
            .map(|entry| entry.id)
            .max()
            .unwrap_or(0)
            + 1;
        format.id = next_id;
        format.priority = if format.priority == 0 {
            next_id
        } else {
            format.priority
        };
        sheet.conditional_formats.push(format);
        Ok(next_id)
    }

    pub fn delete_conditional_format(
        &mut self,
        action: &str,
        sheet_name: &str,
        id: u32,
    ) -> Result<(), SpreadsheetArtifactError> {
        let sheet = self.sheet_lookup_mut(action, Some(sheet_name), None)?;
        let previous_len = sheet.conditional_formats.len();
        sheet.conditional_formats.retain(|entry| entry.id != id);
        if sheet.conditional_formats.len() == previous_len {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!("conditional format `{id}` was not found"),
            });
        }
        Ok(())
    }
}

impl SpreadsheetSheet {
    pub fn conditional_format_collection(
        &self,
        range: &CellRange,
    ) -> SpreadsheetConditionalFormatCollection {
        SpreadsheetConditionalFormatCollection::new(self.name.clone(), range)
    }

    pub fn list_conditional_formats(
        &self,
        range: Option<&CellRange>,
    ) -> Vec<SpreadsheetConditionalFormat> {
        self.conditional_formats
            .iter()
            .filter(|entry| {
                range.is_none_or(|target| {
                    CellRange::parse(&entry.range)
                        .map(|entry_range| entry_range.intersects(target))
                        .unwrap_or(false)
                })
            })
            .cloned()
            .collect()
    }
}

fn validate_conditional_format(
    artifact: &SpreadsheetArtifact,
    format: &SpreadsheetConditionalFormat,
    action: &str,
) -> Result<(), SpreadsheetArtifactError> {
    CellRange::parse(&format.range)?;
    if let Some(dxf_id) = format.dxf_id
        && artifact.get_differential_format(dxf_id).is_none()
    {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("differential format `{dxf_id}` was not found"),
        });
    }

    let has_style = format.dxf_id.is_some();
    let has_intrinsic_visual =
        format.color_scale.is_some() || format.data_bar.is_some() || format.icon_set.is_some();

    match format.rule_type {
        SpreadsheetConditionalFormatType::Expression | SpreadsheetConditionalFormatType::CellIs => {
            if format.formulas.is_empty() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "conditional format formulas are required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::ContainsText
        | SpreadsheetConditionalFormatType::NotContainsText
        | SpreadsheetConditionalFormatType::BeginsWith
        | SpreadsheetConditionalFormatType::EndsWith => {
            if format.text.as_deref().unwrap_or_default().is_empty() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "conditional format text is required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::ColorScale => {
            if format.color_scale.is_none() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "color scale settings are required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::DataBar => {
            if format.data_bar.is_none() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "data bar settings are required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::IconSet => {
            if format.icon_set.is_none() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "icon set settings are required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::Top10 => {
            if format.rank.is_none() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "top10 rank is required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::TimePeriod => {
            if format.time_period.as_deref().unwrap_or_default().is_empty() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "time period is required".to_string(),
                });
            }
        }
        SpreadsheetConditionalFormatType::AboveAverage => {}
        SpreadsheetConditionalFormatType::UniqueValues
        | SpreadsheetConditionalFormatType::DuplicateValues
        | SpreadsheetConditionalFormatType::ContainsBlanks
        | SpreadsheetConditionalFormatType::NotContainsBlanks
        | SpreadsheetConditionalFormatType::ContainsErrors
        | SpreadsheetConditionalFormatType::NotContainsErrors => {}
    }

    if !has_style && !has_intrinsic_visual {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "conditional formatting requires at least one style component".to_string(),
        });
    }
    Ok(())
}
