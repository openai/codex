use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use codex_config::LoaderOverrides;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::RightClickPaste;
use codex_config::types::SessionPickerViewMode;
use codex_config::types::StatusLineSelection;
use codex_terminal_detection::Multiplexer;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn launch_screen_mode_survives_configuration_reload() -> anyhow::Result<()> {
    use crate::transcript_mode::TranscriptMode;
    use codex_config::types::AltScreenMode;

    let home = tempfile::tempdir()?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    config.tui_fullscreen_transcript = true;
    config.tui_copy_on_select = CopyOnSelect::Always;
    config.tui_copy_on_select_clear_selection = true;
    config.tui_right_click_paste = RightClickPaste::On;
    config.tui_alternate_screen = AltScreenMode::Auto;
    config.animations = true;
    config.tui_auto_recap = true;
    config.tui_effects.starfield = true;
    config.disable_paste_burst = false;
    config.show_tooltips = true;

    for (alternate_screen, owned, expected_mode, expected_alt) in [
        (true, true, TranscriptMode::Owned, AltScreenMode::Auto),
        (true, false, TranscriptMode::Terminal, AltScreenMode::Auto),
        (false, true, TranscriptMode::Terminal, AltScreenMode::Never),
    ] {
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_alt_screen_enabled(alternate_screen);
        tui.set_owned_screen(owned)?;
        let local = LocalSettings::for_tui(&config, &tui);
        assert_eq!(
            (local.transcript_mode, local.tui.alternate_screen),
            (expected_mode, expected_alt),
        );

        let mut reloaded_config = config.clone();
        reloaded_config.tui_fullscreen_transcript = false;
        reloaded_config.tui_copy_on_select = CopyOnSelect::Never;
        reloaded_config.tui_copy_on_select_clear_selection = false;
        reloaded_config.tui_right_click_paste = RightClickPaste::Off;
        reloaded_config.tui_alternate_screen = AltScreenMode::Never;
        reloaded_config.tui_theme = Some("nord".into());
        reloaded_config.animations = false;
        reloaded_config.tui_auto_recap = false;
        reloaded_config.tui_effects.starfield = false;
        reloaded_config.disable_paste_burst = true;
        reloaded_config.show_tooltips = false;
        let mut expected_reload = LocalSettings::from(&reloaded_config);
        expected_reload.transcript_mode = expected_mode;
        expected_reload.tui.alternate_screen = expected_alt;
        expected_reload.tui.animations = local.tui.animations;
        expected_reload.tui.auto_recap = local.tui.auto_recap;
        expected_reload.tui.effects.starfield = local.tui.effects.starfield;
        expected_reload.tui.disable_paste_burst = local.tui.disable_paste_burst;
        expected_reload.tui.show_tooltips = local.tui.show_tooltips;
        assert_eq!(local.reloaded(&reloaded_config), expected_reload);

        let mut expected_launch = LocalSettings::from(&reloaded_config);
        expected_launch.transcript_mode = expected_mode;
        expected_launch.tui.alternate_screen = expected_alt;
        assert_eq!(
            LocalSettings::for_tui(&reloaded_config, &tui),
            expected_launch
        );
        tui.set_owned_screen(/*owned*/ false)?;
    }
    Ok(())
}

