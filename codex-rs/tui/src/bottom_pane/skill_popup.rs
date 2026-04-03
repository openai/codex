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
use crate::text_formatting::truncate_text;
use codex_utils_fuzzy_match::fuzzy_match;

#[derive(Clone, Debug)]
pub(crate) struct MentionItem {
    pub(crate) display_name: String,
    pub(crate) description: Option<String>,
    pub(crate) insert_text: String,
    pub(crate) search_terms: Vec<String>,
    pub(crate) path: Option<String>,
    pub(crate) category_tag: Option<String>,
    pub(crate) sort_rank: u8,
}

const MENTION_NAME_TRUNCATE_LEN: usize = 24;

pub(crate) struct SkillPopup {
    query: String,
    mentions: Vec<MentionItem>,
    state: ScrollState,
}

impl SkillPopup {
    pub(crate) fn new(mentions: Vec<MentionItem>) -> Self {
        Self {
            query: String::new(),
            mentions,
            state: ScrollState::new(),
        }
    }

    pub(crate) fn set_mentions(&mut self, mentions: Vec<MentionItem>) {
        self.mentions = mentions;
        self.clamp_selection();
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.clamp_selection();
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

    pub(crate) fn selected_mention(&self) -> Option<&MentionItem> {
        let matches = self.filtered_items();
        let idx = self.state.selected_idx?;
        let mention_idx = matches.get(idx)?;
        self.mentions.get(*mention_idx)
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
                let mention = &self.mentions[idx];
                let name = truncate_text(&mention.display_name, MENTION_NAME_TRUNCATE_LEN);
                let description = match (
                    mention.category_tag.as_deref(),
                    mention.description.as_deref(),
                ) {
                    (Some(tag), Some(description)) if !description.is_empty() => {
                        Some(format!("{tag} {description}"))
                    }
                    (Some(tag), _) => Some(tag.to_string()),
                    (None, Some(description)) if !description.is_empty() => {
                        Some(description.to_string())
                    }
                    _ => None,
                };
                GenericDisplayRow {
                    name,
                    name_prefix_spans: Vec::new(),
                    match_indices: indices,
                    display_shortcut: None,
                    description,
                    category_tag: None,
                    is_disabled: false,
                    disabled_reason: None,
                    wrap_indent: None,
                }
            })
            .collect()
    }

    fn filtered(&self) -> Vec<(usize, Option<Vec<usize>>, i32)> {
        let filter = self.query.trim();
        let mut out: Vec<(usize, Option<Vec<usize>>, i32)> = Vec::new();

        for (idx, mention) in self.mentions.iter().enumerate() {
            if filter.is_empty() {
                out.push((idx, None, 0));
                continue;
            }

            let best_match =
                if let Some((indices, score)) = fuzzy_match(&mention.display_name, filter) {
                    Some((Some(indices), score))
                } else {
                    mention
                        .search_terms
                        .iter()
                        .filter(|term| *term != &mention.display_name)
                        .filter_map(|term| fuzzy_match(term, filter).map(|(_indices, score)| score))
                        .min()
                        .map(|score| (None, score))
                };

            if let Some((indices, score)) = best_match {
                out.push((idx, indices, score));
            }
        }

        out.sort_by(|a, b| {
            if filter.is_empty() {
                self.mentions[a.0]
                    .sort_rank
                    .cmp(&self.mentions[b.0].sort_rank)
            } else {
                a.2.cmp(&b.2).then_with(|| {
                    self.mentions[a.0]
                        .sort_rank
                        .cmp(&self.mentions[b.0].sort_rank)
                })
            }
            .then_with(|| {
                let an = self.mentions[a.0].display_name.as_str();
                let bn = self.mentions[b.0].display_name.as_str();
                an.cmp(bn)
            })
        });

        out
    }
}

impl WidgetRef for SkillPopup {
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
            list_area.inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no matches",
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

fn skill_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to insert or ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to close".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn mention_item(index: usize) -> MentionItem {
        MentionItem {
            display_name: format!("Mention {index:02}"),
            description: Some(format!("Description {index:02}")),
            insert_text: format!("$mention-{index:02}"),
            search_terms: vec![format!("mention-{index:02}")],
            path: Some(format!("skill://mention-{index:02}")),
            category_tag: Some("[Skill]".to_string()),
            sort_rank: 1,
        }
    }

    fn ranked_mention_item(
        display_name: &str,
        search_terms: &[&str],
        sort_rank: u8,
    ) -> MentionItem {
        MentionItem {
            display_name: display_name.to_string(),
            description: None,
            insert_text: format!("${display_name}"),
            search_terms: search_terms
                .iter()
                .map(|term| (*term).to_string())
                .collect(),
            path: None,
            category_tag: Some("[Skill]".to_string()),
            sort_rank,
        }
    }

    fn named_mention_item(display_name: &str, search_terms: &[&str]) -> MentionItem {
        ranked_mention_item(display_name, search_terms, 1)
    }

    #[test]
    fn filtered_mentions_preserve_results_beyond_popup_height() {
        let popup = SkillPopup::new((0..(MAX_POPUP_ROWS + 2)).map(mention_item).collect());

        let filtered_names: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|idx| popup.mentions[idx].display_name.clone())
            .collect();

        assert_eq!(
            filtered_names,
            (0..(MAX_POPUP_ROWS + 2))
                .map(|idx| format!("Mention {idx:02}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            popup.calculate_required_height(72),
            (MAX_POPUP_ROWS as u16) + 2
        );
    }

    fn render_popup(popup: &SkillPopup, width: u16) -> String {
        let area = Rect::new(0, 0, width, popup.calculate_required_height(width));
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);
        format!("{buf:?}")
    }

    #[test]
    fn scrolling_mentions_shifts_rendered_window_snapshot() {
        let mut popup = SkillPopup::new((0..(MAX_POPUP_ROWS + 2)).map(mention_item).collect());

        for _ in 0..=MAX_POPUP_ROWS {
            popup.move_down();
        }

        insta::assert_snapshot!("skill_popup_scrolled", render_popup(&popup, /*width*/ 72));
    }

    #[test]
    fn display_name_match_sorting_beats_worse_secondary_search_term_matches() {
        let mut popup = SkillPopup::new(vec![
            named_mention_item("pr-review-triage", &["pr-review-triage"]),
            named_mention_item("prd", &["prd"]),
            named_mention_item("PR Babysitter", &["babysit-pr", "PR Babysitter"]),
            named_mention_item("Plugin Creator", &["plugin-creator", "Plugin Creator"]),
            named_mention_item(
                "Logging Best Practices",
                &["logging-best-practices", "Logging Best Practices"],
            ),
        ]);
        popup.set_query("pr");

        let filtered_names: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|idx| popup.mentions[idx].display_name.clone())
            .collect();

        assert_eq!(
            filtered_names,
            vec![
                "PR Babysitter".to_string(),
                "pr-review-triage".to_string(),
                "prd".to_string(),
                "Plugin Creator".to_string(),
                "Logging Best Practices".to_string(),
            ]
        );
    }

    #[test]
    fn query_match_score_sorts_before_plugin_rank_bias() {
        let mut popup = SkillPopup::new(vec![
            ranked_mention_item("Copilot PR", &["copilot-pr", "Copilot PR"], 0),
            ranked_mention_item("PR Babysitter", &["babysit-pr", "PR Babysitter"], 1),
        ]);
        popup.set_query("pr");

        let filtered_names: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|idx| popup.mentions[idx].display_name.clone())
            .collect();

        assert_eq!(
            filtered_names,
            vec!["PR Babysitter".to_string(), "Copilot PR".to_string()]
        );
    }
}
