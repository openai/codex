use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn render(view: &FeedbackNoteView, width: u16) -> String {
    let height = view.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);
    render_buffer(area, &buf)
}

fn render_buffer(area: Rect, buf: &Buffer) -> String {
    let mut lines: Vec<String> = (0..area.height)
        .map(|row| {
            let mut line = String::new();
            for col in 0..area.width {
                let symbol = buf[(area.x + col, area.y + row)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(&crate::terminal_hyperlinks::strip_osc8(symbol));
                }
            }
            line.trim_end().to_string()
        })
        .collect();

    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn make_view(category: FeedbackCategory) -> FeedbackNoteView {
    let (tx_raw, _rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    FeedbackNoteView::new(
        category,
        /*turn_id*/ None,
        tx,
        /*include_logs*/ true,
        Some(FeedbackAudience::External),
        /*can_save*/ true,
    )
}

#[test]
fn feedback_category_titles() {
    for (category, title) in [
        (FeedbackCategory::BadResult, "▌ Tell us more (bad result)"),
        (FeedbackCategory::GoodResult, "▌ Tell us more (good result)"),
        (FeedbackCategory::Bug, "▌ Tell us more (bug)"),
        (FeedbackCategory::Other, "▌ Tell us more (other)"),
    ] {
        let rendered = render(&make_view(category), /*width*/ 60);
        assert_eq!(rendered.lines().next(), Some(title));
    }
}

#[test]
fn feedback_view_bug() {
    let view = make_view(FeedbackCategory::Bug);
    let rendered = render(&view, /*width*/ 60);
    insta::assert_snapshot!("feedback_view_bug", rendered);
}

#[test]
fn feedback_view_safety_check() {
    let view = make_view(FeedbackCategory::SafetyCheck);
    let rendered = render(&view, /*width*/ 60);
    insta::assert_snapshot!("feedback_view_safety_check", rendered);
}

#[test]
fn feedback_disclosure_matches_audience_at_normal_and_narrow_widths() {
    let mut rendered = Vec::new();
    for (audience, name, url, link_label) in [
        (
            FeedbackAudience::External,
            "external",
            FEEDBACK_POLICY_URL,
            "Learnmore",
        ),
        (
            FeedbackAudience::OpenAiEmployee,
            "employee",
            EMPLOYEE_FEEDBACK_FAQ_URL,
            "here",
        ),
    ] {
        let mut view = make_view(FeedbackCategory::Bug);
        view.upload_audience = Some(audience);
        for width in [80, 40] {
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                width,
                view.desired_height(width),
            );
            let mut buf = Buffer::empty(area);
            view.render(area, &mut buf);

            let linked_text = buf
                .content
                .iter()
                .filter(|cell| cell.symbol().contains(&format!("\x1b]8;;{url}\x07")))
                .map(|cell| crate::terminal_hyperlinks::strip_osc8(cell.symbol()))
                .collect::<String>();
            assert_eq!(linked_text, link_label);
            rendered.push(format!(
                "{name} ({width} columns)\n{}",
                render_buffer(area, &buf)
            ));
        }
    }
    let mut remote_view = make_view(FeedbackCategory::Bug);
    remote_view.can_save = false;
    rendered.push(format!(
        "remote server (80 columns)\n{}",
        render(&remote_view, /*width*/ 80)
    ));
    insta::assert_snapshot!("feedback_disclosures", rendered.join("\n\n"));
}

#[test]
fn feedback_disclosure_scrolls_without_hiding_input_in_short_panes() {
    let mut rendered = Vec::new();
    for (width, height) in [(40, 12), (80, 8)] {
        let mut view = make_view(FeedbackCategory::Bug);
        view.upload_audience = Some(FeedbackAudience::OpenAiEmployee);
        view.handle_paste("A visible note.".to_string());
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        for (stage, key) in [
            ("initial", None),
            ("scrolled", Some(KeyCode::PageDown)),
            ("restored", Some(KeyCode::PageUp)),
        ] {
            if let Some(key) = key {
                view.handle_key_event(KeyEvent::from(key));
            }
            let mut buf = Buffer::empty(area);
            view.render(area, &mut buf);
            let text = render_buffer(area, &buf);
            assert!(text.contains("A visible note."));
            assert!(text.contains("enter send"));
            assert!(
                view.cursor_pos(area)
                    .is_some_and(|(x, y)| x < width && y < height)
            );
            assert_eq!(view.is_complete(), false);
            if stage == "scrolled" {
                assert!(text.contains("sensitive personal"));
            }
            rendered.push(format!("{width}x{height} {stage}\n{text}"));
        }
    }
    insta::assert_snapshot!("feedback_disclosure_short_panes", rendered.join("\n\n"));
}

#[test]
fn feedback_disclosure_keeps_editor_visible_in_four_rows() {
    let mut view = make_view(FeedbackCategory::Bug);
    view.upload_audience = Some(FeedbackAudience::OpenAiEmployee);
    view.handle_paste("A visible note.".to_string());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);

    assert!(
        view.cursor_pos(area)
            .is_some_and(|(x, y)| area.contains((x, y).into()))
    );
    let initial = render_buffer(area, &buf);
    assert!(
        initial.contains("Feedback sent to OpenAI may be used to improve our models and products")
    );
    let mut saw_sensitive_information_notice = false;
    for _ in view.intro_lines(area.width, IntroContent::Full) {
        view.handle_key_event(KeyEvent::from(KeyCode::PageDown));
        buf = Buffer::empty(area);
        view.render(area, &mut buf);
        saw_sensitive_information_notice |=
            render_buffer(area, &buf).contains("personal information.");
    }
    let scrolled = render_buffer(area, &buf);
    assert!(saw_sensitive_information_notice);
    assert!(scrolled.contains("which is not sent to OpenAI"));
    insta::assert_snapshot!(
        "feedback_disclosure_four_rows",
        format!("{initial}\n\n{scrolled}")
    );

    let narrow_area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 4,
    );
    for audience in [FeedbackAudience::OpenAiEmployee, FeedbackAudience::External] {
        let mut view = make_view(FeedbackCategory::SafetyCheck);
        view.upload_audience = Some(audience);
        let mut buf = Buffer::empty(narrow_area);
        view.render(narrow_area, &mut buf);
        let text = render_buffer(narrow_area, &buf);
        assert!(text.contains("Feedback sent to"));
        assert!(text.contains("save · enter send"));
    }
}

