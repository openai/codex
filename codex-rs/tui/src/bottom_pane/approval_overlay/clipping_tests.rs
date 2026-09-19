//! Clipped approval headers expose the configured route to their complete payload.

use super::*;
use crate::keymap::RuntimeKeymap;
use pretty_assertions::assert_eq;

#[test]
fn clipped_exec_approval_opens_the_complete_command() {
    let command = format!(
        "{}printf destructive_suffix",
        "printf benign\n".repeat(/*n*/ 20)
    );
    let reason = format!(
        "{}reason_suffix",
        "A reason requiring review.\n".repeat(/*n*/ 20)
    );
    let mut snapshots = Vec::new();
    for binding in [Some(key_hint::ctrl(KeyCode::Char('g'))), None] {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut keymap = RuntimeKeymap::defaults();
        keymap.approval.open_fullscreen = binding.into_iter().collect();
        let mut view = ApprovalOverlay::new(
            ApprovalRequest::Exec(ExecApprovalRequest {
                kind: Default::default(),
                thread_id: ThreadId::new(),
                thread_label: None,
                id: "clipped-approval".into(),
                environment_id: Some("local".into()),
                command: vec!["sh".into(), "-c".into(), command.clone()],
                reason: Some(reason.clone()),
                available_decisions: vec![
                    CommandExecutionApprovalDecision::Accept,
                    CommandExecutionApprovalDecision::Cancel,
                ],
                network_approval_context: None,
                additional_permissions: None,
            }),
            AppEventSender::new(tx),
            Features::with_defaults(),
            keymap.approval,
            keymap.list,
        );
        for (width, height) in [(80, 14), (40, 10)] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let mut buffer = Buffer::empty(area);
            view.render(area, &mut buffer);
            let rows: Vec<String> = (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect();
            let elision = rows.iter().find(|row| row.contains("[…")).unwrap();
            assert_eq!(elision.contains("ctrl+g view all"), binding.is_some());
            assert!(rows.last().unwrap().contains("cancel"));
            assert!(!rows.iter().any(|row| row.contains("destructive_suffix")));
            assert!(!rows.iter().any(|row| row.contains("reason_suffix")));
            assert!(rows.iter().any(|row| row.contains("Yes, proceed")));
            snapshots.push(format!(
                "bound={} {width}x{height}\n{}",
                binding.is_some(),
                rows.join("\n")
            ));
        }
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            /*width*/ 80,
            view.desired_height(/*width*/ 80),
        );
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer);
        let complete = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(complete.contains("destructive_suffix"));
        assert!(complete.contains("reason_suffix"));
        assert!(!complete.contains("[…"));
        if binding.is_some() {
            view.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
            let AppEvent::FullScreenApprovalRequest(ApprovalRequest::Exec(request)) =
                rx.try_recv().unwrap()
            else {
                panic!("fullscreen should open the original approval");
            };
            assert_eq!(
                request.command,
                vec!["sh".to_owned(), "-c".to_owned(), command.clone()]
            );
            assert_eq!(request.reason, Some(reason.clone()));
            assert!(!view.is_complete());
            assert!(rx.try_recv().is_err());
        }
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}
