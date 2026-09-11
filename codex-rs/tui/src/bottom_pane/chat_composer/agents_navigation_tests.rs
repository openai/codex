use super::tests::new_test_composer;
use super::tests::snapshot_composer_state_with_width;
use super::tests::type_chars_humanlike;
use super::*;
use assert_matches::assert_matches;
use pretty_assertions::assert_eq;

#[test]
fn parent_owned_thread_allows_bare_navigation_commands() {
    for (command, expected) in [
        ("/agents", SlashCommand::Agents),
        ("/subagents", SlashCommand::MultiAgents),
        ("/side", SlashCommand::Side),
        ("/btw", SlashCommand::Btw),
        ("/diff ", SlashCommand::Diff),
    ] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_parent_owned_thread();
        composer.set_text_content(command.to_string(), Vec::new(), Vec::new());

        assert_eq!(
            composer.handle_submission(/*should_queue*/ false).0,
            InputResult::Command(expected)
        );
    }
}

#[test]
fn parent_owned_thread_allows_safe_command_selected_from_prefix() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_parent_owned_thread();
    type_chars_humanlike(&mut composer, &['/', 'a', 'g']);

    let result = composer
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .0;

    assert_eq!(result, InputResult::Command(SlashCommand::Agents));
}

#[test]
fn left_navigates_only_from_an_empty_prompt() {
    let (mut composer, mut events) = new_test_composer();
    composer.set_agents_navigation_enabled(/*enabled*/ true);
    composer.insert_str("ab");
    composer.handle_key_event(KeyCode::Left.into());
    composer.insert_str("!");
    assert_eq!(composer.current_text(), "a!b");
    assert!(events.try_recv().is_err());

    composer.clear_for_ctrl_c();
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        composer.handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            kind,
        ));
    }
    assert!(events.try_recv().is_err());
    composer.handle_key_event(KeyCode::Left.into());
    assert_matches!(events.try_recv(), Ok(AppEvent::OpenAgentsOverview));
}

#[test]
fn left_respects_attachments_pastes_and_other_input_surfaces() {
    let setups: [fn(&mut ChatComposer); 4] = [
        |composer| composer.config = ChatComposerConfig::plain_text(),
        |composer| composer.set_remote_image_urls(vec!["https://example.com/image.png".into()]),
        |composer| {
            composer.handle_key_event(KeyCode::Char('x').into());
        },
        |composer| {
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        },
    ];
    for setup in setups {
        let (mut composer, mut events) = new_test_composer();
        composer.set_agents_navigation_enabled(/*enabled*/ true);
        setup(&mut composer);
        composer.handle_key_event(KeyCode::Left.into());
        assert!(
            !std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::OpenAgentsOverview))
        );
    }
}

#[test]
fn agents_navigation_hint_snapshots() {
    for name in [
        "agents_navigation_enabled",
        "agents_navigation_help",
        "agents_navigation_status",
    ] {
        snapshot_composer_state_with_width(
            name,
            /*width*/ 80,
            /*enhanced_keys_supported*/ false,
            |composer| {
                composer.set_agents_navigation_enabled(/*enabled*/ true);
                match name {
                    "agents_navigation_help" => {
                        composer.handle_key_event(KeyCode::Char('?').into());
                    }
                    "agents_navigation_status" => {
                        composer.set_status_line_enabled(/*enabled*/ true);
                        composer.set_status_line(Some(Line::from("model · project")));
                    }
                    _ => {}
                }
            },
        );
    }
}

#[test]
fn left_respects_editor_and_vim_history_remaps() {
    use codex_config::types::KeybindingSpec;
    use codex_config::types::KeybindingsSpec;
    use codex_config::types::TuiKeymap;

    for vim in [false, true] {
        let (mut composer, mut events) = new_test_composer();
        composer.set_agents_navigation_enabled(/*enabled*/ true);
        composer.insert_str("previous");
        composer.handle_key_event(KeyCode::Enter.into());
        let mut config = TuiKeymap::default();
        if vim {
            config.vim_normal.move_left = Some(KeybindingsSpec::One(KeybindingSpec("h".into())));
            config.vim_normal.move_up = Some(KeybindingsSpec::One(KeybindingSpec("left".into())));
        } else {
            config.editor.move_left = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-b".into())));
            config.editor.move_up = Some(KeybindingsSpec::One(KeybindingSpec("left".into())));
        }
        composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
        composer.set_vim_enabled(vim);
        assert!(composer.footer_props().key_hints.agents.is_none());
        composer.handle_key_event(KeyCode::Left.into());
        assert_eq!(composer.current_text(), "previous");
        assert!(events.try_recv().is_err());
    }
}
