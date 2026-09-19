//! Copy/export presentation and exact payload routing through their production views.

use super::*;
use crate::app_event::TranscriptExportDestination;
use crate::clipboard_copy::CopyFormat;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn copy_export_picker_custom_keys_preserve_payloads_and_composer_draft() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.list.accept = vec![key_hint::plain(KeyCode::F(/*n*/ 3))];
    keymap.list.cancel = vec![key_hint::plain(KeyCode::F(/*n*/ 2))];
    chat.bottom_pane.set_keymap_bindings(&keymap);
    chat.bottom_pane
        .set_composer_text("Keep this draft".into(), Vec::new(), Vec::new());
    let source =
        "A long preview with 日本語 and cafe\u{301}; keep all of it. ".repeat(/*n*/ 4);
    chat.transcript.last_agent_markdown = Some(source.clone());
    chat.show_copy_picker();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(popup.contains("..."), "{popup}");
    assert!(popup.contains("f3 select · f2 back"), "{popup}");
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    let copied = std::iter::from_fn(|| rx.try_recv().ok()).find_map(|event| match event {
        AppEvent::CopySelection {
            text,
            label,
            format,
        } => Some((text.to_string(), label, format)),
        _ => None,
    });
    assert_eq!(
        copied,
        Some((source, "Whole response".into(), CopyFormat::Markdown))
    );
    assert_eq!(chat.bottom_pane.composer_text(), "Keep this draft");

    chat.show_transcript_export_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 2)));
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok()).all(|event| !matches!(
            event,
            AppEvent::ExportTranscript { .. } | AppEvent::OpenTranscriptExportFilePrompt
        ))
    );
    chat.show_transcript_export_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenTranscriptExportFilePrompt))
    );
    chat.show_transcript_export_file_prompt();
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(chat.no_modal_or_popup_active());
    assert_eq!(chat.bottom_pane.composer_text(), "Keep this draft");
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .all(|event| !matches!(event, AppEvent::ExportTranscript { .. }))
    );

    chat.show_transcript_export_file_prompt();
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    let filename = "reviews/日本語 - result.md";
    chat.handle_paste(filename.into());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let exported = std::iter::from_fn(|| rx.try_recv().ok()).find_map(|event| match event {
        AppEvent::ExportTranscript {
            destination: TranscriptExportDestination::File(path),
        } => Some(path),
        _ => None,
    });
    assert_eq!(exported, Some(PathBuf::from(filename)));
    assert!(chat.no_modal_or_popup_active());
    assert_eq!(chat.bottom_pane.composer_text(), "Keep this draft");
}
