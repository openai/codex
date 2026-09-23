//! Composer undo/redo transactions preserve rich draft data and active Vim commands.

use std::path::PathBuf;

use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

use super::super::super::chat_composer_history::HistoryEntry;
use super::super::ChatComposer;
use super::super::InputResult;
use super::super::LARGE_PASTE_CHAR_THRESHOLD;
use super::super::PasteBurst;
use super::super::tests::new_test_composer;
use super::super::tests::snapshot_composer_state_with_width;
use crate::keymap::RuntimeKeymap;
use crate::slash_command::SlashCommand;

fn vim_composer(text: &str) -> ChatComposer {
    let (mut composer, _receiver) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_text_content(text.to_owned(), Vec::new(), Vec::new());
    composer.set_vim_enabled(/*enabled*/ true);
    composer.draft.textarea.set_cursor(/*pos*/ 0);
    composer
}

fn keys(composer: &mut ChatComposer, keys: &str) {
    for key in keys.chars() {
        let code = if key == '\n' {
            KeyCode::Enter
        } else {
            KeyCode::Char(key)
        };
        let _ = composer.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

fn escape(composer: &mut ChatComposer) {
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

fn ctrl_r(composer: &mut ChatComposer) {
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
}

#[test]
fn undo_and_redo_group_complete_vim_edits() {
    for (original, command, edited) in [
        ("one two", "cwX", "X two"),
        ("abc", "iXYZ", "XYZabc"),
        ("", "i/review", "/review"),
        ("one two", "c/two\nX", "Xtwo"),
        ("one two", "d/two\n", "two"),
        ("abc-def", "df-", "def"),
        ("abc-def", "cf-X", "Xdef"),
    ] {
        let mut composer = vim_composer(original);
        keys(&mut composer, command);
        if composer.popups.active() {
            escape(&mut composer);
        }
        escape(&mut composer);
        assert_eq!(composer.current_text(), edited);
        assert_eq!(composer.vim_history.undo.len(), 1);

        keys(&mut composer, "u");
        assert_eq!(
            (composer.current_text(), composer.cursor()),
            (original.to_owned(), 0)
        );
        ctrl_r(&mut composer);
        assert_eq!(composer.current_text(), edited);
    }
}

#[test]
fn recovered_answers_undo_as_one_vim_edit() {
    for answer in [
        "answer".to_owned(),
        "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1),
    ] {
        let mut composer = vim_composer("abc");
        keys(&mut composer, "iX");
        escape(&mut composer);
        let before = composer.draft_snapshot();
        composer.append_recovered_drafts(&answer);
        keys(&mut composer, "u");
        assert_eq!(composer.draft_snapshot(), before);
        keys(&mut composer, ".");
        assert_eq!(composer.current_text(), "XXabc");

        for (command, mode, edited) in [("iX", "Insert", "Xabc"), ("RX", "Replace", "Xbc")] {
            let mut composer = vim_composer("abc");
            let before = composer.draft_snapshot();
            keys(&mut composer, command);
            composer.append_recovered_drafts(&answer);
            assert_eq!(
                (
                    composer.current_text_with_pending(),
                    composer.draft.textarea.vim_mode_label(),
                ),
                (format!("{edited}\n{answer}"), Some(mode))
            );
            escape(&mut composer);
            let after = composer.draft_snapshot();
            keys(&mut composer, "u");
            assert_eq!(composer.draft_snapshot(), before);
            ctrl_r(&mut composer);
            assert_eq!(composer.draft_snapshot(), after);
            keys(&mut composer, "u");
            keys(&mut composer, ".");
            assert_eq!(composer.current_text(), edited);
        }
        for command in ["", "d", "/draft", "?draft", "d/draft", "c?draft", "y/draft"] {
            let mut composer = vim_composer("existing draft");
            let before = composer.draft_snapshot();
            keys(&mut composer, command);
            composer.append_recovered_drafts(&answer);
            assert_eq!(
                (
                    composer.current_text_with_pending(),
                    composer.draft.textarea.vim_query().is_some(),
                    composer.draft.textarea.is_vim_operator_pending(),
                ),
                (format!("existing draft\n{answer}"), false, false)
            );
            let after = composer.draft_snapshot();
            keys(&mut composer, "u");
            assert_eq!(composer.draft_snapshot(), before);
            ctrl_r(&mut composer);
            assert_eq!(composer.draft_snapshot(), after);
        }

        let mut composer = vim_composer("abc");
        composer
            .history
            .record_local_submission(HistoryEntry::new("archived prompt".into()));
        keys(&mut composer, "x");
        let before = composer.draft_snapshot();
        keys(&mut composer, "d");
        ctrl_r(&mut composer);
        keys(&mut composer, "archive");
        composer.edit_stored_draft(|composer| composer.append_recovered_drafts(&answer));
        assert_eq!(composer.current_text(), "archived prompt");
        assert!(composer.history_search_active());
        escape(&mut composer);
        assert_eq!(
            composer.current_text_with_pending(),
            format!("bc\n{answer}")
        );
        let after = composer.draft_snapshot();
        keys(&mut composer, "u");
        assert_eq!(composer.draft_snapshot(), before);
        ctrl_r(&mut composer);
        assert_eq!(composer.draft_snapshot(), after);
    }
    snapshot_composer_state_with_width(
        "recovered_answer_cancels_vim_search",
        /*width*/ 40,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_disable_paste_burst(/*disabled*/ true);
            composer.set_text_content("existing draft".into(), Vec::new(), Vec::new());
            composer.set_vim_enabled(/*enabled*/ true);
            keys(composer, "d/draft");
            composer.append_recovered_drafts("answer");
        },
    );
}

#[test]
fn undo_preserves_repeat_and_pending_operator_bindings() {
    let mut composer = vim_composer("alpha beta gamma");
    keys(&mut composer, "dw.u");
    assert_eq!(composer.current_text(), "beta gamma");
    keys(&mut composer, "u.");
    assert_eq!(composer.current_text(), "beta gamma");

    let mut composer = vim_composer("one two");
    let mut keymap = RuntimeKeymap::defaults();
    keymap.vim_operator.motion_word_forward = vec![crate::key_hint::plain(KeyCode::Char('u'))];
    composer.set_keymap_bindings(&keymap);
    keys(&mut composer, "duu");
    assert_eq!(composer.current_text(), "one two");
}

#[test]
fn undo_and_redo_preserve_confirmed_vim_search() {
    let mut composer = vim_composer("alpha beta alpha");
    keys(&mut composer, "/alpha\n0xun");
    assert_eq!(
        (composer.current_text(), composer.cursor()),
        ("alpha beta alpha".to_owned(), 11)
    );

    keys(&mut composer, "0");
    composer.handle_paste("Z".to_owned());
    keys(&mut composer, "n");
    assert_eq!(
        (composer.current_text(), composer.cursor()),
        ("Zalpha beta alpha".to_owned(), 12)
    );
    keys(&mut composer, "u");
    ctrl_r(&mut composer);
    keys(&mut composer, "n");
    assert_eq!(
        (composer.current_text(), composer.cursor()),
        ("Zalpha beta alpha".to_owned(), 1)
    );
}

#[test]
fn externally_canceled_search_operator_finishes_the_undo_transaction() {
    let mut composer = vim_composer("abc");
    keys(&mut composer, "xd/");
    assert!(composer.cancel_vim_search());
    keys(&mut composer, "uxu");
    assert_eq!(composer.current_text(), "abc");
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "bc");
}

#[test]
fn normal_mode_explicit_paste_is_its_own_edit_and_replaces_stale_redo() {
    let mut composer = vim_composer("abc");
    keys(&mut composer, "x");
    composer.handle_paste("Z".to_owned());
    assert_eq!(composer.current_text(), "Zbc");
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "bc");
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "abc");

    composer.handle_paste("Z".to_owned());
    assert_eq!(composer.current_text(), "Zabc");
    assert!(composer.vim_history.redo.is_empty());
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "Zabc");
    assert!(!composer.history_search_active());
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "abc");
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "Zabc");
}

