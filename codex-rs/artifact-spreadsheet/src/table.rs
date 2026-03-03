use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCellValue;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetTableColumn {
    pub id: u32,
    pub name: String,
    pub totals_row_label: Option<String>,
    pub totals_row_function: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetTable {
    pub id: u32,
    pub name: String,
    pub display_name: String,
    pub range: String,
    pub header_row_count: u32,
    pub totals_row_count: u32,
    pub style_name: Option<String>,
    pub show_first_column: bool,
    pub show_last_column: bool,
    pub show_row_stripes: bool,
    pub show_column_stripes: bool,
    #[serde(default)]
    pub columns: Vec<SpreadsheetTableColumn>,
    #[serde(default)]
    pub filters: BTreeMap<u32, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadsheetTableView {
    pub id: u32,
    pub name: String,
    pub display_name: String,
    pub address: String,
    pub full_range: String,
    pub header_row_count: u32,
    pub totals_row_count: u32,
    pub totals_row_visible: bool,
    pub header_row_range: Option<String>,
    pub data_body_range: Option<String>,
    pub totals_row_range: Option<String>,
    pub style_name: Option<String>,
    pub show_first_column: bool,
    pub show_last_column: bool,
    pub show_row_stripes: bool,
    pub show_column_stripes: bool,
    pub columns: Vec<SpreadsheetTableColumn>,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadsheetTableLookup<'a> {
    pub name: Option<&'a str>,
    pub display_name: Option<&'a str>,
    pub id: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetCreateTableOptions {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub header_row_count: u32,
    pub totals_row_count: u32,
    pub style_name: Option<String>,
    pub show_first_column: bool,
    pub show_last_column: bool,
    pub show_row_stripes: bool,
    pub show_column_stripes: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetTableStyleOptions {
    pub style_name: Option<String>,
    pub show_first_column: Option<bool>,
    pub show_last_column: Option<bool>,
    pub show_row_stripes: Option<bool>,
    pub show_column_stripes: Option<bool>,
}

impl SpreadsheetTable {
    pub fn range(&self) -> Result<CellRange, SpreadsheetArtifactError> {
        CellRange::parse(&self.range)
    }

    pub fn address(&self) -> String {
        self.range.clone()
    }

    pub fn full_range(&self) -> String {
        self.range.clone()
    }

    pub fn totals_row_visible(&self) -> bool {
        self.totals_row_count > 0
    }

    pub fn header_row_range(&self) -> Result<Option<CellRange>, SpreadsheetArtifactError> {
        if self.header_row_count == 0 {
            return Ok(None);
        }
        let range = self.range()?;
        Ok(Some(CellRange::from_start_end(
            range.start,
            CellAddress {
                column: range.end.column,
                row: range.start.row + self.header_row_count - 1,
            },
        )))
    }

    pub fn data_body_range(&self) -> Result<Option<CellRange>, SpreadsheetArtifactError> {
        let range = self.range()?;
        let start_row = range.start.row + self.header_row_count;
        let end_row = range.end.row.saturating_sub(self.totals_row_count);
        if start_row > end_row {
            return Ok(None);
        }
        Ok(Some(CellRange::from_start_end(
            CellAddress {
                column: range.start.column,
                row: start_row,
            },
            CellAddress {
                column: range.end.column,
                row: end_row,
            },
        )))
    }

    pub fn totals_row_range(&self) -> Result<Option<CellRange>, SpreadsheetArtifactError> {
        if self.totals_row_count == 0 {
            return Ok(None);
        }
        let range = self.range()?;
        Ok(Some(CellRange::from_start_end(
            CellAddress {
                column: range.start.column,
                row: range.end.row - self.totals_row_count + 1,
            },
            range.end,
        )))
    }

    pub fn view(&self) -> Result<SpreadsheetTableView, SpreadsheetArtifactError> {
        Ok(SpreadsheetTableView {
            id: self.id,
            name: self.name.clone(),
            display_name: self.display_name.clone(),
            address: self.address(),
            full_range: self.full_range(),
            header_row_count: self.header_row_count,
            totals_row_count: self.totals_row_count,
            totals_row_visible: self.totals_row_visible(),
            header_row_range: self.header_row_range()?.map(|range| range.to_a1()),
            data_body_range: self.data_body_range()?.map(|range| range.to_a1()),
            totals_row_range: self.totals_row_range()?.map(|range| range.to_a1()),
            style_name: self.style_name.clone(),
            show_first_column: self.show_first_column,
            show_last_column: self.show_last_column,
            show_row_stripes: self.show_row_stripes,
            show_column_stripes: self.show_column_stripes,
            columns: self.columns.clone(),
        })
    }
}

impl SpreadsheetSheet {
    pub fn create_table(
        &mut self,
        action: &str,
        range: &CellRange,
        options: SpreadsheetCreateTableOptions,
    ) -> Result<u32, SpreadsheetArtifactError> {
        validate_table_geometry(
            action,
            range,
            options.header_row_count,
            options.totals_row_count,
        )?;
        for table in &self.tables {
            let table_range = table.range()?;
            if table_range.intersects(range) {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!(
                        "table range `{}` intersects existing table `{}`",
                        range.to_a1(),
                        table.name
                    ),
                });
            }
        }

        let next_id = self.tables.iter().map(|table| table.id).max().unwrap_or(0) + 1;
        let name = options.name.unwrap_or_else(|| format!("Table{next_id}"));
        if name.trim().is_empty() {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "table name cannot be empty".to_string(),
            });
        }
        let display_name = options.display_name.unwrap_or_else(|| name.clone());
        if display_name.trim().is_empty() {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "table display_name cannot be empty".to_string(),
            });
        }
        ensure_unique_table_name(&self.tables, action, &name, &display_name, None)?;

        let columns = build_table_columns(self, range, options.header_row_count);
        self.tables.push(SpreadsheetTable {
            id: next_id,
            name,
            display_name,
            range: range.to_a1(),
            header_row_count: options.header_row_count,
            totals_row_count: options.totals_row_count,
            style_name: options.style_name,
            show_first_column: options.show_first_column,
            show_last_column: options.show_last_column,
            show_row_stripes: options.show_row_stripes,
            show_column_stripes: options.show_column_stripes,
            columns,
            filters: BTreeMap::new(),
        });
        Ok(next_id)
    }

    pub fn list_tables(
        &self,
        range: Option<&CellRange>,
    ) -> Result<Vec<SpreadsheetTableView>, SpreadsheetArtifactError> {
        self.tables
            .iter()
            .filter(|table| {
                range.is_none_or(|target| {
                    table
                        .range()
                        .map(|table_range| table_range.intersects(target))
                        .unwrap_or(false)
                })
            })
            .map(SpreadsheetTable::view)
            .collect()
    }

    pub fn get_table(
        &self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<&SpreadsheetTable, SpreadsheetArtifactError> {
        self.table_lookup_internal(action, lookup)
    }

    pub fn get_table_view(
        &self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<SpreadsheetTableView, SpreadsheetArtifactError> {
        self.get_table(action, lookup)?.view()
    }

    pub fn delete_table(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let index = self.table_index(action, lookup)?;
        self.tables.remove(index);
        Ok(())
    }

    pub fn set_table_style(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
        options: SpreadsheetTableStyleOptions,
    ) -> Result<(), SpreadsheetArtifactError> {
        let table = self.table_lookup_mut(action, lookup)?;
        table.style_name = options.style_name;
        if let Some(value) = options.show_first_column {
            table.show_first_column = value;
        }
        if let Some(value) = options.show_last_column {
            table.show_last_column = value;
        }
        if let Some(value) = options.show_row_stripes {
            table.show_row_stripes = value;
        }
        if let Some(value) = options.show_column_stripes {
            table.show_column_stripes = value;
        }
        Ok(())
    }

    pub fn clear_table_filters(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<(), SpreadsheetArtifactError> {
        self.table_lookup_mut(action, lookup)?.filters.clear();
        Ok(())
    }

    pub fn reapply_table_filters(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<(), SpreadsheetArtifactError> {
        let _ = self.table_lookup_mut(action, lookup)?;
        Ok(())
    }

    pub fn rename_table_column(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
        column_id: Option<u32>,
        column_name: Option<&str>,
        new_name: String,
    ) -> Result<SpreadsheetTableColumn, SpreadsheetArtifactError> {
        if new_name.trim().is_empty() {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "table column name cannot be empty".to_string(),
            });
        }
        let table = self.table_lookup_mut(action, lookup)?;
        if table
            .columns
            .iter()
            .any(|column| column.name == new_name && Some(column.id) != column_id)
        {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: format!("table column `{new_name}` already exists"),
            });
        }
        let column = table_column_lookup_mut(&mut table.columns, action, column_id, column_name)?;
        column.name = new_name;
        Ok(column.clone())
    }

    pub fn set_table_column_totals(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
        column_id: Option<u32>,
        column_name: Option<&str>,
        totals_row_label: Option<String>,
        totals_row_function: Option<String>,
    ) -> Result<SpreadsheetTableColumn, SpreadsheetArtifactError> {
        let table = self.table_lookup_mut(action, lookup)?;
        let column = table_column_lookup_mut(&mut table.columns, action, column_id, column_name)?;
        column.totals_row_label = totals_row_label;
        column.totals_row_function = totals_row_function;
        Ok(column.clone())
    }

    pub fn validate_tables(&self, action: &str) -> Result<(), SpreadsheetArtifactError> {
        let mut seen_names = BTreeSet::new();
        let mut seen_display_names = BTreeSet::new();
        for table in &self.tables {
            let range = table.range()?;
            validate_table_geometry(
                action,
                &range,
                table.header_row_count,
                table.totals_row_count,
            )?;
            if !seen_names.insert(table.name.clone()) {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("duplicate table name `{}`", table.name),
                });
            }
            if !seen_display_names.insert(table.display_name.clone()) {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("duplicate table display_name `{}`", table.display_name),
                });
            }
            let column_names = table
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect::<BTreeSet<_>>();
            if column_names.len() != table.columns.len() {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("table `{}` has duplicate column names", table.name),
                });
            }
        }

        for index in 0..self.tables.len() {
            for other in index + 1..self.tables.len() {
                if self.tables[index]
                    .range()?
                    .intersects(&self.tables[other].range()?)
                {
                    return Err(SpreadsheetArtifactError::InvalidArgs {
                        action: action.to_string(),
                        message: format!(
                            "table `{}` intersects table `{}`",
                            self.tables[index].name, self.tables[other].name
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    fn table_index(
        &self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<usize, SpreadsheetArtifactError> {
        self.tables
            .iter()
            .position(|table| table_matches_lookup(table, lookup.clone()))
            .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: describe_missing_table(lookup),
            })
    }

    fn table_lookup_internal(
        &self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<&SpreadsheetTable, SpreadsheetArtifactError> {
        self.tables
            .iter()
            .find(|table| table_matches_lookup(table, lookup.clone()))
            .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: describe_missing_table(lookup),
            })
    }

    fn table_lookup_mut(
        &mut self,
        action: &str,
        lookup: SpreadsheetTableLookup<'_>,
    ) -> Result<&mut SpreadsheetTable, SpreadsheetArtifactError> {
        let index = self.table_index(action, lookup)?;
        Ok(&mut self.tables[index])
    }
}

fn table_matches_lookup(table: &SpreadsheetTable, lookup: SpreadsheetTableLookup<'_>) -> bool {
    if let Some(name) = lookup.name {
        table.name == name
    } else if let Some(display_name) = lookup.display_name {
        table.display_name == display_name
    } else if let Some(id) = lookup.id {
        table.id == id
    } else {
        false
    }
}

fn describe_missing_table(lookup: SpreadsheetTableLookup<'_>) -> String {
    if let Some(name) = lookup.name {
        format!("table name `{name}` was not found")
    } else if let Some(display_name) = lookup.display_name {
        format!("table display_name `{display_name}` was not found")
    } else if let Some(id) = lookup.id {
        format!("table id `{id}` was not found")
    } else {
        "table name, display_name, or id is required".to_string()
    }
}

fn ensure_unique_table_name(
    tables: &[SpreadsheetTable],
    action: &str,
    name: &str,
    display_name: &str,
    exclude_id: Option<u32>,
) -> Result<(), SpreadsheetArtifactError> {
    if tables.iter().any(|table| {
        Some(table.id) != exclude_id && (table.name == name || table.display_name == name)
    }) {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("table name `{name}` already exists"),
        });
    }
    if tables.iter().any(|table| {
        Some(table.id) != exclude_id
            && (table.display_name == display_name || table.name == display_name)
    }) {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: format!("table display_name `{display_name}` already exists"),
        });
    }
    Ok(())
}

