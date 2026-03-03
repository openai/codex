use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifactError;
use crate::formula::recalculate_workbook;
use crate::parse_column_reference;
use crate::xlsx::import_xlsx;
use crate::xlsx::write_xlsx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpreadsheetFileType {
    Xlsx,
    Json,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SpreadsheetCellValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    DateTime(String),
    Error(String),
}

impl SpreadsheetCellValue {
    pub fn to_json_value(&self) -> Value {
        match self {
            Self::Bool(value) => Value::Bool(*value),
            Self::Integer(value) => Value::Number((*value).into()),
            Self::Float(value) => serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(value.to_string())),
            Self::String(value) | Self::DateTime(value) | Self::Error(value) => {
                Value::String(value.clone())
            }
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }
}

impl TryFrom<Value> for SpreadsheetCellValue {
    type Error = SpreadsheetArtifactError;

    fn try_from(value: Value) -> Result<Self, SpreadsheetArtifactError> {
        match value {
            Value::Object(_) => serde_json::from_value(value).map_err(|error| {
                SpreadsheetArtifactError::Serialization {
                    message: error.to_string(),
                }
            }),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    Ok(Self::Integer(integer))
                } else if let Some(float) = value.as_f64() {
                    Ok(Self::Float(float))
                } else {
                    Err(SpreadsheetArtifactError::Serialization {
                        message: "unsupported JSON number".to_string(),
                    })
                }
            }
            Value::String(value) => Ok(Self::String(value)),
            Value::Null => Err(SpreadsheetArtifactError::Serialization {
                message: "null is represented as an empty cell, not a cell value".to_string(),
            }),
            other => Err(SpreadsheetArtifactError::Serialization {
                message: format!("unsupported JSON cell value `{other}`"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetCitation {
    pub tether_id: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub content_reference_type: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetCell {
    pub value: Option<SpreadsheetCellValue>,
    pub formula: Option<String>,
    pub style_index: u32,
    #[serde(default)]
    pub citations: Vec<SpreadsheetCitation>,
}

impl SpreadsheetCell {
    pub fn to_dict(&self) -> Result<Value, SpreadsheetArtifactError> {
        serde_json::to_value(self).map_err(|error| SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        })
    }

    pub fn data(&self) -> Option<Value> {
        if let Some(formula) = &self.formula {
            return Some(Value::String(formula.clone()));
        }
        self.value.as_ref().map(SpreadsheetCellValue::to_json_value)
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_none()
            && self.formula.is_none()
            && self.style_index == 0
            && self.citations.is_empty()
    }

    pub fn is_calculation_error(&self) -> bool {
        matches!(
            self.value,
            Some(SpreadsheetCellValue::Error(ref value))
                if matches!(
                    value.as_str(),
                    "#DIV/0!"
                        | "#N/A"
                        | "#NAME?"
                        | "#NUM!"
                        | "#REF!"
                        | "#VALUE!"
                        | "#CYCLE!"
                        | "#ERROR!"
                        | "#LIC!"
                )
        )
    }

    pub fn calculation_error_message(&self) -> Option<&'static str> {
        match self.value.as_ref() {
            Some(SpreadsheetCellValue::Error(value)) => match value.as_str() {
                "#DIV/0!" => Some("Division by zero"),
                "#N/A" => Some("Value is not available"),
                "#NAME?" => Some("Unknown function or name"),
                "#NUM!" => Some("Invalid numeric result"),
                "#REF!" => Some("Invalid cell reference"),
                "#VALUE!" => Some("Invalid value type"),
                "#CYCLE!" => Some("Formula cycle detected"),
                "#ERROR!" => Some("Formula parse error"),
                "#LIC!" => Some("Calculation engine license error"),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetCellRef {
    pub sheet_name: String,
    pub address: String,
}

impl SpreadsheetCellRef {
    pub fn new(sheet_name: String, address: CellAddress) -> Self {
        Self {
            sheet_name,
            address: address.to_a1(),
        }
    }

    pub fn cell_address(&self) -> Result<CellAddress, SpreadsheetArtifactError> {
        CellAddress::parse(&self.address)
    }

    pub fn get(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<SpreadsheetCellView, SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        Ok(sheet.get_cell_view(self.cell_address()?))
    }

    pub fn data(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Option<Value>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.data)
    }

    pub fn raw_cell(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Option<SpreadsheetCell>, SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        Ok(sheet.get_cell(self.cell_address()?).cloned())
    }

    pub fn to_dict(&self, sheet: &SpreadsheetSheet) -> Result<Value, SpreadsheetArtifactError> {
        Ok(match self.raw_cell(sheet)? {
            Some(cell) => cell.to_dict()?,
            None => Value::Object(Default::default()),
        })
    }

    pub fn set_value(
        &self,
        sheet: &mut SpreadsheetSheet,
        value: Option<SpreadsheetCellValue>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_value(self.cell_address()?, value)
    }

    pub fn set_formula(
        &self,
        sheet: &mut SpreadsheetSheet,
        formula: Option<String>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_formula(self.cell_address()?, formula)
    }

    pub fn cite(
        &self,
        sheet: &mut SpreadsheetSheet,
        citation: SpreadsheetCitation,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.cite_range(
            &CellRange::from_start_end(self.cell_address()?, self.cell_address()?),
            citation,
        )
    }

    pub fn set_style_index(
        &self,
        sheet: &mut SpreadsheetSheet,
        style_index: u32,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_style_index(
            &CellRange::from_start_end(self.cell_address()?, self.cell_address()?),
            style_index,
        )
    }

    pub fn is_calculation_error(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<bool, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.is_calculation_error)
    }

    pub fn get_calculation_error_message(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Option<String>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.calculation_error_message)
    }

    fn ensure_sheet(&self, sheet: &SpreadsheetSheet) -> Result<(), SpreadsheetArtifactError> {
        if self.sheet_name != sheet.name {
            return Err(SpreadsheetArtifactError::SheetLookup {
                action: "cell_ref".to_string(),
                message: format!(
                    "cell ref points to `{}` but sheet is `{}`",
                    self.sheet_name, sheet.name
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetCellRangeRef {
    pub sheet_name: String,
    pub address: String,
}

impl SpreadsheetCellRangeRef {
    pub fn new(sheet_name: String, range: &CellRange) -> Self {
        Self {
            sheet_name,
            address: range.to_a1(),
        }
    }

    pub fn range(&self) -> Result<CellRange, SpreadsheetArtifactError> {
        CellRange::parse(&self.address)
    }

    pub fn get(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<SpreadsheetRangeView, SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        Ok(sheet.get_range_view(&self.range()?))
    }

    pub fn get_values(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Vec<Vec<Option<SpreadsheetCellValue>>>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.values)
    }

    pub fn get_formulas(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Vec<Vec<Option<String>>>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.formulas)
    }

    pub fn get_style_indices(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Vec<Vec<u32>>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.style_indices)
    }

    pub fn get_data(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<Vec<Vec<Option<Value>>>, SpreadsheetArtifactError> {
        Ok(self.get(sheet)?.data)
    }

    pub fn top_left_style_index(
        &self,
        sheet: &SpreadsheetSheet,
    ) -> Result<u32, SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        Ok(sheet.top_left_style_index(&self.range()?))
    }

    pub fn set_value(
        &self,
        sheet: &mut SpreadsheetSheet,
        value: Option<SpreadsheetCellValue>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_range_to_value(&self.range()?, value)
    }

    pub fn set_values(
        &self,
        sheet: &mut SpreadsheetSheet,
        values: &[Vec<Option<SpreadsheetCellValue>>],
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_values_matrix(&self.range()?, values)
    }

    pub fn set_formula(
        &self,
        sheet: &mut SpreadsheetSheet,
        formula: Option<String>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_range_to_formula(&self.range()?, formula)
    }

    pub fn set_formulas(
        &self,
        sheet: &mut SpreadsheetSheet,
        formulas: &[Vec<Option<String>>],
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_formulas_matrix(&self.range()?, formulas)
    }

    pub fn set_style_index(
        &self,
        sheet: &mut SpreadsheetSheet,
        style_index: u32,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.set_style_index(&self.range()?, style_index)
    }

    pub fn merge(
        &self,
        sheet: &mut SpreadsheetSheet,
        raise_on_conflict: bool,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.merge_cells(&self.range()?, raise_on_conflict)
    }

    pub fn unmerge(&self, sheet: &mut SpreadsheetSheet) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.unmerge_cells(&self.range()?);
        Ok(())
    }

    pub fn cite(
        &self,
        sheet: &mut SpreadsheetSheet,
        citation: SpreadsheetCitation,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_sheet(sheet)?;
        sheet.cite_range(&self.range()?, citation)
    }

    fn ensure_sheet(&self, sheet: &SpreadsheetSheet) -> Result<(), SpreadsheetArtifactError> {
        if self.sheet_name != sheet.name {
            return Err(SpreadsheetArtifactError::SheetLookup {
                action: "range_ref".to_string(),
                message: format!(
                    "range ref points to `{}` but sheet is `{}`",
                    self.sheet_name, sheet.name
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpreadsheetSheetReference {
    Cell { cell_ref: SpreadsheetCellRef },
    Range { range_ref: SpreadsheetCellRangeRef },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetSheetSummary {
    pub name: String,
    pub filled_rows: usize,
    pub filled_columns: usize,
    pub minimum_range_filled: String,
    pub min_row_idx: Option<u32>,
    pub max_row_idx: Option<u32>,
    pub min_column_idx: Option<u32>,
    pub max_column_idx: Option<u32>,
    pub min_column_letter: Option<String>,
    pub max_column_letter: Option<String>,
    pub first_row_address_range: Option<String>,
    pub default_row_height: Option<f64>,
    pub default_column_width: Option<f64>,
    pub show_grid_lines: bool,
    pub merged_range_count: usize,
    pub chart_count: usize,
    pub table_count: usize,
    pub conditional_format_count: usize,
    pub pivot_table_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetSummary {
    pub artifact_id: String,
    pub sheets: Vec<SpreadsheetSheetSummary>,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetCellView {
    pub sheet_name: String,
    pub address: String,
    pub effective_address: String,
    pub exists: bool,
    pub value: Option<SpreadsheetCellValue>,
    pub formula: Option<String>,
    pub style_index: u32,
    pub data: Option<Value>,
    pub is_calculation_error: bool,
    pub calculation_error_message: Option<String>,
    pub citations: Vec<SpreadsheetCitation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetRangeView {
    pub sheet_name: String,
    pub address: String,
    pub values: Vec<Vec<Option<SpreadsheetCellValue>>>,
    pub formulas: Vec<Vec<Option<String>>>,
    pub style_indices: Vec<Vec<u32>>,
    pub data: Vec<Vec<Option<Value>>>,
    pub is_single_cell: bool,
    pub is_single_row: bool,
    pub is_single_column: bool,
    pub contains_merged_cells: bool,
    pub is_exactly_one_merged_cell: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetSheet {
    pub sheet_id: String,
    pub name: String,
    #[serde(default, with = "cell_map_serde")]
    pub cells: BTreeMap<CellAddress, SpreadsheetCell>,
    #[serde(default)]
    pub merged_ranges: Vec<CellRange>,
    #[serde(default)]
    pub charts: Vec<crate::SpreadsheetChart>,
    #[serde(default)]
    pub tables: Vec<crate::SpreadsheetTable>,
    #[serde(default)]
    pub conditional_formats: Vec<crate::SpreadsheetConditionalFormat>,
    #[serde(default)]
    pub pivot_tables: Vec<crate::SpreadsheetPivotTable>,
    pub default_row_height: Option<f64>,
    pub default_column_width: Option<f64>,
    pub show_grid_lines: bool,
    #[serde(default)]
    pub column_widths: BTreeMap<u32, f64>,
    #[serde(default)]
    pub row_heights: BTreeMap<u32, f64>,
}

mod cell_map_serde {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;
    use serde::Serializer;

    use crate::CellAddress;
    use crate::SpreadsheetCell;
    #[derive(Serialize, Deserialize)]
    struct CellEntry {
        address: String,
        cell: SpreadsheetCell,
    }

    pub fn serialize<S>(
        cells: &BTreeMap<CellAddress, SpreadsheetCell>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries = cells
            .iter()
            .map(|(address, cell)| CellEntry {
                address: address.to_a1(),
                cell: cell.clone(),
            })
            .collect::<Vec<_>>();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<CellAddress, SpreadsheetCell>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<CellEntry>::deserialize(deserializer)?;
        let mut cells = BTreeMap::new();
        for entry in entries {
            let address = CellAddress::parse(&entry.address).map_err(serde::de::Error::custom)?;
            cells.insert(address, entry.cell);
        }
        Ok(cells)
    }
}

impl SpreadsheetSheet {
    pub fn new(name: String) -> Self {
        Self {
            sheet_id: format!("sheet_{}", Uuid::new_v4().simple()),
            name,
            cells: BTreeMap::new(),
            merged_ranges: Vec::new(),
            charts: Vec::new(),
            tables: Vec::new(),
            conditional_formats: Vec::new(),
            pivot_tables: Vec::new(),
            default_row_height: None,
            default_column_width: None,
            show_grid_lines: true,
            column_widths: BTreeMap::new(),
            row_heights: BTreeMap::new(),
        }
    }

    pub fn filled_rows(&self) -> usize {
        self.cells
            .keys()
            .map(|address| address.row)
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn filled_columns(&self) -> usize {
        self.cells
            .keys()
            .map(|address| address.column)
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn minimum_range(&self) -> Option<CellRange> {
        let mut iter = self.cells.keys();
        let first = *iter.next()?;
        let mut min_row = first.row;
        let mut max_row = first.row;
        let mut min_column = first.column;
        let mut max_column = first.column;

        for address in iter {
            min_row = min_row.min(address.row);
            max_row = max_row.max(address.row);
            min_column = min_column.min(address.column);
            max_column = max_column.max(address.column);
        }

        Some(CellRange::from_start_end(
            CellAddress {
                column: min_column,
                row: min_row,
            },
            CellAddress {
                column: max_column,
                row: max_row,
            },
        ))
    }

    pub fn minimum_range_ref(&self) -> Option<SpreadsheetCellRangeRef> {
        self.minimum_range()
            .map(|range| SpreadsheetCellRangeRef::new(self.name.clone(), &range))
    }

    pub fn minimum_range_filled(&self) -> String {
        self.minimum_range()
            .map(|range| range.to_a1())
            .unwrap_or_default()
    }

    pub fn min_column_letter(&self) -> Option<String> {
        self.minimum_range()
            .map(|range| crate::column_index_to_letters(range.start.column))
    }

    pub fn max_column_letter(&self) -> Option<String> {
        self.minimum_range()
            .map(|range| crate::column_index_to_letters(range.end.column))
    }

    pub fn first_row_address_range(&self) -> Option<String> {
        self.minimum_range().map(|range| {
            CellRange::from_start_end(
                CellAddress {
                    column: range.start.column,
                    row: range.start.row,
                },
                CellAddress {
                    column: range.end.column,
                    row: range.start.row,
                },
            )
            .to_a1()
        })
    }

    pub fn summary(&self) -> SpreadsheetSheetSummary {
        let minimum = self.minimum_range();
        SpreadsheetSheetSummary {
            name: self.name.clone(),
            filled_rows: self.filled_rows(),
            filled_columns: self.filled_columns(),
            minimum_range_filled: self.minimum_range_filled(),
            min_row_idx: minimum.as_ref().map(|range| range.start.row),
            max_row_idx: minimum.as_ref().map(|range| range.end.row),
            min_column_idx: minimum.as_ref().map(|range| range.start.column),
            max_column_idx: minimum.as_ref().map(|range| range.end.column),
            min_column_letter: self.min_column_letter(),
            max_column_letter: self.max_column_letter(),
            first_row_address_range: self.first_row_address_range(),
            default_row_height: self.default_row_height,
            default_column_width: self.default_column_width,
            show_grid_lines: self.show_grid_lines,
            merged_range_count: self.merged_ranges.len(),
            chart_count: self.charts.len(),
            table_count: self.tables.len(),
            conditional_format_count: self.conditional_formats.len(),
            pivot_table_count: self.pivot_tables.len(),
        }
    }

    pub fn to_dict(&self) -> Result<Value, SpreadsheetArtifactError> {
        serde_json::to_value(self).map_err(|error| SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        })
    }

    pub fn cell_ref(
        &self,
        address: impl AsRef<str>,
    ) -> Result<SpreadsheetCellRef, SpreadsheetArtifactError> {
        Ok(SpreadsheetCellRef::new(
            self.name.clone(),
            CellAddress::parse(address.as_ref())?,
        ))
    }

    pub fn range_ref(
        &self,
        address: impl AsRef<str>,
    ) -> Result<SpreadsheetCellRangeRef, SpreadsheetArtifactError> {
        let range = CellRange::parse(address.as_ref())?;
        Ok(SpreadsheetCellRangeRef::new(self.name.clone(), &range))
    }

    pub fn reference(
        &self,
        address: impl AsRef<str>,
    ) -> Result<SpreadsheetSheetReference, SpreadsheetArtifactError> {
        let address = address.as_ref();
        if let Ok(cell_ref) = self.cell_ref(address) {
            return Ok(SpreadsheetSheetReference::Cell { cell_ref });
        }
        Ok(SpreadsheetSheetReference::Range {
            range_ref: self.range_ref(address)?,
        })
    }

    pub fn set_column_widths(
        &mut self,
        reference: &str,
        width: f64,
    ) -> Result<(), SpreadsheetArtifactError> {
        if width <= 0.0 {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "set_column_widths".to_string(),
                message: "column width must be positive".to_string(),
            });
        }
        let (start, end) = parse_column_reference(reference)?;
        for column in start..=end {
            self.column_widths.insert(column, width);
        }
        Ok(())
    }

    pub fn set_column_widths_bulk(
        &mut self,
        widths: &BTreeMap<String, f64>,
    ) -> Result<(), SpreadsheetArtifactError> {
        for (reference, width) in widths {
            self.set_column_widths(reference, *width)?;
        }
        Ok(())
    }

    pub fn get_column_width(
        &self,
        reference: &str,
    ) -> Result<Option<f64>, SpreadsheetArtifactError> {
        let (start, end) = parse_column_reference(reference)?;
        if start != end {
            return Ok((start..=end)
                .find_map(|column| self.column_widths.get(&column).copied())
                .or(self.default_column_width));
        }
        Ok(self
            .column_widths
            .get(&start)
            .copied()
            .or(self.default_column_width))
    }

    pub fn set_row_height(
        &mut self,
        row_index: u32,
        height: Option<f64>,
    ) -> Result<(), SpreadsheetArtifactError> {
        if row_index == 0 {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "set_row_height".to_string(),
                message: "row index must be positive".to_string(),
            });
        }
        if let Some(height) = height {
            if height <= 0.0 {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: "set_row_height".to_string(),
                    message: "row height must be positive".to_string(),
                });
            }
            self.row_heights.insert(row_index, height);
        } else {
            self.row_heights.remove(&row_index);
        }
        Ok(())
    }

    pub fn set_row_heights(
        &mut self,
        start_row_index: u32,
        end_row_index: u32,
        height: Option<f64>,
    ) -> Result<(), SpreadsheetArtifactError> {
        if start_row_index == 0 || end_row_index == 0 {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "set_row_heights".to_string(),
                message: "row indices must be positive".to_string(),
            });
        }
        let start = start_row_index.min(end_row_index);
        let end = start_row_index.max(end_row_index);
        for row_index in start..=end {
            self.set_row_height(row_index, height)?;
        }
        Ok(())
    }

    pub fn set_row_heights_bulk(
        &mut self,
        heights: &BTreeMap<u32, Option<f64>>,
    ) -> Result<(), SpreadsheetArtifactError> {
        for (row_index, height) in heights {
            self.set_row_height(*row_index, *height)?;
        }
        Ok(())
    }

    pub fn get_row_height(&self, row_index: u32) -> Option<f64> {
        self.row_heights
            .get(&row_index)
            .copied()
            .or(self.default_row_height)
    }

    pub fn cell_exists(&self, address: CellAddress) -> bool {
        self.cells.contains_key(&address)
    }

    pub fn merged_range_for(&self, address: CellAddress) -> Option<&CellRange> {
        self.merged_ranges
            .iter()
            .find(|range| range.contains(address))
    }

    pub fn effective_address(&self, address: CellAddress) -> CellAddress {
        self.merged_range_for(address)
            .map(|range| range.start)
            .unwrap_or(address)
    }

    pub fn get_cell(&self, address: CellAddress) -> Option<&SpreadsheetCell> {
        let effective = self.effective_address(address);
        self.cells.get(&effective)
    }

    pub fn get_cell_by_indices(&self, column: u32, row: u32) -> Option<&SpreadsheetCell> {
        self.get_cell(CellAddress { column, row })
    }

    pub fn get_cell_mut(&mut self, address: CellAddress) -> Option<&mut SpreadsheetCell> {
        let effective = self.effective_address(address);
        self.cells.get_mut(&effective)
    }

    pub fn get_or_create_cell_mut(&mut self, address: CellAddress) -> &mut SpreadsheetCell {
        let effective = self.effective_address(address);
        self.cells.entry(effective).or_insert(SpreadsheetCell {
            value: None,
            formula: None,
            style_index: 0,
            citations: Vec::new(),
        })
    }

    pub fn clear_range(
        &mut self,
        range: &CellRange,
        fields: Option<&[String]>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "clear_range")?;
        for address in range.addresses() {
            if let Some(cell) = self.get_cell_mut(address) {
                match fields {
                    Some(fields) => {
                        for field in fields {
                            match field.as_str() {
                                "value" => cell.value = None,
                                "formula" => cell.formula = None,
                                "style_index" => cell.style_index = 0,
                                "citations" => cell.citations.clear(),
                                other => {
                                    return Err(SpreadsheetArtifactError::InvalidArgs {
                                        action: "clear_range".to_string(),
                                        message: format!("unsupported cell field `{other}`"),
                                    });
                                }
                            }
                        }
                    }
                    None => {
                        cell.value = None;
                        cell.formula = None;
                        cell.style_index = 0;
                        cell.citations.clear();
                    }
                }
            }
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn set_value(
        &mut self,
        address: CellAddress,
        value: Option<SpreadsheetCellValue>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let range = CellRange::from_start_end(address, address);
        self.ensure_range_write_allowed(&range, false, "set_value")?;
        let cell = self.get_or_create_cell_mut(address);
        cell.formula = None;
        cell.value = value;
        if cell.is_empty() {
            self.cells.remove(&address);
        }
        Ok(())
    }

    pub fn set_range_to_value(
        &mut self,
        range: &CellRange,
        value: Option<SpreadsheetCellValue>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "set_range_to_value")?;
        for address in range.addresses() {
            let cell = self.get_or_create_cell_mut(address);
            cell.formula = None;
            cell.value = value.clone();
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn set_formula(
        &mut self,
        address: CellAddress,
        formula: Option<String>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let range = CellRange::from_start_end(address, address);
        self.ensure_range_write_allowed(&range, false, "set_formula")?;
        let cell = self.get_or_create_cell_mut(address);
        cell.formula = formula;
        if cell.formula.is_none() {
            cell.value = None;
        }
        if cell.is_empty() {
            self.cells.remove(&address);
        }
        Ok(())
    }

    pub fn set_range_to_formula(
        &mut self,
        range: &CellRange,
        formula: Option<String>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "set_range_to_formula")?;
        for address in range.addresses() {
            let cell = self.get_or_create_cell_mut(address);
            cell.formula = formula.clone();
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn set_cell_values_to(
        &mut self,
        address: &str,
        value: Option<SpreadsheetCellValue>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let range = CellRange::parse(address)?;
        self.set_range_to_value(&range, value)
    }

    pub fn set_cell_formulas_to(
        &mut self,
        address: &str,
        formula: Option<String>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let range = CellRange::parse(address)?;
        self.set_range_to_formula(&range, formula)
    }

    pub fn set_style_index(
        &mut self,
        range: &CellRange,
        style_index: u32,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "set_style_index")?;
        for address in range.addresses() {
            let cell = self.get_or_create_cell_mut(address);
            cell.style_index = style_index;
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn cite_range(
        &mut self,
        range: &CellRange,
        citation: SpreadsheetCitation,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, true, "cite_range")?;
        for address in range.addresses() {
            let cell = self.get_or_create_cell_mut(address);
            cell.citations.push(citation.clone());
        }
        Ok(())
    }

    pub fn set_values_matrix(
        &mut self,
        range: &CellRange,
        values: &[Vec<Option<SpreadsheetCellValue>>],
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "set_values_matrix")?;
        if values.len() != range.height() || values.iter().any(|row| row.len() != range.width()) {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "set_range_values".to_string(),
                message: format!(
                    "matrix dimensions {}x{} do not match range {}x{}",
                    values.len(),
                    values.first().map(Vec::len).unwrap_or(0),
                    range.height(),
                    range.width()
                ),
            });
        }

        for (row_offset, row) in values.iter().enumerate() {
            for (column_offset, value) in row.iter().enumerate() {
                let address = CellAddress {
                    column: range.start.column + column_offset as u32,
                    row: range.start.row + row_offset as u32,
                };
                let cell = self.get_or_create_cell_mut(address);
                cell.formula = None;
                cell.value = value.clone();
            }
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn set_formulas_matrix(
        &mut self,
        range: &CellRange,
        formulas: &[Vec<Option<String>>],
    ) -> Result<(), SpreadsheetArtifactError> {
        self.ensure_range_write_allowed(range, false, "set_formulas_matrix")?;
        if formulas.len() != range.height() || formulas.iter().any(|row| row.len() != range.width())
        {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "set_range_formulas".to_string(),
                message: format!(
                    "matrix dimensions {}x{} do not match range {}x{}",
                    formulas.len(),
                    formulas.first().map(Vec::len).unwrap_or(0),
                    range.height(),
                    range.width()
                ),
            });
        }

        for (row_offset, row) in formulas.iter().enumerate() {
            for (column_offset, formula) in row.iter().enumerate() {
                let address = CellAddress {
                    column: range.start.column + column_offset as u32,
                    row: range.start.row + row_offset as u32,
                };
                let cell = self.get_or_create_cell_mut(address);
                cell.formula = formula.clone();
            }
        }
        self.cells.retain(|_, cell| !cell.is_empty());
        Ok(())
    }

    pub fn merge_cells(
        &mut self,
        range: &CellRange,
        raise_on_conflict: bool,
    ) -> Result<(), SpreadsheetArtifactError> {
        for existing in &self.merged_ranges {
            if existing.intersects(range) && existing != range {
                if raise_on_conflict {
                    return Err(SpreadsheetArtifactError::MergeConflict {
                        action: "merge_range".to_string(),
                        range: range.to_a1(),
                        conflict: existing.to_a1(),
                    });
                }
                return Ok(());
            }
        }
        self.merged_ranges.push(range.clone());
        self.merged_ranges
            .sort_by_key(|entry| (entry.start.row, entry.start.column));
        Ok(())
    }

    pub fn unmerge_cells(&mut self, range: &CellRange) {
        self.merged_ranges.retain(|entry| entry != range);
    }

    pub fn contains_merged_cells(&self, range: &CellRange) -> bool {
        self.merged_ranges
            .iter()
            .any(|entry| entry.intersects(range))
    }

    pub fn is_exactly_one_merged_cell(&self, range: &CellRange) -> bool {
        self.merged_ranges.iter().any(|entry| entry == range)
    }

    pub fn get_cell_view(&self, address: CellAddress) -> SpreadsheetCellView {
        let effective = self.effective_address(address);
        let cell = self.get_cell(address);
        SpreadsheetCellView {
            sheet_name: self.name.clone(),
            address: address.to_a1(),
            effective_address: effective.to_a1(),
            exists: cell.is_some(),
            value: cell.and_then(|entry| entry.value.clone()),
            formula: cell.and_then(|entry| entry.formula.clone()),
            style_index: cell.map(|entry| entry.style_index).unwrap_or(0),
            data: cell.and_then(SpreadsheetCell::data),
            is_calculation_error: cell
                .map(SpreadsheetCell::is_calculation_error)
                .unwrap_or(false),
            calculation_error_message: cell
                .and_then(SpreadsheetCell::calculation_error_message)
                .map(str::to_string),
            citations: cell
                .map(|entry| entry.citations.clone())
                .unwrap_or_default(),
        }
    }

    pub fn get_cell_view_by_indices(&self, column: u32, row: u32) -> SpreadsheetCellView {
        self.get_cell_view(CellAddress { column, row })
    }

    pub fn get_cell_field(
        &self,
        address: CellAddress,
        field: &str,
    ) -> Result<Option<Value>, SpreadsheetArtifactError> {
        let cell = self.get_cell(address);
        Ok(match field {
            "value" => cell.and_then(|entry| {
                entry
                    .value
                    .as_ref()
                    .map(SpreadsheetCellValue::to_json_value)
            }),
            "formula" => cell.and_then(|entry| entry.formula.clone().map(Value::String)),
            "style_index" => cell.map(|entry| Value::Number(entry.style_index.into())),
            "data" => cell.and_then(SpreadsheetCell::data),
            "citations" => cell
                .map(|entry| serde_json::to_value(&entry.citations))
                .transpose()
                .map_err(|error| SpreadsheetArtifactError::Serialization {
                    message: error.to_string(),
                })?,
            other => {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: "get_cell_field".to_string(),
                    message: format!("unsupported field `{other}`"),
                });
            }
        })
    }

    pub fn get_cell_field_by_indices(
        &self,
        column: u32,
        row: u32,
        field: &str,
    ) -> Result<Option<Value>, SpreadsheetArtifactError> {
        self.get_cell_field(CellAddress { column, row }, field)
    }

    pub fn get_raw_cell(&self, address: CellAddress) -> Option<SpreadsheetCell> {
        self.get_cell(address).cloned()
    }

    pub fn top_left_style_index(&self, range: &CellRange) -> u32 {
        self.get_cell(range.start)
            .map(|cell| cell.style_index)
            .unwrap_or(0)
    }

    pub fn get_range_view(&self, range: &CellRange) -> SpreadsheetRangeView {
        let mut values = Vec::new();
        let mut formulas = Vec::new();
        let mut style_indices = Vec::new();
        let mut data = Vec::new();

        for row in range.start.row..=range.end.row {
            let mut value_row = Vec::new();
            let mut formula_row = Vec::new();
            let mut style_row = Vec::new();
            let mut data_row = Vec::new();
            for column in range.start.column..=range.end.column {
                let cell = self.get_cell(CellAddress { column, row });
                value_row.push(cell.and_then(|entry| entry.value.clone()));
                formula_row.push(cell.and_then(|entry| entry.formula.clone()));
                style_row.push(cell.map(|entry| entry.style_index).unwrap_or(0));
                data_row.push(cell.and_then(SpreadsheetCell::data));
            }
            values.push(value_row);
            formulas.push(formula_row);
            style_indices.push(style_row);
            data.push(data_row);
        }

        SpreadsheetRangeView {
            sheet_name: self.name.clone(),
            address: range.to_a1(),
            values,
            formulas,
            style_indices,
            data,
            is_single_cell: range.is_single_cell(),
            is_single_row: range.is_single_row(),
            is_single_column: range.is_single_column(),
            contains_merged_cells: self.contains_merged_cells(range),
            is_exactly_one_merged_cell: self.is_exactly_one_merged_cell(range),
        }
    }

    pub fn to_rendered_text(&self, range: Option<&CellRange>) -> String {
        let target = range
            .cloned()
            .or_else(|| self.minimum_range())
            .unwrap_or_else(|| {
                CellRange::from_start_end(
                    CellAddress { column: 1, row: 1 },
                    CellAddress { column: 1, row: 1 },
                )
            });
        let mut lines = Vec::new();
        for row in target.start.row..=target.end.row {
            let mut entries = Vec::new();
            for column in target.start.column..=target.end.column {
                let view = self.get_cell_view(CellAddress { column, row });
                let text = match view.data {
                    Some(Value::String(value)) => value,
                    Some(Value::Bool(value)) => value.to_string(),
                    Some(Value::Number(value)) => value.to_string(),
                    Some(Value::Null) | None => String::new(),
                    Some(other) => other.to_string(),
                };
                entries.push(text);
            }
            lines.push(entries.join("\t"));
        }
        lines.join("\n")
    }

    pub fn cleanup_and_validate_sheet(&mut self) -> Result<(), SpreadsheetArtifactError> {
        self.cells.retain(|_, cell| !cell.is_empty());

        for (column, width) in &self.column_widths {
            if *column == 0 || *width <= 0.0 {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: "cleanup_and_validate_sheet".to_string(),
                    message: format!("invalid column width entry for column {column}"),
                });
            }
        }
        for (row, height) in &self.row_heights {
            if *row == 0 || *height <= 0.0 {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: "cleanup_and_validate_sheet".to_string(),
                    message: format!("invalid row height entry for row {row}"),
                });
            }
        }

        self.merged_ranges.sort_by_key(|range| {
            (
                range.start.row,
                range.start.column,
                range.end.row,
                range.end.column,
            )
        });
        self.merged_ranges.dedup();
        for index in 0..self.merged_ranges.len() {
            for other in index + 1..self.merged_ranges.len() {
                if self.merged_ranges[index].intersects(&self.merged_ranges[other]) {
                    return Err(SpreadsheetArtifactError::MergeConflict {
                        action: "cleanup_and_validate_sheet".to_string(),
                        range: self.merged_ranges[index].to_a1(),
                        conflict: self.merged_ranges[other].to_a1(),
                    });
                }
            }
        }
        self.validate_tables("cleanup_and_validate_sheet")?;
        self.validate_charts("cleanup_and_validate_sheet")?;
        self.validate_pivot_tables("cleanup_and_validate_sheet")?;
        Ok(())
    }

    fn ensure_range_write_allowed(
        &self,
        range: &CellRange,
        allow_exact_merged_cell: bool,
        action: &str,
    ) -> Result<(), SpreadsheetArtifactError> {
        for merged in &self.merged_ranges {
            if !merged.intersects(range) {
                continue;
            }
            if allow_exact_merged_cell && merged == range {
                continue;
            }
            if merged.contains_range(range) || range.contains_range(merged) {
                if allow_exact_merged_cell || merged != range {
                    return Err(SpreadsheetArtifactError::MergeConflict {
                        action: action.to_string(),
                        range: range.to_a1(),
                        conflict: merged.to_a1(),
                    });
                }
            } else {
                return Err(SpreadsheetArtifactError::MergeConflict {
                    action: action.to_string(),
                    range: range.to_a1(),
                    conflict: merged.to_a1(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetArtifact {
    pub artifact_id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub sheets: Vec<SpreadsheetSheet>,
    pub auto_recalculate: bool,
    #[serde(default)]
    pub text_styles: BTreeMap<u32, crate::SpreadsheetTextStyle>,
    #[serde(default)]
    pub fills: BTreeMap<u32, crate::SpreadsheetFill>,
    #[serde(default)]
    pub borders: BTreeMap<u32, crate::SpreadsheetBorder>,
    #[serde(default)]
    pub number_formats: BTreeMap<u32, crate::SpreadsheetNumberFormat>,
    #[serde(default)]
    pub cell_formats: BTreeMap<u32, crate::SpreadsheetCellFormat>,
    #[serde(default)]
    pub differential_formats: BTreeMap<u32, crate::SpreadsheetDifferentialFormat>,
}

impl SpreadsheetArtifact {
    pub fn new(name: Option<String>) -> Self {
        Self {
            artifact_id: format!("spreadsheet_{}", Uuid::new_v4().simple()),
            name,
            sheets: Vec::new(),
            auto_recalculate: false,
            text_styles: BTreeMap::new(),
            fills: BTreeMap::new(),
            borders: BTreeMap::new(),
            number_formats: BTreeMap::new(),
            cell_formats: BTreeMap::new(),
            differential_formats: BTreeMap::new(),
        }
    }

    pub fn allowed_file_extensions() -> &'static [&'static str] {
        &["xlsx", "json", "bin"]
    }

    pub fn allowed_file_mime_types() -> &'static [&'static str] {
        &[
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/json",
            "application/octet-stream",
        ]
    }

    pub fn allowed_file_types() -> &'static [SpreadsheetFileType] {
        &[
            SpreadsheetFileType::Xlsx,
            SpreadsheetFileType::Json,
            SpreadsheetFileType::Binary,
        ]
    }

    pub fn get_output_file_name(
        &self,
        suffix: Option<&str>,
        file_type: SpreadsheetFileType,
    ) -> String {
        let extension = match file_type {
            SpreadsheetFileType::Xlsx => "xlsx",
            SpreadsheetFileType::Json => "json",
            SpreadsheetFileType::Binary => "bin",
        };
        match suffix {
            Some(suffix) => format!("{}_{}.{}", self.artifact_id, suffix, extension),
            None => format!("{}.{}", self.artifact_id, extension),
        }
    }

    pub fn create_sheet(
        &mut self,
        name: String,
    ) -> Result<&mut SpreadsheetSheet, SpreadsheetArtifactError> {
        if name.trim().is_empty() {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "create_sheet".to_string(),
                message: "sheet name cannot be empty".to_string(),
            });
        }
        if self.sheets.iter().any(|sheet| sheet.name == name) {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "create_sheet".to_string(),
                message: format!("sheet `{name}` already exists"),
            });
        }
        self.sheets.push(SpreadsheetSheet::new(name));
        self.sheets
            .last_mut()
            .ok_or_else(|| SpreadsheetArtifactError::Serialization {
                message: "created sheet was not available".to_string(),
            })
    }

    pub fn get_sheet(&self, name: Option<&str>, index: Option<usize>) -> Option<&SpreadsheetSheet> {
        if let Some(name) = name {
            self.sheets.iter().find(|sheet| sheet.name == name)
        } else if let Some(index) = index {
            self.sheets.get(index)
        } else {
            None
        }
    }

    pub fn get_sheet_mut(
        &mut self,
        name: Option<&str>,
        index: Option<usize>,
    ) -> Option<&mut SpreadsheetSheet> {
        if let Some(name) = name {
            self.sheets.iter_mut().find(|sheet| sheet.name == name)
        } else if let Some(index) = index {
            self.sheets.get_mut(index)
        } else {
            None
        }
    }

    pub fn sheet_lookup(
        &self,
        action: &str,
        name: Option<&str>,
        index: Option<usize>,
    ) -> Result<&SpreadsheetSheet, SpreadsheetArtifactError> {
        self.get_sheet(name, index)
            .ok_or_else(|| SpreadsheetArtifactError::SheetLookup {
                action: action.to_string(),
                message: match (name, index) {
                    (Some(name), _) => format!("sheet `{name}` was not found"),
                    (None, Some(index)) => format!("sheet index {index} was not found"),
                    (None, None) => "sheet name or index is required".to_string(),
                },
            })
    }

    pub fn sheet_lookup_mut(
        &mut self,
        action: &str,
        name: Option<&str>,
        index: Option<usize>,
    ) -> Result<&mut SpreadsheetSheet, SpreadsheetArtifactError> {
        self.get_sheet_mut(name, index)
            .ok_or_else(|| SpreadsheetArtifactError::SheetLookup {
                action: action.to_string(),
                message: match (name, index) {
                    (Some(name), _) => format!("sheet `{name}` was not found"),
                    (None, Some(index)) => format!("sheet index {index} was not found"),
                    (None, None) => "sheet name or index is required".to_string(),
                },
            })
    }

    pub fn rename_sheet(
        &mut self,
        new_name: String,
        old_name: Option<&str>,
        index: Option<usize>,
    ) -> Result<(), SpreadsheetArtifactError> {
        if self.sheets.iter().any(|sheet| sheet.name == new_name) {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "rename_sheet".to_string(),
                message: format!("sheet `{new_name}` already exists"),
            });
        }
        let sheet = self.sheet_lookup_mut("rename_sheet", old_name, index)?;
        sheet.name = new_name;
        Ok(())
    }

    pub fn delete_sheet(
        &mut self,
        name: Option<&str>,
        index: Option<usize>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let remove_index = if let Some(name) = name {
            self.sheets
                .iter()
                .position(|sheet| sheet.name == name)
                .ok_or_else(|| SpreadsheetArtifactError::SheetLookup {
                    action: "delete_sheet".to_string(),
                    message: format!("sheet `{name}` was not found"),
                })?
        } else if let Some(index) = index {
            if index >= self.sheets.len() {
                return Err(SpreadsheetArtifactError::IndexOutOfRange {
                    action: "delete_sheet".to_string(),
                    index,
                    len: self.sheets.len(),
                });
            }
            index
        } else {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: "delete_sheet".to_string(),
                message: "sheet name or index is required".to_string(),
            });
        };
        self.sheets.remove(remove_index);
        Ok(())
    }