#[test]
fn normal_mode_direct_insert_and_image_attachment_are_undoable() {
    let mut composer = vim_composer("abc");
    keys(&mut composer, "xu");
    composer.insert_str("Z");
    assert_eq!(composer.current_text(), "Zabc");
    assert!(composer.vim_history.redo.is_empty());
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "abc");
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "Zabc");
    keys(&mut composer, "u");

    let path = PathBuf::from("example.png");
    composer.attach_image(path.clone());
    let with_image = composer.draft_snapshot();
    assert_eq!(composer.local_image_paths(), vec![path]);
    assert!(composer.vim_history.redo.is_empty());
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "abc");
    assert!(composer.local_image_paths().is_empty());
    ctrl_r(&mut composer);
    assert_eq!(composer.draft_snapshot(), with_image);
    keys(&mut composer, "u\nABC");
    assert_eq!(composer.current_text(), "ABC");
    escape(&mut composer);
    keys(&mut composer, "u");
    assert!(composer.is_empty());
    keys(&mut composer, "u");
    assert!(composer.is_empty());
}

#[test]
fn deleting_a_selected_remote_image_is_one_undoable_vim_edit() {
    for key in [KeyCode::Delete, KeyCode::Backspace] {
        let mut composer = vim_composer("abc");
        let url = "https://example.com/remote.png".to_owned();
        composer.set_remote_image_urls(vec![url.clone()]);
        keys(&mut composer, "xu");
        assert_eq!(composer.vim_history.redo.len(), 1);
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert!(composer.vim_history.pending.is_none());
        let _ = composer.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
        assert!(composer.remote_image_urls().is_empty());
        assert!(composer.vim_history.redo.is_empty());
        keys(&mut composer, "u");
        assert_eq!(composer.remote_image_urls(), vec![url]);
        assert_eq!(composer.current_text(), "abc");
        ctrl_r(&mut composer);
        assert!(composer.remote_image_urls().is_empty());
        assert_eq!(composer.current_text(), "abc");
    }
}

