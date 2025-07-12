// Datadog-specific code for the DASH demo. Most of this is hacked together but some of it will be useful for Codex eventually

use crate::cell_widget::CellWidget;
use crate::text_block::TextBlock;
use chrono::DateTime;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Axis;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Cell;
use ratatui::widgets::Chart;
use ratatui::widgets::Dataset;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Row;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Table;
use ratatui::widgets::Wrap;
use serde::Deserialize;
use tui_scrollview::ScrollView;
use tui_scrollview::ScrollViewState;

// Example of what's returned by the get_logs tool (see get_logs.json)

#[derive(Deserialize, Debug)]
pub struct Log {
    #[allow(dead_code)]
    host: String,
    #[allow(dead_code)]
    id: String,
    message: String,
    #[allow(dead_code)]
    service: String,
    status: String,
    #[allow(dead_code)]
    timestamp: String,
}

#[derive(Deserialize, Debug)]
pub struct LogsResponse {
    pub data: Vec<Log>,
}

// Combined widget that renders both header and logs
pub struct LogsWithHeaderWidget {
    header_view: TextBlock,
    logs: String,
    header_height: usize,
}

impl Widget for LogsWithHeaderWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render header at the top
        self.header_view.render_window(
            0,
            Rect::new(area.x, area.y, area.width, self.header_height as u16),
            buf,
        );

        // Render logs below the header
        let logs_area = Rect::new(
            area.x,
            area.y + self.header_height as u16,
            area.width,
            area.height.saturating_sub(self.header_height as u16),
        );

        tracing::info!("logs area: {:?}", logs_area);

        // Try to deserialize the logs string
        let response: LogsResponse = match serde_json::from_str(&self.logs) {
            Ok(response) => response,
            Err(e) => {
                // If deserialization fails, render an error message
                let error_msg = format!("Failed to parse logs: {}", e);
                tracing::error!("{}, raw data: {:?}", error_msg, self.logs);
                buf.set_string(
                    logs_area.x,
                    logs_area.y,
                    &error_msg,
                    Style::default().fg(Color::Red),
                );
                return;
            }
        };

        // If there's no data, render an empty state
        if response.data.is_empty() {
            buf.set_string(
                logs_area.x,
                logs_area.y,
                "No logs available",
                Style::default().fg(Color::DarkGray),
            );
            return;
        }

        let header_cells = ["Status", "Service", "Message"]
            .iter()
            .map(|h| {
                Span::styled(
                    *h,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<Span>>();

        let mut longest_status_length = response
            .data
            .iter()
            .map(|log_entry| log_entry.status.len())
            .max()
            .unwrap_or(0);
        longest_status_length = longest_status_length.max("Status".len());

        let mut longest_service_length = response
            .data
            .iter()
            .map(|log_entry| log_entry.service.len())
            .max()
            .unwrap_or(0);
        longest_service_length = longest_service_length.max("Service".len());

        let constraints = [
            Constraint::Length(longest_status_length as u16),
            Constraint::Length(longest_service_length as u16),
            Constraint::Min(50),
        ];

        // Calculate available width for the message column
        let message_column_width = logs_area
            .width
            .saturating_sub(longest_status_length as u16)
            .saturating_sub(longest_service_length as u16)
            .saturating_sub(2) // For borders
            .saturating_sub(2); // For column spacing (2 separators)

        let truncated_rows: Vec<Row> = response
            .data
            .iter()
            .map(|log_entry| {
                let status_style = get_status_style(&log_entry.status);
                let mut message = log_entry.message.clone();
                if message.len() > message_column_width as usize {
                    let target_text_byte_len = message_column_width.saturating_sub(3) as usize;

                    if target_text_byte_len == 0 {
                        message.truncate(0);
                    } else {
                        let mut len_to_keep = 0;
                        // Iterate over characters to find a safe truncation point that
                        // respects UTF-8 character boundaries and fits within the target length.
                        for (char_start_index, char_val) in message.char_indices() {
                            let char_byte_len = char_val.len_utf8();
                            if char_start_index + char_byte_len <= target_text_byte_len {
                                len_to_keep = char_start_index + char_byte_len;
                            } else {
                                // This character would make the text part exceed target_text_byte_len
                                break;
                            }
                        }
                        message.truncate(len_to_keep);
                    }
                    message.push_str("...");
                }
                let cells = vec![
                    Cell::from(log_entry.status.clone()).style(status_style),
                    Cell::from(log_entry.service.clone()),
                    Cell::from(Text::styled(message, Style::default())),
                ];
                Row::new(cells).height(1)
            })
            .collect();

        let table_with_truncated_messages = Table::new(truncated_rows, constraints)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .column_spacing(1)
            .widths(constraints);

        // Render the table
        Widget::render(table_with_truncated_messages, logs_area, buf);

        // Render headers
        let first_row = Rect::new(logs_area.x + 1, logs_area.y, logs_area.width - 2, 1);
        let header_rects = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .spacing(1)
            .split(first_row);

        for (i, cell) in header_cells.iter().enumerate() {
            cell.render(header_rects[i], buf);
        }

        // Draw vertical separators
        let separator_style = Style::default().fg(Color::Gray);
        let first_separator_x_offset = match constraints[0] {
            Constraint::Length(val) => val + 1, // +1 for the left border
            Constraint::Min(val) => val + 1,
            _ => 1,
        };
        draw_vertical_separator(buf, logs_area, first_separator_x_offset, separator_style);

        let second_separator_x_offset = match (constraints[0], constraints[1]) {
            (Constraint::Length(val1), Constraint::Length(val2)) => val1 + val2 + 2, // +2 for left border and first separator
            _ => first_separator_x_offset + longest_service_length as u16 + 1, // Fallback, adjust as needed
        };
        draw_vertical_separator(buf, logs_area, second_separator_x_offset, separator_style);
    }
}

pub struct ScrollableLogsWidget {
    pub logs: String,
    pub header_view: TextBlock,
}

impl StatefulWidget for ScrollableLogsWidget {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate the actual height needed for the header
        let header_height = self.header_view.height(area.width);

        // Parse logs to calculate the actual height needed for the logs widget
        let logs_height = match serde_json::from_str::<LogsResponse>(&self.logs) {
            Ok(response) => {
                if response.data.is_empty() {
                    3 // Empty state message + borders
                } else {
                    response.data.len() + 2 // Data rows + header + borders
                }
            }
            Err(_) => 3, // Error message + borders
        };

        let total_content_height = header_height + logs_height;

        let content_size = Size::new(area.width, total_content_height as u16);
        let mut scroll_view = ScrollView::new(content_size)
            .scrollbars_visibility(tui_scrollview::ScrollbarVisibility::Never);

        // Create and render the combined widget
        let combined_widget = LogsWithHeaderWidget {
            header_view: self.header_view,
            logs: self.logs,
            header_height,
        };

        scroll_view.render_widget(
            combined_widget,
            Rect::new(0, 0, area.width, total_content_height as u16),
        );

        // Render the scroll view with the provided state
        scroll_view.render(area, buf, state);
    }
}

