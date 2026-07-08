use crate::SESSION_TITLE_MAX_LEN;
use crate::truncate;

pub(super) const IMPORTED_SESSION_FALLBACK_TITLE: &str = "Imported session";
const RECOGNIZED_CONTROL_WRAPPERS: [(&str, &str); 10] = [
    ("<command-message>", "</command-message>"),
    ("<command-name>", "</command-name>"),
    ("<command-args>", "</command-args>"),
    ("<local-command-caveat>", "</local-command-caveat>"),
    ("<local-command-stderr>", "</local-command-stderr>"),
    ("<local-command-stdout>", "</local-command-stdout>"),
    ("<task-notification>", "</task-notification>"),
    ("<system-reminder>", "</system-reminder>"),
    ("<ide_opened_file>", "</ide_opened_file>"),
    ("<ide_selection>", "</ide_selection>"),
];

pub(super) struct SessionTitleCandidates {
    pub custom_title: Option<String>,
    pub ai_title: Option<String>,
    pub fallback_title: Option<String>,
}

impl SessionTitleCandidates {
    pub fn select(self) -> Option<String> {
        self.custom_title.or(self.ai_title).or(self.fallback_title)
    }
}

pub(super) fn fallback_title_from_user_message(message: &str) -> Option<String> {
    let message = strip_leading_control_wrappers(message);
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| truncate(line, SESSION_TITLE_MAX_LEN))
}

fn strip_leading_control_wrappers(message: &str) -> &str {
    let mut remainder = message.trim_start();
    while let Some(wrapper_end) = leading_control_wrapper_end(remainder) {
        remainder = remainder[wrapper_end..].trim_start();
    }
    remainder
}

fn leading_control_wrapper_end(text: &str) -> Option<usize> {
    let (outer_tag, opening_len) = recognized_opening_tag(text)?;
    let mut open_tags = vec![outer_tag];
    let mut cursor = opening_len;

    while !open_tags.is_empty() {
        cursor += text.get(cursor..)?.find('<')?;
        let candidate = text.get(cursor..)?;
        if let Some((tag, token_len)) = recognized_opening_tag(candidate) {
            open_tags.push(tag);
            cursor += token_len;
            continue;
        }
        if let Some((tag, token_len)) = recognized_closing_tag(candidate) {
            if open_tags.last().copied() != Some(tag) {
                return None;
            }
            open_tags.pop();
            cursor += token_len;
            continue;
        }
        cursor += 1;
    }

    Some(cursor)
}

fn recognized_opening_tag(text: &str) -> Option<(usize, usize)> {
    RECOGNIZED_CONTROL_WRAPPERS
        .iter()
        .enumerate()
        .find_map(|(index, (opening, _closing))| {
            text.starts_with(opening).then_some((index, opening.len()))
        })
}

fn recognized_closing_tag(text: &str) -> Option<(usize, usize)> {
    RECOGNIZED_CONTROL_WRAPPERS
        .iter()
        .enumerate()
        .find_map(|(index, (_opening, closing))| {
            text.starts_with(closing).then_some((index, closing.len()))
        })
}

#[cfg(test)]
#[path = "title_tests.rs"]
mod tests;