#[test]
fn direct_edits_join_the_active_vim_insert_transaction() {
    let mut composer = vim_composer("abc");
    composer.enable_vim_in_insert_mode();
    composer.handle_paste("ABC".to_owned());
    composer.attach_image(PathBuf::from("example.png"));
    keys(&mut composer, "D");
    escape(&mut composer);
    let edited = composer.draft_snapshot();
    assert_eq!(composer.vim_history.undo.len(), 1);
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "abc");
    assert!(composer.local_image_paths().is_empty());
    ctrl_r(&mut composer);
    assert_eq!(composer.draft_snapshot(), edited);
}

#[test]
fn vim_search_query_paste_does_not_invalidate_draft_redo() {
    let mut composer = vim_composer("alpha beta alpha");
    keys(&mut composer, "xu/");
    composer.handle_paste("beta".to_owned());
    assert_eq!(composer.current_text(), "alpha beta alpha");
    assert_eq!(
        composer.draft.textarea.vim_query().unwrap().editor.text(),
        "beta"
    );
    assert!(composer.vim_history.pending.is_none());

    escape(&mut composer);
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "lpha beta alpha");
}

#[test]
fn vim_redo_keeps_its_binding_when_history_is_empty() {
    let mut composer = vim_composer("abc");
    let before = composer.draft_snapshot();
    ctrl_r(&mut composer);
    assert_eq!(composer.draft_snapshot(), before);
    assert!(!composer.history_search_active());
    keys(&mut composer, "xu");
    assert_eq!(composer.current_text(), "abc");
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "bc");
    assert!(!composer.history_search_active());
    let after = composer.draft_snapshot();
    ctrl_r(&mut composer);
    assert_eq!(composer.draft_snapshot(), after);
    assert!(composer.draft.textarea.is_vim_normal_mode());
    assert!(!composer.history_search_active());
    super::super::tests::snapshot_composer_state_with_width(
        "vim_empty_redo_stays_normal",
        /*width*/ 60,
        /*enhanced_keys_supported*/ true,
        |target| *target = composer,
    );

    let original = "a".repeat(super::MAX_VIM_UNDO_BYTES / 2 + 1);
    let mut composer = vim_composer(&original);
    keys(&mut composer, "xu");
    for command in ["d", "i"] {
        keys(&mut composer, command);
        escape(&mut composer);
        assert_eq!(composer.vim_history.redo.len(), 1);
    }
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), original[1..]);
    assert!(!composer.history_search_active());

    keys(&mut composer, "uiZ");
    escape(&mut composer);
    assert!(composer.vim_history.redo.is_empty());
    keys(&mut composer, "x");
    assert_eq!(composer.vim_history.undo.len(), 1);
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), format!("Z{original}"));
}

#[test]
fn ctrl_r_searches_history_in_insert_and_non_vim_modes() {
    let mut composer = vim_composer("draft");
    keys(&mut composer, "i");
    ctrl_r(&mut composer);
    assert!(composer.history_search_active());
    escape(&mut composer);
    assert_eq!(composer.draft.textarea.vim_mode_label(), Some("Insert"));

    composer.set_vim_enabled(/*enabled*/ false);
    ctrl_r(&mut composer);
    assert!(composer.history_search_active());
}

