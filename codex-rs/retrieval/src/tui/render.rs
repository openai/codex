//! Rendering methods for the TUI application.
//!
//! This module extracts all render_* methods from `app.rs` to improve
//! maintainability and separation of concerns.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;

use crate::tui::app::App;
use crate::tui::app_event::ViewMode;
use crate::tui::views::DebugView;
use crate::tui::views::IndexView;
use crate::tui::views::RepoMapView;
use crate::tui::views::SearchView;
use crate::tui::views::WatchView;

/// Application renderer trait.
pub trait AppRenderer {
    /// Render the error banner.
    fn render_error_banner(&self, area: Rect, buf: &mut Buffer);

    /// Render the tab bar.
    fn render_tabs(&self, area: Rect, buf: &mut Buffer);

    /// Render the search view.
    fn render_search_view(&self, area: Rect, buf: &mut Buffer);

    /// Render the index view.
    fn render_index_view(&self, area: Rect, buf: &mut Buffer);

    /// Render the repomap view.
    fn render_repomap_view(&self, area: Rect, buf: &mut Buffer);

    /// Render the watch view.
    fn render_watch_view(&self, area: Rect, buf: &mut Buffer);

    /// Render the debug view.
    fn render_debug_view(&self, area: Rect, buf: &mut Buffer);

    /// Get context-sensitive keyboard hints for the current view.
    fn get_context_hints(&self) -> &'static str;

    /// Render the status bar.
    fn render_status_bar(&self, area: Rect, buf: &mut Buffer);

    /// Render the help overlay.
    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer);
}

impl AppRenderer for App {
    fn render_error_banner(&self, area: Rect, buf: &mut Buffer) {
        if let Some(ref error) = self.error_banner {
            let error_text = format!(" Error: {} ", error);
            let banner = Paragraph::new(error_text).style(Style::default().red().bold().reversed());
            banner.render(area, buf);
        }
    }

    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<Line> = ViewMode::all()
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                let num = format!("{}", i + 1);
                Line::from(vec![
                    Span::raw("["),
                    Span::raw(num).cyan(),
                    Span::raw("] "),
                    Span::raw(mode.name()),
                ])
            })
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Retrieval TUI "),
            )
            .select(self.view_mode.index())
            .highlight_style(ratatui::style::Style::default().bold().reversed());

        tabs.render(area, buf);
    }

    fn render_search_view(&self, area: Rect, buf: &mut Buffer) {
        let view = SearchView::new(&self.search, &self.event_log);
        view.render(area, buf);
    }

    fn render_index_view(&self, area: Rect, buf: &mut Buffer) {
        let view = IndexView::new(&self.index, &self.event_log);
        view.render(area, buf);
    }

    fn render_repomap_view(&self, area: Rect, buf: &mut Buffer) {
        let view = RepoMapView::new(&self.repomap);
        view.render(area, buf);
    }

    fn render_watch_view(&self, area: Rect, buf: &mut Buffer) {
        let view = WatchView::new(self.index.watching, &self.event_log);
        view.render(area, buf);
    }

    fn render_debug_view(&self, area: Rect, buf: &mut Buffer) {
        let view = DebugView::new(&self.event_log);
        view.render(area, buf);
    }

    fn get_context_hints(&self) -> &'static str {
        match self.view_mode {
            ViewMode::Search => {
                if self.search.focus_input {
                    "Enter: Search | Ctrl+←→: Mode | ↑↓: Results | Ctrl+P/N: History | ?: Help"
                } else {
                    "Enter: Open | ↑↓: Navigate | /: Input | PgUp/Dn: Page | ?: Help"
                }
            }
            ViewMode::Index => "b: Build | c: Clean | w: Watch | s: Stop | ?: Help",
            ViewMode::RepoMap => "g: Generate | r: Refresh | +/-: Budget | ↑↓: Scroll | ?: Help",
            ViewMode::Watch => "w: Toggle | c: Clear | ?: Help",
            ViewMode::Debug => "↑↓: Scroll | PgUp/Dn: Page | c: Clear | a: Auto-scroll | ?: Help",
        }
    }

    fn render_status_bar(&self, area: Rect, buf: &mut Buffer) {
        let elapsed = self.start_time.elapsed();

        // Show search elapsed time if searching
        let search_elapsed = self
            .search_start_time
            .map(|t| format!(" ({:.1}s)", t.elapsed().as_secs_f64()))
            .unwrap_or_default();

        // Show status message if present, otherwise show context-sensitive keybindings
        let status = if let Some(ref msg) = self.status_message {
            format!(
                " {}{} | Session: {:.0}s ",
                msg,
                search_elapsed,
                elapsed.as_secs_f64()
            )
        } else if self.build_running {
            format!(
                " Building index... | s: Stop | ?: Help | Session: {:.0}s ",
                elapsed.as_secs_f64()
            )
        } else if self.index.watching {
            format!(
                " Watching {} paths | {} | Session: {:.0}s ",
                self.watched_path_count,
                self.get_context_hints(),
                elapsed.as_secs_f64()
            )
        } else {
            format!(
                " {} | Session: {:.0}s ",
                self.get_context_hints(),
                elapsed.as_secs_f64()
            )
        };

        let style = if self.status_message.is_some() || self.build_running {
            ratatui::style::Style::default().yellow().reversed()
        } else if self.index.watching {
            ratatui::style::Style::default().green().reversed()
        } else {
            ratatui::style::Style::default().reversed()
        };

        let status_bar = Paragraph::new(status).style(style);
        status_bar.render(area, buf);
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer) {
        let help_text = r#"
 Keyboard Shortcuts
 ==================

 Global:
   q            Quit application
   Ctrl+C       Cancel search (or quit)
   ?            Toggle help
   Tab          Next view
   Shift+Tab    Previous view
   1-5          Switch to view

 Search View (Input):
   Type         Enter query
   Enter        Execute search
   Ctrl+P/N     Browse query history
   Up/Down      Switch to results
   Ctrl+←/→     Change search mode
   Home/End     Cursor start/end
   Esc          Clear query

 Search View (Results):
   Up/Down      Navigate results
   Enter        Open file in editor
   PgUp/PgDn    Page through results
   / or i       Focus input

 Result Indicators:
   [BM25]       Full-text match (blue)
   [Vector]     Semantic match (magenta)
   [Hybrid]     Combined score (green)
   ⚠            Stale (file modified)
   ✓            Fresh (file unchanged)

 Index View:
   b            Build index (incremental)
   c            Clean rebuild (full)
   w            Toggle watch mode
   s            Stop current build

 RepoMap View:
   g            Generate map
   r            Refresh
   +/-          Adjust token budget

 Watch/Debug View:
   w            Toggle watch mode
   c            Clear event log
   a            Toggle auto-scroll

 Press Escape or Enter to close
"#;

        // Center the help overlay
        let help_width = 42;
        let help_height = 46;
        let x = (area.width.saturating_sub(help_width)) / 2;
        let y = (area.height.saturating_sub(help_height)) / 2;
        let help_area = Rect::new(
            x,
            y,
            help_width.min(area.width),
            help_height.min(area.height),
        );

        // Clear background with reversed style (theme-aware)
        let bg_style = Style::default().reversed();
        for y in help_area.y..help_area.y + help_area.height {
            for x in help_area.x..help_area.x + help_area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(bg_style);
        help.render(help_area, buf);
    }
}
