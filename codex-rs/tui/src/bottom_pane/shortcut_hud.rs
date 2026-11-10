//! Passive shortcut hints arranged in responsive columns for composer menus and floating key chord guides.

use super::selection_popup_common::menu_surface_inset;
use super::selection_popup_common::menu_surface_padding_height;
use super::selection_popup_common::render_menu_surface;
use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::Renderable;
use crate::style::accent_color;
use crate::style::secondary_text_style;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

const COLUMN_GAP: usize = 2;
const MIN_COLUMN_WIDTH: usize = 32;
const COMMAND_SEPARATOR: &str = "   ";
const SINGLE_COLUMN_ENTRY_LIMIT: usize = 5;

pub(crate) struct ShortcutHud {
    title: String,
    pub(crate) viewport: Option<Rect>,
    entries: HudEntries,
    commands: Vec<Line<'static>>,
}

enum HudEntries {
    Flat(Vec<Line<'static>>),
    Grouped(Vec<(&'static str, Vec<Line<'static>>)>),
}

impl ShortcutHud {
    pub(crate) fn new(
        title: impl Into<String>,
        entries: Vec<Line<'static>>,
        commands: Vec<Line<'static>>,
    ) -> Self {
        Self {
            title: title.into(),
            viewport: None,
            entries: HudEntries::Flat(entries),
            commands,
        }
    }

    pub(crate) fn leader(title: String, groups: Vec<(&'static str, Vec<Line<'static>>)>) -> Self {
        Self {
            title,
            viewport: None,
            entries: HudEntries::Grouped(groups),
            commands: vec!["Esc cancel".dim().into()],
        }
    }

    pub(crate) fn item(key: impl Into<String>, label: Span<'static>) -> Line<'static> {
        vec![
            key.into().fg(accent_color()),
            " · ".set_style(secondary_text_style()),
            label,
        ]
        .into()
    }

    fn body_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![Line::default()];
        lines.extend(match &self.entries {
            HudEntries::Flat(entries) => {
                let limit = if entries.len() > SINGLE_COLUMN_ENTRY_LIMIT {
                    2
                } else {
                    1
                };
                grid_lines(entries, width, limit)
            }
            HudEntries::Grouped(groups) => grouped_lines(groups, width),
        });
        lines
    }

    fn command_lines(&self, width: u16) -> Vec<Line<'static>> {
        crate::footer_hint::wrap_hint_rows(
            self.commands.iter().cloned(),
            width,
            COMMAND_SEPARATOR.len(),
            Line::width,
        )
        .into_iter()
        .map(|row| {
            let mut spans = Vec::new();
            for (index, command) in row.into_iter().enumerate() {
                if index > 0 {
                    spans.push(COMMAND_SEPARATOR.into());
                }
                spans.extend(command.spans);
            }
            spans.into()
        })
        .collect()
    }

    pub(crate) fn paint(&self, frame: &mut crate::custom_terminal::Frame) -> Rect {
        let viewport = self
            .viewport
            .map_or(frame.area(), |area| area.intersection(frame.area()));
        let area = self.render_overlay(viewport, frame.buffer);
        if frame
            .cursor_position
            .is_some_and(|cursor| area.contains(cursor))
        {
            frame.cursor_position = None;
        }
        frame
            .image_placements
            .retain(|image| !area.intersects(image.area));
        area
    }

