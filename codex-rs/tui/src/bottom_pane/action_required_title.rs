use super::TerminalTitleItem;

pub(crate) const ACTION_REQUIRED_PREVIEW_PREFIX: &str = "[ ! ] Action Required";

pub(crate) fn build_action_required_title_text<I, F>(
    prefix: &str,
    items: I,
    excluded_items: &[TerminalTitleItem],
    mut value_for: F,
) -> String
where
    I: IntoIterator<Item = TerminalTitleItem>,
    F: FnMut(TerminalTitleItem) -> Option<String>,
{
    let mut parts = vec![prefix.to_string()];
    for item in items {
        if item == TerminalTitleItem::Spinner || excluded_items.contains(&item) {
            continue;
        }
        if let Some(value) = value_for(item) {
            parts.push(value);
        }
    }
    parts.join(" | ")
}
