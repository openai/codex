//! Selects recent visible exchanges without reading tools, reasoning, or another thread's history.
//! A newer unanswered request is retained once; excerpts preserve both ends within a byte limit.

use std::sync::Arc;

use codex_context_fragments::RecapPrompt;

use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;

pub(super) const RECAP_HISTORY_MAX_TURNS: usize = 8;
const OMITTED_HISTORY: &str = "[Earlier exchanges omitted]\n\n";
const EXCERPT_MARKER: &str = "\n[... excerpted ...]\n";

pub(super) fn recap_history(cells: &[Arc<dyn HistoryCell>]) -> String {
    let exchanges = recent_exchanges(cells);
    let Some(latest) = exchanges.last() else {
        return String::new();
    };
    let blocks = exchanges
        .iter()
        .map(|exchange| {
            exchange
                .fields()
                .map(|(label, text)| format!("{label}: {text}"))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .collect::<Vec<_>>();
    let mut bytes = blocks.iter().map(String::len).sum::<usize>() + 2 * (blocks.len() - 1);
    if bytes <= RecapPrompt::HISTORY_MAX_BYTES {
        return blocks.join("\n\n");
    }

    // Keep the newest answer and any newer unanswered correction together.
    let retained = if latest.assistant.is_empty() { 2 } else { 1 };
    let oldest_retained = exchanges.len().saturating_sub(retained);
    let mut start = 0;
    while bytes > RecapPrompt::HISTORY_MAX_BYTES - OMITTED_HISTORY.len() && start < oldest_retained
    {
        bytes -= blocks[start].len() + 2;
        start += 1;
    }
    let omission = if start > 0 { OMITTED_HISTORY } else { "" };
    let budget = RecapPrompt::HISTORY_MAX_BYTES - omission.len();
    if bytes <= budget {
        return format!("{omission}{}", blocks[start..].join("\n\n"));
    }

    let fields = exchanges[start..]
        .iter()
        .flat_map(Exchange::fields)
        .collect::<Vec<_>>();
    let field_count = fields.len();
    let overhead = fields
        .iter()
        .map(|(label, _)| label.len() + 2)
        .sum::<usize>()
        + 2 * (field_count - 1);
    let mut remaining = budget.saturating_sub(overhead);
    let excerpts = fields
        .iter()
        .enumerate()
        .map(|(index, (label, text))| {
            let share = remaining / (field_count - index);
            // Reserve a share for later fields, without wasting space on short replies.
            let reserved = fields[index + 1..]
                .iter()
                .map(|(_, text)| text.len().min(share))
                .sum::<usize>();
            let excerpt = excerpt(text, remaining - reserved);
            remaining -= excerpt.len();
            format!("{label}: {excerpt}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{omission}{excerpts}")
}

#[derive(Default)]
struct Exchange {
    user: String,
    assistant: String,
}

impl Exchange {
    fn fields(&self) -> impl Iterator<Item = (&'static str, &str)> {
        let user_label = if self.assistant.is_empty() {
            "Pending user request"
        } else {
            "User"
        };
        [
            (user_label, self.user.as_str()),
            ("Assistant", self.assistant.as_str()),
        ]
        .into_iter()
        .filter(|(_, text)| !text.is_empty())
    }
}

// Adjacent user cells preserve steering as part of one request.
fn recent_exchanges(cells: &[Arc<dyn HistoryCell>]) -> Vec<Exchange> {
    let mut exchanges = Vec::new();
    let mut current = Exchange::default();
    let mut answered = 0;
    for cell in cells.iter().rev() {
        let is_user = if cell.as_any().is::<UserHistoryCell>() {
            true
        } else if cell.as_any().is::<AgentMarkdownCell>() || cell.as_any().is::<AgentMessageCell>()
        {
            false
        } else {
            continue;
        };
        let mut content = cell
            .raw_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();
        if content.is_empty() {
            continue;
        }
        // In reverse order, assistant text before a request belongs to the preceding exchange.
        if !is_user && !current.user.is_empty() {
            answered += usize::from(!current.assistant.is_empty());
            exchanges.push(std::mem::take(&mut current));
            if answered == RECAP_HISTORY_MAX_TURNS {
                break;
            }
        }
        let field = if is_user {
            &mut current.user
        } else {
            &mut current.assistant
        };
        if !field.is_empty() {
            content.push_str("\n\n");
            content.push_str(field);
        }
        *field = content;
    }
    if !current.user.is_empty() {
        exchanges.push(current);
    }
    exchanges.reverse();
    exchanges
}

fn excerpt(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let Some(content_bytes) = max_bytes.checked_sub(EXCERPT_MARKER.len()) else {
        return text[..text.floor_char_boundary(max_bytes)].to_owned();
    };
    let head = text.floor_char_boundary(content_bytes / 2);
    let tail = text.ceil_char_boundary(text.len() - (content_bytes - content_bytes / 2));
    format!("{}{EXCERPT_MARKER}{}", &text[..head], &text[tail..])
}
