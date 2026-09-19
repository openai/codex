use super::super::actionable_banner::BannerDismissal;
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn promoted_banner_uses_picker_hints_and_preserves_the_draft() {
    let width = 40;
    for confirm in [false, true] {
        let (tx, mut rx) = unbounded_channel();
        let mut pane = test_pane(AppEventSender::new(tx));
        let mut keymap = RuntimeKeymap::defaults();
        keymap.list.accept = vec![crate::key_hint::plain(KeyCode::F(/*n*/ 3))];
        keymap.list.cancel = vec![crate::key_hint::plain(KeyCode::F(/*n*/ 2))];
        pane.set_keymap_bindings(&keymap);
        pane.set_composer_text("draft in progress".into(), Vec::new(), Vec::new());
        pane.handle_key_event(KeyEvent::from(KeyCode::Left));
        let draft = pane.composer.draft_snapshot();
        pane.show_actionable_banner(ActionableBanner {
            title: "Usage limit reached".into(),
            description: "Review your usage or keep working with another model.".into(),
            actions: vec![SelectionItem {
                name: "View usage".into(),
                dismiss_on_select: true,
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenUrlInBrowser {
                        url: "https://example.com/usage".into(),
                    });
                })],
                ..Default::default()
            }],
            ..Default::default()
        });
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            pane.desired_height(width),
        );
        let rendered = render_snapshot(&pane, area);
        assert!(rendered.contains("View usage"), "{rendered}");
        assert!(rendered.contains("f3 select · f2 back"), "{rendered}");
        if confirm {
            assert_snapshot!(format!("promoted_action_banner_{width}"), rendered);
        }
        assert_eq!(pane.composer.draft_snapshot(), draft);

        let key = if confirm {
            KeyCode::F(/*n*/ 3)
        } else {
            KeyCode::F(/*n*/ 2)
        };
        pane.handle_key_event(KeyEvent::from(key));
        assert!(pane.view_stack.is_empty());
        assert_eq!(pane.composer.draft_snapshot(), draft);
        if confirm {
            assert!(
                matches!(rx.try_recv(), Ok(AppEvent::OpenUrlInBrowser { url })
                    if url == "https://example.com/usage")
            );
        }
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn hidden_banner_preserves_input_before_render_and_after_resize() {
    let (tx, mut rx) = unbounded_channel();
    let mut pane = test_pane_with_disable_paste_burst(
        AppEventSender::new(tx),
        /*disable_paste_burst*/ true,
    );
    pane.set_inline_banner(Some(ActionableBanner {
        title: "Usage limit reached".into(),
        actions: vec![SelectionItem {
            name: "View usage".into(),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenUrlInBrowser {
                    url: "https://example.com/usage".into(),
                });
            })],
            ..Default::default()
        }],
        ..Default::default()
    }));
    let width = 70;
    let hidden_area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        pane.composer.desired_height(width),
    );
    let visible_area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        pane.desired_height(width),
    );

    // Keys remain composer input before the first frame and after resizing a
    // previously visible banner out of view.
    for render_hidden in [false, true] {
        if render_hidden {
            let rendered = render_snapshot(&pane, hidden_area);
            assert!(!rendered.contains("View usage"));
        }
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(pane.composer_text(), "1");
        assert!(rx.try_recv().is_err());
        pane.set_composer_text(String::new(), Vec::new(), Vec::new());
        assert!(pane.is_normal_backtrack_mode());
        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        let rendered = render_snapshot(&pane, visible_area);
        assert!(rendered.contains("View usage"));
        assert!(!pane.is_normal_backtrack_mode());
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(pane.composer_text(), "");
        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::OpenUrlInBrowser { url })
            if url == "https://example.com/usage")
        );
    }

    pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(pane.is_normal_backtrack_mode());
    assert!(!render_snapshot(&pane, visible_area).contains("View usage"));
}

#[test]
fn partially_clipped_banner_preserves_hidden_action_shortcuts() {
    let (tx, mut rx) = unbounded_channel();
    let mut pane = test_pane_with_disable_paste_burst(
        AppEventSender::new(tx),
        /*disable_paste_burst*/ true,
    );
    pane.set_inline_banner(Some(ActionableBanner {
        title: "Usage limit reached".into(),
        actions: vec![SelectionItem {
            name: "View usage".into(),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::OpenUrlInBrowser {
                    url: "https://example.com/usage".into(),
                });
            })],
            ..Default::default()
        }],
        ..Default::default()
    }));
    let width = 70;
    let footer_only_area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        pane.composer.desired_height(width) + 1,
    );

    let rendered = render_snapshot(&pane, footer_only_area);
    assert!(!rendered.contains("View usage"));
    pane.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
    assert_eq!(pane.composer_text(), "1");
    assert!(rx.try_recv().is_err());
}

#[test]
fn banner_hidden_during_task_waits_for_redraw_before_accepting_dismissal() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_inline_banner(Some(ActionableBanner {
        title: "Usage limit reached".into(),
        ..Default::default()
    }));
    let width = 70;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        pane.desired_height(width),
    );
    assert!(render_snapshot(&pane, area).contains("Usage limit reached"));

    pane.set_task_running(/*running*/ true);
    assert!(!render_snapshot(&pane, area).contains("Usage limit reached"));
    pane.set_task_running(/*running*/ false);
    assert!(pane.is_normal_backtrack_mode());
    pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(render_snapshot(&pane, area).contains("Usage limit reached"));
    assert!(!pane.is_normal_backtrack_mode());
}

#[test]
fn information_banner_preserves_input_and_honors_dismissal() {
    for dismissal in [BannerDismissal::Persistent, BannerDismissal::Dismissible] {
        let (tx, _rx) = unbounded_channel();
        let mut pane = test_pane_with_disable_paste_burst(
            AppEventSender::new(tx),
            /*disable_paste_burst*/ true,
        );
        pane.set_inline_banner(Some(ActionableBanner {
            title: "Usage limit reached".into(),
            description: "You can change models or request more usage.".into(),
            dismissal,
            ..Default::default()
        }));
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            /*width*/ 70,
            pane.desired_height(/*width*/ 70),
        );
        let rendered = render_snapshot(&pane, area);
        assert!(!rendered.contains("no matches"));
        assert!(!rendered.contains("Press a number"));
        assert_eq!(
            rendered.contains("esc to dismiss"),
            dismissal == BannerDismissal::Dismissible
        );
        if dismissal == BannerDismissal::Persistent {
            assert_snapshot!("information_banner_persistent", rendered);
        } else {
            assert_snapshot!("information_banner_dismissible", rendered);
        }
        pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(
            render_snapshot(&pane, area).contains("Usage limit reached"),
            dismissal == BannerDismissal::Persistent
        );
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(pane.composer_text(), "1");
    }
}
