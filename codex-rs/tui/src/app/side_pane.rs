//! Lay out paired conversations and resize the shared sidebar split.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::side_companion::SideCompanion;
use super::sidebar;
use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar::SidebarLayout;
use crate::bottom_pane::render_unfocused_status;
use crate::custom_terminal::Frame;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
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
use ratatui::widgets::StatefulWidgetRef;
use std::time::Duration;

pub(super) const GAP: u16 = 1;
pub(super) const RESIZE_HINT: &str = "Resize · ←/h →/l · Shift+←/→ 10 cols · Enter/Esc done";

#[derive(Clone, Copy, PartialEq, Eq)]
struct ResizeTarget {
    displayed: Option<ThreadId>,
    content: SidebarContent,
}

#[derive(Clone, Copy)]
pub(super) struct PaneResize {
    // Keyboard resize belongs to the parent split, including while a fork replaces the chooser.
    pub(super) parent: Option<ThreadId>,
    preview_width: Option<u16>,
}

#[derive(Clone, Copy)]
pub(super) struct DividerDrag {
    target: ResizeTarget,
    screen: Rect,
    press: u16,
    initial_width: u16,
    width: u16,
    guide_only: bool,
}

impl DividerDrag {
    pub(super) fn matches(&self, screen: Rect, app: &App) -> bool {
        screen == self.screen && app.sidebar_resize_target() == Some(self.target)
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
    pub(super) divider: u16,
    pub(super) tab_strip: Rect,
}

/// The settled chat split used to check pointer events against the last painted frame.
pub(super) fn layout(app: &App, tui: &Tui, area: Rect) -> Option<SidePaneLayout> {
    match visible_split(app, tui, area)? {
        SidebarLayout::Chat(panes) => Some(panes),
        SidebarLayout::Chooser { .. } => None,
    }
}

fn visible_split(app: &App, tui: &Tui, area: Rect) -> Option<SidebarLayout> {
    if app.zoomed_thread.is_some() && app.zoomed_thread == app.current_displayed_thread_id() {
        return None;
    }
    split_layout(app, tui, area)
}

pub(super) fn split_layout(app: &App, tui: &Tui, area: Rect) -> Option<SidebarLayout> {
    let content = app.sidebar()?.visible_content()?;
    if area.width < 100
        || app.conversation_frame.has_caller_inset()
        || !app.side_retention_enabled(tui)
    {
        return None;
    }
    sidebar::content_layout(
        app,
        content,
        app.chooser_focus().unwrap_or(PaneFocus::Main),
        sidebar_areas(app, tui, area, /*fraction*/ 1.0)?,
    )
}

pub(super) fn pair_layout(
    app: &App,
    child: ThreadId,
    [parent, sidebar]: [Rect; 2],
) -> Option<SidePaneLayout> {
    if parent.is_empty()
        || app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
    {
        return None;
    }
    let active = app.current_displayed_thread_id()?;
    let side = app.side_threads.get(&child)?;
    let [tab_strip, sidebar] = tab_and_body(sidebar);
    let (focused, passive, passive_thread) = if active == child {
        (sidebar, parent, side.parent_thread_id)
    } else {
        (parent, sidebar, child)
    };
    if !app.side_companions.contains_key(&passive_thread) {
        return None;
    }
    Some(SidePaneLayout {
        focused,
        passive,
        passive_thread,
        tab_strip,
        divider: parent.right(),
    })
}

pub(super) fn tab_and_body(area: Rect) -> [Rect; 2] {
    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area)
}

pub(super) fn sidebar_areas(app: &App, tui: &Tui, area: Rect, fraction: f64) -> Option<[Rect; 2]> {
    let target = app.sidebar_resize_target();
    let preview = app
        .divider_drag
        .filter(|drag| {
            !drag.guide_only && drag.matches(area, app) && app.conversation_focus_available(tui)
        })
        .and_then(|drag| drag.percent());
    let saved = app.local_settings.tui.sidebar_width_percent;
    let keyboard_width = app
        .pane_resize
        .filter(|mode| mode.parent == app.sidebar_parent())
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
    // A newly focused composer needs a cell even on the first opening frame.
    let open = target.is_some();
    let child_width = ((f64::from(child_width) * fraction).round() as u16).max(u16::from(open));
    (child_width > 0).then(|| {
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(child_width)])
            .spacing(GAP)
            .areas(area)
    })
}