#[test]
fn removing_the_shell_prefix_is_its_own_undoable_edit() {
    let mut composer = vim_composer("");
    keys(&mut composer, "!foo");
    escape(&mut composer);
    keys(&mut composer, "0");
    let before = composer.draft_snapshot();
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(composer.current_text(), "foo");
    keys(&mut composer, "u");
    assert_eq!(composer.draft_snapshot(), before);
}

#[test]
fn canceled_reverse_history_preview_preserves_vim_undo_and_repeat() {
    let mut composer = vim_composer("alpha beta alpha");
    let mut keymap = RuntimeKeymap::defaults();
    keymap.vim_normal.redo.clear();
    composer.set_keymap_bindings(&keymap);
    composer
        .history
        .record_local_submission(HistoryEntry::new("archived prompt".to_owned()));
    keys(&mut composer, "/alpha\n0x");
    ctrl_r(&mut composer);
    keys(&mut composer, "archive");
    assert_eq!(composer.current_text(), "archived prompt");
    escape(&mut composer);
    assert_eq!(composer.current_text(), "lpha beta alpha");
    keys(&mut composer, "u");
    assert_eq!(composer.current_text(), "alpha beta alpha");
    keys(&mut composer, ".");
    assert_eq!(composer.current_text(), "lpha beta alpha");
    keys(&mut composer, "n");
    assert_eq!(composer.cursor(), 10);
}

#[test]
fn canceled_history_preview_preserves_active_vim_commands() {
    for query in ["", "archive", "missing"] {
        for (command, edited) in [
            ("iX", "XYalpha beta"),
            ("cwX", "XY beta"),
            ("RX", "XYpha beta"),
        ] {
            let mut composer = vim_composer("alpha beta");
            composer
                .history
                .record_local_submission(HistoryEntry::new("archived prompt".to_owned()));
            keys(&mut composer, command);
            ctrl_r(&mut composer);
            keys(&mut composer, query);
            escape(&mut composer);
            if command == "RX" {
                let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
                assert_eq!(composer.current_text(), "alpha beta");
                keys(&mut composer, "X");
            }
            keys(&mut composer, "Y");
            escape(&mut composer);
            assert_eq!(composer.current_text(), edited);
            keys(&mut composer, "u.");
            assert_eq!(composer.current_text(), edited);
        }
    }
}

#[test]
fn accepted_reverse_history_preview_starts_a_fresh_vim_edit_history() {
    let mut composer = vim_composer("abc");
    composer
        .history
        .record_local_submission(HistoryEntry::new("archived prompt".to_owned()));
    keys(&mut composer, "xiX");
    ctrl_r(&mut composer);
    keys(&mut composer, "archive\n");
    assert_eq!(composer.current_text(), "archived prompt");
    assert!(!composer.history_search_active());
    escape(&mut composer);
    keys(&mut composer, "u.");
    assert_eq!(composer.current_text(), "archived prompt");
}

#[test]
fn undo_and_redo_use_configured_bindings_and_support_unbinding() {
    let mut composer = vim_composer("abc");
    let mut keymap = RuntimeKeymap::defaults();
    keymap.vim_normal.undo = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    keymap.vim_normal.redo = vec![crate::key_hint::plain(KeyCode::Char('v'))];
    composer.set_keymap_bindings(&keymap);
    keys(&mut composer, "xu");
    assert_eq!(composer.current_text(), "bc");
    keys(&mut composer, "z");
    assert_eq!(composer.current_text(), "abc");
    keys(&mut composer, "v");
    assert_eq!(composer.current_text(), "bc");
    keys(&mut composer, "z");
    keymap.vim_normal.undo.clear();
    keymap.vim_normal.redo.clear();
    composer.set_keymap_bindings(&keymap);
    keys(&mut composer, "xzv");
    assert_eq!(composer.current_text(), "bc");

    let mut config = TuiKeymap::default();
    config.composer.history_search_previous =
        Some(KeybindingsSpec::One(KeybindingSpec("u".to_owned())));
    composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    keys(&mut composer, "u");
    assert!(composer.history_search_active());
    escape(&mut composer);
    config.vim_normal.undo = Some(KeybindingsSpec::One(KeybindingSpec("u".to_owned())));
    composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    keys(&mut composer, "u");
    assert!(!composer.history_search_active());

    let mut composer = vim_composer("abc");
    keys(&mut composer, "xu");
    let mut config = TuiKeymap::default();
    config.composer.history_search_previous =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-r".to_owned())));
    composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    ctrl_r(&mut composer);
    assert!(composer.history_search_active());
    escape(&mut composer);
    config.vim_normal.redo = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-r".to_owned())));
    composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    ctrl_r(&mut composer);
    assert!(!composer.history_search_active());
    assert_eq!(composer.current_text(), "bc");
    ctrl_r(&mut composer);
    assert!(!composer.history_search_active());

    let mut composer = vim_composer("abc");
    let mut config = TuiKeymap::default();
    config.composer.submit = Some(KeybindingsSpec::One(KeybindingSpec("u".to_owned())));
    composer.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    assert_eq!(
        composer
            .handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))
            .0,
        InputResult::Submitted {
            text: "abc".to_owned(),
            text_elements: Vec::new()
        }
    );
}