fn validate_table_geometry(
    action: &str,
    range: &CellRange,
    header_row_count: u32,
    totals_row_count: u32,
) -> Result<(), SpreadsheetArtifactError> {
    if range.width() == 0 {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "table range must include at least one column".to_string(),
        });
    }
    if header_row_count + totals_row_count > range.height() as u32 {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "table range is smaller than header and totals rows".to_string(),
        });
    }
    Ok(())
}

fn build_table_columns(
    sheet: &SpreadsheetSheet,
    range: &CellRange,
    header_row_count: u32,
) -> Vec<SpreadsheetTableColumn> {
    let header_row = range.start.row + header_row_count.saturating_sub(1);
    let default_names = (0..range.width())
        .map(|index| format!("Column{}", index + 1))
        .collect::<Vec<_>>();
    let names = unique_table_column_names(
        (range.start.column..=range.end.column)
            .enumerate()
            .map(|(index, column)| {
                if header_row_count == 0 {
                    return default_names[index].clone();
                }
                sheet
                    .get_cell(CellAddress {
                        column,
                        row: header_row,
                    })
                    .and_then(|cell| cell.value.as_ref())
                    .map(cell_value_to_table_header)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| default_names[index].clone())
            })
            .collect::<Vec<_>>(),
    );
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| SpreadsheetTableColumn {
            id: index as u32 + 1,
            name,
            totals_row_label: None,
            totals_row_function: None,
        })
        .collect()
}

