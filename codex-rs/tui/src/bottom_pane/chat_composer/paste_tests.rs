//! Composer paste handling for literal paths and raw tabs.

use super::tests::new_test_composer;
use super::*;
use image::ImageBuffer;
use image::Rgba;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn pasting_fifo_path_keeps_it_editable() {
    let dir = tempfile::tempdir().unwrap();
    let name = "tty.usbserial-110";
    let status = std::process::Command::new("mkfifo")
        .arg(dir.path().join(name))
        .status()
        .unwrap();
    assert!(status.success());

    let (mut composer, _rx) = new_test_composer();
    let prefix = shlex::try_quote(dir.path().to_str().unwrap()).unwrap();
    composer.handle_paste(format!("{prefix}/{name}"));
    composer.handle_key_event(KeyEvent::from(KeyCode::Backspace));
    insta::assert_snapshot!(
        composer.current_text().strip_prefix(prefix.as_ref()).unwrap(),
        @"/tty.usbserial-11"
    );
}

#[test]
fn paste_burst_tabs_preserve_multiline_draft() {
    for running in [false, true] {
        for first_line in ["x", "界", "first line\n"] {
            let (mut composer, _rx) = new_test_composer();
            composer.set_task_running(running);
            let mut now = Instant::now();
            let payload = format!("{first_line}\t\tsecond line\n\tthird line\n");

            for ch in payload.chars() {
                let (result, _) = match ch {
                    '\t' => {
                        assert!(composer.handle_paste_tab(KeyEvent::from(KeyCode::Tab), now));
                        (InputResult::None, true)
                    }
                    '\n' => composer.handle_submission_with_time(/*should_queue*/ false, now),
                    ch => composer
                        .handle_input_basic_with_time(KeyEvent::from(KeyCode::Char(ch)), now),
                };
                assert_eq!(result, InputResult::None);
                now += Duration::from_millis(/*millis*/ 1);
            }

            composer.handle_paste_burst_flush(now + PasteBurst::recommended_active_flush_delay());
            assert_eq!(composer.current_text(), payload);

            if running && first_line == "first line\n" {
                let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
                terminal
                    .draw(|frame| composer.render(frame.area(), frame.buffer_mut()))
                    .unwrap();
                insta::assert_snapshot!("paste_burst_tab_indentation", terminal.backend());
            }

            let (result, _) = composer.handle_key_event(KeyEvent::from(KeyCode::Tab));
            let expected = if running {
                InputResult::Queued {
                    text: payload.trim().to_string(),
                    text_elements: Vec::new(),
                    action: QueuedInputAction::Plain,
                    pending_pastes: Vec::new(),
                }
            } else {
                InputResult::Submitted {
                    text: payload.trim().to_string(),
                    text_elements: Vec::new(),
                }
            };
            assert_eq!(result, expected);
        }
    }
}

#[test]
fn paste_burst_tabs_refresh_idle_timeout() {
    for first_char in ['x', '界'] {
        let (mut composer, _rx) = new_test_composer();
        let mut now = Instant::now();
        composer.handle_input_basic_with_time(KeyEvent::from(KeyCode::Char(first_char)), now);
        assert!(composer.handle_paste_tab(KeyEvent::from(KeyCode::Tab), now));

        for _ in 0..3 {
            now += PasteBurst::recommended_active_flush_delay() / 2;
            assert!(composer.handle_paste_tab(KeyEvent::from(KeyCode::Tab), now));
        }

        composer.handle_paste_burst_flush(now + PasteBurst::recommended_active_flush_delay());
        assert_eq!(composer.current_text(), format!("{first_char}\t\t\t\t"));
        assert!(!composer.handle_paste_tab(
            KeyEvent::from(KeyCode::Tab),
            now + PasteBurst::recommended_active_flush_delay()
        ));
    }
}

