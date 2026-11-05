//! Lay out the two conversations and resize their divider. Only the displayed thread owns a live composer.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::side_companion::SideCompanion;
use crate::bottom_pane::inset_footer_hint_area;
use crate::custom_terminal::Frame;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Drag;
use crossterm::event::MouseEventKind::Up;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use std::time::Duration;

const GAP: u16 = 1;

#[derive(Clone, Copy)]
pub(super) struct PaneResize {
    focused: ThreadId,
    companion: ThreadId,
    preview_width: Option<u16>,
}

#[derive(Clone, Copy)]
pub(super) struct DividerDrag {
    passive_thread: ThreadId,
    screen: Rect,
    press: u16,
    initial_width: u16,
    width: u16,
    guide_only: bool,
}

impl DividerDrag {
    pub(super) fn matches(&self, screen: Rect, pane: Option<&SidePaneLayout>) -> bool {
        screen == self.screen && pane.is_some_and(|pane| pane.passive_thread == self.passive_thread)
    }

    fn percent(&self) -> Option<u8> {
        (self.width != self.initial_width).then(|| width_percent(self.width, self.screen.width))
    }
}

fn width_percent(width: u16, total: u16) -> u8 {
    let total = u32::from(total);
    ((u32::from(width) * 100 + total / 2) / total).clamp(1, 99) as u8
}

fn sidebar_width(width: i32, total: u16) -> u16 {
    width.clamp(36, i32::from(total.saturating_sub(64 + GAP).max(36))) as u16
}

fn preferred_width(total: u16, percent: Option<u8>) -> u16 {
    let width = percent.map_or(total / 3, |p| {
        ((u32::from(total) * u32::from(p) + 50) / 100) as u16
    });
    sidebar_width(i32::from(width), total)
}

#[derive(PartialEq)]
pub(super) struct SidePaneLayout {
    pub(super) focused: Rect,
    pub(super) passive_thread: ThreadId,
    pub(super) passive: Rect,
    divider: u16,
}

/// A wide terminal can show a pair once the outgoing conversation has been captured.
pub(super) fn layout(app: &App, tui: &crate::tui::Tui, area: Rect) -> Option<SidePaneLayout> {
    if app.zoomed_thread.is_some() && app.zoomed_thread == app.current_displayed_thread_id() {
        return None;
    }
    split_layout(app, tui, area)
}

