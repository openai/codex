//! Verify provisional composer geometry while startup owns the terminal.

use super::*;

#[test]
fn owned_startup_keeps_the_live_bottom_geometry() {
    let mut pump = crate::startup_draft::tests::quiet_startup_test_pump();
    pump.bottom_pane.set_status_line_enabled(/*enabled*/ true);
    pump.bottom_pane
        .set_composer_text("first line\nsecond line".into(), Vec::new(), Vec::new());
    pump.bottom_pane.set_footer_hint_override(Some(vec![
        ("Waiting for startup".into(), String::new()),
        ("esc".into(), "cancel".into()),
    ]));
    let layout = OwnedStartupLayout::new(
        &pump.header,
        &pump.bottom_pane,
        StartupDraftSessionAction::New,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 16,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer);
    let frame = (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(
        "owned_startup_layout",
        format!("cursor={:?}\n{frame}", layout.cursor_pos(area))
            .replace(crate::version::CODEX_CLI_VERSION, "<VERSION>")
    );
}
