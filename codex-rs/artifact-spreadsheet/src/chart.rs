use serde::Deserialize;
use serde::Serialize;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetSheet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpreadsheetChartType {
    Area,
    Bar,
    Doughnut,
    Line,
    Pie,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpreadsheetChartLegendPosition {
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChartLegend {
    pub visible: bool,
    pub position: SpreadsheetChartLegendPosition,
    pub overlay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChartAxis {
    pub linked_number_format: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChartSeries {
    pub id: u32,
    pub name: Option<String>,
    pub category_sheet_name: Option<String>,
    pub category_range: String,
    pub value_sheet_name: Option<String>,
    pub value_range: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChart {
    pub id: u32,
    pub chart_type: SpreadsheetChartType,
    pub source_sheet_name: Option<String>,
    pub source_range: Option<String>,
    pub title: Option<String>,
    pub style_index: u32,
    pub display_blanks_as: String,
    pub legend: SpreadsheetChartLegend,
    pub category_axis: SpreadsheetChartAxis,
    pub value_axis: SpreadsheetChartAxis,
    #[serde(default)]
    pub series: Vec<SpreadsheetChartSeries>,
}

#[derive(Debug, Clone, Default)]
pub struct SpreadsheetChartLookup {
    pub id: Option<u32>,
    pub index: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChartCreateOptions {
    pub id: Option<u32>,
    pub title: Option<String>,
    pub legend_visible: Option<bool>,
    pub legend_position: Option<SpreadsheetChartLegendPosition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetChartProperties {
    pub title: Option<String>,
    pub legend_visible: Option<bool>,
    pub legend_position: Option<SpreadsheetChartLegendPosition>,
}

impl SpreadsheetSheet {
    pub fn list_charts(
        &self,
        range: Option<&CellRange>,
    ) -> Result<Vec<SpreadsheetChart>, SpreadsheetArtifactError> {
        Ok(self
            .charts
            .iter()
            .filter(|chart| {
                range.is_none_or(|target| {
                    chart
                        .source_range
                        .as_deref()
                        .map(CellRange::parse)
                        .transpose()
                        .ok()
                        .flatten()
                        .is_some_and(|chart_range| chart_range.intersects(target))
                })
            })
            .cloned()
            .collect())
    }

    pub fn get_chart(
        &self,
        action: &str,
        lookup: SpreadsheetChartLookup,
    ) -> Result<&SpreadsheetChart, SpreadsheetArtifactError> {
        if let Some(id) = lookup.id {
            return self
                .charts
                .iter()
                .find(|chart| chart.id == id)
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("chart id `{id}` was not found"),
                });
        }
        if let Some(index) = lookup.index {
            return self.charts.get(index).ok_or_else(|| {
                SpreadsheetArtifactError::IndexOutOfRange {
                    action: action.to_string(),
                    index,
                    len: self.charts.len(),
                }
            });
        }
        Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "chart id or index is required".to_string(),
        })
    }

    pub fn create_chart(
        &mut self,
        action: &str,
        chart_type: SpreadsheetChartType,
        source_sheet_name: Option<String>,
        source_range: &CellRange,
        options: SpreadsheetChartCreateOptions,
    ) -> Result<u32, SpreadsheetArtifactError> {
        if source_range.width() < 2 {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "chart source range must include at least two columns".to_string(),
            });
        }
        let id = if let Some(id) = options.id {
            if self.charts.iter().any(|chart| chart.id == id) {
                return Err(SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("chart id `{id}` already exists"),
                });
            }
            id
        } else {
            self.charts.iter().map(|chart| chart.id).max().unwrap_or(0) + 1
        };
        let series = (source_range.start.column + 1..=source_range.end.column)
            .enumerate()
            .map(|(index, value_column)| SpreadsheetChartSeries {
                id: index as u32 + 1,
                name: None,
                category_sheet_name: source_sheet_name.clone(),
                category_range: CellRange::from_start_end(
                    source_range.start,
                    CellAddress {
                        column: source_range.start.column,
                        row: source_range.end.row,
                    },
                )
                .to_a1(),
                value_sheet_name: source_sheet_name.clone(),
                value_range: CellRange::from_start_end(
                    CellAddress {
                        column: value_column,
                        row: source_range.start.row,
                    },
                    CellAddress {
                        column: value_column,
                        row: source_range.end.row,
                    },
                )
                .to_a1(),
            })
            .collect::<Vec<_>>();
        self.charts.push(SpreadsheetChart {
            id,
            chart_type,
            source_sheet_name,
            source_range: Some(source_range.to_a1()),
            title: options.title,
            style_index: 102,
            display_blanks_as: "gap".to_string(),
            legend: SpreadsheetChartLegend {
                visible: options.legend_visible.unwrap_or(true),
                position: options
                    .legend_position
                    .unwrap_or(SpreadsheetChartLegendPosition::Bottom),
                overlay: false,
            },
            category_axis: SpreadsheetChartAxis {
                linked_number_format: true,
            },
            value_axis: SpreadsheetChartAxis {
                linked_number_format: true,
            },
            series,
        });
        Ok(id)
    }

    pub fn add_chart_series(
        &mut self,
        action: &str,
        lookup: SpreadsheetChartLookup,
        mut series: SpreadsheetChartSeries,
    ) -> Result<u32, SpreadsheetArtifactError> {
        validate_chart_series(action, &series)?;
        let chart = self.get_chart_mut(action, lookup)?;
        let next_id = chart.series.iter().map(|entry| entry.id).max().unwrap_or(0) + 1;
        series.id = next_id;
        chart.series.push(series);
        Ok(next_id)
    }

    pub fn delete_chart(
        &mut self,
        action: &str,
        lookup: SpreadsheetChartLookup,
    ) -> Result<(), SpreadsheetArtifactError> {
        let index = if let Some(id) = lookup.id {
            self.charts
                .iter()
                .position(|chart| chart.id == id)
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("chart id `{id}` was not found"),
                })?
        } else if let Some(index) = lookup.index {
            if index >= self.charts.len() {
                return Err(SpreadsheetArtifactError::IndexOutOfRange {
                    action: action.to_string(),
                    index,
                    len: self.charts.len(),
                });
            }
            index
        } else {
            return Err(SpreadsheetArtifactError::InvalidArgs {
                action: action.to_string(),
                message: "chart id or index is required".to_string(),
            });
        };
        self.charts.remove(index);
        Ok(())
    }

    pub fn set_chart_properties(
        &mut self,
        action: &str,
        lookup: SpreadsheetChartLookup,
        properties: SpreadsheetChartProperties,
    ) -> Result<(), SpreadsheetArtifactError> {
        let chart = self.get_chart_mut(action, lookup)?;
        if let Some(title) = properties.title {
            chart.title = Some(title);
        }
        if let Some(visible) = properties.legend_visible {
            chart.legend.visible = visible;
        }
        if let Some(position) = properties.legend_position {
            chart.legend.position = position;
        }
        Ok(())
    }

    pub fn validate_charts(&self, action: &str) -> Result<(), SpreadsheetArtifactError> {
        for chart in &self.charts {
            if let Some(source_range) = &chart.source_range {
                let range = CellRange::parse(source_range)?;
                if range.width() < 2 {
                    return Err(SpreadsheetArtifactError::InvalidArgs {
                        action: action.to_string(),
                        message: format!(
                            "chart `{}` source range `{source_range}` is too narrow",
                            chart.id
                        ),
                    });
                }
            }
            for series in &chart.series {
                validate_chart_series(action, series)?;
            }
        }
        Ok(())
    }

    fn get_chart_mut(
        &mut self,
        action: &str,
        lookup: SpreadsheetChartLookup,
    ) -> Result<&mut SpreadsheetChart, SpreadsheetArtifactError> {
        if let Some(id) = lookup.id {
            return self
                .charts
                .iter_mut()
                .find(|chart| chart.id == id)
                .ok_or_else(|| SpreadsheetArtifactError::InvalidArgs {
                    action: action.to_string(),
                    message: format!("chart id `{id}` was not found"),
                });
        }
        if let Some(index) = lookup.index {
            let len = self.charts.len();
            return self.charts.get_mut(index).ok_or_else(|| {
                SpreadsheetArtifactError::IndexOutOfRange {
                    action: action.to_string(),
                    index,
                    len,
                }
            });
        }
        Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "chart id or index is required".to_string(),
        })
    }
}

fn validate_chart_series(
    action: &str,
    series: &SpreadsheetChartSeries,
) -> Result<(), SpreadsheetArtifactError> {
    let category_range = CellRange::parse(&series.category_range)?;
    let value_range = CellRange::parse(&series.value_range)?;
    if !category_range.is_single_column() || !value_range.is_single_column() {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "chart category and value ranges must be single-column ranges".to_string(),
        });
    }
    if category_range.height() != value_range.height() {
        return Err(SpreadsheetArtifactError::InvalidArgs {
            action: action.to_string(),
            message: "chart category and value series lengths must match".to_string(),
        });
    }
    Ok(())
}