#[test]
fn paste_burst_tab_does_not_accept_a_completion() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("/");
    assert!(matches!(composer.popups.active, ActivePopup::Command(_)));
    composer
        .draft
        .paste_burst
        .begin_with_retro_grabbed("review this".to_string(), Instant::now());

    let (result, _) = composer.handle_key_event(KeyEvent::from(KeyCode::Tab));

    assert_eq!(result, InputResult::None);
    let pasted = composer
        .draft
        .paste_burst
        .flush_before_modified_input()
        .unwrap();
    composer.handle_paste(pasted);
    assert_eq!(composer.current_text(), "/review this\t");
}

#[test]
fn paste_burst_expired_before_tab_still_queues() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_task_running(/*running*/ true);
    composer.handle_input_basic_with_time(
        KeyEvent::from(KeyCode::Char('x')),
        Instant::now() - Duration::from_secs(/*secs*/ 1),
    );

    let (result, _) = composer.handle_key_event(KeyEvent::from(KeyCode::Tab));

    assert_eq!(
        result,
        InputResult::Queued {
            text: "x".to_string(),
            text_elements: Vec::new(),
            action: QueuedInputAction::Plain,
            pending_pastes: Vec::new(),
        }
    );
}

#[test]
fn paste_burst_modified_queue_binding_still_dispatches() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_task_running(/*running*/ true);
    composer.queue_keys = vec![key_hint::ctrl(KeyCode::Char('q'))];
    composer.handle_input_basic_with_time(KeyEvent::from(KeyCode::Char('x')), Instant::now());

    let (result, _) =
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));

    assert_eq!(
        result,
        InputResult::Queued {
            text: "x".to_string(),
            text_elements: Vec::new(),
            action: QueuedInputAction::Plain,
            pending_pastes: Vec::new(),
        }
    );
}

#[test]
fn pasting_filepath_attaches_image() {
    let tmp = tempdir().expect("create TempDir");
    let tmp_path: PathBuf = tmp.path().join("codex_tui_test_paste_image.png");
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_fn(3, 2, |_x, _y| Rgba([1, 2, 3, 255]));
    img.save(&tmp_path).expect("failed to write temp png");

    let (mut composer, _rx) = new_test_composer();

    let needs_redraw = composer.handle_paste(tmp_path.to_string_lossy().to_string());
    assert!(needs_redraw);
    assert!(composer.draft.textarea.text().starts_with("[Image #1] "));

    let imgs = composer.take_recent_submission_images();
    let [attachment] = imgs.as_slice() else {
        panic!("expected one image attachment");
    };
    let expected = std::fs::read(&tmp_path).unwrap();
    std::fs::remove_file(&tmp_path).unwrap();
    assert_eq!(std::fs::read(attachment).unwrap(), expected);
    std::fs::remove_file(attachment).unwrap();
}

#[cfg(unix)]
#[test]
fn oversized_pasted_image_reports_error_without_copying() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("oversized.png");
    ImageBuffer::from_pixel(1, 1, Rgba([0u8, 0, 0, 255]))
        .save(&path)
        .unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(crate::clipboard_paste::MAX_PASTED_IMAGE_BYTES + 1)
        .unwrap();
    let (mut composer, mut rx) = new_test_composer();
    composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());
    let before = composer.draft_snapshot();
    let now = Instant::now();
    composer.draft.paste_burst.on_plain_char_no_hold(now);
    composer
        .draft
        .paste_burst
        .begin_with_retro_grabbed(path.to_string_lossy().into_owned(), now);
    let flushed_at = now + PasteBurst::recommended_active_flush_delay();
    assert!(composer.handle_paste_burst_flush(flushed_at));
    assert_eq!(composer.draft_snapshot(), before);
    assert!(!composer.handle_paste_enter(flushed_at));
    let AppEvent::InsertHistoryCell(cell) = rx.try_recv().unwrap() else {
        panic!("expected an image paste error");
    };
    let message = cell
        .display_lines(/*width*/ 80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("oversized_pasted_image_error", message);
}