fn draw_horizontal_separator(buf: &mut Buffer, area: Rect, y_offset: u16, style: Style) {
    // Draw left edge
    buf.set_span(area.x, area.y + y_offset, &Span::styled("├", style), 1);

    // Draw horizontal lines (from position x+1 to x+width-2)
    for i in 1..(area.width - 1) {
        buf.set_span(area.x + i, area.y + y_offset, &Span::styled("─", style), 1);
    }

    // Draw right edge
    buf.set_span(
        area.x + area.width - 1,
        area.y + y_offset,
        &Span::styled("┤", style),
        1,
    );
}

fn draw_vertical_separator(buf: &mut Buffer, area: Rect, x_offset: u16, style: Style) {
    if area.height == 0 {
        return;
    }

    let x = area.x + x_offset;

    // Draw top character
    buf.set_span(x, area.y, &Span::styled("┬", style), 1);

    // Draw bottom character if the line is taller than 1 cell
    if area.height > 1 {
        buf.set_span(x, area.y + area.height - 1, &Span::styled("┴", style), 1);
    }

    // Draw middle characters (vertical bars) if the line is taller than 2 cells
    if area.height > 2 {
        for i in 1..(area.height - 1) {
            buf.set_span(x, area.y + i, &Span::styled("│", style), 1);
        }
    }
}