fn unique_table_column_names(names: Vec<String>) -> Vec<String> {
    let mut seen = BTreeMap::<String, u32>::new();
    names
        .into_iter()
        .map(|name| {
            let entry = seen.entry(name.clone()).or_insert(0);
            *entry += 1;
            if *entry == 1 {
                name
            } else {
                format!("{name}_{}", *entry)
            }
        })
        .collect()
}

fn cell_value_to_table_header(value: &SpreadsheetCellValue) -> String {
    match value {
        SpreadsheetCellValue::Bool(value) => value.to_string(),
        SpreadsheetCellValue::Integer(value) => value.to_string(),
        SpreadsheetCellValue::Float(value) => value.to_string(),
        SpreadsheetCellValue::String(value)
        | SpreadsheetCellValue::DateTime(value)
        | SpreadsheetCellValue::Error(value) => value.clone(),
    }
}

fn table_column_lookup_mut<'a>(
    columns: &'a mut [SpreadsheetTableColumn],
    action: &str,
    column_id: Option<u32>,
    column_name: Option<&str>,
) -> Result<&'a mut SpreadsheetTableColumn, SpreadsheetArtifactError> {
    columns
        .iter_mut()
        .find(|column| {
            if let Some(column_id) = column_id {
                column.id == column_id
            } else if let Some(column_name) = column_name {
                column.name == column_name
            } else {
                false
            }
        })
        .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: if let Some(column_id) = column_id {
                format!("table column id `{column_id}` was not found")
            } else if let Some(column_name) = column_name {
                format!("table column `{column_name}` was not found")
            } else {
                "table column id or name is required".to_string()
            },
        })
}
