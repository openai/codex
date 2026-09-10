use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use codex_config::LoaderOverrides;
use codex_config::types::SessionPickerViewMode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn system_reduced_motion_renders_astra_composer_without_sparkles() -> anyhow::Result<()> {
    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::BottomPane;
    use crate::bottom_pane::BottomPaneParams;
    use crate::motion::MotionMode;
    use crate::render::renderable::Renderable;
    use crate::tui::FrameRequester;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides {
            ignore_project_config: true,
            ..LoaderOverrides::without_managed_config_for_tests()
        })
        .build()
        .await?;
    let settings = LocalSettings::with_system_motion(&config, MotionMode::Reduced);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: AppEventSender::new(tx),
        frame_requester: FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".into(),
        disable_paste_burst: true,
        animations_enabled: settings.tui.animations,
        skills: None,
    });
    pane.set_astra_sparkle("astra", &settings.tui);
    pane.set_composer_text("Explore the night sky".into(), Vec::new(), Vec::new());
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            for width in [40, 80] {
                let area = Rect::new(
                    /*x*/ 0,
                    /*y*/ 0,
                    width,
                    pane.desired_height(width),
                );
                let mut buffer = Buffer::empty(area);
                pane.render(area, &mut buffer);
                let rows = buffer
                    .content
                    .chunks(usize::from(width))
                    .map(|row| {
                        row.iter()
                            .map(ratatui::buffer::Cell::symbol)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                insta::assert_snapshot!(format!("system_reduced_motion_astra_{width}"), rows);
            }
        },
    );
    Ok(())
}

#[tokio::test]
async fn system_motion_suppresses_animations_without_changing_saved_preferences()
-> anyhow::Result<()> {
    use crate::motion::MotionMode;

    for configured in [true, false] {
        let home = tempfile::tempdir()?;
        let config_text = format!("[tui]\nanimations = {configured}\nwhimsy = true\n");
        std::fs::write(home.path().join("config.toml"), &config_text)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await?;
        let animated = LocalSettings::with_system_motion(&config, MotionMode::Animated);
        let reduced = LocalSettings::with_system_motion(&config, MotionMode::Reduced);
        let mut expected = animated.clone();
        expected.tui.animations = false;
        assert_eq!(reduced, expected);
        assert_eq!(animated.tui.animations, configured);
        assert_eq!(config.animations, configured);
        assert_eq!(
            std::fs::read_to_string(home.path().join("config.toml"))?,
            config_text
        );
    }
    Ok(())
}

#[tokio::test]
async fn local_load_preserves_defaults_and_resolved_overrides() -> anyhow::Result<()> {
    for config_text in [
        "",
        r#"
[tui]
animations = false
whimsy = false
show_tooltips = false
show_server_version_notice = false
auto_recap = false
vim_mode_default = true
terminal_resize_reflow_max_rows = 0
session_picker_view = "comfortable"
[history]
persistence = "none"
max_bytes = 4096
[notice]
fast_default_opt_out = true
"#,
    ] {
        let home = tempfile::tempdir()?;
        std::fs::write(home.path().join("config.toml"), config_text)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .cli_overrides(vec![("tui.disable_paste_burst".into(), true.into())])
            .build()
            .await?;
        let local = LocalSettings::from(&config);
        let mut expected: Tui = toml::from_str("")?;
        expected.disable_paste_burst = Some(true);
        expected.session_picker_view = Some(SessionPickerViewMode::Dense);
        if !config_text.is_empty() {
            expected.animations = false;
            expected.whimsy = false;
            expected.show_tooltips = false;
            expected.show_server_version_notice = false;
            expected.auto_recap = false;
            expected.vim_mode_default = true;
            expected.terminal_resize_reflow_max_rows = Some(0);
            expected.session_picker_view = Some(SessionPickerViewMode::Comfortable);
        }
        assert_eq!(local.tui, expected);
        assert_eq!(
            local.terminal_resize_reflow(),
            config.terminal_resize_reflow
        );
        assert_eq!(
            (&local.history, &local.notices),
            (&config.history, &config.notices)
        );
    }
    Ok(())
}

#[tokio::test]
async fn local_writes_preserve_selected_user_file_and_home_destinations() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let selected = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    std::fs::write(&selected, "[tui]\ntheme = \"dracula\"\n")?;
    let overrides = LoaderOverrides {
        user_config_path: Some(selected.clone()),
        user_config_profile: Some("work".parse()?),
        ignore_project_config: true,
        ..LoaderOverrides::without_managed_config_for_tests()
    };
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides.clone())
        .build()
        .await?;
    let local = LocalSettings::from(&config);
    assert_eq!(local.user_config_path, selected);
    ConfigEditsBuilder::for_config_path(local.user_config_path.as_path())
        .with_edits([crate::legacy_core::config::edit::syntax_theme_edit("nord")])
        .apply()
        .await?;
    ConfigEditsBuilder::new(local.codex_home.as_path())
        .set_session_picker_view(SessionPickerViewMode::Comfortable)
        .apply()
        .await?;
    let reloaded = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides)
        .build()
        .await?;
    assert_eq!(
        LocalSettings::from(&reloaded).tui.theme.as_deref(),
        Some("nord")
    );
    let home_config: toml::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml"))?)?;
    assert_eq!(
        home_config["tui"]["session_picker_view"].as_str(),
        Some("comfortable")
    );
    assert_eq!(home_config["tui"].get("theme"), None);
    Ok(())
}