fn get_status_style(status: &str) -> Style {
    match status.to_lowercase().as_str() {
        "success" | "ok" | "completed" | "passed" | "active" | "running" => {
            Style::default().fg(Color::Green)
        }
        "error" | "err" | "fail" | "failed" | "critical" => Style::default().fg(Color::Red),
        "warn" | "warning" => Style::default().fg(Color::Yellow),
        "info" | "information" | "notice" | "debug" => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::DarkGray), // Default for unknown or less critical statuses
    }
}

#[derive(Deserialize, Debug)]
struct Incident {
    commander: Option<String>,
    created_at: String,
    #[allow(dead_code)] // May be used later
    created_by: String,
    id: u32,
    resolved_at: String,
    severity: String,
    title: String,
}

#[derive(Deserialize, Debug)]
struct IncidentsResponse {
    incidents: Vec<Incident>,
}

pub struct IncidentWidget {
    pub incidents: String,
}

impl IncidentWidget {
    pub fn new(incidents: String) -> Self {
        Self { incidents }
    }
}

impl Widget for IncidentWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Debug log the raw incidents string
        tracing::debug!("IncidentWidget raw incidents: {:?}", self.incidents);

        // Try to deserialize the incidents string
        let response: IncidentsResponse = match serde_json::from_str(&self.incidents) {
            Ok(response) => response,
            Err(e) => {
                // If deserialization fails, render an error message
                let error_msg = format!("Failed to parse incidents: {}", e);
                tracing::error!("{}, raw data: {:?}", error_msg, self.incidents);
                buf.set_string(area.x, area.y, &error_msg, Style::default().fg(Color::Red));
                return;
            }
        };

        // If there's no data, render an empty state
        if response.incidents.is_empty() {
            buf.set_string(
                area.x,
                area.y,
                "No incidents available",
                Style::default().fg(Color::DarkGray),
            );
            return;
        }

        // Assume only one incident as specified by user
        let incident = &response.incidents[0];

        let state = if incident.resolved_at.is_empty() {
            "Open".to_string()
        } else {
            "Resolved".to_string()
        };

        // TODO: generate this on the fly
        let summary = "The incident is attributed to a performance regression and was detected based on customer reports, with customer communications actively maintained by the response team. Key impact is ongoing for webstore users experiencing degraded search performance; the root cause has not yet been documented.";

        let state_color = match state.to_lowercase().as_str() {
            "open" | "active" => Color::Red,
            "resolved" => Color::Green,
            _ => Color::Yellow, // Default for "Stable" and any other states
        };

        let severity_color = match incident.severity.to_lowercase().as_str() {
            "sev-1" | "sev-2" => Color::Red,
            _ => Color::Yellow,
        };

        // Calculate width available for the main layout (metadata, separator, summary)
        // area.width minus the block borders and padding
        let metadata_content_total_width = area.width.saturating_sub(4);

        // Create a list of metadata items: (Key String, Value String, Style for Value)
        let mut metadata_items = vec![
            (
                "Severity".to_string(),
                incident.severity.clone(),
                Style::default().fg(severity_color),
            ),
            (
                "State".to_string(),
                state.clone(),
                Style::default().fg(state_color),
            ),
            (
                "Created At".to_string(),
                incident.created_at.clone(),
                Style::default().fg(Color::Gray),
            ),
        ];

        if let Some(cmdr) = &incident.commander {
            metadata_items.push((
                "Commander".to_string(),
                cmdr.clone(),
                Style::default().fg(Color::Gray),
            ));
        }

        let num_metadata_columns = metadata_items.len();
        let metadata_column_width = if num_metadata_columns > 0 {
            (metadata_content_total_width / num_metadata_columns as u16).max(1)
        } else {
            metadata_content_total_width
        };

        // Calculate metadata_height based on the new layout (Key above Value)
        let metadata_height = if num_metadata_columns > 0 {
            let mut max_h = 0;
            for (key, value, style) in &metadata_items {
                let key_line = Line::from(Span::raw(key.clone()));
                let value_line = Line::from(Span::styled(value.clone(), *style));
                let item_paragraph_for_height_calc =
                    Paragraph::new(vec![key_line, value_line]).wrap(Wrap { trim: true });
                max_h = max_h.max(item_paragraph_for_height_calc.line_count(metadata_column_width));
            }
            max_h.max(1)
        } else {
            0
        };

        let summary_lines = vec![
            Line::styled(
                "Summary: ",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::styled(summary, Style::default().fg(Color::White)),
            Line::from(vec![
                Span::styled(
                    "Open in Datadog",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(" | ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("Open #incident-{}", incident.id),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ])
            .alignment(Alignment::Right),
        ];

        let summary_paragraph = Paragraph::new(summary_lines).wrap(Wrap { trim: true });
        let summary_height = summary_paragraph.line_count(metadata_content_total_width);

        let formatted_incident_title = format!("Incident: {}", incident.title);
        // Create the pill-styled title
        let title_pill_line = Line::from(vec![
            Span::styled("─", Style::default().fg(severity_color)),
            Span::styled(
                formatted_incident_title,
                Style::default().fg(Color::Black).bg(severity_color),
            ),
            Span::styled("", Style::default().fg(severity_color)),
        ]);

        let panel_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(severity_color))
            .title(title_pill_line)
            .padding(Padding::horizontal(1));
        panel_block.render(area, buf);

        // Main content area within the panel (after block's own padding and additional margin)
        let main_content_layout_area = area.inner(Margin::new(2, 1));

        let main_layout_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(metadata_height as u16),
                Constraint::Length(1), // Separator
                Constraint::Length(summary_height as u16),
            ])
            .split(main_content_layout_area);

        // Split the metadata area into N columns
        let metadata_rect = main_layout_chunks[0];
        if num_metadata_columns > 0 {
            let constraints = (0..num_metadata_columns)
                .map(|_| Constraint::Ratio(1, num_metadata_columns as u32))
                .collect::<Vec<_>>();
            let metadata_columns_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(metadata_rect);

            // Render the metadata items
            for (i, (key, value, style)) in metadata_items.iter().enumerate() {
                let key_line = Line::from(Span::raw(key.clone()));
                let value_line = Line::from(Span::styled(value.clone(), *style));
                let item_paragraph =
                    Paragraph::new(vec![key_line, value_line]).wrap(Wrap { trim: true });
                item_paragraph.render(metadata_columns_chunks[i], buf);
            }
        }

        // Paint a separator line
        draw_horizontal_separator(
            buf,
            area,
            main_layout_chunks[1].y - area.y,
            Style::default().fg(severity_color),
        );

        // Render the summary
        summary_paragraph.render(main_layout_chunks[2], buf);
    }
}

