#[cfg(test)]
mod tests {
    use std::sync::mpsc::{channel, Receiver};
    use std::time::Duration;

    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
use codex_core::protocol::{
    AgentMessageDeltaEvent, AgentMessageEvent, AgentReasoningDeltaEvent, AgentReasoningEvent, Event, EventMsg,
};

    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::chatwidget::ChatWidget;

    fn test_config() -> Config {
        let overrides = ConfigOverrides {
            cwd: Some(std::env::current_dir().unwrap()),
            ..Default::default()
        };
        Config::load_with_cli_overrides(vec![], overrides).expect("load test config")
    }

    fn recv_insert_history(
        rx: &Receiver<AppEvent>,
        timeout_ms: u64,
    ) -> Option<Vec<ratatui::text::Line<'static>>> {
        let to = Duration::from_millis(timeout_ms);
        match rx.recv_timeout(to) {
            Ok(AppEvent::InsertHistory(lines)) => Some(lines),
            Ok(_) => None,
            Err(_) => None,
        }
    }

    #[test]
    fn widget_streams_on_newline_and_header_once() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let config = test_config();

        let mut w = ChatWidget::new(config.clone(), tx.clone(), None, Vec::new(), false);

        // Start reasoning stream with partial content (no newline): expect no history yet.
        w.handle_codex_event(Event {
            id: "1".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                delta: "Hello".into(),
            }),
        });

        // No history commit before newline.
        assert!(
            recv_insert_history(&rx, 50).is_none(),
            "unexpected history before newline"
        );

        // Live ring should show thinking header immediately.
        let live = w.test_live_ring_rows();
        let live_text: String = live
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect();
        assert!(
            live_text.contains("thinking"),
            "expected thinking header in live ring"
        );

        // Push a newline which should cause commit of the first logical line.
        w.handle_codex_event(Event {
            id: "1".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                delta: " world\nNext".into(),
            }),
        });

        let lines = recv_insert_history(&rx, 200).expect("expected history after newline");
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();

        // First commit should include the header and the completed first line once.
        assert!(
            rendered.iter().any(|s| s.contains("thinking")),
            "missing reasoning header: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|s| s.contains("Hello world")),
            "missing committed line: {rendered:?}"
        );

        // Send finalize; expect remaining content to flush and a trailing blank line.
        w.handle_codex_event(Event {
            id: "1".into(),
            msg: EventMsg::AgentReasoning(AgentReasoningEvent {
                text: String::new(),
            }),
        });

        let lines2 = recv_insert_history(&rx, 200).expect("expected history after finalize");
        let rendered2: Vec<String> = lines2
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect();
        // Ensure header not repeated on finalize and a blank spacer exists at the end.
        let header_count = rendered
            .iter()
            .chain(rendered2.iter())
            .filter(|s| s.contains("thinking"))
            .count();
        assert_eq!(header_count, 1, "reasoning header should be emitted exactly once");
        assert!(
            rendered2.last().is_some_and(|s| s.is_empty()),
            "expected trailing blank line on finalize"
        );
    }
}

#[cfg(test)]
mod widget_stream_extra {
    use super::*;

    #[test]
    fn widget_fenced_code_slow_streaming_no_dup() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let config = test_config();
        let mut w = ChatWidget::new(config.clone(), tx.clone(), None, Vec::new(), false);

        // Begin answer stream: push opening fence in pieces with no newline -> no history.
        for d in ["```", ""] {
            w.handle_codex_event(Event {
                id: "a".into(),
                msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: d.into() }),
            });
            assert!(super::recv_insert_history(&rx, 30).is_none(), "no history before newline for fence");
        }
        // Newline after fence line.
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "\n".into() }),
        });
        // This may or may not produce a visible line depending on renderer; accept either.
        let _ = super::recv_insert_history(&rx, 100);

        // Stream the code line without newline -> no history.
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "code line".into() }),
        });
        assert!(super::recv_insert_history(&rx, 30).is_none(), "no history before newline for code line");

        // Now newline to commit the code line.
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "\n".into() }),
        });
        let commit1 = super::recv_insert_history(&rx, 200).expect("history after code line newline");

        // Close fence slowly then newline.
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "```".into() }),
        });
        assert!(super::recv_insert_history(&rx, 30).is_none(), "no history before closing fence newline");
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "\n".into() }),
        });
        let _ = super::recv_insert_history(&rx, 100);

        // Finalize should not duplicate the code line and should add a trailing blank.
        w.handle_codex_event(Event {
            id: "a".into(),
            msg: EventMsg::AgentMessage(AgentMessageEvent { message: String::new() }),
        });
        let commit2 = super::recv_insert_history(&rx, 200).expect("history after finalize");

        let texts1: Vec<String> = commit1
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        let texts2: Vec<String> = commit2
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        let all = [texts1, texts2].concat();
        let code_count = all.iter().filter(|s| s.contains("code line")).count();
        assert_eq!(code_count, 1, "code line should appear exactly once in history: {all:?}");
        assert!(all.iter().all(|s| !s.contains("```")), "backticks should not be shown in history: {all:?}");
    }

    #[test]
    fn widget_rendered_trickle_live_ring_head() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let config = test_config();
        let mut w = ChatWidget::new(config.clone(), tx.clone(), None, Vec::new(), false);

        // Increase live ring capacity so it can include queue head.
        w.test_set_live_max_rows(4);

        // Enqueue 5 completed lines in a single delta.
        let payload = "l1\nl2\nl3\nl4\nl5\n".to_string();
        w.handle_codex_event(Event {
            id: "b".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: payload }),
        });

        // First batch commit: expect header + 3 lines.
        let lines = super::recv_insert_history(&rx, 200).expect("history after batch");
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(rendered.iter().any(|s| s.contains("codex")), "answer header missing");
        let committed: Vec<_> = rendered.into_iter().filter(|s| s.starts_with('l')).collect();
        assert_eq!(committed.len(), 3, "expected 3 committed lines in first batch");

        // Live ring should include the newest 3 committed plus one queued head (l4).
        let live = w.test_live_ring_rows();
        let live_texts: Vec<String> = live
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(live_texts.iter().any(|s| s.contains("l4")), "expected queue head l4 in live ring: {live_texts:?}");
        assert!(live_texts.iter().all(|s| !s.contains("l5")), "l5 should not be visible in live ring yet: {live_texts:?}");

        // Finalize: drain the remaining lines.
        w.handle_codex_event(Event {
            id: "b".into(),
            msg: EventMsg::AgentMessage(AgentMessageEvent { message: String::new() }),
        });
        let lines2 = super::recv_insert_history(&rx, 200).expect("history after finalize");
        let rendered2: Vec<String> = lines2
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(rendered2.iter().any(|s| s == "l4"));
        assert!(rendered2.iter().any(|s| s == "l5"));
        assert!(rendered2.last().is_some_and(|s| s.is_empty()), "expected trailing blank line after finalize");
    }
}

