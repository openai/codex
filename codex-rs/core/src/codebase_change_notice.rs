use std::fmt::Write;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use crate::codebase_snapshot::SnapshotDiff;

pub(crate) const CODEBASE_CHANGE_NOTICE_MAX_PATHS: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CodebaseChangeNotice {
    added: Vec<String>,
    removed: Vec<String>,
    modified: Vec<String>,
    truncated: bool,
}

impl CodebaseChangeNotice {
    pub(crate) fn new(diff: SnapshotDiff, limit: usize) -> Self {
        let mut remaining = limit;
        let mut truncated = false;

        let added = take_paths(diff.added, &mut remaining, &mut truncated);
        let removed = take_paths(diff.removed, &mut remaining, &mut truncated);
        let modified = take_paths(diff.modified, &mut remaining, &mut truncated);

        Self {
            added,
            removed,
            modified,
            truncated,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    pub(crate) fn serialize_to_xml(&self) -> String {
        let mut output = String::new();
        if self.truncated {
            let _ = writeln!(output, "<codebase_changes truncated=\"true\">");
        } else {
            let _ = writeln!(output, "<codebase_changes>");
        }

        let mut summary_parts = Vec::new();
        if !self.added.is_empty() {
            summary_parts.push(format!("added {}", self.added.len()));
        }
        if !self.removed.is_empty() {
            summary_parts.push(format!("removed {}", self.removed.len()));
        }
        if !self.modified.is_empty() {
            summary_parts.push(format!("modified {}", self.modified.len()));
        }

        if summary_parts.is_empty() {
            let _ = writeln!(output, "  <summary>no changes</summary>");
        } else {
            let summary = summary_parts.join(", ");
            let _ = writeln!(output, "  <summary>{summary}</summary>");
        }

        serialize_section(&mut output, "added", &self.added);
        serialize_section(&mut output, "removed", &self.removed);
        serialize_section(&mut output, "modified", &self.modified);
        if self.truncated {
            let _ = writeln!(output, "  <note>additional paths omitted</note>");
        }

        let _ = writeln!(output, "</codebase_changes>");
        output
    }
}

fn take_paths(mut paths: Vec<String>, remaining: &mut usize, truncated: &mut bool) -> Vec<String> {
    if *remaining == 0 {
        if !paths.is_empty() {
            *truncated = true;
        }
        return Vec::new();
    }

    if paths.len() > *remaining {
        paths.truncate(*remaining);
        *truncated = true;
    }

    *remaining -= paths.len();
    paths
}

fn serialize_section(output: &mut String, tag: &str, paths: &[String]) {
    if paths.is_empty() {
        return;
    }

    let _ = writeln!(output, "  <{tag}>");
    for path in paths {
        let _ = writeln!(output, "    <path>{}</path>", escape_xml(path));
    }
    let _ = writeln!(output, "  </{tag}>");
}

fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

impl From<CodebaseChangeNotice> for ResponseItem {
    fn from(notice: CodebaseChangeNotice) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: notice.serialize_to_xml(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn constructs_notice_with_limit() {
        let diff = SnapshotDiff {
            added: vec!["a.rs".to_string(), "b.rs".to_string()],
            removed: vec!["c.rs".to_string()],
            modified: vec!["d.rs".to_string(), "e.rs".to_string()],
        };

        let notice = CodebaseChangeNotice::new(diff, 3);
        assert!(notice.truncated);
        assert_eq!(
            notice.added.len() + notice.removed.len() + notice.modified.len(),
            3
        );
    }

    #[test]
    fn serializes_notice() {
        let diff = SnapshotDiff {
            added: vec!["src/lib.rs".to_string()],
            removed: Vec::new(),
            modified: vec!["src/main.rs".to_string()],
        };
        let notice = CodebaseChangeNotice::new(diff, CODEBASE_CHANGE_NOTICE_MAX_PATHS);
        let xml = notice.serialize_to_xml();
        assert!(xml.contains("<added>"));
        assert!(xml.contains("<modified>"));
        assert!(xml.contains("src/lib.rs"));
        assert!(xml.contains("src/main.rs"));
    }
}