// Combined widget that renders both header and incident
pub struct IncidentWithHeaderWidget {
    header_view: TextBlock,
    incidents: String,
    header_height: usize,
}

impl Widget for IncidentWithHeaderWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render header at the top
        self.header_view.render_window(
            0,
            Rect::new(area.x, area.y, area.width, self.header_height as u16),
            buf,
        );

        // Render incident below the header
        let incident_area = Rect::new(
            area.x + 1,
            area.y + self.header_height as u16,
            area.width - 2,
            area.height.saturating_sub(self.header_height as u16),
        );

        if incident_area.height > 0 {
            IncidentWidget::new(self.incidents).render(incident_area, buf);
        }
    }
}

pub struct ScrollableIncidentWidget {
    pub incidents: String,
    pub header_view: TextBlock,
}

impl StatefulWidget for ScrollableIncidentWidget {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate the actual height needed for the header
        let header_height = self.header_view.height(area.width);

        // Fixed height for incident widget (same as in history_cell.rs)
        let incident_height = 10;

        let total_content_height = header_height + incident_height;

        let content_size = Size::new(area.width, total_content_height as u16);
        let mut scroll_view = ScrollView::new(content_size)
            .scrollbars_visibility(tui_scrollview::ScrollbarVisibility::Never);

        // Create and render the combined widget
        let combined_widget = IncidentWithHeaderWidget {
            header_view: self.header_view,
            incidents: self.incidents,
            header_height,
        };

        scroll_view.render_widget(
            combined_widget,
            Rect::new(0, 0, area.width, total_content_height as u16),
        );

