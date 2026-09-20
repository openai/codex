//! Picker confirmation and selected-file persistence leave the running renderer unchanged.

use crate::app::tests::make_test_app_with_channels;
use crate::app_event::AppEvent;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::legacy_core::config::ConfigBuilder;
use codex_config::LoaderOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn tui_mode_picker_requires_confirmation_and_explains_restart() {
    let mut screens = Vec::new();
    for current in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.chat_widget.local_settings.tui.fullscreen_transcript = current;
        let before = app.chat_widget.local_settings.clone();
        let other = if current { KeyCode::Up } else { KeyCode::Down };
        app.chat_widget.show_tui_mode_picker();
        for width in [80, 40] {
            screens.push(format!(
                "fullscreen={current}, width={width}\n{}",
                render_bottom_popup(&app.chat_widget, width)
            ));
        }
        app.chat_widget.handle_key_event(other.into());
        app.chat_widget.handle_key_event(KeyCode::Esc.into());
        assert_eq!(app.chat_widget.local_settings, before);
        while let Ok(event) = events.try_recv() {
            assert!(!matches!(
                event,
                AppEvent::FullscreenTranscriptSelected { .. }
            ));
        }
        app.chat_widget.show_tui_mode_picker();
        app.chat_widget.handle_key_event(other.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        let mut choices = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let AppEvent::FullscreenTranscriptSelected { enabled } = event {
                choices.push(enabled);
            }
        }
        assert_eq!(choices, vec![!current]);
        assert_eq!(app.chat_widget.local_settings, before);
    }
    insta::assert_snapshot!(screens.join("\n\n"));
}

#[tokio::test]
async fn tui_mode_picker_saves_selected_config_without_changing_the_live_mode() -> anyhow::Result<()>
{
    let home = tempfile::tempdir()?;
    let base = home.path().join("config.toml");
    let selected = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    std::fs::write(&base, "# untouched base config\n")?;
    std::fs::write(&selected, "[tui]\nanimations = false\n")?;
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.local_settings.user_config_path = selected.clone();
    let mut expected_app = app.local_settings.clone();
    let mut expected_widget = app.chat_widget.local_settings.clone();
    for enabled in [true, false] {
        app.save_fullscreen_transcript(enabled).await;
        expected_app.tui.fullscreen_transcript = enabled;
        expected_widget.tui.fullscreen_transcript = enabled;
        assert_eq!(
            (&app.local_settings, &app.chat_widget.local_settings),
            (&expected_app, &expected_widget)
        );
        let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&selected)?)?;
        assert_eq!(
            serde_json::to_value(saved)?,
            serde_json::json!({"tui": {"animations": false, "fullscreen_transcript": enabled}})
        );
        let reloaded = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                user_config_path: Some(selected.clone()),
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await?;
        assert_eq!(reloaded.tui_fullscreen_transcript, enabled);
    }
    assert_eq!(std::fs::read_to_string(base)?, "# untouched base config\n");
    Ok(())
}

#[tokio::test]
async fn tui_mode_picker_failed_save_preserves_the_preference() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let path = home.path().join("config.toml");
    std::fs::write(&path, "[invalid\n")?;
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.local_settings.user_config_path = AbsolutePathBuf::from_absolute_path(&path)?;
    let before = (
        app.local_settings.clone(),
        app.chat_widget.local_settings.clone(),
    );
    app.save_fullscreen_transcript(!before.0.tui.fullscreen_transcript)
        .await;
    assert_eq!(
        (&app.local_settings, &app.chat_widget.local_settings),
        (&before.0, &before.1)
    );
    assert_eq!(std::fs::read_to_string(path)?, "[invalid\n");
    let mut messages = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            messages.extend(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .map(ToString::to_string),
            );
        }
    }
    assert!(messages.join("\n").contains("Failed to save TUI mode"));
    Ok(())
}
