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
    let layout = OwnedStartupLayout::new(&pump);
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

#[test]
fn new_startup_decoration_tracks_draft_and_session_action() {
    let mut pump = crate::startup_draft::tests::quiet_startup_test_pump();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 32,
    );
    let mut blossom_visible = Vec::new();
    for (draft, action, pending) in [
        ("", StartupDraftSessionAction::New, false),
        (" ", StartupDraftSessionAction::New, false),
        ("", StartupDraftSessionAction::New, false),
        ("", StartupDraftSessionAction::New, true),
        ("", StartupDraftSessionAction::Resume, false),
        ("", StartupDraftSessionAction::Fork, false),
        ("", StartupDraftSessionAction::NewFromCommandCenter, false),
    ] {
        pump.session_action = action;
        pump.submission_pending = pending;
        pump.bottom_pane
            .set_composer_text(draft.into(), Vec::new(), Vec::new());
        let layout = OwnedStartupLayout::new(&pump);
        let mut buffer = Buffer::empty(area);
        layout.render(area, &mut buffer);
        blossom_visible.push(buffer.content.iter().any(|cell| {
            cell.symbol()
                .chars()
                .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
        }));
    }
    assert_eq!(
        blossom_visible,
        [true, false, true, false, false, false, true]
    );
}

#[tokio::test]
async fn configured_welcome_opt_out_disables_blossom_and_clicks() -> anyhow::Result<()> {
    use clap::Parser;
    use codex_config::LoaderOverrides;
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;
    let codex_home = tempfile::tempdir()?;
    let cli = crate::Cli::try_parse_from(["codex"])?;
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 44,
    );
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 40,
        row: 22,
        modifiers: KeyModifiers::NONE,
    };
    for (settings, enabled) in [
        ("", true),
        ("[tui]\nanimations = false\n", false),
        ("[tui.effects]\nwelcome = false\n", false),
        (
            "[tui]\nanimations = false\n[tui.effects]\nwelcome = false\n",
            false,
        ),
    ] {
        std::fs::write(codex_home.path().join("config.toml"), settings)?;
        let presentation = crate::startup_presentation::load(
            &cli,
            codex_home.path(),
            LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            },
            Vec::new(),
            /*config_cwd*/ None,
        )
        .await?;
        let config = crate::legacy_core::config::ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        let mut pump = crate::startup_draft::tests::quiet_startup_test_pump();
        // The provisional frame uses the bootstrap value; subsequent frames use effective config.
        pump.motion = presentation.screen.welcome_motion;
        for effective in [false, true] {
            if effective {
                pump.apply_config(&config);
            }
            let mut buffer = Buffer::empty(area);
            OwnedStartupLayout::new(&pump).render(area, &mut buffer);
            let visible = buffer.content.iter().any(|cell| {
                cell.symbol()
                    .chars()
                    .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
            });
            assert_eq!(
                visible, enabled,
                "settings: {settings}, effective: {effective}"
            );
            assert_eq!(pump.blossom.borrow_mut().handle_mouse(click), enabled);
        }
    }
    Ok(())
}
