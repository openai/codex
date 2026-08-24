use super::*;
use crate::mention_codec::LinkedMention;
use crate::mention_codec::decode_history_mentions_with_at_mentions;
use crate::mention_codec::encode_history_mentions_at_elements;
use codex_protocol::user_input::TextElement;
use pretty_assertions::assert_eq;

#[test]
fn task_mention_history_round_trips_multiword_and_escaped_titles() {
    let title = "Fix ]( parser \\ safely";
    let text = format!("Inspect x@{title} next");
    let mention = LinkedMention {
        sigil: '@',
        mention: title.to_string(),
        path: "thread://task-123".to_string(),
    };

    let encoded = encode_history_mentions_at_elements(
        &text,
        std::slice::from_ref(&mention),
        &[TextElement::new(
            ("Inspect x".len().."Inspect x@".len() + title.len()).into(),
            /*placeholder*/ None,
        )],
    );
    assert_eq!(
        encoded,
        "Inspect x[@Fix \\]\\( parser \\\\ safely](thread://task-123) next"
    );
    let decoded =
        decode_history_mentions_with_at_mentions(&encoded, /*at_mentions_enabled*/ true);
    assert_eq!(decoded.text, text);
    assert_eq!(decoded.mentions, vec![mention]);
    assert!(
        parse_task_link(
            &format!(
                "[@{}](thread://task-123)",
                "x".repeat(MAX_TASK_TITLE_CHARS + 1)
            ),
            /*start*/ 0,
        )
        .is_none()
    );
    for path in ["thread://", "thread://../settings", "thread://task?target"] {
        assert_eq!(valid_thread_path(path), None);
    }
    assert!(valid_thread_path(&format!("thread://{}", "x".repeat(65))).is_none());
    let malformed = "[@é](thread://task-123)";
    for end in [3, malformed.len() + 1] {
        assert_eq!(
            decode_task_links(
                malformed,
                vec![TextElement::new((0..end).into(), /*placeholder*/ None)],
            ),
            (malformed.to_string(), Vec::new())
        );
    }
    let literal = "[@actual](thread://task-456)";
    assert_eq!(
        decode_task_links(
            &format!("{literal} {literal}"),
            vec![
                TextElement::new((0..literal.len()).into(), /*placeholder*/ None),
                TextElement::new(
                    (literal.len() + 1..literal.len() * 2 + 1).into(),
                    Some("@actual".to_string()),
                ),
            ],
        )
        .0,
        format!("{literal} @actual")
    );
}

#[test]
fn task_reference_context_deduplicates_and_merges_with_ide_context() {
    let title = "Review the migration";
    let visible = format!("Compare plain @{title} with plugin @{title} and selected @{title}x");
    let binding = MentionBinding {
        sigil: '@',
        mention: title.to_string(),
        path: "thread://task-123".to_string(),
    };
    let text = format!("# IDE: @{title}\n## My request for Codex:\n{visible}");
    let plugin_start = text.find("plugin @").expect("plugin mention") + "plugin ".len();
    let selected_start = text.rfind(&format!("@{title}")).expect("selected task");
    let mut items = vec![UserInput::Text {
        text,
        text_elements: [plugin_start, selected_start]
            .into_iter()
            .map(|start| {
                codex_app_server_protocol::TextElement::new(
                    ByteRange {
                        start,
                        end: start + title.len() + 1,
                    },
                    Some(format!("@{title}")),
                )
            })
            .collect(),
    }];

    apply_task_references(
        &mut items,
        &[
            MentionBinding {
                sigil: '@',
                mention: title.to_string(),
                path: "plugin://sample@test".to_string(),
            },
            binding.clone(),
            binding,
        ],
        /*current_thread_id*/ None,
    );

    let [UserInput::Text { text, .. }] = items.as_slice() else {
        panic!("expected text with deduplicated task references");
    };
    assert!(text.starts_with("# IDE: @Review the migration\n## Referenced chats"));
    assert_eq!(text.matches("## My request for Codex:").count(), 1);
    assert_eq!(text.matches("\"threadId\":\"task-123\"").count(), 1);
    assert!(text.contains("MUST call `read_thread`"));
    assert!(text.ends_with(
        "Compare plain @Review the migration with plugin @Review the migration and selected [@Review the migration](thread://task-123)x"
    ));

    for id_len in [36, 64] {
        let bindings = (0..MAX_REFERENCED_TASKS)
            .map(|index| MentionBinding {
                sigil: '@',
                mention: "task".to_string(),
                path: format!("thread://{index:0id_len$}"),
            })
            .collect::<Vec<_>>();
        let mut items = [UserInput::Text {
            text: String::new(),
            text_elements: Vec::new(),
        }];
        apply_task_references(&mut items, &bindings, /*current_thread_id*/ None);
        let UserInput::Text { text, .. } = &items[0] else {
            unreachable!();
        };
        assert_eq!(
            text.matches("\"threadId\":").count(),
            MAX_REFERENCED_TASKS.min(MAX_REFERENCED_THREAD_ID_BYTES / id_len)
        );
    }
}

#[test]
fn task_reference_heading_inside_selected_title_is_not_a_context_boundary() {
    let title = format!("Review {REQUEST_HEADING} carefully");
    let mut items = vec![UserInput::Text {
        text: format!("@{title}"),
        text_elements: vec![codex_app_server_protocol::TextElement::new(
            ByteRange {
                start: 0,
                end: title.len() + 1,
            },
            /*placeholder*/ None,
        )],
    }];

    apply_task_references(
        &mut items,
        &[MentionBinding {
            sigil: '@',
            mention: title.clone(),
            path: "thread://task-123".to_string(),
        }],
        /*current_thread_id*/ None,
    );

    let [UserInput::Text { text, .. }] = items.as_slice() else {
        panic!("expected text with a task reference");
    };
    assert!(text.starts_with("## Referenced chats with Codex:"));
    assert!(text.ends_with(&format!("[@{title}](thread://task-123)")));
}
