use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;

const MCP_SEARCH_TOOL_NAME: &str = "MCPSearch";

pub(crate) struct McpSearchToggleView {
    enabled: bool,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    header: Box<dyn Renderable>,
    footer_hint: Line<'static>,
}

impl McpSearchToggleView {
    pub(crate) fn new(enabled: bool, app_event_tx: AppEventSender) -> Self {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("MCP tool discovery".bold()));
        header.push(Line::from(
            "Toggle MCPSearch to consolidate MCP tools into a single tool.".dim(),
        ));
        header.push(Line::from("Disable to restore direct MCP tools.".dim()));

        let mut view = Self {
            enabled,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            header: Box::new(header),
            footer_hint: toggle_hint_line(),
        };
        view.state.selected_idx = Some(0);
        view
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        let prefix = if self.state.selected_idx == Some(0) {
            '>'
        } else {
            ' '
        };
        let marker = if self.enabled { 'x' } else { ' ' };
        let name = format!("{prefix} [{marker}] {MCP_SEARCH_TOOL_NAME}");
        let description = Some("Hide direct MCP tools and use MCPSearch instead.".to_string());
        vec![GenericDisplayRow {
            name,
            description,
            selected_description: Some("Enter toggles".to_string()),
            ..Default::default()
        }]
    }

    fn toggle_selected(&mut self) {
        self.enabled = !self.enabled;
        self.app_event_tx.send(AppEvent::UpdateMcpSearchEnabled {
            enabled: self.enabled,
        });
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }
}

impl BottomPaneView for McpSearchToggleView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for McpSearchToggleView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let header_height = self
            .header
            .desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );
        let [header_area, _, list_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(rows_height),
        ])
        .areas(content_area.inset(Insets::vh(1, 2)));

        self.header.render(header_area, buf);

        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows(
                render_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                "  MCPSearch toggle unavailable",
            );
        }

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let rows = self.build_rows();
        let rows_width = Self::rows_width(width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );

        let mut height = self.header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 3);
        height.saturating_add(1)
    }
}

fn toggle_hint_line() -> Line<'static> {
    Line::from(vec![
        key_hint::plain(KeyCode::Enter).into(),
        " toggle · ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " close".into(),
    ])
}
