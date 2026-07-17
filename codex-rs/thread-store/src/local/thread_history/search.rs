use std::borrow::Cow;

use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::strip_user_message_prefix;
use futures::TryStreamExt;
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::TagEnd;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;

use super::super::LocalThreadStore;
use super::read::CursorScope;
use super::read::serialize_cursor;
use super::read::validate_thread_for_paginated_reads;
use super::thread_history_error;
use crate::SearchTextRange;
use crate::SearchThreadOccurrencesParams;
use crate::StoredThreadOccurrence;
use crate::ThreadOccurrenceSearchPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const SNIPPET_CONTEXT_BEFORE_CHARS: usize = 48;
const SNIPPET_CONTEXT_AFTER_CHARS: usize = 96;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchCursor {
    thread_id: ThreadId,
    search_term: String,
    next_rollout_ordinal: i64,
    next_occurrence_index: usize,
}

struct CandidateRow {
    turn_id: String,
    item_id: String,
    rollout_ordinal: i64,
    item_json: String,
    turn_rollout_ordinal: i64,
}

pub(in crate::local) async fn search_thread_occurrences(
    store: &LocalThreadStore,
    params: SearchThreadOccurrencesParams,
) -> ThreadStoreResult<ThreadOccurrenceSearchPage> {
    if params.search_term.trim().is_empty() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "thread/searchOccurrences requires search_term".to_string(),
        });
    }
    if params.page_size == 0 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "thread/searchOccurrences requires page_size greater than zero".to_string(),
        });
    }
    validate_thread_for_paginated_reads(
        store,
        params.thread_id,
        /*include_archived*/ true,
        "thread/searchOccurrences",
    )
    .await?;
    let cursor = parse_cursor(
        params.cursor.as_deref(),
        params.thread_id,
        &params.search_term,
    )?;
    let next_rollout_ordinal = cursor
        .as_ref()
        .map_or(0, |cursor| cursor.next_rollout_ordinal);
    let matcher = LiteralMatcher::new(params.search_term.as_str());
    let pool = store.thread_history_db().await?;
    let mut rows = sqlx::query(
        r#"
SELECT turn_id, item_id, rollout_ordinal, item_json, turn_rollout_ordinal
FROM (
    SELECT
        items.turn_id,
        items.item_id,
        items.rollout_ordinal,
        items.item_json,
        turns.rollout_ordinal AS turn_rollout_ordinal
    FROM thread_items AS items
    JOIN thread_turns AS turns
      ON turns.thread_id = items.thread_id
     AND turns.turn_id = items.turn_id
    WHERE items.thread_id = ?
      AND items.item_type = 'userMessage'
      AND items.rollout_ordinal >= ?

    UNION ALL

    SELECT
        items.turn_id,
        items.item_id,
        items.rollout_ordinal,
        items.item_json,
        turns.rollout_ordinal AS turn_rollout_ordinal
    FROM thread_turns AS turns
    JOIN thread_items AS items
      ON items.thread_id = turns.thread_id
     AND items.turn_id = turns.turn_id
     AND items.item_id = turns.final_agent_item_id
    WHERE turns.thread_id = ?
      AND turns.final_agent_item_id IS NOT NULL
      AND items.rollout_ordinal >= ?
)
ORDER BY rollout_ordinal ASC
        "#,
    )
    .bind(params.thread_id.to_string())
    .bind(next_rollout_ordinal)
    .bind(params.thread_id.to_string())
    .bind(next_rollout_ordinal)
    .fetch(pool);

    let mut items = Vec::with_capacity(params.page_size);
    while let Some(row) = rows.try_next().await.map_err(thread_history_error)? {
        let row = candidate_row(row)?;
        let item = serde_json::from_str::<ThreadItem>(row.item_json.as_str()).map_err(|err| {
            ThreadStoreError::Internal {
                message: format!("failed to deserialize stored thread item: {err}"),
            }
        })?;
        let Some(text) = searchable_text(&item) else {
            continue;
        };
        let first_occurrence_index = cursor
            .as_ref()
            .filter(|cursor| cursor.next_rollout_ordinal == row.rollout_ordinal)
            .map_or(0, |cursor| cursor.next_occurrence_index);
        let remaining = params
            .page_size
            .saturating_add(1)
            .saturating_sub(items.len());
        let turn_cursor = serialize_cursor(
            params.thread_id,
            &CursorScope::Turns,
            row.turn_rollout_ordinal,
            /*include_anchor*/ true,
        )?;
        for (occurrence_index, matched) in matcher
            .find_ranges(
                text.as_ref(),
                first_occurrence_index.saturating_add(remaining),
            )
            .into_iter()
            .enumerate()
            .skip(first_occurrence_index)
        {
            if items.len() == params.page_size {
                return Ok(ThreadOccurrenceSearchPage {
                    items,
                    next_cursor: Some(serialize_cursor_for_search(SearchCursor {
                        thread_id: params.thread_id,
                        search_term: params.search_term,
                        next_rollout_ordinal: row.rollout_ordinal,
                        next_occurrence_index: occurrence_index,
                    })?),
                });
            }
            items.push(occurrence_in_item(
                row.turn_id.as_str(),
                row.item_id.as_str(),
                text.as_ref(),
                matched,
                turn_cursor.as_str(),
            ));
        }
    }

    Ok(ThreadOccurrenceSearchPage {
        items,
        next_cursor: None,
    })
}