    fn render_overlay(&self, viewport: Rect, buf: &mut Buffer) -> Rect {
        // Inline composers can allocate just a few rows. Put caller commands in the title
        // when they fit; otherwise reserve rows for them below the completions.
        if viewport.height < 7
            && let HudEntries::Flat(entries) = &self.entries
        {
            Clear.render(viewport, buf);
            buf.set_style(viewport, crate::style::shortcut_hud_style());
            let mut title = Line::from(self.title.as_str().bold());
            let mut commands = self.command_lines(viewport.width);
            if commands.len() == 1
                && title.width() + COMMAND_SEPARATOR.len() + commands[0].width()
                    <= usize::from(viewport.width)
            {
                title.spans.push(COMMAND_SEPARATOR.into());
                title.spans.extend(commands.remove(/*index*/ 0).spans);
            }
            title.render(viewport, buf);
            let body = viewport.inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
            ));
            let lines = grid_lines(entries, body.width, /*max_columns*/ 2);
            if commands.is_empty() {
                render_clipped(lines, body, buf);
            } else {
                Self::render_lines(lines, commands, body, buf);
            }
            return viewport;
        }
        let bounds = viewport.inset(Insets::tlbr(
            /*top*/ 0, /*left*/ 0, /*bottom*/ 1, /*right*/ 2,
        ));
        let width = viewport.width.div_ceil(2).max(32).min(bounds.width);
        let inner_width = width.saturating_sub(4);
        let body = self.body_lines(inner_width);
        let commands = self.command_lines(inner_width);
        // Reserve a row for the title and a gap above the commands.
        let height = u16::try_from(body.len() + 2 + commands.len())
            .unwrap_or(u16::MAX)
            .min(bounds.height);
        let area = Rect::new(
            bounds.right() - width,
            bounds.bottom() - height,
            width,
            height,
        );
        Clear.render(area, buf);
        buf.set_style(area, crate::style::shortcut_hud_style());
        Line::from(vec![
            "─ ".dim(),
            self.title.as_str().bold(),
            " ".into(),
            "─".repeat(usize::from(width)).dim(),
        ])
        .render(area, buf);
        Self::render_lines(
            body,
            commands,
            area.inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 2, /*bottom*/ 0, /*right*/ 2,
            )),
            buf,
        );
        area
    }

    fn render_lines(
        lines: Vec<Line<'static>>,
        commands: Vec<Line<'static>>,
        content: Rect,
        buf: &mut Buffer,
    ) {
        let command_height = u16::try_from(commands.len())
            .unwrap_or(u16::MAX)
            .min(content.height);
        let gap = content
            .height
            .saturating_sub(command_height)
            .min(/*other*/ 1);
        let body = Rect {
            height: content.height.saturating_sub(command_height + gap),
            ..content
        };
        let command_area = Rect {
            y: content.bottom().saturating_sub(command_height),
            height: command_height,
            ..content
        };
        render_clipped(lines, body, buf);
        Paragraph::new(commands).render(command_area, buf);
    }
}

impl Renderable for ShortcutHud {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let content = render_menu_surface(area, buf);
        let commands = self.command_lines(content.width);
        let mut lines = self.body_lines(content.width);
        lines.insert(0, self.title.clone().bold().into());
        Self::render_lines(lines, commands, content, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let content_width = menu_surface_inset(Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            /*height*/ u16::MAX,
        ))
        .width;
        let body_height = 1 + self.body_lines(content_width).len();
        let command_height = self.command_lines(content_width).len();
        u16::try_from(body_height + 1 + command_height)
            .unwrap_or(u16::MAX)
            .saturating_add(menu_surface_padding_height())
    }
}

fn grouped_lines(groups: &[(&'static str, Vec<Line<'static>>)], width: u16) -> Vec<Line<'static>> {
    let count = column_count(width, groups.len());
    let groups_per_column = groups.len().div_ceil(count).max(1);
    let mut columns = vec![Vec::new(); count];
    for (index, (title, entries)) in groups.iter().enumerate() {
        let column = &mut columns[index / groups_per_column];
        if !column.is_empty() {
            column.push(Line::default());
        }
        column.push((*title).dim().into());
        column.extend(entries.iter().cloned());
    }
    column_lines(&columns, width)
}

fn column_count(width: u16, limit: usize) -> usize {
    ((usize::from(width) + COLUMN_GAP) / (MIN_COLUMN_WIDTH + COLUMN_GAP)).clamp(1, limit.max(1))
}

fn grid_lines(items: &[Line<'static>], width: u16, max_columns: usize) -> Vec<Line<'static>> {
    let count = column_count(width, max_columns.min(items.len()));
    let rows = items.len().div_ceil(count).max(1);
    column_lines(&items.chunks(rows).collect::<Vec<_>>(), width)
}

fn column_lines(columns: &[impl AsRef<[Line<'static>]>], width: u16) -> Vec<Line<'static>> {
    let rows = columns
        .iter()
        .map(|column| column.as_ref().len())
        .max()
        .unwrap_or_default();
    let count = columns.len().max(1);
    let cell_width = usize::from(width).saturating_sub(COLUMN_GAP * (count - 1)) / count;
    (0..rows)
        .map(|row| {
            let mut line = Line::default();
            for (index, column) in columns.iter().enumerate() {
                let item = column.as_ref().get(row).cloned().unwrap_or_default();
                let item = truncate_line_with_ellipsis_if_overflow(item, cell_width);
                let padding = cell_width.saturating_sub(line_width(&item)) + COLUMN_GAP;
                line.extend(item.spans);
                if index + 1 < count {
                    line.push_span(" ".repeat(padding));
                }
            }
            line
        })
        .collect()
}

fn render_clipped(mut lines: Vec<Line<'static>>, area: Rect, buf: &mut Buffer) {
    if area.is_empty() {
        return;
    }
    let height = usize::from(area.height);
    if lines.len() > height {
        lines.truncate(height);
        lines[height - 1] = "… more shortcuts".dim().into();
    }
    Paragraph::new(lines).render(area, buf);
}