#[test]
fn feedback_paste_burst_preserves_multiline_note_until_submit() {
    for include_logs in [false, true] {
        for note in ["x\nrest", "id\n\nbody", "foo\nbar", "x\t\nrest"] {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let mut view = FeedbackNoteView::new(
                FeedbackCategory::Bug,
                /*turn_id*/ None,
                AppEventSender::new(tx),
                include_logs,
                Some(FeedbackAudience::External),
                /*can_save*/ true,
            );
            let now = Instant::now();
            for (index, ch) in note.chars().enumerate() {
                let code = match ch {
                    '\n' => KeyCode::Enter,
                    '\t' => KeyCode::Tab,
                    ch => KeyCode::Char(ch),
                };
                view.handle_key_event_at(
                    KeyEvent::from(code),
                    now + Duration::from_millis(index as u64),
                );
                assert!(!view.is_complete());
                assert!(rx.try_recv().is_err());
            }
            let expected_note = note.replace('\t', "");
            assert_eq!(view.textarea.text(), expected_note);
            if !include_logs && note == "foo\nbar" {
                insta::assert_snapshot!("feedback_multiline_paste", render(&view, /*width*/ 60));
            }

            view.handle_key_event_at(
                KeyEvent::from(KeyCode::Enter),
                now + Duration::from_millis(200),
            );

            let AppEvent::SubmitFeedback {
                category,
                reason,
                turn_id,
                include_logs: submitted_logs,
                destination: FeedbackDestination::OpenAi,
            } = rx.try_recv().expect("submit feedback event")
            else {
                panic!("expected feedback submission");
            };
            assert_eq!(
                (category, reason, turn_id, submitted_logs),
                (
                    FeedbackCategory::Bug,
                    Some(expected_note),
                    None,
                    include_logs
                )
            );
            assert!(view.is_complete());
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn feedback_bracketed_paste_does_not_reach_chat_composer() {
    for include_logs in [false, true] {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = AppEventSender::new(tx);
        let mut pane = super::super::tests::test_pane(tx.clone());
        pane.show_view(Box::new(FeedbackNoteView::new(
            FeedbackCategory::Bug,
            /*turn_id*/ None,
            tx,
            include_logs,
            Some(FeedbackAudience::External),
            /*can_save*/ true,
        )));
        // An explicit paste ends any preceding keystroke burst.
        pane.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        pane.handle_paste("\nrest".to_string());
        assert!(rx.try_recv().is_err());
        assert!(matches!(
            pane.handle_key_event(KeyEvent::from(KeyCode::Enter)),
            super::super::InputResult::None
        ));
        let AppEvent::SubmitFeedback {
            reason,
            include_logs: submitted_logs,
            ..
        } = rx.try_recv().expect("submit feedback event")
        else {
            panic!("expected feedback submission");
        };
        assert_eq!(
            (reason, submitted_logs),
            (Some("x\nrest".into()), include_logs)
        );
        assert!(pane.composer_is_empty());
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn remote_save_shortcut_keeps_the_note_for_explicit_upload() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::Bug,
        Some("turn-123".to_string()),
        tx,
        /*include_logs*/ true,
        Some(FeedbackAudience::OpenAiEmployee),
        /*can_save*/ false,
    );
    view.textarea.insert_str("  something broke  ");
    view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(!view.is_complete());
    assert!(rx.try_recv().is_err());

    view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let event = rx.try_recv().expect("submit feedback event");
    assert!(matches!(
        event,
        AppEvent::SubmitFeedback {
            category: FeedbackCategory::Bug,
            reason: Some(reason),
            turn_id: Some(turn_id),
            include_logs: true,
            destination: FeedbackDestination::OpenAi,
        } if reason == "something broke" && turn_id == "turn-123"
    ));
    assert_eq!(view.is_complete(), true);
}

#[test]
fn submit_feedback_omits_empty_note() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::GoodResult,
        /*turn_id*/ None,
        tx,
        /*include_logs*/ false,
        Some(FeedbackAudience::External),
        /*can_save*/ true,
    );

    view.handle_key_event(KeyCode::Enter.into());

    let event = rx.try_recv().expect("submit feedback event");
    assert!(matches!(
        event,
        AppEvent::SubmitFeedback {
            category: FeedbackCategory::GoodResult,
            reason: None,
            turn_id: None,
            include_logs: false,
            destination: FeedbackDestination::OpenAi,
        }
    ));
}

#[test]
fn save_shortcut_keeps_note_and_consent_with_or_without_uploads() {
    for audience in [None, Some(FeedbackAudience::External)] {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut view = FeedbackNoteView::new(
            FeedbackCategory::Bug,
            Some("turn-123".into()),
            AppEventSender::new(tx),
            /*include_logs*/ false,
            audience,
            /*can_save*/ true,
        );
        view.handle_paste("Review before sharing".into());
        if audience.is_none() {
            insta::assert_snapshot!(
                "feedback_note_uploads_disabled",
                render(&view, /*width*/ 60)
            );
            view.handle_key_event(KeyCode::Enter.into());
            assert!(!view.is_complete());
            assert!(rx.try_recv().is_err());
        }
        view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(rx.try_recv(), Ok(AppEvent::SubmitFeedback {
            destination: FeedbackDestination::LocalZip,
            category: FeedbackCategory::Bug,
            reason: Some(note),
            turn_id: Some(turn),
            include_logs: false,
        }) if note == "Review before sharing" && turn == "turn-123"));
        assert!(view.is_complete());
    }
}

#[test]
fn feedback_note_cancel_does_not_submit() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::Bug,
        /*turn_id*/ None,
        AppEventSender::new(tx_raw),
        /*include_logs*/ false,
        Some(FeedbackAudience::OpenAiEmployee),
        /*can_save*/ true,
    );
    view.textarea.insert_str("unfinished feedback");

    view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(view.is_complete(), true);
    assert!(rx.try_recv().is_err());
}
