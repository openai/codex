//! Composer-side Ctrl+R reverse history search state and rendering helpers.
//!
//! The persistent and local history stores live in `chat_composer_history`, but the composer owns
//! the active search session because it has to snapshot/restore the editable draft, preview matches
//! in the textarea, and render the footer prompt while the footer line is acting as the search
//! input.

use std::ops::Range;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use super::super::chat_composer_history::HistorySearchDirection;
use super::super::chat_composer_history::HistorySearchResult;
use super::super::footer::footer_height;
use super::super::footer::reset_mode_after_activity;
use super::ActivePopup;
use super::ChatComposer;
use super::ComposerDraft;
use super::InputResult;
use crate::app_event::AppEvent;
use crate::key_hint;
use crate::key_hint::has_ctrl_or_alt;
use crate::ui_consts::FOOTER_INDENT_COLS;

#[derive(Clone, Debug)]
pub(super) struct HistorySearchSession {
    /// Draft to restore when search is canceled or a query has no match.
    original_draft: ComposerDraft,
    /// Footer-owned query text typed while Ctrl+R search is active.
    query: String,
    /// User-visible search status used to choose footer hints and composer preview behavior.
    status: HistorySearchStatus,
}

/// User-visible phase of the active Ctrl+R search session.
///
/// Search keeps the footer query and the composer preview separate: `Idle` leaves the original
/// draft untouched, `Searching` waits for persistent history, `Match` previews a found entry, and
/// `NoMatch` restores the original draft while leaving the search input open for more typing.
#[derive(Clone, Debug)]
enum HistorySearchStatus {
    Idle,
    Searching,
    Match,
    NoMatch,
}

impl ChatComposer {
    #[cfg(test)]
    pub(super) fn history_search_active(&self) -> bool {
        self.history_search.is_some()
    }

