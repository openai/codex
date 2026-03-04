use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::CellRange;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCellRangeRef;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotFieldItem {
    pub item_type: Option<String>,
    pub index: Option<u32>,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotField {
    pub index: u32,
    pub name: Option<String>,
    pub axis: Option<String>,
    #[serde(default)]
    pub items: Vec<SpreadsheetPivotFieldItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotFieldReference {
    pub field_index: u32,
    pub field_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotPageField {
    pub field_index: u32,
    pub field_name: Option<String>,
    pub selected_item: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotDataField {
    pub field_index: u32,
    pub field_name: Option<String>,
    pub name: Option<String>,
    pub subtotal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotFilter {
    pub field_index: Option<u32>,
    pub field_name: Option<String>,
    pub filter_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotTable {
    pub name: String,
    pub cache_id: u32,
    pub address: Option<String>,
    #[serde(default)]
    pub row_fields: Vec<SpreadsheetPivotFieldReference>,
    #[serde(default)]
    pub column_fields: Vec<SpreadsheetPivotFieldReference>,
    #[serde(default)]
    pub page_fields: Vec<SpreadsheetPivotPageField>,
    #[serde(default)]
    pub data_fields: Vec<SpreadsheetPivotDataField>,
    #[serde(default)]
    pub filters: Vec<SpreadsheetPivotFilter>,
    #[serde(default)]
    pub pivot_fields: Vec<SpreadsheetPivotField>,
    pub style_name: Option<String>,
    pub part_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadsheetPivotTableLookup<'a> {
    pub name: Option<&'a str>,
    pub index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetPivotCacheDefinition {
    pub definition_path: String,
    #[serde(default)]
    pub field_names: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpreadsheetPivotPreservation {
    #[serde(default)]
    pub caches: BTreeMap<u32, SpreadsheetPivotCacheDefinition>,
    #[serde(default)]
    pub parts: BTreeMap<String, String>,
}

impl SpreadsheetPivotTable {
    pub fn range(&self) -> Result<Option<CellRange>, SpreadsheetArtifactError> {
        self.address.as_deref().map(CellRange::parse).transpose()
    }

    pub fn range_ref(
        &self,
        sheet_name: &str,
    ) -> Result<Option<SpreadsheetCellRangeRef>, SpreadsheetArtifactError> {
        Ok(self
            .range()?
            .map(|range| SpreadsheetCellRangeRef::new(sheet_name.to_string(), &range)))
    }
}

impl SpreadsheetSheet {
    pub fn list_pivot_tables(
        &self,
        range: Option<&CellRange>,
    ) -> Result<Vec<SpreadsheetPivotTable>, SpreadsheetArtifactError> {
        Ok(self
            .pivot_tables
            .iter()
            .filter(|pivot_table| {
                range.is_none_or(|target| {
                    pivot_table
                        .range()
                        .ok()
                        .flatten()
                        .is_some_and(|pivot_range| pivot_range.intersects(target))
                })
            })
            .cloned()
            .collect())
    }

    pub fn get_pivot_table(
        &self,
        action: &str,
        lookup: SpreadsheetPivotTableLookup,
    ) -> Result<&SpreadsheetPivotTable, SpreadsheetArtifactError> {
        if let Some(name) = lookup.name {
            return self
                .pivot_tables
                .iter()
                .find(|pivot_table| pivot_table.name == name)
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("pivot table `{name}` was not found"),
                });
        }
        if let Some(index) = lookup.index {
            return self.pivot_tables.get(index).ok_or_else(|| {
                SpreadsheetArtifactError::IndexOutOfRange {
                    action: action.to_string(),
                    index,
                    len: self.pivot_tables.len(),
                }
            });
        }
        Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "pivot table name or index is required".to_string(),
        })
    }

    pub fn validate_pivot_tables(&self, action: &str) -> Result<(), SpreadsheetArtifactError> {
        for pivot_table in &self.pivot_tables {
            if pivot_table.name.trim().is_empty() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: "pivot table name cannot be empty".to_string(),
                });
            }
            if let Some(address) = &pivot_table.address {
                CellRange::parse(address)?;
            }
        }
        Ok(())
    }
}