        // Render the scroll view with the provided state
        scroll_view.render(area, buf, state);
    }
}

// Combined widget that renders both header and metrics chart
pub struct MetricsWithHeaderWidget {
    header_view: TextBlock,
    metrics: String,
    header_height: usize,
}

impl Widget for MetricsWithHeaderWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render header at the top
        self.header_view.render_window(
            0,
            Rect::new(area.x, area.y, area.width, self.header_height as u16),
            buf,
        );

        // Render chart below the header
        let chart_area = Rect::new(
            area.x + 1,
            area.y + self.header_height as u16,
            area.width - 2,
            area.height.saturating_sub(self.header_height as u16),
        );

        if chart_area.height > 0 {
            ChartWidget::new(
                self.metrics,
                "Metrics".to_string(),
                "Time".to_string(),
                "Value".to_string(),
            )
            .render(chart_area, buf);
        }
    }
}

pub struct ScrollableMetricsWidget {
    pub metrics: String,
    pub header_view: TextBlock,
}

impl StatefulWidget for ScrollableMetricsWidget {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate the actual height needed for the header
        let header_height = self.header_view.height(area.width);

        // Fixed height for chart widget (same as in history_cell.rs)
        let chart_height = 15;

        let total_content_height = header_height + chart_height;

        let content_size = Size::new(area.width, total_content_height as u16);
        let mut scroll_view = ScrollView::new(content_size)
            .scrollbars_visibility(tui_scrollview::ScrollbarVisibility::Never);

        // Create and render the combined widget
        let combined_widget = MetricsWithHeaderWidget {
            header_view: self.header_view,
            metrics: self.metrics,
            header_height,
        };

        scroll_view.render_widget(
            combined_widget,
            Rect::new(0, 0, area.width, total_content_height as u16),
        );

        // Render the scroll view with the provided state
        scroll_view.render(area, buf, state);
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChartDataSet {
    name: String,
    data: Vec<(f64, f64)>,
}

// Structs for deserializing Datadog metrics JSON
#[derive(Deserialize, Debug)]
struct MetricUnit {
    #[allow(dead_code)]
    family: Option<String>,
    #[allow(dead_code)]
    id: Option<u32>,
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    plural: Option<String>,
    #[allow(dead_code)]
    scale_factor: Option<f64>,
    #[allow(dead_code)]
    short_name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct MetricSeries {
    #[allow(dead_code)]
    aggr: Option<String>,
    #[allow(dead_code)]
    attributes: serde_json::Value,
    display_name: String,
    #[allow(dead_code)]
    end: i64,
    #[allow(dead_code)]
    expression: String,
    #[allow(dead_code)]
    interval: u32,
    #[allow(dead_code)]
    length: u32,
    #[allow(dead_code)]
    metric: String,
    pointlist: Vec<[f64; 2]>,
    #[allow(dead_code)]
    query_index: u32,
    #[allow(dead_code)]
    scope: String,
    #[allow(dead_code)]
    start: i64,
    #[allow(dead_code)]
    tag_set: Vec<String>,
    #[allow(dead_code)]
    unit: Vec<MetricUnit>,
}

#[derive(Deserialize, Debug)]
struct MetricsResponse {
    series: Vec<MetricSeries>,
    #[allow(dead_code)]
    series_count: u32,
    #[allow(dead_code)]
    truncation_info: serde_json::Value,
}

pub struct ChartWidget {
    pub metrics: String,
    pub title: String,
    pub x_axis_title: String,
    pub y_axis_title: String,
}

impl ChartWidget {
    pub fn new(metrics: String, title: String, x_axis_title: String, y_axis_title: String) -> Self {
        Self {
            metrics,
            title,
            x_axis_title,
            y_axis_title,
        }
    }
}

impl Widget for ChartWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Debug log the raw metrics string
        tracing::debug!("ChartWidget raw metrics: {:?}", self.metrics);

        // Try to deserialize the metrics string
        let response: MetricsResponse = match serde_json::from_str(&self.metrics) {
            Ok(response) => response,
            Err(e) => {
                // If deserialization fails, render an error message
                let error_msg = format!("Failed to parse metrics: {}", e);
                tracing::error!("{}, raw data: {:?}", error_msg, self.metrics);
                buf.set_string(area.x, area.y, &error_msg, Style::default().fg(Color::Red));
                return;
            }
        };