fn candidate_row(row: sqlx::sqlite::SqliteRow) -> ThreadStoreResult<CandidateRow> {
    let rollout_ordinal = row.try_get::<i64, _>("rollout_ordinal")?;
    let turn_rollout_ordinal = row.try_get::<i64, _>("turn_rollout_ordinal")?;
    if rollout_ordinal < 0 || turn_rollout_ordinal < 0 {
        return Err(ThreadStoreError::Internal {
            message: "invalid stored thread history ordinal".to_string(),
        });
    }
    Ok(CandidateRow {
        turn_id: row.try_get("turn_id")?,
        item_id: row.try_get("item_id")?,
        rollout_ordinal,
        item_json: row.try_get("item_json")?,
        turn_rollout_ordinal,
    })
}

fn parse_cursor(
    cursor: Option<&str>,
    thread_id: ThreadId,
    search_term: &str,
) -> ThreadStoreResult<Option<SearchCursor>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor_value: SearchCursor =
        serde_json::from_str(cursor).map_err(|_| invalid_cursor(cursor))?;
    if cursor_value.thread_id != thread_id
        || cursor_value.search_term != search_term
        || cursor_value.next_rollout_ordinal < 0
    {
        return Err(invalid_cursor(cursor));
    }
    Ok(Some(cursor_value))
}

fn serialize_cursor_for_search(cursor: SearchCursor) -> ThreadStoreResult<String> {
    serde_json::to_string(&cursor).map_err(thread_history_error)
}

fn invalid_cursor(cursor: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("invalid cursor: {cursor}"),
    }
}

fn searchable_text(item: &ThreadItem) -> Option<Cow<'_, str>> {
    match item {
        ThreadItem::UserMessage { content, .. } => {
            let mut text_parts = content
                .iter()
                .filter_map(|input| match input {
                    UserInput::Text { text, .. } => Some(strip_user_message_prefix(text)),
                    UserInput::Image { .. }
                    | UserInput::LocalImage { .. }
                    | UserInput::Audio { .. }
                    | UserInput::LocalAudio { .. }
                    | UserInput::Skill { .. }
                    | UserInput::Mention { .. } => None,
                })
                .filter(|text| !text.is_empty())
                .peekable();
            let first = text_parts.next()?;
            match text_parts.next() {
                None => Some(Cow::Borrowed(first)),
                Some(second) => {
                    let mut parts = vec![first, second];
                    parts.extend(text_parts);
                    Some(Cow::Owned(parts.concat()))
                }
            }
        }
        ThreadItem::AgentMessage { text, .. } => {
            let text = markdown_to_search_text(text);
            (!text.is_empty()).then_some(Cow::Owned(text))
        }
        ThreadItem::HookPrompt { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::CommandExecution { .. }
        | ThreadItem::FileChange { .. }
        | ThreadItem::McpToolCall { .. }
        | ThreadItem::DynamicToolCall { .. }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::SubAgentActivity { .. }
        | ThreadItem::WebSearch(_)
        | ThreadItem::ImageView { .. }
        | ThreadItem::Sleep(_)
        | ThreadItem::ImageGeneration(_)
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => None,
    }
}

