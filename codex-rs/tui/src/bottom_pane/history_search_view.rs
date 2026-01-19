use std::fs;
use std::io::Read as _;
use std::io::Seek as _;
use std::io::SeekFrom;
use std::path::Path;

use ratatui::style::Stylize;
use ratatui::text::Line;
use serde::Deserialize;

use crate::app_event::AppEvent;

use super::SelectionAction;
use super::SelectionItem;
use super::SelectionViewParams;
use super::popup_consts::standard_popup_hint_line;

const HISTORY_FILENAME: &str = "history.jsonl";
const HISTORY_SEARCH_MAX_BYTES: u64 = 1024 * 1024;
const HISTORY_SEARCH_MAX_ENTRIES: usize = 2000;
const HISTORY_PREVIEW_MAX_CHARS: usize = 200;

#[derive(Deserialize)]
struct HistoryLine {
    text: String,
}

pub(crate) fn history_search_view_params(codex_home: &Path) -> SelectionViewParams {
    let history_path = codex_home.join(HISTORY_FILENAME);
    let entries = load_history_entries(&history_path);
    let items = entries
        .into_iter()
        .map(|entry| {
            let preview = history_preview_lines(&entry);
            let name = preview.first.clone();
            let description = preview.rest;
            let search_value = Some(entry.clone());
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::SetComposerText(entry.clone()));
            })];

            SelectionItem {
                name,
                description,
                selected_description: None,
                is_current: false,
                actions,
                dismiss_on_select: true,
                search_value,
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        title: Some("History".to_string()),
        subtitle: Some("Search past prompts.".to_string()),
        footer_hint: Some(standard_popup_hint_line()),
        footer_note: Some(Line::from(
            "Type to filter; Enter pastes into the composer.".dim(),
        )),
        items,
        is_searchable: true,
        search_placeholder: Some("search history".to_string()),
        ..Default::default()
    }
}

fn load_history_entries(history_path: &Path) -> Vec<String> {
    let Ok(mut file) = fs::File::open(history_path) else {
        return Vec::new();
    };

    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return Vec::new();
    };

    let start = len.saturating_sub(HISTORY_SEARCH_MAX_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }

    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }

    // If we did not start from 0, drop the partial first line so parsing
    // starts at a JSON object boundary.
    if start > 0 {
        let Some(first_newline) = buf.iter().position(|&b| b == b'\n') else {
            return Vec::new();
        };
        buf.drain(..=first_newline);
    }

    let mut out: Vec<String> = Vec::new();
    let mut last: Option<String> = None;
    for line in String::from_utf8_lossy(&buf).lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(parsed) = serde_json::from_str::<HistoryLine>(line) else {
            continue;
        };
        let text = parsed.text;
        if text.is_empty() {
            continue;
        }
        if last.as_ref().is_some_and(|prev| prev == &text) {
            continue;
        }
        last = Some(text.clone());
        out.push(text);
        if out.len() >= HISTORY_SEARCH_MAX_ENTRIES {
            break;
        }
    }

    out
}

struct HistoryPreview {
    first: String,
    rest: Option<String>,
}

fn history_preview_lines(text: &str) -> HistoryPreview {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim();
    let rest = lines.collect::<Vec<_>>().join(" ").trim().to_string();

    HistoryPreview {
        first: truncate_preview(first),
        rest: (!rest.is_empty()).then_some(truncate_preview(&rest)),
    }
}

fn truncate_preview(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= HISTORY_PREVIEW_MAX_CHARS {
        return trimmed.to_string();
    }

    trimmed
        .chars()
        .take(HISTORY_PREVIEW_MAX_CHARS.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_most_recent_entries_first() {
        let tmp = tempfile::tempdir().unwrap();
        let history = tmp.path().join(HISTORY_FILENAME);
        fs::write(
            &history,
            concat!(
                r#"{"session_id":"s","ts":1,"text":"first"}"#,
                "\n",
                r#"{"session_id":"s","ts":2,"text":"second"}"#,
                "\n",
                r#"{"session_id":"s","ts":3,"text":"third"}"#,
                "\n"
            ),
        )
        .unwrap();

        assert_eq!(
            vec![
                "third".to_string(),
                "second".to_string(),
                "first".to_string()
            ],
            load_history_entries(&history)
        );
    }

    #[test]
    fn drops_duplicate_consecutive_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let history = tmp.path().join(HISTORY_FILENAME);
        fs::write(
            &history,
            concat!(
                r#"{"session_id":"s","ts":1,"text":"dup"}"#,
                "\n",
                r#"{"session_id":"s","ts":2,"text":"dup"}"#,
                "\n",
                r#"{"session_id":"s","ts":3,"text":"unique"}"#,
                "\n"
            ),
        )
        .unwrap();

        assert_eq!(
            vec!["unique".to_string(), "dup".to_string()],
            load_history_entries(&history)
        );
    }
}