impl App {
    fn sidebar_resize_target(&self) -> Option<ResizeTarget> {
        Some(ResizeTarget {
            displayed: self.current_displayed_thread_id(),
            content: self.sidebar()?.visible_content()?,
        })
    }

    pub(super) fn begin_pane_resize(&mut self, tui: &mut Tui) -> Result<()> {
        let screen = tui.prepare_draw_size()?.into();
        if split_layout(self, tui, screen).is_some() {
            self.divider_drag = None;
            self.passive_link_drag = None;
            self.pending_right_click_paste = None;
            self.zoomed_thread = None;
            self.pane_resize = Some(PaneResize {
                parent: self.sidebar_parent(),
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
        let divider = visible_split(self, tui, screen).and_then(|layout| layout.divider_area());
        let Some(divider) = divider.filter(|_| {
            !matches!(event, TuiEvent::FocusLost)
                && self.conversation_focus_available(tui)
                && self.sidebar_parent() == mode.parent
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
        let current = screen.right() - divider.right();
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
        let side_percent = if self.sidebar_has_focus() {
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

    pub(super) fn can_resize_sidebar(&self, tui: &Tui) -> bool {
        split_layout(self, tui, tui.terminal.last_known_screen_size.into()).is_some()
    }

    pub(super) fn divider_is_settled(&self, tui: &Tui, screen: Rect) -> bool {
        self.conversation_frame.sidebar.is_some()
            && visible_split(self, tui, screen) == self.conversation_frame.sidebar
    }

    pub(super) fn can_nudge_side_width(&self, tui: &Tui) -> bool {
        self.sidebar_has_focus()
            && visible_split(self, tui, tui.terminal.last_known_screen_size.into()).is_some()
    }

    pub(super) async fn nudge_side_width(&mut self, tui: &mut Tui, percent: i32) -> Result<()> {
        if self.sidebar_has_focus() {
            let screen: Rect = tui.prepare_draw_size()?.into();
            if let Some(divider) =
                visible_split(self, tui, screen).and_then(|layout| layout.divider_area())
            {
                let current = screen.right() - divider.right();
                let total = screen.width;
                let step = (i32::from(total) * percent.abs() + 50) / 100;
                let next = sidebar_width(i32::from(current) + step * percent.signum(), total);
                if next != current {
                    self.save_sidebar_width(width_percent(next, total)).await;
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
                let Some(divider) = self
                    .conversation_frame
                    .divider_area()
                    .filter(|area| area.contains((mouse.column, mouse.row).into()))
                else {
                    return Ok(false);
                };
                let screen = tui.prepare_draw_size()?.into();
                if !self.divider_is_settled(tui, screen) {
                    // The painted divider still owns this press while it moves.
                    return Ok(true);
                }
                let Some(target) = self.sidebar_resize_target() else {
                    return Ok(false);
                };
                let width = screen.right() - divider.right();
                self.divider_drag = Some(DividerDrag {
                    target,
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
                if drag.matches(screen, self) && visible_split(self, tui, screen).is_some() {
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
    /// Paint the divider, retained transcript and draft without claiming the terminal cursor.
    pub(super) fn paint_companion(
        &self,
        companion: &mut SideCompanion,
        frame: &mut Frame<'_>,
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
            render_unfocused_status(
                status.clone(),
                Rect {
                    y: composer.bottom(),
                    height: status_height,
                    ..area
                },
                frame.buffer,
            );
        }
    }
}

pub(super) fn paint_divider(
    frame: &mut Frame<'_>,
    divider: u16,
    drag: Option<DividerDrag>,
    highlight_divider: bool,
) {
    // Draw after both panes registered their images. Clearing a guide to the left
    // of retained pixels can erase the rest of that terminal row.
    let guide = drag.filter(|drag| drag.guide_only).map_or(divider, |drag| {
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
    for y in frame.area().y..frame.area().bottom() {
        let x = if guide != divider
            && frame.image_placements.iter().any(|image| {
                (image.area.y..image.area.bottom()).contains(&y) && guide < image.area.right()
            }) {
            divider
        } else {
            guide
        };
        frame.buffer[(x, y)].set_symbol("│").set_style(style);
    }
}