fn markdown_to_search_text(markdown: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(markdown.trim()) {
        match event {
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::Table
                | TagEnd::TableHead
                | TagEnd::TableRow
                | TagEnd::TableCell,
            ) => text.push(' '),
            Event::Start(_)
            | Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::HtmlBlock
                | TagEnd::FootnoteDefinition
                | TagEnd::Image
                | TagEnd::MetadataBlock(_),
            )
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_) => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct LiteralMatcher {
    lowercase_needle: String,
}

impl LiteralMatcher {
    fn new(needle: &str) -> Self {
        Self {
            lowercase_needle: needle.to_lowercase(),
        }
    }

    fn find_ranges(&self, text: &str, limit: usize) -> Vec<std::ops::Range<usize>> {
        let lowercase_text = text.to_lowercase();
        let mut spans = Vec::with_capacity(text.chars().count());
        let mut lowercase_start = 0;
        for (original_start, character) in text.char_indices() {
            let lowercase_end =
                lowercase_start + character.to_lowercase().map(char::len_utf8).sum::<usize>();
            spans.push((
                lowercase_start..lowercase_end,
                original_start..original_start + character.len_utf8(),
            ));
            lowercase_start = lowercase_end;
        }

        lowercase_text
            .match_indices(self.lowercase_needle.as_str())
            .take(limit)
            .filter_map(|(start, matched)| {
                let end = start.saturating_add(matched.len());
                let original_start = spans
                    .iter()
                    .find(|(lowercase, _)| lowercase.contains(&start))?
                    .1
                    .start;
                let original_end = spans
                    .iter()
                    .find(|(lowercase, _)| lowercase.contains(&end.saturating_sub(1)))?
                    .1
                    .end;
                Some(original_start..original_end)
            })
            .collect()
    }
}

fn occurrence_in_item(
    turn_id: &str,
    item_id: &str,
    text: &str,
    matched: std::ops::Range<usize>,
    turn_cursor: &str,
) -> StoredThreadOccurrence {
    let snippet_start = char_start_before(text, matched.start, SNIPPET_CONTEXT_BEFORE_CHARS);
    let snippet_end = char_end_after(text, matched.end, SNIPPET_CONTEXT_AFTER_CHARS);
    let leading_ellipsis = snippet_start > 0;
    let trailing_ellipsis = snippet_end < text.len();
    let mut snippet = String::new();
    if leading_ellipsis {
        snippet.push_str("... ");
    }
    snippet.push_str(&text[snippet_start..snippet_end]);
    if trailing_ellipsis {
        snippet.push_str(" ...");
    }
    let snippet_match_start =
        if leading_ellipsis { 4 } else { 0 } + utf16_len(&text[snippet_start..matched.start]);
    let match_len = utf16_len(&text[matched]);

    StoredThreadOccurrence {
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        snippet,
        snippet_match_range: SearchTextRange {
            start: snippet_match_start,
            end: snippet_match_start.saturating_add(match_len),
        },
        turn_cursor: turn_cursor.to_string(),
    }
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

fn char_start_before(text: &str, byte_index: usize, chars_before: usize) -> usize {
    text[..byte_index]
        .char_indices()
        .rev()
        .nth(chars_before)
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn char_end_after(text: &str, byte_index: usize, chars_after: usize) -> usize {
    text[byte_index..]
        .char_indices()
        .nth(chars_after)
        .map(|(offset, _)| byte_index.saturating_add(offset))
        .unwrap_or(text.len())
}