fn split_layout(app: &App, tui: &Tui, area: Rect) -> Option<SidePaneLayout> {
    if area.width < 100
        || app.conversation_frame.has_caller_inset()
        || !app.side_retention_enabled(tui)
        || app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
    {
        return None;
    }
    let active = app.current_displayed_thread_id()?;
    let child = app.side_pair_key(active)?;
    let side = app.side_threads.get(&child)?;
    if !side.pane_open {
        return None;
    }
    let focused_child = active == child;
    let passive_thread = if focused_child {
        side.parent_thread_id
    } else {
        child
    };
    if !app.side_companions.contains_key(&passive_thread) {
        return None;
    }
    let preview = app
        .divider_drag
        .filter(|drag| {
            !drag.guide_only
                && drag.screen == area
                && drag.passive_thread == passive_thread
                && app.conversation_focus_available(tui)
        })
        .and_then(|drag| drag.percent());
    let saved = app.local_settings.tui.sidebar_width_percent;
    let keyboard_width = app
        .pane_resize
        .filter(|mode| mode.focused == active && mode.companion == passive_thread)
        .and_then(|mode| mode.preview_width);
    let child_width = match (keyboard_width, preview, app.exact_sidebar_width) {
        (Some(width), _, _) => sidebar_width(i32::from(width), area.width),
        (_, None, Some((total, width)))
            if total == area.width && saved == Some(width_percent(width, total)) =>
        {
            width
        }
        _ => preferred_width(area.width, preview.or(saved)),
    };
    let [parent, child] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(child_width)])
            .spacing(GAP)
            .areas(area);
    let (focused, passive) = if focused_child {
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

impl App {
    pub(super) fn begin_pane_resize(&mut self, tui: &mut Tui) -> Result<()> {
        let screen = tui.prepare_draw_size()?.into();
        if let Some(panes) = split_layout(self, tui, screen)
            && let Some(focused) = self.current_displayed_thread_id()
        {
            self.divider_drag = None;
            self.passive_link_drag = None;
            self.pending_right_click_paste = None;
            self.zoomed_thread = None;
            self.pane_resize = Some(PaneResize {
                focused,
                companion: panes.passive_thread,
                preview_width: None,
            });
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    pub(super) async fn handle_pane_resize(&mut self, tui: &Tui, event: &TuiEvent) -> bool {
        let Some(mut mode) = self.pane_resize else {
            return false;
        };
        let screen: Rect = match event {
            TuiEvent::Resize(size) => *size,
            _ => tui.terminal.last_known_screen_size,
        }
        .into();
        let Some(panes) = layout(self, tui, screen).filter(|panes| {
            !matches!(event, TuiEvent::FocusLost)
                && self.conversation_focus_available(tui)
                && self.current_displayed_thread_id() == Some(mode.focused)
                && panes.passive_thread == mode.companion
        }) else {
            self.pane_resize = None;
            tui.frame_requester().schedule_frame();
            return false;
        };
        let TuiEvent::Key(key) = event else {
            return matches!(event, TuiEvent::Mouse(_) | TuiEvent::Paste(_));
        };
        if key.kind == KeyEventKind::Release {
            return true;
        }
        let current = screen.right() - panes.divider - GAP;
        let step = match (key.code, key.modifiers) {
            (KeyCode::Left | KeyCode::Char('h'), KeyModifiers::NONE) => 1,
            (KeyCode::Right | KeyCode::Char('l'), KeyModifiers::NONE) => -1,
            (KeyCode::Left, KeyModifiers::SHIFT) => 10,
            (KeyCode::Right, KeyModifiers::SHIFT) => -10,
            (KeyCode::Enter | KeyCode::Esc, KeyModifiers::NONE)
                if key.kind == KeyEventKind::Press =>
            {
                self.pane_resize = None;
                if mode.preview_width.is_some()
                    && self
                        .save_sidebar_width(width_percent(current, screen.width))
                        .await
                {
                    self.exact_sidebar_width = Some((screen.width, current));
                }
                tui.frame_requester().schedule_frame();
                self.chat_widget.suppress_current_view_key_until_release(
                    crate::key_hint::KeyBinding::from_event(*key),
                    std::time::Instant::now(),
                    crate::activation_key_guard::ActivationKeySuppression::ViewTransition,
                );
                return true;
            }
            _ => return true,
        };
        let next = sidebar_width(i32::from(current) + step, screen.width);
        if next != current {
            mode.preview_width = Some(next);
            self.pane_resize = Some(mode);
            tui.frame_requester().schedule_frame();
        }
        true
    }

    pub(super) async fn set_focused_pane_width(
        &mut self,
        tui: &mut Tui,
        focused_percent: u8,
    ) -> Result<()> {
        let screen: Rect = tui.prepare_draw_size()?.into();
        if split_layout(self, tui, screen).is_none() {
            return Ok(());
        }
        let side_percent = if self.active_side_parent_thread_id().is_some() {
            focused_percent
        } else {
            100 - focused_percent
        };
        // The sidebar gets the spare column when an equal split is odd.
        let available = u32::from(screen.width - GAP);
        let preferred = (available * u32::from(side_percent)).div_ceil(100);
        let width = sidebar_width(preferred as i32, screen.width);
        let percent = width_percent(width, screen.width);
        if self.save_sidebar_width(percent).await {
            self.exact_sidebar_width = Some((screen.width, width));
            self.zoomed_thread = None;
            self.divider_drag = None;
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    pub(super) fn can_nudge_side_width(&self, tui: &Tui) -> bool {
        self.active_side_parent_thread_id().is_some()
            && layout(self, tui, tui.terminal.last_known_screen_size.into()).is_some()
    }

    pub(super) async fn nudge_side_width(&mut self, tui: &mut Tui, percent: i32) -> Result<()> {
        if self.active_side_parent_thread_id().is_some() {
            let screen: Rect = tui.prepare_draw_size()?.into();
            if let Some(panes) = layout(self, tui, screen) {
                let step = (i32::from(screen.width) * percent.abs() + 50) / 100;
                let current = panes.focused.width;
                let width =
                    sidebar_width(i32::from(current) + step * percent.signum(), screen.width);
                if width != current {
                    self.save_sidebar_width(width_percent(width, screen.width))
                        .await;
                    tui.frame_requester().schedule_frame();
                }
            }
        }
        Ok(())
    }

    async fn save_sidebar_width(&mut self, percent: u8) -> bool {
        if self.local_settings.tui.sidebar_width_percent != Some(percent)
            && let Err(err) =
                ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
                    .with_edits([ConfigEdit::SetPath {
                        segments: vec!["tui".into(), "sidebar_width_percent".into()],
                        value: i64::from(percent).into(),
                    }])
                    .apply()
                    .await
        {
            self.chat_widget
                .add_error_message(format!("Could not save sidebar width: {err}"));
            return false;
        }
        self.local_settings.tui.sidebar_width_percent = Some(percent);
        self.exact_sidebar_width = None;
        true
    }

    pub(super) async fn handle_divider_event(
        &mut self,
        tui: &mut Tui,
        event: &TuiEvent,
    ) -> Result<bool> {
        let mouse = match event {
            TuiEvent::Mouse(mouse) if self.conversation_focus_available(tui) => mouse,
            TuiEvent::Draw if self.conversation_focus_available(tui) => return Ok(false),
            _ => {
                if self.divider_drag.take().is_some() {
                    tui.frame_requester().schedule_frame();
                }
                return Ok(false);
            }
        };
        if matches!(mouse.kind, Down(_)) && self.divider_drag.take().is_some() {
            tui.frame_requester().schedule_frame();
        }
        match mouse.kind {
            Down(Left) => {
                let Some(painted) = &self.conversation_frame.side_pane else {
                    return Ok(false);
                };
                if !painted.divider_contains(*mouse) {
                    return Ok(false);
                }
                let screen = tui.prepare_draw_size()?.into();
                if layout(self, tui, screen).as_ref() != Some(painted) {
                    return Ok(false);
                }
                let width = screen.right() - painted.divider - GAP;
                self.divider_drag = Some(DividerDrag {
                    passive_thread: painted.passive_thread,
                    screen,
                    press: mouse.column,
                    initial_width: width,
                    width,
                    // Both panes share the divider. Reflowing either can move terminal-owned
                    // image pixels across it, so defer both layouts until release.
                    guide_only: !tui.retained_images.is_empty(),
                });
                self.cancel_pending_key_chord();
            }
            Drag(Left) | Up(Left) => {
                let Some(mut drag) = self.divider_drag.take() else {
                    return Ok(false);
                };
                let screen = tui.prepare_draw_size()?.into();
                if drag.matches(screen, layout(self, tui, screen).as_ref()) {
                    let width = i32::from(drag.initial_width) + i32::from(drag.press)
                        - i32::from(mouse.column);
                    drag.width = sidebar_width(width, screen.width);
                    if mouse.kind == Drag(Left) {
                        self.divider_drag = Some(drag);
                    } else if let Some(percent) = drag.percent() {
                        self.save_sidebar_width(percent).await;
                    }
                }
            }
            _ => return Ok(false),
        }
        tui.frame_requester().schedule_frame();
        Ok(true)
    }
}

impl SidePaneLayout {
    pub(super) fn divider_contains(&self, mouse: crossterm::event::MouseEvent) -> bool {
        (self.divider..self.divider + GAP).contains(&mouse.column)
            && (self.focused.y..self.focused.bottom()).contains(&mouse.row)
    }

    /// Paint the divider, retained transcript and draft without claiming the terminal cursor.
    pub(super) fn paint_companion(
        &self,
        companion: &mut SideCompanion,
        frame: &mut Frame<'_>,
        drag: Option<DividerDrag>,
        highlight_divider: bool,
        frames: &FrameRequester,
    ) {
        companion.flush_answer();
        let area = self.passive;
        // Keep the live composer's bottom hint row blank on the inactive side.
        let available = area.height.saturating_sub(1);
        let composer_height = available.min(3);
        let status_height =
            u16::from(companion.display.status.is_some() && available > composer_height);
        let above = available - composer_height - status_height;
        let mut transcript = Rect {
            height: above.saturating_sub(1),
            ..area
        };
        if let Some(work) = &companion.display.work {
            let row = work.row.with_timer(&work.timer);
            let height = row.desired_height(area.width).min(transcript.height);
            if height > 0 {
                transcript.height -= height;
                let status_area = Rect::new(area.x, transcript.bottom(), area.width, height);
                row.render(status_area, frame.buffer);
                frames.schedule_frame_in(Duration::from_secs(1));
            }
        }
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
        frame
            .buffer
            .set_style(composer, crate::style::quiet_workspace_style());
        let inset = u16::from(composer_height > 1);
        let text_area = Rect::new(
            composer.x.saturating_add(/*rhs*/ 2),
            composer.y + inset,
            composer.width.saturating_sub(/*rhs*/ 2),
            composer_height - inset,
        );
        if !text_area.is_empty() {
            frame.buffer[(composer.x, text_area.y)].set_symbol("›");
            StatefulWidgetRef::render_ref(
                &companion.display.draft.as_ref(),
                text_area,
                frame.buffer,
                &mut Default::default(),
            );
        }
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

        // Draw after both panes registered their images. Clearing a guide to the left
        // of retained pixels can erase the rest of that terminal row.
        let guide = drag
            .filter(|drag| drag.guide_only)
            .map_or(self.divider, |drag| {
                drag.screen.right()
                    - drag.percent().map_or(drag.initial_width, |p| {
                        preferred_width(drag.screen.width, Some(p))
                    })
                    - GAP
            });
        let style = if highlight_divider || drag.is_some() {
            crate::style::accent_style()
        } else {
            ratatui::style::Style::default().dim()
        };
        for y in area.y..area.bottom() {
            let x = if guide != self.divider
                && frame.image_placements.iter().any(|image| {
                    (image.area.y..image.area.bottom()).contains(&y) && guide < image.area.right()
                }) {
                self.divider
            } else {
                guide
            };
            frame.buffer[(x, y)].set_symbol("│").set_style(style);
        }
    }
}