    pub(super) fn is_history_search_key(key_event: &KeyEvent) -> bool {
        matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'r')
        ) || matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char('\u{0012}'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }
        )
    }

    fn is_history_search_forward_key(key_event: &KeyEvent) -> bool {
        matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'s')
        ) || matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char('\u{0013}'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }
        )
    }

    pub(super) fn begin_history_search(&mut self) -> (InputResult, bool) {
        if self.current_file_query.is_some() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(String::new()));
            self.current_file_query = None;
        }
        self.active_popup = ActivePopup::None;
        self.selected_remote_image_index = None;
        self.history_search = Some(HistorySearchSession {
            original_draft: self.snapshot_draft(),
            query: String::new(),
            status: HistorySearchStatus::Idle,
        });
        self.history.reset_search();
        (InputResult::None, true)
    }

    pub(super) fn handle_history_search_key(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        if key_event.kind == KeyEventKind::Release {
            return (InputResult::None, false);
        }

        if Self::is_history_search_key(&key_event) || matches!(key_event.code, KeyCode::Up) {
            let result = self.history_search_in_direction(HistorySearchDirection::Older);
            return (result, true);
        }

        if Self::is_history_search_forward_key(&key_event)
            || matches!(key_event.code, KeyCode::Down)
        {
            let result = self.history_search_in_direction(HistorySearchDirection::Newer);
            return (result, true);
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.cancel_history_search();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self
                    .history_search
                    .as_ref()
                    .is_some_and(|search| matches!(search.status, HistorySearchStatus::Match))
                {
                    self.history_search = None;
                    self.history.reset_search();
                    self.footer_mode = reset_mode_after_activity(self.footer_mode);
                    self.move_cursor_to_end();
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if let Some(search) = self.history_search.as_ref() {
                    let mut query = search.query.clone();
                    query.pop();
                    self.update_history_search_query(query);
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.update_history_search_query(String::new());
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Char(ch),
                modifiers,
                ..
            } if !has_ctrl_or_alt(modifiers) => {
                if let Some(search) = self.history_search.as_ref() {
                    let mut query = search.query.clone();
                    query.push(ch);
                    self.update_history_search_query(query);
                }
                (InputResult::None, true)
            }
            _ => (InputResult::None, true),
        }
    }

    fn history_search_in_direction(&mut self, direction: HistorySearchDirection) -> InputResult {
        let Some((query, original_draft)) = self
            .history_search
            .as_ref()
            .map(|search| (search.query.clone(), search.original_draft.clone()))
        else {
            return InputResult::None;
        };
        if query.is_empty() {
            self.history.reset_search();
            if let Some(search) = self.history_search.as_mut() {
                search.status = HistorySearchStatus::Idle;
            }
            self.restore_draft(original_draft);
            return InputResult::None;
        }
        let result = self.history.search(
            &query,
            direction,
            /*restart*/ false,
            &self.app_event_tx,
        );
        self.apply_history_search_result(result);
        InputResult::None
    }

    fn update_history_search_query(&mut self, query: String) {
        let Some(original_draft) = self
            .history_search
            .as_ref()
            .map(|search| search.original_draft.clone())
        else {
            return;
        };
        if let Some(search) = self.history_search.as_mut() {
            search.query = query.clone();
            search.status = HistorySearchStatus::Searching;
        }
        self.restore_draft(original_draft);
        if query.is_empty() {
            self.history.reset_search();
            if let Some(search) = self.history_search.as_mut() {
                search.status = HistorySearchStatus::Idle;
            }
            return;
        }
        let result = self.history.search(
            &query,
            HistorySearchDirection::Older,
            /*restart*/ true,
            &self.app_event_tx,
        );
        self.apply_history_search_result(result);
    }

    fn cancel_history_search(&mut self) {
        if let Some(search) = self.history_search.take() {
            self.history.reset_search();
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
            self.restore_draft(search.original_draft);
        }
    }

    pub(super) fn apply_history_search_result(&mut self, result: HistorySearchResult) {
        match result {
            HistorySearchResult::Found(entry) => {
                if let Some(search) = self.history_search.as_mut() {
                    search.status = HistorySearchStatus::Match;
                }
                self.apply_history_entry(entry);
            }
            HistorySearchResult::Pending => {
                if let Some(search) = self.history_search.as_mut() {
                    search.status = HistorySearchStatus::Searching;
                }
            }
            HistorySearchResult::AtBoundary => {
                if let Some(search) = self.history_search.as_mut() {
                    search.status = HistorySearchStatus::Match;
                }
            }
            HistorySearchResult::NotFound => {
                let original_draft = self
                    .history_search
                    .as_ref()
                    .map(|search| search.original_draft.clone());
                if let Some(search) = self.history_search.as_mut() {
                    search.status = HistorySearchStatus::NoMatch;
                }
                if let Some(original_draft) = original_draft {
                    self.restore_draft(original_draft);
                }
            }
        }
    }

    pub(super) fn history_search_footer_line(&self) -> Option<Line<'static>> {
        let search = self.history_search.as_ref()?;
        let mut line = Line::from(vec![
            "reverse-i-search: ".dim(),
            search.query.clone().cyan(),
        ]);
        match search.status {
            HistorySearchStatus::Idle => {}
            HistorySearchStatus::Searching => line.push_span("  searching".dim()),
            HistorySearchStatus::Match => {
                line.push_span("  ".dim());
                line.push_span(Self::history_search_action_key_span(KeyCode::Enter));
                line.push_span(" accept".dim());
                line.push_span(" · ".dim());
                line.push_span(Self::history_search_action_key_span(KeyCode::Esc));
                line.push_span(" cancel".dim());
            }
            HistorySearchStatus::NoMatch => line.push_span("  no match".red()),
        }
        Some(line)
    }

    fn history_search_action_key_span(key: KeyCode) -> Span<'static> {
        Span::from(key_hint::plain(key)).cyan().bold().not_dim()
    }

    pub(super) fn history_search_highlight_ranges(&self) -> Vec<Range<usize>> {
        let Some(search) = self.history_search.as_ref() else {
            return Vec::new();
        };
        if !matches!(search.status, HistorySearchStatus::Match) || search.query.is_empty() {
            return Vec::new();
        }
        Self::case_insensitive_match_ranges(self.textarea.text(), &search.query)
    }

    fn case_insensitive_match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query
            .chars()
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if query_lower.is_empty() {
            return Vec::new();
        }

        let mut folded = String::new();
        let mut folded_spans: Vec<(Range<usize>, Range<usize>)> = Vec::new();
        for (original_start, ch) in text.char_indices() {
            let original_range = original_start..original_start + ch.len_utf8();
            for lower in ch.to_lowercase() {
                let folded_start = folded.len();
                folded.push(lower);
                folded_spans.push((folded_start..folded.len(), original_range.clone()));
            }
        }

        let mut ranges = Vec::new();
        let mut search_from = 0;
        while search_from <= folded.len()
            && let Some(relative_start) = folded[search_from..].find(&query_lower)
        {
            let folded_start = search_from + relative_start;
            let folded_end = folded_start + query_lower.len();
            if let Some((_, first_original)) = folded_spans.iter().find(|(folded_range, _)| {
                folded_range.end > folded_start && folded_range.start < folded_end
            }) {
                let original_end = folded_spans
                    .iter()
                    .rev()
                    .find(|(folded_range, _)| {
                        folded_range.end > folded_start && folded_range.start < folded_end
                    })
                    .map(|(_, original_range)| original_range.end)
                    .unwrap_or(first_original.end);
                ranges.push(first_original.start..original_end);
            }
            search_from = folded_end;
        }
        ranges
    }

    pub(super) fn history_search_cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let search = self.history_search.as_ref()?;
        let [_, _, _, popup_rect] = self.layout_areas(area);
        if popup_rect.is_empty() {
            return None;
        }

        let footer_props = self.footer_props();
        let footer_hint_height = self
            .custom_footer_height()
            .unwrap_or_else(|| footer_height(&footer_props));
        let footer_spacing = Self::footer_spacing(footer_hint_height);
        let hint_rect = if footer_spacing > 0 && footer_hint_height > 0 {
            let [_, hint_rect] = Layout::vertical([
                Constraint::Length(footer_spacing),
                Constraint::Length(footer_hint_height),
            ])
            .areas(popup_rect);
            hint_rect
        } else {
            popup_rect
        };
        if hint_rect.is_empty() {
            return None;
        }

        let prompt_width = Line::from("reverse-i-search: ").width() as u16;
        let query_width = Line::from(search.query.clone()).width() as u16;
        let desired_x = hint_rect
            .x
            .saturating_add(FOOTER_INDENT_COLS as u16)
            .saturating_add(prompt_width)
            .saturating_add(query_width);
        let max_x = hint_rect
            .x
            .saturating_add(hint_rect.width.saturating_sub(1));
        Some((desired_x.min(max_x), hint_rect.y))
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use tokio::sync::mpsc::unbounded_channel;

    use super::super::super::chat_composer_history::HistoryEntry;
    use super::super::super::footer::FooterMode;
    use super::super::ChatComposer;
    use super::HistorySearchStatus;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::render::renderable::Renderable;

    #[test]
    fn history_search_opens_without_previewing_latest_entry() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("remembered command".to_string()));
        composer.set_text_content(String::new(), Vec::new(), Vec::new());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

        assert!(composer.history_search_active());
        assert!(composer.textarea.is_empty());
        assert_eq!(composer.footer_mode(), FooterMode::HistorySearch);
    }

    #[test]
    fn history_search_match_ranges_are_case_insensitive() {
        assert_eq!(
            ChatComposer::case_insensitive_match_ranges("git status git", "GIT"),
            vec![0..3, 11..14]
        );
        assert_eq!(
            ChatComposer::case_insensitive_match_ranges("aİ i", "i"),
            vec![1..3, 4..5]
        );
        assert!(ChatComposer::case_insensitive_match_ranges("git", "").is_empty());
    }

    #[test]
    fn history_search_accepts_matching_entry() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("git status".to_string()));
        composer
            .history
            .record_local_submission(HistoryEntry::new("cargo test".to_string()));
        composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(composer.history_search_active());
        assert_eq!(composer.textarea.text(), "draft");

        for ch in ['g', 'i', 't'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(composer.textarea.text(), "git status");
        assert_eq!(composer.footer_mode(), FooterMode::HistorySearch);

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!composer.history_search_active());
        assert_eq!(composer.textarea.text(), "git status");
        assert_eq!(composer.textarea.cursor(), composer.textarea.text().len());
    }

    #[test]
    fn history_search_stays_on_single_match_at_boundaries() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer.history.record_local_submission(HistoryEntry::new(
            "Find and fix a bug in @filename".to_string(),
        ));
        composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        for ch in ['b', 'u', 'g'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(composer.textarea.text(), "Find and fix a bug in @filename");

        for _ in 0..3 {
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        }
        assert_eq!(composer.textarea.text(), "Find and fix a bug in @filename");
        assert!(
            composer
                .history_search
                .as_ref()
                .is_some_and(|search| matches!(search.status, HistorySearchStatus::Match))
        );

        for _ in 0..3 {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(composer.textarea.text(), "Find and fix a bug in @filename");
        assert!(
            composer
                .history_search
                .as_ref()
                .is_some_and(|search| matches!(search.status, HistorySearchStatus::Match))
        );
    }

    #[test]
    fn history_search_footer_action_hints_are_emphasized() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ true,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("cargo test".to_string()));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        let line = composer
            .history_search_footer_line()
            .expect("expected history search footer line");
        assert_eq!(
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>(),
            vec![
                "reverse-i-search: ",
                "c",
                "  ",
                "enter",
                " accept",
                " · ",
                "esc",
                " cancel"
            ]
        );

        let query_style = line.spans[1].style;
        assert_eq!(query_style.fg, Some(ratatui::style::Color::Cyan));

        let enter_style = line.spans[3].style;
        assert_eq!(enter_style.fg, Some(ratatui::style::Color::Cyan));
        assert!(enter_style.add_modifier.contains(Modifier::BOLD));
        assert!(enter_style.sub_modifier.contains(Modifier::DIM));

        let accept_style = line.spans[4].style;
        assert!(accept_style.add_modifier.contains(Modifier::DIM));

        let separator_style = line.spans[5].style;
        assert!(separator_style.add_modifier.contains(Modifier::DIM));

        let esc_style = line.spans[6].style;
        assert_eq!(esc_style.fg, Some(ratatui::style::Color::Cyan));
        assert!(esc_style.add_modifier.contains(Modifier::BOLD));
        assert!(esc_style.sub_modifier.contains(Modifier::DIM));

        let cancel_style = line.spans[7].style;
        assert!(cancel_style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn history_search_highlights_matches_until_accepted() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ true,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("cargo test".to_string()));
        composer
            .history
            .record_local_submission(HistoryEntry::new("git status".to_string()));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        for ch in ['g', 'i', 't'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        let area = Rect::new(0, 0, 60, 8);
        let [_, _, textarea_rect, _] = composer.layout_areas(area);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        let x = textarea_rect.x;
        let y = textarea_rect.y;
        assert_eq!(buf[(x, y)].symbol(), "g");
        for offset in 0..3 {
            let modifier = buf[(x + offset, y)].style().add_modifier;
            assert!(modifier.contains(Modifier::REVERSED));
            assert!(modifier.contains(Modifier::BOLD));
        }
        assert!(
            !buf[(x + 3, y)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let [_, _, accepted_textarea_rect, _] = composer.layout_areas(area);
        let mut accepted_buf = Buffer::empty(area);
        composer.render(area, &mut accepted_buf);
        for offset in 0..3 {
            let modifier = accepted_buf
                [(accepted_textarea_rect.x + offset, accepted_textarea_rect.y)]
                .style()
                .add_modifier;
            assert!(!modifier.contains(Modifier::REVERSED));
            assert!(!modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn history_search_esc_restores_original_draft() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("remembered command".to_string()));
        composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());
        composer.textarea.set_cursor(/*pos*/ 2);

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(composer.textarea.text(), "draft");
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert_eq!(composer.textarea.text(), "remembered command");

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!composer.history_search_active());
        assert_eq!(composer.textarea.text(), "draft");
        assert_eq!(composer.textarea.cursor(), 2);
    }

    #[test]
    fn history_search_no_match_restores_preview_but_keeps_search_open() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            /*has_input_focus*/ true,
            sender,
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        );
        composer
            .history
            .record_local_submission(HistoryEntry::new("git status".to_string()));
        composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        for ch in ['z', 'z', 'z'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert!(composer.history_search_active());
        assert_eq!(composer.textarea.text(), "draft");
        assert_eq!(composer.footer_mode(), FooterMode::HistorySearch);
    }
}
