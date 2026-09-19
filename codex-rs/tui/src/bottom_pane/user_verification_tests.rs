use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn render_view_lines(view: &UserVerificationView, width: u16, height: u16) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    view.render(Rect::new(0, 0, width, height), &mut buf);
    (0..buf.area.height)
        .map(|row| {
            (0..buf.area.width)
                .map(|col| buf[(col, row)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn make_view() -> (
    UserVerificationView,
    tokio::sync::mpsc::UnboundedReceiver<UserVerificationDecision>,
) {
    make_view_for_request(UserVerificationRequest {
        title: "Production deployment needs your approval.".to_string(),
        description: "Approve deploying the reviewed release?".to_string(),
        thread_label: None,
        server_name: "deployments".to_string(),
        request_id: RequestId::Integer(12),
    })
}

fn make_view_for_request(
    request: UserVerificationRequest,
) -> (
    UserVerificationView,
    tokio::sync::mpsc::UnboundedReceiver<UserVerificationDecision>,
) {
    let (app_tx, _) = unbounded_channel();
    let (decision_tx, decision_rx) = unbounded_channel();
    let keymap = crate::keymap::RuntimeKeymap::defaults();
    let view = UserVerificationView::new(
        request,
        AppEventSender::new(app_tx),
        keymap.approval,
        keymap.list,
        Box::new(move |decision| {
            let _ = decision_tx.send(decision);
        }),
    );
    (view, decision_rx)
}

#[test]
fn prompt_snapshot() {
    let (view, _app_rx) = make_view();
    assert_snapshot!(
        "user_verification_prompt",
        render_view_lines(&view, /*width*/ 80, view.desired_height(/*width*/ 80))
    );
}

#[test]
fn clipped_prompt_exposes_fullscreen_hint() {
    let (view, _app_rx) = make_view();
    assert_snapshot!(
        "user_verification_narrow",
        render_view_lines(&view, /*width*/ 40, /*height*/ 8)
    );
}

#[test]
fn waiting_snapshot_and_duplicate_approval_suppression() {
    let (mut view, mut app_rx) = make_view();
    view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_snapshot!(
        "user_verification_waiting",
        render_view_lines(&view, /*width*/ 80, view.desired_height(/*width*/ 80))
    );
    view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app_rx.try_recv().expect("approval"),
        UserVerificationDecision::Verify
    );
    assert!(app_rx.try_recv().is_err());
    assert!(!view.is_complete());
}

#[test]
fn cancellation_is_available_before_and_during_verification() {
    for state in [
        UserVerificationState::Prompt,
        UserVerificationState::Waiting,
    ] {
        for cancel_key in [KeyCode::Esc, KeyCode::Char('c'), KeyCode::Char('x')] {
            let (mut view, mut app_rx) = make_view();
            view.state = state;
            if cancel_key == KeyCode::Char('x') {
                view.approval_keymap.cancel = vec![crate::key_hint::plain(KeyCode::Char('x'))];
            }
            view.handle_key_event(KeyEvent::new(cancel_key, KeyModifiers::NONE));
            assert_eq!(
                app_rx.try_recv().expect("cancellation"),
                UserVerificationDecision::Cancel
            );
            assert!(app_rx.try_recv().is_err());
            assert_eq!(view.completion(), Some(ViewCompletion::Cancelled));
        }
    }
}

#[test]
fn resolved_request_dismisses_the_matching_waiting_view() {
    let (mut view, _app_rx) = make_view();
    view.state = UserVerificationState::Waiting;
    assert!(
        !view.dismiss_app_server_request(&ResolvedAppServerRequest::McpElicitation {
            server_name: "other".to_string(),
            request_id: RequestId::Integer(12)
        })
    );
    assert!(
        view.dismiss_app_server_request(&ResolvedAppServerRequest::McpElicitation {
            server_name: "deployments".to_string(),
            request_id: RequestId::Integer(12)
        })
    );
    assert!(view.is_complete());
}

#[test]
fn full_screen_request_is_available_before_and_during_verification() {
    for state in [
        UserVerificationState::Prompt,
        UserVerificationState::Waiting,
    ] {
        let (mut view, mut decisions) = make_view();
        let (app_tx, mut app_rx) = unbounded_channel();
        view.app_event_tx = AppEventSender::new(app_tx);
        view.approval_keymap.open_fullscreen = vec![crate::key_hint::ctrl(KeyCode::Char('v'))];
        view.state = state;
        view.handle_key_event(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL));
        let crate::app_event::AppEvent::FullScreenUserVerificationRequest(request) =
            app_rx.try_recv().expect("full-screen request")
        else {
            panic!("expected full-screen request");
        };
        assert_eq!(request.description, view.request.description);
        assert!(decisions.try_recv().is_err());
        assert!(!view.is_complete());
        assert!(view.terminal_title_requires_action());
    }
}

#[test]
fn long_url_tokens_remain_visible_in_a_narrow_verification_prompt() {
    let (view, _) = make_view_for_request(UserVerificationRequest {
        title: "Approve https://example.com/production/release/candidate?".to_string(),
        description: "Deploy https://example.com/production/critical-release-candidate".to_string(),
        thread_label: None,
        server_name: "deployments".to_string(),
        request_id: RequestId::Integer(12),
    });
    assert_snapshot!(
        "user_verification_long_urls",
        render_view_lines(&view, /*width*/ 30, view.desired_height(/*width*/ 30))
    );
}

#[tokio::test]
async fn full_screen_pager_reaches_the_end_of_a_long_request() -> std::io::Result<()> {
    let (view, _) = make_view_for_request(UserVerificationRequest {
        title: "Approve release?".to_string(),
        description: format!(
            "{}Final approval details.",
            "Deployment details. ".repeat(30)
        ),
        thread_label: None,
        server_name: "deployments".to_string(),
        request_id: RequestId::Integer(12),
    });
    let mut pager = crate::pager_overlay::StaticOverlay::with_renderables(
        vec![prompt_header(&view.request)],
        "USER VERIFICATION".to_string(),
        crate::keymap::RuntimeKeymap::defaults().pager,
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let area = Rect::new(0, 0, 40, 12);
    let mut buf = Buffer::empty(area);
    pager.render(area, &mut buf);
    pager.handle_event(
        &mut tui,
        crate::tui::TuiEvent::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
    )?;
    pager.render(area, &mut buf);
    assert_snapshot!("user_verification_full_screen_bottom", format!("{buf:?}"));
    Ok(())
}
