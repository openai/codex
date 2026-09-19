//! Shared title, description, and CTA content for banners in the TUI bottom pane.
//!
//! Workspace notices and backend banners share the existing selection list for CTA dispatch.
//! The inline wrapper owns presentation and dismissal while preserving normal composer input.

use super::BottomPane;
use super::BottomPaneView;
use super::SelectionItem;
use super::SelectionViewParams;
use super::list_selection_view::ListSelectionView;
use crate::render::Insets;
use crate::render::RectExt;
use crate::render::renderable::Renderable;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_fg;
use crate::wrapping::word_wrap_lines;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::cell::Cell;

#[derive(Default)]
pub(crate) struct ActionableBanner {
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) actions: Vec<SelectionItem>,
    pub(crate) initial_selected_idx: Option<usize>,
    pub(crate) dismissal: BannerDismissal,
    pub(crate) view_id: Option<&'static str>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BannerDismissal {
    #[default]
    Dismissible,
    Persistent,
}

pub(super) struct InlineBanner {
    content: InlineBannerContent,
    dismissal: BannerDismissal,
    shown: Cell<bool>,
    // Current visibility is separate from whether this banner has ever been shown.
    pub(super) visible: Cell<bool>,
    dismissed: bool,
    visible_action_count: Cell<usize>,
}

enum InlineBannerContent {
    Actions(Box<ListSelectionView>),
    Information {
        header: Box<dyn Renderable>,
        hint: Line<'static>,
    },
}

impl Renderable for InlineBanner {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.visible_action_count.set(0);
        self.visible.set(!self.dismissed && !area.is_empty());
        if !self.visible.get() {
            return;
        }
        self.shown.set(true);
        match &self.content {
            InlineBannerContent::Actions(view) => {
                view.render(area, buf);
                self.visible_action_count.set(view.rendered_item_count());
            }
            InlineBannerContent::Information { header, hint } => {
                let content = area.inset(Insets::tlbr(
                    /*top*/ 1, /*left*/ 2, /*bottom*/ 1, /*right*/ 2,
                ));
                header.render(content, buf);
                let y = content
                    .y
                    .saturating_add(header.desired_height(content.width) + 1);
                if y < area.bottom() {
                    Renderable::render(
                        hint,
                        Rect::new(content.x, y, content.width, /*height*/ 1),
                        buf,
                    );
                }
            }
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if self.dismissed {
            return 0;
        }
        match &self.content {
            InlineBannerContent::Actions(view) => view.desired_height(width),
            InlineBannerContent::Information { header, hint } => {
                header.desired_height(width.saturating_sub(4))
                    + if hint.width() == 0 { 2 } else { 4 }
            }
        }
    }
}

/// Multiline banner copy uses the same word wrapping for measurement and rendering.
struct BannerContent(Vec<Line<'static>>);

impl BannerContent {
    fn wrapped_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = word_wrap_lines(&self.0, width as usize);
        if lines.len() > 8 {
            lines.truncate(7);
            lines.push("…".dim().into());
        }
        lines
    }
}

impl Renderable for BannerContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.wrapped_lines(area.width);
        // Explicit foreground avoids a terminal's separate bold color washing out the title.
        let foreground = default_fg().map(best_color).unwrap_or(Color::Reset);
        Widget::render(Paragraph::new(lines).fg(foreground), area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.wrapped_lines(width).len() as u16
    }
}

impl From<ActionableBanner> for SelectionViewParams {
    fn from(banner: ActionableBanner) -> Self {
        let lines = banner
            .title
            .lines()
            .map(|line| Line::from(line.to_owned().bold()))
            .chain(
                banner
                    .description
                    .lines()
                    .map(|line| Line::from(line.to_owned())),
            )
            .collect();
        Self {
            view_id: banner.view_id,
            header: Box::new(BannerContent(lines)),
            items: banner.actions,
            initial_selected_idx: banner.initial_selected_idx,
            ..Self::picker()
        }
    }
}

impl BottomPane {
    pub(crate) fn show_actionable_banner(&mut self, banner: ActionableBanner) {
        self.show_selection_view(banner.into());
    }

    /// Display a banner above the composer without interrupting active dialogs or losing a draft.
    pub(crate) fn set_inline_banner(&mut self, banner: Option<ActionableBanner>) {
        self.inline_banner = banner.map(|banner| {
            let dismissal = banner.dismissal;
            let has_actions = !banner.actions.is_empty();
            let mut params: SelectionViewParams = banner.into();
            params.header_gap = 0;
            params.allow_cancel = dismissal == BannerDismissal::Dismissible;
            for item in &mut params.items {
                item.dismiss_on_select = false;
            }
            let hint = match (dismissal, has_actions) {
                (BannerDismissal::Persistent, true) => "Press a number to choose",
                (BannerDismissal::Persistent, false) => "",
                (BannerDismissal::Dismissible, true) => {
                    "Press a number to choose · esc to dismiss · type to continue"
                }
                (BannerDismissal::Dismissible, false) => "esc to dismiss · type to continue",
            };
            let hint: Line<'static> = hint.dim().into();
            params.footer_hint = Some(hint.clone());
            InlineBanner {
                content: if has_actions {
                    InlineBannerContent::Actions(Box::new(ListSelectionView::new(
                        params,
                        self.app_event_tx.clone(),
                        self.keymap.list.clone(),
                    )))
                } else {
                    InlineBannerContent::Information {
                        header: params.header,
                        hint,
                    }
                },
                dismissal,
                shown: Cell::new(false),
                visible: Cell::new(false),
                dismissed: false,
                visible_action_count: Cell::new(0),
            }
        });
        self.request_redraw();
    }

    /// Observation for the owning controller; model hiding is not a dismissal.
    pub(crate) fn inline_banner_lifecycle(&self) -> (bool, bool) {
        self.inline_banner
            .as_ref()
            .map_or((false, false), |banner| {
                (banner.shown.get(), banner.dismissed)
            })
    }

    pub(super) fn inline_banner_accepts_dismissal(&self) -> bool {
        self.inline_banner.as_ref().is_some_and(|banner| {
            banner.visible.get()
                && !banner.dismissed
                && banner.dismissal == BannerDismissal::Dismissible
        })
    }

    pub(super) fn handle_inline_banner_key(&mut self, key: KeyEvent) -> bool {
        // Draft input, completion menus, and paste bursts retain all of their normal keys.
        if !self.composer_is_empty()
            || self.composer.popup_active()
            || self.composer.is_in_paste_burst()
            || self.composer_should_handle_vim_insert_escape(key)
            || self.is_task_running
            || key.kind != KeyEventKind::Press
            || key.modifiers != KeyModifiers::NONE
        {
            return false;
        }
        let Some(banner) = self.inline_banner.as_mut() else {
            return false;
        };
        if !banner.visible.get() || banner.dismissed {
            return false;
        }
        match key.code {
            KeyCode::Esc if banner.dismissal == BannerDismissal::Persistent => return false,
            KeyCode::Esc => {
                banner.dismissed = true;
            }
            KeyCode::Char(digit @ '1'..='9') => {
                if digit as usize - '1' as usize >= banner.visible_action_count.get() {
                    return false;
                }
                let InlineBannerContent::Actions(view) = &mut banner.content else {
                    return false;
                };
                view.handle_key_event(key);
                let _ = view.take_last_selected_index();
            }
            _ => return false,
        }
        self.request_redraw();
        true
    }
}
