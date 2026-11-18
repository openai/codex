use super::*;
use pretty_assertions::assert_eq;

const ID: &str = "019e60fc-0518-7c21-9596-980fe97225ba";

#[test]
fn only_exact_thread_links_are_navigable() {
    assert_eq!(
        thread_id(&format!("codex://threads/{ID}")),
        ThreadId::from_string(ID).ok()
    );
    for destination in [
        format!("https://threads/{ID}"),
        format!("codex://threads/{ID}/extra"),
        format!("codex://threads/{ID}?other=1"),
        format!("codex://threads/{ID}#fragment"),
        format!("codex://threads/\u{7}{ID}"),
        "codex://threads/not-a-thread".to_string(),
    ] {
        assert_eq!(thread_id(&destination), None, "{destination:?}");
    }
}

#[test]
fn thread_links_are_clickable_without_desktop_hyperlinks() {
    use crate::terminal_hyperlinks::HyperlinkLine;
    use ratatui::text::Line;

    let destination = format!("codex://threads/{ID}");
    let mut line = HyperlinkLine::new(Line::default());
    line.push_span("Other thread".into(), Some(&destination));
    assert_eq!(line.hyperlinks.len(), 1);
    let link = &line.hyperlinks[0];
    assert_eq!(
        (link.interaction_destination(), link.terminal_destination()),
        (Some(destination), None)
    );
}
