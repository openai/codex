//! Renders the skill selection popup for the bottom pane.
//!
//! This module owns the transient UI state needed to filter, rank, and render a
//! list of [`SkillMetadata`] entries as the user types a query. It is
//! responsible for fuzzy matching against display names, tracking scroll and
//! selection state, and building single-line rows that the shared selection
//! renderer can draw, including truncating long display names to keep the list
//! compact.
//!
//! It does not own the underlying skill registry, execute selections, or apply
//! actions. Callers provide the skill list and read the selected entry to drive
//! higher-level behavior. Sorting is deterministic: results are ordered by
//! fuzzy score and then display name so keyboard navigation remains stable
//! across re-renders. The popup height calculation reserves space for the
//! one-line footer hint when enough vertical space is available.
//!
//! Display names are truncated to a fixed width to keep the list compact, while
//! descriptions remain intact so users can still distinguish similar skills.
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows_single_line;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt;
use codex_common::fuzzy_match::fuzzy_match;
use codex_core::skills::model::SkillMetadata;

use crate::text_formatting::truncate_text;

/// Shows a filterable list of skills with a scrollable selection cursor.
///
/// The popup keeps the current query, derived selection state, and the skill
/// metadata needed to render rows. It does not mutate the skills themselves;
/// callers are expected to update the list and read back the selection when the
/// user confirms.
pub(crate) struct SkillPopup {
    /// Current user query used to filter and rank skills.
    query: String,
    /// Skill metadata entries provided by the caller for display and selection.
    skills: Vec<SkillMetadata>,
    /// Scroll and selection state for the filtered list.
    state: ScrollState,
}

impl SkillPopup {
    /// Creates a popup seeded with the available skills and an empty query.
    pub(crate) fn new(skills: Vec<SkillMetadata>) -> Self {
        Self {
            query: String::new(),
            skills,
            state: ScrollState::new(),
        }
    }

    /// Replaces the skill list and keeps the selection cursor in range.
    pub(crate) fn set_skills(&mut self, skills: Vec<SkillMetadata>) {
        self.skills = skills;
        self.clamp_selection();
    }

    /// Updates the query string and re-clamps the selection to filtered rows.
    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.clamp_selection();
    }

    /// Returns the height required to render the list plus footer padding.
    ///
    /// This is based on the number of filtered rows, clamped to the popup
    /// maximum, plus two rows reserved for the spacer and footer hint.
    pub(crate) fn calculate_required_height(&self, _width: u16) -> u16 {
        let rows = self.rows_from_matches(self.filtered());
        let visible = rows.len().clamp(1, MAX_POPUP_ROWS);
        (visible as u16).saturating_add(2)
    }

    /// Moves the selection up, wrapping to the bottom when needed.
    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Moves the selection down, wrapping to the top when needed.
    pub(crate) fn move_down(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Returns the skill metadata for the currently selected row, if any.
    pub(crate) fn selected_skill(&self) -> Option<&SkillMetadata> {
        let matches = self.filtered_items();
        let idx = self.state.selected_idx?;
        let skill_idx = matches.get(idx)?;
        self.skills.get(*skill_idx)
    }

    /// Clamps the selection index and scroll window to the filtered list.
    fn clamp_selection(&mut self) {
        let len = self.filtered_items().len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Collects the indices of skills that match the current query.
    fn filtered_items(&self) -> Vec<usize> {
        self.filtered().into_iter().map(|(idx, _, _)| idx).collect()
    }

    /// Converts matched skills into display rows for the shared renderer.
    ///
    /// Display names are truncated to keep the popup compact, while the
    /// description remains unmodified so the underlying metadata is still
    /// visible to the user.
    fn rows_from_matches(
        &self,
        matches: Vec<(usize, Option<Vec<usize>>, i32)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .map(|(idx, indices, _score)| {
                let skill = &self.skills[idx];
                let name = truncate_text(skill_display_name(skill), 21);
                let description = skill_description(skill).to_string();
                GenericDisplayRow {
                    name,
                    match_indices: indices,
                    display_shortcut: None,
                    description: Some(description),
                    disabled_reason: None,
                    wrap_indent: None,
                }
            })
            .collect()
    }

    /// Returns the fuzzy-matched skills with optional highlight indices.
    ///
    /// An empty query yields all skills without match indices. Otherwise the
    /// display name is matched first, with a fallback to the raw `name` when it
    /// differs, and results are sorted by score then display name to ensure
    /// stable ordering.
    fn filtered(&self) -> Vec<(usize, Option<Vec<usize>>, i32)> {
        let filter = self.query.trim();
        let mut out: Vec<(usize, Option<Vec<usize>>, i32)> = Vec::new();

        if filter.is_empty() {
            for (idx, _skill) in self.skills.iter().enumerate() {
                out.push((idx, None, 0));
            }
            return out;
        }

        for (idx, skill) in self.skills.iter().enumerate() {
            let display_name = skill_display_name(skill);
            if let Some((indices, score)) = fuzzy_match(display_name, filter) {
                out.push((idx, Some(indices), score));
            } else if display_name != skill.name
                && let Some((_indices, score)) = fuzzy_match(&skill.name, filter)
            {
                out.push((idx, None, score));
            }
        }

        out.sort_by(|a, b| {
            a.2.cmp(&b.2).then_with(|| {
                let an = skill_display_name(&self.skills[a.0]);
                let bn = skill_display_name(&self.skills[b.0]);
                an.cmp(bn)
            })
        });

        out
    }
}

impl WidgetRef for SkillPopup {
    /// Renders the list of skills and the footer hint when space allows.
    ///
    /// The list uses a single-line renderer with an inset so rows line up with
    /// other bottom-pane popups, and the footer hint is only shown when the
    /// area is tall enough to include a spacer row.
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
        render_rows_single_line(
            list_area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no skills",
        );
        if let Some(hint_area) = hint_area {
            let hint_area = Rect {
                x: hint_area.x + 2,
                y: hint_area.y,
                width: hint_area.width.saturating_sub(2),
                height: hint_area.height,
            };
            skill_popup_hint_line().render(hint_area, buf);
        }
    }
}

/// Builds the footer hint line describing the selection shortcuts.
fn skill_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to select or ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to close".into(),
    ])
}

/// Returns the display name shown in the popup for a skill.
fn skill_display_name(skill: &SkillMetadata) -> &str {
    skill
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .unwrap_or(&skill.name)
}

/// Returns the short description shown under the display name, if any.
fn skill_description(skill: &SkillMetadata) -> &str {
    skill
        .interface
        .as_ref()
        .and_then(|interface| interface.short_description.as_deref())
        .or(skill.short_description.as_deref())
        .unwrap_or(&skill.description)
}
