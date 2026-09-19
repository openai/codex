//! Closing nested or App transcript overlays must not release Linux native ownership.

use super::*;
use crate::clipboard_copy::ClipboardLease;
use crate::history_cell::PlainHistoryCell;
use crate::keymap::RuntimeKeymap;
use crate::pager_overlay::Overlay;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test]
async fn selection_clipboard_survives_overlay_close_and_later_terminal_requests()
-> std::io::Result<()> {
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for nested in [false, true] {
        let mut overlay = Overlay::new_transcript(
            vec![Arc::new(PlainHistoryCell::new(vec!["selected".into()]))],
            RuntimeKeymap::defaults().pager,
        );
        assert_eq!(
            tui.copy_transcript_selection_with("selected", |text| {
                assert_eq!(text, "selected");
                Ok(CopyOutcome::Copied(Some(ClipboardLease::test())))
            }),
            Ok(CopyStatus::Confirmed)
        );
        overlay.handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
        )?;
        assert!(overlay.is_done());
        drop(overlay);
        if !nested {
            tui.leave_alt_screen()?;
        }
        assert!(tui.selection_clipboard_lease.is_some());
        for outcome in [CopyOutcome::Requested, CopyOutcome::Copied(None)] {
            tui.copy_transcript_selection_with("retry", |_| Ok(outcome))
                .unwrap();
            assert!(tui.selection_clipboard_lease.is_some());
        }
        assert_eq!(
            tui.copy_transcript_selection_with("retry", |_| Err("blocked".into())),
            Err("blocked".into())
        );
        assert!(tui.selection_clipboard_lease.is_some());
    }
    Ok(())
}
