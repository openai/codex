//! Status text and, for reasoning, the source line that can still extend it.

use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatusHeader {
    pub(crate) text: String,
    source: Option<ReasoningLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReasoningLine {
    item_id: Option<String>,
    part: usize,
    start: usize,
}

impl From<String> for StatusHeader {
    fn from(text: String) -> Self {
        Self { text, source: None }
    }
}

impl StatusHeader {
    pub(crate) fn from_summary(item_id: Option<&str>, part: usize, summary: &str) -> Option<Self> {
        let mut start = summary.len();
        summary.split_inclusive('\n').rev().find_map(|line| {
            start -= line.len();
            Some(Self {
                text: heading_text(line)?.into_owned(),
                source: Some(ReasoningLine {
                    item_id: item_id.map(str::to_owned),
                    part,
                    start,
                }),
            })
        })
    }

    pub(super) fn is_reasoning(&self) -> bool {
        self.source.is_some()
    }

    /// Read through the end of this line even when a chunk contains the next one too.
    pub(super) fn extend(&mut self, item_id: Option<&str>, part: usize, summary: &str) {
        if let Some(line) = &self.source
            && line.item_id.as_deref() == item_id
            && line.part == part
            && let Some((prefix, tail)) = summary.split_at_checked(line.start)
            && (prefix.is_empty() || prefix.ends_with('\n'))
            && let Some(text) = tail.lines().next().and_then(heading_text)
            && let Some(suffix) = text.strip_prefix(self.text.as_str())
        {
            self.text.push_str(suffix);
        }
    }
}

fn heading_text(raw: &str) -> Option<Cow<'_, str>> {
    let line = raw.trim().trim_start_matches('#').trim();
    if matches!(line, "" | "*" | "<" | "<!" | "<!-") || line.starts_with("<!--") {
        return None;
    }
    let Some(stripped) = line.strip_prefix("**") else {
        return Some(line.into());
    };
    let (bold, trailing) = stripped.split_once("**")?;
    match (bold, trailing) {
        ("", "") => None,
        (bold, "") => Some(bold.into()),
        _ => Some(format!("{bold}{trailing}").into()),
    }
}
