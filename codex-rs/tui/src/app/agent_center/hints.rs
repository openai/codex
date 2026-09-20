//! Status-tab hints reflect shortcuts that are not claimed by configured actions.

use super::*;
use crate::key_hint;
use crate::style::footer_hint_label_style;

pub(super) fn hint_line(items: &[(String, String)]) -> Line<'static> {
    let mut line = Line::default();
    for (key, label) in items.iter().filter(|(key, _)| !key.is_empty()) {
        if !line.spans.is_empty() {
            line.spans.push("  ".dim());
        }
        line.spans.extend(key_hint::key_label_spans(key));
        line.spans.push(Span::styled(
            format!(" {label}"),
            footer_hint_label_style().not_bold(),
        ));
    }
    line
}

impl AgentsOverviewView {
    pub(super) fn center_filter_hint(&self) -> String {
        [key_hint::plain(KeyCode::Tab), key_hint::shift(KeyCode::Tab)]
            .into_iter()
            .filter(|hint| {
                let (code, modifiers) = hint.parts();
                !self
                    .center_shortcut_keys
                    .is_pressed(KeyEvent::new(code, modifiers))
            })
            .map(|hint| hint.display_label())
            .collect::<Vec<_>>()
            .join("/")
    }
}
