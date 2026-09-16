use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn seeds_only_missing_preferences_and_never_probes_again() -> anyhow::Result<()> {
    for (input, detected, expected) in [
        (
            "",
            true,
            "[tui]\nanimations = false\nscreen_reader_detection_done = true\n",
        ),
        ("", false, "[tui]\nscreen_reader_detection_done = true\n"),
        (
            "[tui]\nanimations = true\n",
            true,
            "[tui]\nanimations = true\nscreen_reader_detection_done = true\n",
        ),
        (
            "[tui]\nanimations = false\n",
            true,
            "[tui]\nanimations = false\nscreen_reader_detection_done = true\n",
        ),
        (
            "[tui]\nscreen_reader_detection_done = false\n",
            true,
            "[tui]\nscreen_reader_detection_done = false\n",
        ),
        (
            "[tui]\nscreen_reader_detection_done = true\n",
            true,
            "[tui]\nscreen_reader_detection_done = true\n",
        ),
    ] {
        let home = tempfile::tempdir()?;
        let path = home.path().join("config.toml");
        std::fs::write(&path, input)?;
        initialize_file(
            &path,
            /*animations_configured*/ false,
            std::future::ready(detected),
        )
        .await?;
        initialize_file(
            &path,
            /*animations_configured*/ false,
            async { panic!("repeated detection") },
        )
        .await?;
        assert_eq!(std::fs::read_to_string(&path)?, expected);
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn timeout_is_recorded_without_changing_animations() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let path = home.path().join("config.toml");
    let started = tokio::time::Instant::now();
    initialize_file(
        &path,
        /*animations_configured*/ false,
        std::future::pending(),
    )
    .await?;
    assert_eq!(tokio::time::Instant::now() - started, DETECTION_TIMEOUT);
    assert_eq!(
        std::fs::read_to_string(path)?,
        "[tui]\nscreen_reader_detection_done = true\n"
    );
    Ok(())
}

#[tokio::test]
async fn preserves_preferences_from_other_layers_and_edits_during_detection() -> anyhow::Result<()>
{
    let home = tempfile::tempdir()?;
    let path = home.path().join("config.toml");
    initialize_file(
        &path,
        /*animations_configured*/ true,
        std::future::ready(true),
    )
    .await?;
    assert_eq!(
        std::fs::read_to_string(&path)?,
        "[tui]\nscreen_reader_detection_done = true\n"
    );
    std::fs::write(&path, "")?;
    initialize_file(&path, /*animations_configured*/ false, async {
        std::fs::write(
            &path,
            "# My preferences\n[tui]\nanimations = true # keep this\n",
        )
        .unwrap();
        true
    })
    .await?;
    assert_eq!(
        std::fs::read_to_string(path)?,
        "# My preferences\n[tui]\nanimations = true # keep this\nscreen_reader_detection_done = true\n"
    );
    Ok(())
}

#[tokio::test]
async fn persisted_default_loads_and_renders_without_animation() -> anyhow::Result<()> {
    use crate::app_event_sender::AppEventSender;
    use crate::legacy_core::config::ConfigBuilder;
    use crate::render::renderable::Renderable;
    use crate::status_indicator_widget::StatusIndicatorWidget;
    use crate::status_indicator_widget::StatusTimer;
    use crate::tui::FrameRequester;
    use codex_config::LoaderOverrides;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let home = tempfile::tempdir()?;
    initialize_file(
        &home.path().join("config.toml"),
        /*animations_configured*/ false,
        std::future::ready(true),
    )
    .await?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    assert!(!config.animations);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let (frame_requester, mut frames) = FrameRequester::test_channel();
    let mut status =
        StatusIndicatorWidget::new(AppEventSender::new(tx), frame_requester, config.animations);
    status.update_header("Reading the terminal output".into());
    let mut timer = StatusTimer::default();
    timer.pause_at(std::time::Instant::now());
    let mut terminal = Terminal::new(TestBackend::new(/*width*/ 60, /*height*/ 1))?;
    terminal.draw(|frame| {
        status
            .with_timer(&timer)
            .render(frame.area(), frame.buffer_mut())
    })?;
    assert_eq!(
        frames.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
    insta::assert_snapshot!(terminal.backend());
    Ok(())
}

#[tokio::test]
async fn temporary_cli_override_does_not_suppress_the_saved_default() -> anyhow::Result<()> {
    use crate::legacy_core::config::ConfigBuilder;
    use codex_config::LoaderOverrides;

    let home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .cli_overrides(vec![("tui.animations".into(), toml::Value::Boolean(true))])
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let (_, result) =
        initialize_with_probe(&config.config_layer_stack, std::future::ready(true)).await;
    result?;
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "[tui]\nanimations = false\nscreen_reader_detection_done = true\n"
    );
    Ok(())
}

#[tokio::test]
async fn detected_preference_survives_failed_persistence() -> anyhow::Result<()> {
    use crate::legacy_core::config::ConfigBuilder;
    use codex_config::LoaderOverrides;

    let home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let (mode, result) = initialize_with_probe(&config.config_layer_stack, async {
        // Make the destination unwritable after detection has started.
        std::fs::create_dir(home.path().join("config.toml")).unwrap();
        true
    })
    .await;
    assert_eq!(mode, MotionMode::Reduced);
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn malformed_edit_during_detection_does_not_expose_config_contents() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let path = home.path().join("config.toml");
    let error = initialize_file(&path, /*animations_configured*/ false, async {
        std::fs::write(&path, "token = secret-token").unwrap();
        true
    })
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "invalid TOML in user config");
    Ok(())
}
