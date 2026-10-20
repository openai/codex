//! Combines model-visible context with the user request while preserving transcript rendering.

use codex_app_server_protocol::ByteRange;
use codex_app_server_protocol::TextElement;
use codex_app_server_protocol::UserInput;

// Context precedes this delimiter so transcript rendering across Codex surfaces can show the user's request.
const PROMPT_REQUEST_BEGIN: &str = "## My request for Codex:";

pub(crate) fn apply_prompt_context_to_user_input(context_text: &str, items: &mut Vec<UserInput>) {
    let prefix = format!("{context_text}\n{PROMPT_REQUEST_BEGIN}\n");
    if let Some(text_index) = items
        .iter()
        .position(|item| matches!(item, UserInput::Text { .. }))
    {
        // Prefix the existing text item in place so image and text items keep
        // the same relative order they had in the user's original submission.
        let item = std::mem::replace(
            &mut items[text_index],
            UserInput::Text {
                text: String::new(),
                text_elements: Vec::new(),
            },
        );
        let UserInput::Text {
            text,
            text_elements,
        } = item
        else {
            unreachable!("position matched a text item");
        };
        items[text_index] = prefixed_text_input(prefix, text, text_elements);
    } else {
        items.insert(
            0,
            UserInput::Text {
                text: prefix,
                text_elements: Vec::new(),
            },
        );
    }
}

pub(crate) fn extract_prompt_request_with_offset(message: &str) -> (&str, usize) {
    let Some((before_request, request)) = message.rsplit_once(PROMPT_REQUEST_BEGIN) else {
        return (message, 0);
    };

    let request_start = before_request.len() + PROMPT_REQUEST_BEGIN.len();
    let trimmed_request = request.trim();
    let leading_trimmed_len = request.len() - request.trim_start().len();
    (trimmed_request, request_start + leading_trimmed_len)
}

fn prefixed_text_input(prefix: String, text: String, text_elements: Vec<TextElement>) -> UserInput {
    let prefix_len = prefix.len();
    UserInput::Text {
        text: format!("{prefix}{text}"),
        text_elements: text_elements
            .into_iter()
            .map(|element| {
                let range = element.byte_range.clone();
                TextElement::new(
                    ByteRange {
                        start: range.start + prefix_len,
                        end: range.end + prefix_len,
                    },
                    element.placeholder().map(str::to_string),
                )
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "user_prompt_tests.rs"]
mod tests;
