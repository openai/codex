//! Permission picker presentation and configured navigation, without applying real policies.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn clipped_full_access_confirmation_discloses_hidden_warning_text() {
    let mut snapshots = Vec::new();
    for (specialty, height) in [(None, 10), (Some("cyber"), 16)] {
        let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
        let current_model = chat.current_model().to_owned();
        Arc::make_mut(&mut chat.model_catalog)
            .models
            .iter_mut()
            .find(|model| model.model == current_model)
            .expect("the selected model is in the catalog")
            .model_specialty = specialty.map(str::to_owned);
        let preset = builtin_approval_presets()
            .into_iter()
            .find(|preset| preset.id == "full-access")
            .unwrap();
        chat.open_full_access_confirmation(
            preset, /*return_to_permissions*/ true, /*profile_selection*/ None,
        );

        let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 40, height);
        let mut buffer = Buffer::empty(area);
        chat.bottom_pane.render(area, &mut buffer);
        let rows = (0..height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|row| row.contains("Yes, continue anyway")));
        assert!(rows.iter().any(|row| row.contains("[…")));
        assert_eq!(rows.last().unwrap().trim(), "enter select · esc back");
        snapshots.push(format!(
            "specialty={specialty:?} 40x{height}\n{}",
            rows.join("\n")
        ));

        let complete = render_bottom_popup(&chat, /*width*/ 40)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!complete.contains("[…"));
        assert!(complete.contains("without your approval"));
        if specialty.is_some() {
            assert!(complete.contains("Cyber models carry a higher risk"));
            assert!(complete.contains("Ask for approval"));
        } else {
            assert!(complete.contains("risk of data loss"));
        }
    }
    assert_chatwidget_snapshot!("clipped_full_access_confirmation", snapshots.join("\n\n"));
}

#[tokio::test]
async fn permission_picker_retry_and_confirmation_follow_configured_bindings() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.list.accept = vec![key_hint::plain(KeyCode::F(/*n*/ 3))];
    keymap.list.cancel = vec![key_hint::plain(KeyCode::F(/*n*/ 2))];
    chat.bottom_pane.set_keymap_bindings(&keymap);
    chat.request_permission_profiles();
    rx.try_recv().unwrap();
    chat.on_permission_profiles_loaded(
        chat.permission_popup_request_id.unwrap(),
        Err("Disconnected".into()),
    );
    let popup = render_bottom_popup(&chat, /*width*/ 40);
    assert_eq!(popup.lines().last().unwrap().trim(), "f3 select · f2 back");
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenPermissionsPopup));
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "full-access")
        .unwrap();
    chat.open_full_access_confirmation(
        preset, /*return_to_permissions*/ true, /*profile_selection*/ None,
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenPermissionsPopup));
    assert!(rx.try_recv().is_err(), "cancel must not apply permissions");

    chat.request_permission_profiles();
    rx.try_recv().unwrap();
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 2)));
    assert!(chat.no_modal_or_popup_active());
    assert!(rx.try_recv().is_err());
}