#[test]
fn undo_and_redo_restore_deleted_large_paste_payload() {
    let mut composer = vim_composer("alpha ");
    composer.draft.textarea.set_cursor("alpha ".len());
    keys(&mut composer, "i");
    let pasted = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);
    composer.handle_paste(pasted.clone());
    escape(&mut composer);
    let before = composer.draft_snapshot();
    keys(&mut composer, "x");
    assert_eq!(composer.current_text(), "alpha ");
    assert!(composer.draft.pending_pastes.is_empty());
    keys(&mut composer, "u");
    assert_eq!(composer.draft_snapshot(), before);
    assert_eq!(
        composer.current_text_with_pending(),
        format!("alpha {pasted}")
    );
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "alpha ");
    assert!(composer.draft.pending_pastes.is_empty());
}

#[test]
fn undo_and_redo_restore_deleted_image_attachment() {
    let mut composer = vim_composer("alpha ");
    composer.draft.textarea.set_cursor("alpha ".len());
    composer.attach_image(PathBuf::from("example.png"));
    composer.draft.textarea.set_cursor("alpha ".len());
    let before = composer.draft_snapshot();
    keys(&mut composer, "RX");
    ctrl_r(&mut composer);
    escape(&mut composer);
    let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(composer.draft_snapshot(), before);
    escape(&mut composer);
    composer.draft.textarea.set_cursor("alpha ".len());
    keys(&mut composer, "x");
    assert_eq!(composer.current_text(), "alpha ");
    assert!(composer.local_image_paths().is_empty());
    keys(&mut composer, "u");
    assert_eq!(composer.draft_snapshot(), before);
    ctrl_r(&mut composer);
    assert_eq!(composer.current_text(), "alpha ");
    assert!(composer.local_image_paths().is_empty());
}

#[test]
fn find_jump_and_search_navigation_never_snapshot_the_draft() {
    let mut composer = vim_composer("alpha beta\nalpha gamma");
    for key in ['f', 'F', 'G', '/', '?', 'n', 'N'] {
        composer.begin_vim_edit(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
        assert!(
            composer.vim_history.pending.is_none(),
            "navigation binding {key:?} snapshotted the draft"
        );
    }
    let (code, modifiers) = composer
        .vim_normal_keymap
        .jump_top
        .first()
        .expect("native jump-top chord")
        .parts();
    composer.begin_vim_edit(KeyEvent::new(code, modifiers));
    assert!(composer.vim_history.pending.is_none());

    for command in ["llwbh0$", "fb", "Fa", "G", "/beta\nnN", "?alpha\n"] {
        let text = "alpha beta\nalpha gamma";
        let mut composer = vim_composer(text);
        for key in command.chars() {
            keys(&mut composer, &key.to_string());
            assert!(
                composer.vim_history.pending.is_none(),
                "navigation command {command:?} snapshotted the draft"
            );
            assert!(composer.vim_history.undo.is_empty());
        }
        assert_eq!(composer.current_text(), text);
    }
    let mut keymap = RuntimeKeymap::defaults();
    keymap.vim_normal.redo.clear();
    composer.set_keymap_bindings(&keymap);
    composer
        .history
        .record_local_submission(HistoryEntry::new("alpha archived".to_owned()));
    ctrl_r(&mut composer);
    keys(&mut composer, "alpha");
    assert_eq!(composer.current_text(), "alpha archived");
    assert!(composer.vim_history.pending.is_none());
    assert!(composer.vim_history.undo.is_empty());
    escape(&mut composer);
    assert_eq!(composer.current_text(), "alpha beta\nalpha gamma");
}

#[test]
fn vim_replace_preserves_slash_completion_and_dispatch() {
    let mut composer = vim_composer("");
    keys(&mut composer, "R/dif");
    let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Tab));
    assert_eq!(composer.current_text(), "/diff ");
    let (result, _) = composer.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_eq!(result, InputResult::Command(SlashCommand::Diff));
}

