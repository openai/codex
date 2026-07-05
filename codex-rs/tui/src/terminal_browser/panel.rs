use std::sync::Arc;

use codex_terminal_browser::BrowserCell;
use codex_terminal_browser::BrowserColor;
use codex_terminal_browser::BrowserStatus;
use codex_terminal_browser::BrowserView;
use codex_terminal_browser::TerminalBrowser;
use codex_terminal_browser::TerminalSize;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

const HEADER_HEIGHT: u16 = 2;
const FOOTER_HEIGHT: u16 = 1;

/// Exact sub-rectangles used to render a terminal-browser panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BrowserPanelAreas {
    pub(crate) header: Rect,
    pub(crate) viewport: Rect,
    pub(crate) footer: Rect,
}

/// Splits an already-assigned panel rectangle into browser chrome and content.
///
/// This helper never centers, shrinks, or otherwise relocates `area`; the application frame owns
/// the outer panel geometry.
pub(crate) fn browser_panel_areas(area: Rect) -> BrowserPanelAreas {
    let header_height = area.height.min(HEADER_HEIGHT);
    let remaining_height = area.height.saturating_sub(header_height);
    let footer_height = remaining_height.min(FOOTER_HEIGHT);
    let viewport_height = remaining_height.saturating_sub(footer_height);
    let header = Rect::new(area.x, area.y, area.width, header_height);
    let viewport = Rect::new(
        area.x,
        area.y.saturating_add(header_height),
        area.width,
        viewport_height,
    );
    let footer = Rect::new(area.x, viewport.bottom(), area.width, footer_height);
    BrowserPanelAreas {
        header,
        viewport,
        footer,
    }
}

/// Returns the Carbonyl content viewport for an exact frame-assigned panel rectangle.
pub(crate) fn browser_viewport(area: Rect) -> Rect {
    browser_panel_areas(area).viewport
}

/// Renders a shared browser runtime inside a frame-owned panel.
pub(crate) struct TerminalBrowserPanel {
    browser: Arc<TerminalBrowser>,
}

impl TerminalBrowserPanel {
    pub(crate) fn new(browser: Arc<TerminalBrowser>) -> Self {
        Self { browser }
    }

    /// Resizes Carbonyl to the content viewport derived from `area`.
    pub(crate) fn resize(&self, area: Rect) -> anyhow::Result<()> {
        let viewport = browser_viewport(area);
        if viewport.is_empty() {
            return Ok(());
        }
        self.browser.resize(TerminalSize {
            rows: viewport.height,
            cols: viewport.width,
        })
    }

    /// Renders into exactly `area` and returns the browser content viewport used for input.
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) -> Rect {
        render_view(&self.browser.view(), area, buf)
    }
}

fn render_view(view: &BrowserView, area: Rect, buf: &mut Buffer) -> Rect {
    Clear.render(area, buf);
    let areas = browser_panel_areas(area);
    render_header(view, areas.header, buf);
    render_screen_or_status(view, areas.viewport, buf);
    render_footer(view, areas.footer, buf);
    areas.viewport
}

fn render_header(view: &BrowserView, area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }
    let title = view.title.as_deref().unwrap_or("Carbonyl");
    let status = status_label(&view.status);
    Line::from(vec![format!(" {status} ").cyan().bold(), title.into()]).render_ref(area, buf);
    if area.height > 1 {
        let url = view.url.as_deref().unwrap_or("about:blank");
        Line::from(vec![" ".into(), url.dim()]).render_ref(
            Rect::new(
                area.x,
                area.y.saturating_add(/*rhs*/ 1),
                area.width,
                /*height*/ 1,
            ),
            buf,
        );
    }
}

fn render_footer(view: &BrowserView, area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
        return;
    }
    if view.human_control {
        let line = if area.width >= 31 {
            Line::from(vec![" Ctrl+] ".cyan(), "return control to Codex".dim()])
        } else if area.width >= 15 {
            Line::from(vec![" Ctrl+] ".cyan(), "return".dim()])
        } else {
            Line::from(" Ctrl+] ".cyan())
        };
        line.render_ref(area, buf);
    } else {
        let line = if area.width >= 63 {
            Line::from(vec![
                " Esc ".cyan(),
                "hide".dim(),
                "   ".into(),
                "/browser control".cyan(),
                " take control".dim(),
                "   ".into(),
                "/browser close".cyan(),
                " stop".dim(),
            ])
        } else if area.width >= 41 {
            Line::from(vec![
                " Esc ".cyan(),
                "hide".dim(),
                "   ".into(),
                "/browser control".cyan(),
                " take control".dim(),
            ])
        } else {
            Line::from(vec![" Esc ".cyan(), "hide".dim()])
        };
        line.render_ref(area, buf);
    }
}