    pub fn list_sheet_names(&self) -> Vec<String> {
        self.sheets.iter().map(|sheet| sheet.name.clone()).collect()
    }

    pub fn summary(&self) -> SpreadsheetSummary {
        SpreadsheetSummary {
            artifact_id: self.artifact_id.clone(),
            sheets: self.sheets.iter().map(SpreadsheetSheet::summary).collect(),
            size_bytes: self.to_bytes().len(),
        }
    }

    pub fn calculate(&mut self) {
        recalculate_workbook(self);
    }

    pub fn recalculate(&mut self) {
        self.calculate();
    }

    pub fn to_dict(&self) -> Result<Value, SpreadsheetArtifactError> {
        serde_json::to_value(self).map_err(|error| SpreadsheetArtifactError::Serialization {
            message: error.to_string(),
        })
    }

    pub fn to_json(&self) -> Result<String, SpreadsheetArtifactError> {
        serde_json::to_string_pretty(self).map_err(|error| {
            SpreadsheetArtifactError::Serialization {
                message: error.to_string(),
            }
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_json()
            .unwrap_or_else(|_| "{}".to_string())
            .into_bytes()
    }

    pub fn to_bytes_base64(&self) -> String {
        BASE64_STANDARD.encode(self.to_bytes())
    }

    pub fn from_dict(
        data: Value,
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        let mut artifact: Self = serde_json::from_value(data).map_err(|error| {
            SpreadsheetArtifactError::Serialization {
                message: error.to_string(),
            }
        })?;
        if let Some(artifact_id) = artifact_id {
            artifact.artifact_id = artifact_id;
        } else if artifact.artifact_id.is_empty() {
            artifact.artifact_id = format!("spreadsheet_{}", Uuid::new_v4().simple());
        }
        Ok(artifact)
    }

    pub fn from_json(
        json: impl AsRef<[u8]>,
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        let text = std::str::from_utf8(json.as_ref()).map_err(|error| {
            SpreadsheetArtifactError::Serialization {
                message: error.to_string(),
            }
        })?;
        Self::from_dict(
            serde_json::from_str(text).map_err(|error| {
                SpreadsheetArtifactError::Serialization {
                    message: error.to_string(),
                }
            })?,
            artifact_id,
        )
    }

    pub fn from_bytes(
        bytes: &[u8],
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        Self::from_json(bytes, artifact_id)
    }

    pub fn from_source_file(
        path: &Path,
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "xlsx" => import_xlsx(path, artifact_id),
            "json" => {
                let bytes = std::fs::read(path).map_err(|error| {
                    SpreadsheetArtifactError::ImportFailed {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    }
                })?;
                Self::from_json(bytes, artifact_id)
            }
            "bin" => {
                let bytes = std::fs::read(path).map_err(|error| {
                    SpreadsheetArtifactError::ImportFailed {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    }
                })?;
                Self::from_bytes(&bytes, artifact_id)
            }
            other => Err(SpreadsheetArtifactError::ImportFailed {
                path: path.to_path_buf(),
                message: format!("unsupported import file type `{other}`"),
            }),
        }
    }

    pub fn read(
        path: &Path,
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        Self::from_source_file(path, artifact_id)
    }

    pub fn load(
        path: &Path,
        artifact_id: Option<String>,
    ) -> Result<Self, SpreadsheetArtifactError> {
        Self::from_source_file(path, artifact_id)
    }

    pub fn save(
        &mut self,
        path: &Path,
        file_type: Option<&str>,
    ) -> Result<PathBuf, SpreadsheetArtifactError> {
        self.to_source_file(path, file_type)
    }

    pub fn export(&mut self, path: &Path) -> Result<PathBuf, SpreadsheetArtifactError> {
        self.to_source_file(path, Some("xlsx"))
    }

    pub fn to_source_file(
        &mut self,
        path: &Path,
        file_type: Option<&str>,
    ) -> Result<PathBuf, SpreadsheetArtifactError> {
        let selected = file_type
            .map(str::to_string)
            .or_else(|| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "xlsx".to_string())
            .to_ascii_lowercase();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                SpreadsheetArtifactError::ExportFailed {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                }
            })?;
        }

        match selected.as_str() {
            "xlsx" => {
                for sheet in &self.sheets {
                    if !sheet.charts.is_empty()
                        || !sheet.tables.is_empty()
                        || !sheet.conditional_formats.is_empty()
                        || !sheet.pivot_tables.is_empty()
                    {
                        return Err(SpreadsheetArtifactError::ExportFailed {
                            path: path.to_path_buf(),
                            message: format!(
                                "xlsx export does not yet support charts, tables, conditional formats, or pivot tables on sheet `{}`; use json or bin export instead",
                                sheet.name
                            ),
                        });
                    }
                }
                write_xlsx(self, path)
            }
            "json" => {
                let json = self.to_json()?;
                std::fs::write(path, json).map_err(|error| {
                    SpreadsheetArtifactError::ExportFailed {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    }
                })?;
                Ok(path.to_path_buf())
            }
            "bin" => {
                std::fs::write(path, self.to_bytes()).map_err(|error| {
                    SpreadsheetArtifactError::ExportFailed {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    }
                })?;
                Ok(path.to_path_buf())
            }
            other => Err(SpreadsheetArtifactError::ExportFailed {
                path: path.to_path_buf(),
                message: format!("unsupported export file type `{other}`"),
            }),
        }
    }
}
