use std::collections::HashMap;

use codex_common::fuzzy_match::fuzzy_match;
use codex_core::mcp::split_qualified_tool_name;
use mcp_types::Tool as McpTool;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt;
use crate::text_formatting::truncate_text;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows_single_line;

#[derive(Clone)]
pub(crate) struct McpToolItem {
    pub qualified_name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub required_fields: Vec<String>,
    pub property_keys: Vec<String>,
}

pub(crate) fn build_mcp_tool_items(tools: &HashMap<String, McpTool>) -> Vec<McpToolItem> {
    let mut items = Vec::new();
    for (qualified_name, tool) in tools {
        let Some((server, tool_name)) = split_qualified_tool_name(qualified_name) else {
            continue;
        };
        let display_name = format!("{server}::{tool_name}");
        let description = tool.title.clone().or_else(|| tool.description.clone());
        let mut required_fields = tool.input_schema.required.clone().unwrap_or_default();
        required_fields.sort();
        let mut property_keys = tool
            .input_schema
            .properties
            .as_ref()
            .and_then(|value| value.as_object())
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        property_keys.sort();
        items.push(McpToolItem {
            qualified_name: qualified_name.clone(),
            display_name,
            description,
            required_fields,
            property_keys,
        });
    }
    items.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    items
}

pub(crate) struct McpToolPopup {
    query: String,
    tools: Vec<McpToolItem>,
    state: ScrollState,
    waiting: bool,
}

impl McpToolPopup {
    pub(crate) fn new(tools: Vec<McpToolItem>) -> Self {
        let waiting = tools.is_empty();
        Self {
            query: String::new(),
            tools,
            state: ScrollState::new(),
            waiting,
        }
    }

    pub(crate) fn set_tools(&mut self, tools: Vec<McpToolItem>) {
        self.tools = tools;
        self.waiting = false;
        self.clamp_selection();
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.clamp_selection();
    }

    pub(crate) fn set_waiting(&mut self, waiting: bool) {
        self.waiting = waiting;
    }

    pub(crate) fn calculate_required_height(&self, _width: u16) -> u16 {
        let rows = self.rows_from_matches(self.filtered());
        let visible = rows.len().clamp(1, MAX_POPUP_ROWS);
        (visible as u16).saturating_add(2)
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn selected_tool(&self) -> Option<&McpToolItem> {
        let matches = self.filtered_items();
        let idx = self.state.selected_idx?;
        let tool_idx = matches.get(idx)?;
        self.tools.get(*tool_idx)
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_items().len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn filtered_items(&self) -> Vec<usize> {
        self.filtered().into_iter().map(|(idx, _, _)| idx).collect()
    }

    fn rows_from_matches(
        &self,
        matches: Vec<(usize, Option<Vec<usize>>, i32)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .map(|(idx, indices, _score)| {
                let item = &self.tools[idx];
                let name = truncate_text(&item.display_name, 28);
                let fields = format_fields_hint(item);
                let description = match (item.description.as_ref(), fields.as_ref()) {
                    (Some(desc), Some(fields)) => Some(format!("{desc} · {fields}")),
                    (Some(desc), None) => Some(desc.clone()),
                    (None, Some(fields)) => Some(fields.clone()),
                    (None, None) => None,
                };
                let selected_description = format_selected_description(item);
                GenericDisplayRow {
                    name,
                    match_indices: indices,
                    display_shortcut: None,
                    description,
                    selected_description,
                    wrap_indent: None,
                }
            })
            .collect()
    }

    fn filtered(&self) -> Vec<(usize, Option<Vec<usize>>, i32)> {
        let filter = self.query.trim();
        let mut out: Vec<(usize, Option<Vec<usize>>, i32)> = Vec::new();

        if filter.is_empty() {
            for (idx, _tool) in self.tools.iter().enumerate() {
                out.push((idx, None, 0));
            }
            return out;
        }

        for (idx, tool) in self.tools.iter().enumerate() {
            if let Some((indices, score)) = fuzzy_match(&tool.display_name, filter) {
                out.push((idx, Some(indices), score));
                continue;
            }
            if let Some((_indices, score)) = fuzzy_match(&tool.qualified_name, filter) {
                out.push((idx, None, score));
                continue;
            }
            if let Some(description) = tool.description.as_ref()
                && let Some((_indices, score)) = fuzzy_match(description, filter)
            {
                out.push((idx, None, score));
            }
        }

        out.sort_by(|a, b| {
            a.2.cmp(&b.2).then_with(|| {
                let an = &self.tools[a.0].display_name;
                let bn = &self.tools[b.0].display_name;
                an.cmp(bn)
            })
        });

        out
    }
}

fn format_fields_hint(item: &McpToolItem) -> Option<String> {
    let fields = if !item.required_fields.is_empty() {
        &item.required_fields
    } else if !item.property_keys.is_empty() {
        &item.property_keys
    } else {
        return None;
    };

    let mut listed = fields.iter().take(4).cloned().collect::<Vec<_>>();
    if fields.len() > listed.len() {
        listed.push("…".to_string());
    }
    Some(format!("fields: {}", listed.join(", ")))
}

fn format_selected_description(item: &McpToolItem) -> Option<String> {
    let mention = format!("@{}", item.qualified_name);
    if !item.required_fields.is_empty() {
        let fields = format_fields_hint(item).unwrap_or_else(|| "fields".to_string());
        return Some(format!("{fields} · enter inserts JSON"));
    }
    if let Some(description) = item.description.as_ref() {
        return Some(format!("{description} · enter inserts {mention}"));
    }
    if let Some(fields) = format_fields_hint(item) {
        return Some(format!("{fields} · enter inserts {mention}"));
    }
    Some(format!("enter inserts {mention}"))
}

impl WidgetRef for &McpToolPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let (list_area, hint_area) = if area.height > 2 {
            let [list_area, _spacer_area, hint_area] = Layout::vertical([
                Constraint::Length(area.height - 2),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .areas(area);
            (list_area, Some(hint_area))
        } else {
            (area, None)
        };

        let rows = self.rows_from_matches(self.filtered());
        let empty_message = if self.waiting {
            "loading..."
        } else {
            "no MCP tools"
        };
        render_rows_single_line(
            list_area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );

        if let Some(hint_area) = hint_area {
            let hint_area = Rect {
                x: hint_area.x + 2,
                y: hint_area.y,
                width: hint_area.width.saturating_sub(2),
                height: hint_area.height,
            };
            mcp_tool_popup_hint_line().render(hint_area, buf);
        }
    }
}

fn mcp_tool_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(crossterm::event::KeyCode::Enter).into(),
        " to select or ".into(),
        key_hint::plain(crossterm::event::KeyCode::Esc).into(),
        " to close".into(),
    ])
}