fn render_screen_or_status(view: &BrowserView, area: Rect, buf: &mut Buffer) {
    if area.is_empty() {
        return;
    }
    if !matches!(&view.status, BrowserStatus::Running)
        || view.screen.rows == 0
        || view.screen.cols == 0
        || view.screen.cells.is_empty()
    {
        let message = status_message(&view.status);
        let lines = textwrap::wrap(&message, usize::from(area.width).max(/*other*/ 1))
            .into_iter()
            .map(|line| Line::from(line.into_owned()).dim())
            .collect::<Vec<_>>();
        Paragraph::new(lines).render(area, buf);
        return;
    }

    for row in 0..view.screen.rows.min(area.height) {
        for col in 0..view.screen.cols.min(area.width) {
            let Some(cell) = view.screen.cell(row, col) else {
                continue;
            };
            let clipped_wide_glyph = col.saturating_add(/*rhs*/ 1) >= area.width
                && view
                    .screen
                    .cell(row, col.saturating_add(/*rhs*/ 1))
                    .is_some_and(|next| next.wide_continuation);
            let symbol = if clipped_wide_glyph || cell.wide_continuation || cell.text.is_empty() {
                " "
            } else {
                cell.text.as_str()
            };
            render_cell(cell, symbol, area.x + col, area.y + row, buf);
        }
    }

    if let Some((row, col)) = view.screen.cursor
        && row < area.height
        && col < area.width
    {
        let cell = &mut buf[(area.x + col, area.y + row)];
        let style = cell.style().reversed();
        cell.set_style(style);
    }
}

fn render_cell(cell: &BrowserCell, symbol: &str, x: u16, y: u16, buf: &mut Buffer) {
    let mut style = Style::default();
    if let Some(foreground) = color(cell.foreground) {
        style = style.fg(foreground);
    }
    if let Some(background) = color(cell.background) {
        style = style.bg(background);
    }
    if cell.bold {
        style = style.bold();
    }
    if cell.dim {
        style = style.dim();
    }
    if cell.italic {
        style = style.italic();
    }
    if cell.underlined {
        style = style.underlined();
    }
    if cell.reversed {
        style = style.reversed();
    }
    buf[(x, y)].set_symbol(symbol).set_style(style);
}

#[expect(
    clippy::disallowed_methods,
    reason = "Carbonyl output carries terminal-authored indexed and RGB colors that must render exactly"
)]
fn color(color: BrowserColor) -> Option<Color> {
    match color {
        BrowserColor::Default => None,
        BrowserColor::Indexed(index) => Some(Color::Indexed(index)),
        BrowserColor::Rgb(red, green, blue) => Some(Color::Rgb(red, green, blue)),
    }
}

fn status_label(status: &BrowserStatus) -> &'static str {
    match status {
        BrowserStatus::Unavailable { .. } => "unavailable",
        BrowserStatus::Idle => "idle",
        BrowserStatus::Starting => "starting",
        BrowserStatus::Running => "running",
        BrowserStatus::Crashed { .. } => "crashed",
    }
}

fn status_message(status: &BrowserStatus) -> String {
    match status {
        BrowserStatus::Unavailable { reason } => format!("Browser unavailable: {reason}"),
        BrowserStatus::Idle => "Open a page with terminal_browser.open.".to_string(),
        BrowserStatus::Starting => "Starting Carbonyl...".to_string(),
        BrowserStatus::Running => "Waiting for Carbonyl to render the page...".to_string(),
        BrowserStatus::Crashed { message } => format!("Carbonyl exited: {message}"),
    }
}

#[cfg(test)]
pub(super) fn style_for_test(cell: &BrowserCell) -> Style {
    let mut buf = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 1,
    ));
    let symbol = if cell.wide_continuation || cell.text.is_empty() {
        " "
    } else {
        cell.text.as_str()
    };
    render_cell(cell, symbol, /*x*/ 0, /*y*/ 0, &mut buf);
    buf[(0, 0)].style()
}

#[cfg(test)]
pub(super) fn render_view_for_test(view: &BrowserView, area: Rect, buf: &mut Buffer) -> Rect {
    render_view(view, area, buf)
}

#[cfg(test)]
pub(super) fn render_screen_for_test(view: &BrowserView, area: Rect, buf: &mut Buffer) {
    render_screen_or_status(view, area, buf);
}
