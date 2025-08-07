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
            cwd: std::env::current_dir().ok(),
            ..Default::default()
        };
        match Config::load_with_cli_overrides(vec![], overrides) {
            Ok(c) => c,
            Err(e) => panic!("load test config: {e}"),
        }
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

        // No live overlay anymore; nothing visible until commit.

        // Push a newline which should cause commit of the first logical line.
        w.handle_codex_event(Event {
            id: "1".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                delta: " world\nNext".into(),
            }),
        });

        let lines = match recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("expected history after newline"),
        };
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

        let lines2 = match recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("expected history after finalize"),
        };
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
        let commit1 = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("history after code line newline"),
        };

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
        let commit2 = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("history after finalize"),
        };

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
        let lines = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("history after batch"),
        };
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(rendered.iter().any(|s| s.contains("codex")), "answer header missing");
        let committed: Vec<_> = rendered.into_iter().filter(|s| s.starts_with('l')).collect();
        assert_eq!(committed.len(), 3, "expected 3 committed lines in first batch");

        // No live overlay anymore; only committed lines appear in history.

        // Finalize: drain the remaining lines.
        w.handle_codex_event(Event {
            id: "b".into(),
            msg: EventMsg::AgentMessage(AgentMessageEvent { message: String::new() }),
        });
        let lines2 = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("history after finalize"),
        };
        let rendered2: Vec<String> = lines2
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(rendered2.iter().any(|s| s == "l4"));
        assert!(rendered2.iter().any(|s| s == "l5"));
        assert!(rendered2.last().is_some_and(|s| s.is_empty()), "expected trailing blank line after finalize");
    }

    #[test]
    fn widget_reasoning_then_answer_ordering() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let config = test_config();
        let mut w = ChatWidget::new(config.clone(), tx.clone(), None, Vec::new(), false);

        // Reasoning: one completed line then finalize.
        w.handle_codex_event(Event {
            id: "ra".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta: "think1\n".into() }),
        });
        let r_commit = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("reasoning history"),
        };
        w.handle_codex_event(Event {
            id: "ra".into(),
            msg: EventMsg::AgentReasoning(AgentReasoningEvent { text: String::new() }),
        });
        let r_final = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("reasoning finalize"),
        };

        // Answer: one completed line then finalize.
        w.handle_codex_event(Event {
            id: "ra".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: "ans1\n".into() }),
        });
        let a_commit = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("answer history"),
        };
        w.handle_codex_event(Event {
            id: "ra".into(),
            msg: EventMsg::AgentMessage(AgentMessageEvent { message: String::new() }),
        });
        let a_final = match super::recv_insert_history(&rx, 200) {
            Some(v) => v,
            None => panic!("answer finalize"),
        };

        let to_texts = |lines: &Vec<ratatui::text::Line<'static>>| -> Vec<String> {
            lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
                .collect()
        };
        let r_all = [to_texts(&r_commit), to_texts(&r_final)].concat();
        let a_all = [to_texts(&a_commit), to_texts(&a_final)].concat();

        // Expect headers present and in order: reasoning first, then answer.
        let r_header_idx = match r_all.iter().position(|s| s.contains("thinking")) {
            Some(i) => i,
            None => panic!("missing reasoning header"),
        };
        let a_header_idx = match a_all.iter().position(|s| s.contains("codex")) {
            Some(i) => i,
            None => panic!("missing answer header"),
        };
        assert!(r_all.iter().any(|s| s == "think1"), "missing reasoning content: {:?}", r_all);
        assert!(a_all.iter().any(|s| s == "ans1"), "missing answer content: {:?}", a_all);
        // Implicitly, reasoning events happened before answer events if we got here without timeouts.
        assert_eq!(r_header_idx, 0, "reasoning header should be first in its batch");
        assert_eq!(a_header_idx, 0, "answer header should be first in its batch");
    }

    #[test]
    fn header_not_repeated_across_pauses() {
        let (tx_raw, rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let config = test_config();
        let mut w = ChatWidget::new(config.clone(), tx.clone(), None, Vec::new(), false);

        // Begin reasoning, enqueue first line, start animation.
        w.handle_codex_event(Event {
            id: "r1".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta: "first\n".into() }),
        });
        // Simulate one animation tick: should emit header + first.
        w.on_commit_tick();
        let lines1 = super::recv_insert_history(&rx, 200).expect("history after first tick");
        let texts1: Vec<String> = lines1
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        assert!(texts1.iter().any(|s| s.contains("thinking")), "missing header on first tick: {texts1:?}");
        assert!(texts1.iter().any(|s| s == "first"), "missing first line: {texts1:?}");

        // Stop ticks naturally by draining queue (second tick consumes nothing).
        w.on_commit_tick();
        let _ = super::recv_insert_history(&rx, 100);

        // Later, enqueue another completed line; header must NOT repeat.
        w.handle_codex_event(Event {
            id: "r1".into(),
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta: "second\n".into() }),
        });
        w.on_commit_tick();
        let lines2 = super::recv_insert_history(&rx, 200).expect("history after second tick");
        let texts2: Vec<String> = lines2
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        let header_count2 = texts2.iter().filter(|s| s.contains("thinking")).count();
        assert_eq!(header_count2, 0, "header should not repeat after pause: {texts2:?}");
        assert!(texts2.iter().any(|s| s == "second"), "missing second line: {texts2:?}");

        // Finalize; trailing blank should be added; no extra header.
        w.handle_codex_event(Event {
            id: "r1".into(),
            msg: EventMsg::AgentReasoning(AgentReasoningEvent { text: String::new() }),
        });
        // Drain remaining with ticks.
        w.on_commit_tick();
        let lines3 = super::recv_insert_history(&rx, 200).expect("history after finalize tick");
        let texts3: Vec<String> = lines3
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.clone()).collect::<String>())
            .collect();
        let header_total = texts1
            .into_iter()
            .chain(texts2.into_iter())
            .chain(texts3.iter().cloned())
            .filter(|s| s.contains("thinking"))
            .count();
        assert_eq!(header_total, 1, "header should appear exactly once across pauses and finalize");
        assert!(texts3.last().is_some_and(|s| s.is_empty()), "expected trailing blank line");
    }
}