#[test]
fn vim_replace_backspace_restores_characters_in_new_slash_element() {
    snapshot_composer_state_with_width(
        "vim_replace_slash_recovery",
        /*width*/ 100,
        /*enhanced_keys_supported*/ true,
        |composer| {
            composer.set_disable_paste_burst(/*disabled*/ true);
            composer.set_text_content("abcde tail".into(), Vec::new(), Vec::new());
            composer.set_vim_enabled(/*enabled*/ true);
            composer.draft.textarea.set_cursor(/*pos*/ 0);
            keys(composer, "R/diff");
            assert_eq!(composer.draft.textarea.element_payloads(), vec!["/diff"]);
            for expected in [("/dife tail", 4), ("/dide tail", 3)] {
                let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
                assert_eq!(
                    (composer.current_text(), composer.current_cursor()),
                    (expected.0.to_string(), expected.1)
                );
            }
        },
    );
}

#[test]
fn vim_editing_modes_use_the_same_large_and_image_paste_handling() {
    let temporary = tempfile::tempdir().expect("create image directory");
    let path = temporary.path().join("image.png");
    image::RgbaImage::new(/*width*/ 1, /*height*/ 1)
        .save(&path)
        .expect("write image");
    for pasted in [
        "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1),
        path.to_string_lossy().into_owned(),
    ] {
        let mut insert = vim_composer("next");
        keys(&mut insert, "i");
        insert.handle_paste(pasted.clone());
        let mut replace = vim_composer("next");
        keys(&mut replace, "R");
        replace.handle_paste(pasted);
        assert_eq!(replace.draft_snapshot(), insert.draft_snapshot());
    }
}

#[test]
fn vim_replace_paste_bursts_preserve_overwritten_text_and_repeat() {
    for pasted in ["XYZ", "日\u{3000}語"] {
        let mut composer = vim_composer("abcd");
        keys(&mut composer, "R");
        composer.set_disable_paste_burst(/*disabled*/ false);
        let now = std::time::Instant::now();
        for ch in pasted.chars() {
            let _ = composer.handle_input_basic_with_time(KeyEvent::from(KeyCode::Char(ch)), now);
        }
        assert!(composer.is_in_paste_burst());
        composer.handle_paste_burst_flush(now + PasteBurst::recommended_active_flush_delay());
        assert_eq!(composer.current_text(), format!("{pasted}d"));
        escape(&mut composer);
        keys(&mut composer, "u.");
        assert_eq!(composer.current_text(), format!("{pasted}d"));
    }
}

#[test]
fn vim_replace_completion_preserves_suffix() {
    snapshot_composer_state_with_width(
        "vim_replace_completion_suffix",
        /*width*/ 100,
        /*enhanced_keys_supported*/ true,
        |composer| {
            composer.set_disable_paste_burst(/*disabled*/ true);
            for (text, cursor, command, path) in [
                ("@ma next", 3, "R", "src/main.rs"),
                ("abc next", 0, "R@ma", "ma"),
            ] {
                composer.set_vim_enabled(/*enabled*/ true);
                composer.set_text_content(text.into(), Vec::new(), Vec::new());
                composer.draft.textarea.set_cursor(cursor);
                keys(composer, command);
                composer.insert_selected_file_path(0.."@ma".len(), path);
                composer.sync_popups();
                assert_eq!(
                    (composer.current_text(), composer.current_cursor()),
                    (format!("{path}  next"), path.len() + 1)
                );
                let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
                assert_eq!(
                    (composer.current_text(), composer.current_cursor()),
                    (format!("{path} next"), path.len())
                );
            }
        },
    );
}

#[test]
fn external_editor_import_preserves_replace_backspace_recovery() {
    let mut composer = vim_composer("old");
    keys(&mut composer, "RX");
    composer.apply_external_edit("abc".into());
    composer.draft.textarea.set_cursor(/*pos*/ 1);
    let _ = composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(composer.current_text(), "bc");
    for code in [KeyCode::Char('X'), KeyCode::Backspace] {
        let _ = composer.handle_key_event(KeyEvent::from(code));
    }
    let textarea = &composer.draft.textarea;
    assert_eq!(
        (textarea.text(), textarea.vim_mode_label()),
        ("bc", Some("Replace"))
    );
}
