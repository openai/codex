//! Verify that memory settings reserve enough space for wrapped choices.

use super::*;

#[test]
fn reset_confirmation_keeps_both_choices_at_wrap_boundary() {
    let (app_tx, _app_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut view = MemoriesSettingsView::new(
        /*use_memories*/ true,
        /*generate_memories*/ true,
        AppEventSender::new(app_tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    view.open_reset_confirmation();

    let mut snapshots = Vec::new();
    for width in [70, 71, 72] {
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            view.desired_height(width),
        );
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer);
        let text = buffer
            .content
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("Go back"),
            "missing cancel choice at width {width}"
        );
        snapshots.push(format!("width={width}\n{text}"));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}