        // If there's no data, render an empty state
        if response.series.is_empty() {
            buf.set_string(
                area.x,
                area.y,
                "No metrics available",
                Style::default().fg(Color::DarkGray),
            );
            return;
        }

        // Convert MetricSeries to ChartDataSet
        let datasets: Vec<ChartDataSet> = response
            .series
            .iter()
            .map(|series| {
                let data: Vec<(f64, f64)> = series
                    .pointlist
                    .iter()
                    .map(|point| (point[0], point[1]))
                    .collect();
                ChartDataSet {
                    name: series.display_name.clone(),
                    data,
                }
            })
            .collect();

        // dynamically calculate x and y bounds based on the data
        let mut x_min = f64::MAX;
        let mut x_max = f64::MIN;
        let mut y_min = f64::MAX;
        let mut y_max = f64::MIN;

        for dataset in datasets.iter() {
            for (x, y) in &dataset.data {
                if *x < x_min {
                    x_min = *x;
                }
                if *x > x_max {
                    x_max = *x;
                }
                if *y < y_min {
                    y_min = *y;
                }
                if *y > y_max {
                    y_max = *y;
                }
            }
        }

        // round to the nearest integer so they look nicer when displayed on axis labels, but only if the numbers are far apart
        if x_max - x_min > 5.0 {
            x_min = x_min.round();
            x_max = x_max.round();
        }
        if y_max - y_min > 5.0 {
            y_min = y_min.round();
            y_max = y_max.round();
        }

        // Helper function to format labels
        // Detects if a value is likely a Unix timestamp in milliseconds and formats it as ISO 8601 UTC.
        // Otherwise, formats as a number.
        fn format_label_value(value: f64) -> String {
            if value.fract() == 0.0 && value >= 0.0 {
                let millis = value as i64;

                // Heuristic: Check if the timestamp is within a reasonable range
                // (e.g., between year 2000 and 2050).
                // 2000-01-01T00:00:00Z = 946,684,800,000 ms
                // 2050-01-01T00:00:00Z = 2,524,608,000,000 ms
                const MIN_TIMESTAMP_MS: i64 = 946_684_800_000;
                const MAX_TIMESTAMP_MS: i64 = 2_524_608_000_000;

                if (MIN_TIMESTAMP_MS..=MAX_TIMESTAMP_MS).contains(&millis) {
                    // Value is a whole non-negative number within the plausible timestamp range,
                    // try to parse as a millisecond timestamp.
                    if let Some(dt) = DateTime::from_timestamp_millis(millis) {
                        return dt.format("%H:%M:%S").to_string(); // e.g., "07:30:00"
                    }
                }
            }
            // Default formatting if not a timestamp or parsing failed
            format!("{:.1}", value) // Format with one decimal place for non-timestamps
        }

        let colors = [
            Color::Cyan,
            Color::Red,
            Color::Green,
            Color::Blue,
            Color::Yellow,
        ];

        let chart_datasets = datasets
            .iter()
            .enumerate()
            .map(|(i, dataset)| {
                Dataset::default()
                    .name(dataset.name.clone())
                    .graph_type(ratatui::widgets::GraphType::Line)
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(colors[i % colors.len()]))
                    .data(&dataset.data)
            })
            .collect();

        let block = Block::default()
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray))
            .border_type(BorderType::Rounded)
            .title(self.title);

        let x_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            .labels([
                Span::styled(
                    format_label_value(x_min),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format_label_value(x_min + (x_max - x_min) / 2.0)),
                Span::styled(
                    format_label_value(x_max),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ])
            .bounds([x_min, x_max])
            .title(self.x_axis_title);

        let y_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            .labels([
                Span::styled(
                    format_label_value(y_min),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format_label_value(y_min + (y_max - y_min) / 2.0)),
                Span::styled(
                    format_label_value(y_max),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ])
            .bounds([y_min, y_max])
            .title(self.y_axis_title);

        let chart = Chart::new(chart_datasets)
            .block(block)
            .x_axis(x_axis)
            .y_axis(y_axis);

        chart.render(area, buf);
    }
}
