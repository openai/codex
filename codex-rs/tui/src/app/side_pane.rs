//! Layout and paint the two conversations. Only the displayed thread owns a live composer.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::side::SideCompanion;
use crate::bottom_pane::inset_footer_hint_area;
use crate::custom_terminal::Frame;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use codex_protocol::ThreadId;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::Widget;

pub(super) struct SidePaneLayout {
    pub(super) focused: Rect,
    pub(super) passive_thread: ThreadId,
    passive: Rect,
    divider: u16,
}

/// A wide terminal can show a pair once the outgoing conversation has been captured.
pub(super) fn layout(app: &App, tui: &crate::tui::Tui, area: Rect) -> Option<SidePaneLayout> {
    if area.width < 100
        || !app.side_retention_enabled(tui)
        || app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
    {
        return None;
    }
    let active = app.current_displayed_thread_id()?;
    let side = app.side_threads.get(&active);
    let passive_thread = match side {
        Some(child) => child.parent_thread_id,
        None => app.side_thread_for_parent(active)?,
    };
    if !app.side_companions.contains_key(&passive_thread) {
        return None;
    }
    let child_width = (area.width / 3).max(36);
    let [parent, child] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(child_width)])
            .spacing(2)
            .areas(area);
    let (focused, passive) = if side.is_some() {
        (child, parent)
    } else {
        (parent, child)
    };
    Some(SidePaneLayout {
        focused,
        passive,
        passive_thread,
        divider: parent.right(),
    })
}

impl SidePaneLayout {
    /// Paint the divider, retained transcript and draft without claiming the terminal cursor.
    pub(super) fn paint_companion(&self, companion: &mut SideCompanion, frame: &mut Frame<'_>) {
        let area = self.passive;
        for y in area.y..area.bottom() {
            frame.buffer[(self.divider, y)]
                .set_symbol("│")
                .set_style(ratatui::style::Style::default().dim());
        }
        // Keep the live composer's bottom hint row blank on the inactive side.
        let available = area.height.saturating_sub(1);
        let composer_height = available.min(3);
        let status_height =
            u16::from(companion.display.status.is_some() && available > composer_height);
        let above = available - composer_height - status_height;
        let transcript = Rect {
            height: above.saturating_sub(1),
            ..area
        };
        companion.view.render_parked(
            transcript,
            frame.buffer,
            &companion.cells,
            companion.display.active.as_deref(),
            &companion.display.auxiliary,
        );
        frame
            .image_placements
            .extend(companion.view.image_placements());

        let composer = Rect {
            y: area.y + above,
            height: composer_height,
            ..area
        };
        let gutter = u16::from(composer.x > self.divider);
        frame.buffer.set_style(
            Rect {
                x: composer.x - gutter,
                width: composer.width + gutter,
                ..composer
            },
            crate::style::quiet_workspace_style(),
        );
        let inset = u16::from(composer_height > 1);
        let draft = Text::from(format!("› {}", companion.display.draft));
        let lines = crate::wrapping::word_wrap_lines(
            draft.lines,
            crate::wrapping::RtOptions::new(usize::from(composer.width))
                .word_splitter(textwrap::WordSplitter::NoHyphenation),
        );
        Text::from(lines).render(
            Rect {
                y: composer.y + inset,
                height: composer_height - inset,
                ..composer
            },
            frame.buffer,
        );
        if let Some(status) = &companion.display.status {
            let footer = inset_footer_hint_area(Rect {
                y: composer.bottom(),
                height: status_height,
                ..area
            });
            truncate_line_with_ellipsis_if_overflow(
                status.clone().dim().into(),
                usize::from(footer.width),
            )
            .render(footer, frame.buffer);
        }
    }
}