#[tokio::test]
async fn system_motion_suppresses_animations_without_changing_saved_preferences()
-> anyhow::Result<()> {
    use crate::motion::MotionMode;

    for configured in [true, false] {
        let home = tempfile::tempdir()?;
        let config_text = format!("[tui]\nanimations = {configured}\n");
        std::fs::write(home.path().join("config.toml"), &config_text)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await?;
        let animated = LocalSettings::with_accessibility_preferences(
            &config,
            MotionMode::Animated,
            MotionMode::Animated,
        );
        let reduced = LocalSettings::with_accessibility_preferences(
            &config,
            MotionMode::Reduced,
            MotionMode::Animated,
        );
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
    for (config_text, expected_server) in [
        ("", None),
        (
            r#"
[tui]
animations = false
show_tooltips = false
show_server_version_notice = false
auto_recap = false
fullscreen_transcript = true
mouse_scroll_speed = 0.5
copy_on_select = "never"
copy_on_select_clear_selection = true
right_click_paste = "off"
vim_mode_default = true
terminal_resize_reflow_max_rows = 0
session_picker_view = "comfortable"
status_line_provider = { server = "remote.work", tool = "usage" }
[tui.default_hosts.project_a]
user_a = "remote.work"
[tui.effects]
shimmer = false
[tui.rendering]
mermaid = false
math = false
tables = false
lists = false
[history]
persistence = "none"
max_bytes = 4096
[notice]
fast_default_opt_out = true
"#,
            Some("remote.work"),
        ),
    ] {
        let home = tempfile::tempdir()?;
        std::fs::write(home.path().join("config.toml"), config_text)?;
        let project = tempfile::tempdir()?;
        let project_cwd = AbsolutePathBuf::from_absolute_path(project.path())?;
        std::fs::create_dir(project.path().join(".codex"))?;
        std::fs::write(
            project.path().join(".codex/config.toml"),
            "[tui]\ntheme = 'project-theme'\nstatus_line_provider = { server = 'project', tool = 'usage' }\n[tui.default_hosts.project_a]\nuser_a = 'project-override'\nuser_b = 'project-only'\n",
        )?;
        crate::legacy_core::config::set_project_trust_level(
            home.path(),
            project.path(),
            codex_protocol::config_types::TrustLevel::Trusted,
        )?;
        let cli_overrides = vec![
            ("tui.disable_paste_burst".into(), true.into()),
            ("tui.status_line_provider".into(), "default".into()),
            ("tui.right_click_paste".into(), "on".into()),
            ("tui.mouse_scroll_speed".into(), 1.5.into()),
            // The removed flag must not override the TUI preference.
            (
                "features.transcript_v2".into(),
                config_text.is_empty().into(),
            ),
        ];
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .fallback_cwd(Some(project.path().to_path_buf()))
            .strict_config(true)
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .cli_overrides(cli_overrides.clone())
            .build()
            .await?;
        assert_eq!(config.startup_warnings, Vec::<String>::new());
        assert_eq!(config.tui_mouse_scroll_speed, Some(1.5));
        assert!(config.tui_fullscreen_transcript);
        let bootstrap = crate::legacy_core::config::load_config_toml_with_layer_stack(
            home.path(),
            Some(&project_cwd),
            cli_overrides,
            codex_config::ConfigLoadOptions {
                loader_overrides: LoaderOverrides::without_managed_config_for_tests(),
                strict_config: true,
                ..Default::default()
            },
        )
        .await?;
        assert_eq!(
            LocalSettings::from_bootstrap(&bootstrap, config.codex_home.clone())?,
            LocalSettings::from(&config),
        );
        assert_eq!(config.features.legacy_feature_usages().count(), 1);
        assert_eq!(config.tui_theme.as_deref(), Some("project-theme"));
        assert_eq!(
            config.tui_status_line_provider.as_deref(),
            Some(&StatusLineSelection::Builtin(BuiltinStatusLine::Default)),
        );
        assert!(
            LocalSettings::from(&config)
                .tui
                .default_hosts
                .get("project_a")
                .and_then(|users| users.get("user_b"))
                .is_none()
        );
        assert_eq!(
            LocalSettings::from(&config)
                .tui
                .default_hosts
                .get("project_a")
                .and_then(|users| users.get("user_a"))
                .map(String::as_str),
            expected_server,
        );
    }
    Ok(())
}

#[tokio::test]
async fn malformed_status_line_provider_uses_project_and_user_validation_rules()
-> anyhow::Result<()> {
    use codex_config::ConfigLayerSource;

    let home = tempfile::tempdir()?;
    let project = tempfile::tempdir()?;
    let project_config = project.path().join(".codex");
    std::fs::create_dir(&project_config)?;
    let malformed = "[tui]\nstatus_line_provider = 42\n";
    std::fs::write(
        project_config.join("instructions.md"),
        "project instructions",
    )?;
    let project_file = project_config.join("config.toml");
    std::fs::write(
        &project_file,
        format!("model_instructions_file = 'instructions.md'\n{malformed}"),
    )?;
    let system_config = home.path().join("system.toml");
    std::fs::write(
        &system_config,
        "[tui]\nstatus_line_provider = { server = 'system', tool = 'usage' }\n",
    )?;
    let user_config = home.path().join("config.toml");
    std::fs::write(&user_config, "[tui]\nstatus_line_provider = 'default'\n")?;
    crate::legacy_core::config::set_project_trust_level(
        home.path(),
        project.path(),
        codex_protocol::config_types::TrustLevel::Trusted,
    )?;
    let build = |strict| {
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .fallback_cwd(Some(project.path().to_path_buf()))
            .loader_overrides(LoaderOverrides {
                system_config_path: Some(system_config.clone()),
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .strict_config(strict)
            .build()
    };
    let config = build(/*strict*/ false).await?;
    let layers = &config.config_layer_stack;
    let project_layer = layers
        .layers_low_to_high()
        .find(|layer| matches!(layer.name, ConfigLayerSource::Project { .. }))
        .expect("trusted project layer");
    assert_eq!(
        project_layer.version,
        codex_config::version_for_toml(&project_layer.config)
    );
    assert_eq!(
        project_layer.config["tui"]["status_line_provider"].as_integer(),
        Some(42)
    );
    assert_eq!(
        project_layer.config["model_instructions_file"].as_str(),
        project_config.join("instructions.md").to_str()
    );
    assert_eq!(
        layers.effective_config()["tui"]["status_line_provider"].as_str(),
        Some("default")
    );
    assert_eq!(
        layers.origins_with_path_filter(
            |path| path.starts_with(&["tui".into(), "status_line_provider".into()])
        ),
        std::collections::HashMap::from([(
            "tui.status_line_provider".into(),
            layers
                .get_active_user_layer()
                .expect("user layer")
                .metadata()
        )])
    );

    let assert_error_at = |error: std::io::Error, path: &std::path::Path, line| {
        let diagnostic = error
            .get_ref()
            .and_then(|error| error.downcast_ref::<codex_config::ConfigLoadError>())
            .expect("config diagnostic")
            .config_error();
        assert_eq!(
            (diagnostic.path.as_path(), diagnostic.range.start.line),
            (path, line)
        );
    };
    let error = build(/*strict*/ true)
        .await
        .expect_err("malformed project preference");
    assert_error_at(error, &project_file, 3);

    std::fs::write(&user_config, malformed)?;
    for strict in [false, true] {
        assert_error_at(
            build(strict).await.expect_err("malformed user preference"),
            &user_config,
            2,
        );
    }
    Ok(())
}

#[tokio::test]
async fn status_line_provider_requires_a_complete_identity_in_the_selected_profile()
-> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let profile = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    std::fs::write(
        home.path().join("config.toml"),
        "[tui]\nstatus_line_provider = { server = 'personal', tool = 'usage' }\n",
    )?;
    for partial in ["server = 'work'", "tool = 'quota'"] {
        std::fs::write(
            &profile,
            format!("[tui]\nstatus_line_provider = {{ {partial} }}\n"),
        )?;
        let result = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                user_config_path: Some(profile.clone()),
                user_config_profile: Some("work".parse()?),
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await;
        assert!(
            result.is_err(),
            "accepted an incomplete provider: {partial}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn copy_on_select_respects_terminal_defaults_and_config_overrides() -> anyhow::Result<()> {
    for (configured, launch_override, expected) in [
        (None, None, CopyOnSelect::Auto),
        (Some("auto"), None, CopyOnSelect::Auto),
        (Some("always"), None, CopyOnSelect::Always),
        (Some("never"), None, CopyOnSelect::Never),
        (Some("never"), Some("always"), CopyOnSelect::Always),
        (Some("always"), Some("never"), CopyOnSelect::Never),
        (Some("always"), Some("auto"), CopyOnSelect::Auto),
        (Some("never"), Some("auto"), CopyOnSelect::Auto),
    ] {
        let home = tempfile::tempdir()?;
        let config_path = home.path().join("config.toml");
        let config_text = configured
            .map(|mode| format!("[tui]\ncopy_on_select = \"{mode}\"\n"))
            .unwrap_or_default();
        std::fs::write(&config_path, &config_text)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .strict_config(true)
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .cli_overrides(
                launch_override
                    .map(|mode| ("tui.copy_on_select".into(), mode.into()))
                    .into_iter()
                    .collect(),
            )
            .build()
            .await?;
        let local = LocalSettings::from(&config);
        assert_eq!(
            (config.tui_copy_on_select, local.tui.copy_on_select),
            (expected, expected),
        );
        for (name, version, multiplexer, default_enabled) in [
            (TerminalName::Iterm2, None, None, true),
            (TerminalName::AppleTerminal, None, None, true),
            (TerminalName::Ghostty, Some("1.2.0"), None, false),
            (TerminalName::Ghostty, Some("1.3.0"), None, false),
            (TerminalName::Ghostty, Some("1.1.3"), None, true),
            (TerminalName::Ghostty, Some("1.2.0-dev"), None, true),
            (TerminalName::Ghostty, Some("invalid"), None, true),
            (TerminalName::Ghostty, None, None, true),
            (TerminalName::Kitty, None, None, !cfg!(target_os = "macos")),
            (TerminalName::WindowsTerminal, None, None, false),
            (
                TerminalName::VsCode,
                None,
                None,
                !cfg!(target_os = "windows"),
            ),
            (TerminalName::Alacritty, None, None, true),
            (TerminalName::GnomeTerminal, None, None, true),
            (TerminalName::Konsole, None, None, true),
            (TerminalName::Vte, None, None, true),
            (TerminalName::WarpTerminal, None, None, true),
            (TerminalName::WezTerm, None, None, true),
            (TerminalName::Dumb, None, None, true),
            (TerminalName::Unknown, None, None, true),
            (
                TerminalName::Ghostty,
                Some("1.3.0"),
                Some(Multiplexer::Tmux { version: None }),
                true,
            ),
            (
                TerminalName::Kitty,
                None,
                Some(Multiplexer::Zellij { version: None }),
                true,
            ),
            (
                TerminalName::WindowsTerminal,
                None,
                Some(Multiplexer::Tmux { version: None }),
                true,
            ),
        ] {
            let terminal = TerminalInfo {
                backend: None,
                name,
                multiplexer,
                term_program: None,
                version: version.map(str::to_owned),
                term: None,
            };
            assert_eq!(
                local.copy_on_select(&terminal),
                match expected {
                    CopyOnSelect::Auto => default_enabled,
                    CopyOnSelect::Always => true,
                    CopyOnSelect::Never => false,
                },
                "terminal={terminal:?}, override={expected:?}",
            );
        }
        assert_eq!(std::fs::read_to_string(config_path)?, config_text);
    }
    Ok(())
}

#[tokio::test]
async fn local_writes_preserve_selected_user_file_and_home_destinations() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "[tui]\nnotifications = [\"agent-turn-complete\"]\n",
    )?;
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
    assert!(local.custom_notification_filters);
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

#[tokio::test]
async fn screen_reader_default_yields_to_preferences_on_reload() -> anyhow::Result<()> {
    use crate::motion::MotionMode;

    let home = tempfile::tempdir()?;
    for (config_text, expected) in [
        ("", false),
        ("[tui]\nanimations = true\n", true),
        ("[tui]\nanimations = false\n", false),
    ] {
        std::fs::write(home.path().join("config.toml"), config_text)?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            })
            .build()
            .await?;
        let local = LocalSettings::with_accessibility_preferences(
            &config,
            MotionMode::Animated,
            MotionMode::Reduced,
        );
        assert_eq!(local.tui.animations, expected);
    }
    Ok(())
}
