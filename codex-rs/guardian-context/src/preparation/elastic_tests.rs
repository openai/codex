#![cfg(test)]

use super::*;
use crate::BudgetPriority;
use crate::Budgeted;
use crate::ConversationTranscriptEntryKind;
use crate::ElasticSection;
use crate::ElasticSectionLimits;
use crate::Retention;
use crate::TranscriptFormat;
use crate::TranscriptRecord;
use crate::composition::SectionOutput;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn entry_caps_bound_required_and_optional_text_preserving_metadata_and_opaque_payloads() {
    const ENTRY_TOKENS: usize = 256;
    const REPEATS: usize = 2_000;
    let required = Budgeted::required("required evidence ".repeat(REPEATS).into());
    let short = Budgeted::optional("small result".to_owned().into(), BudgetPriority::Tool);
    let record = TranscriptRecord::new(
        &ConversationTranscriptEntryKind::ToolOutput("result".to_owned()),
        /*index*/ 1,
        "quoted \"text\" 💥\n".repeat(REPEATS),
        /*retained_source_order*/ None,
        TranscriptFormat::Json,
        "\n",
    );
    let mut expected_record = serde_json::to_value(&record).unwrap();
    let mut expected_source = codex_history::RetainedSource {
        id: codex_history::RetainedSourceId {
            message_id: "assistant".to_owned(),
            turn_id: "turn".to_owned(),
            role: codex_history::RetainedSourceRole::Assistant,
        },
        revision: codex_protocol::ResponseItemId::from_server("revision".to_owned()),
        complete: true,
    };
    let mut required = required;
    required.source = Some(expected_source.clone());
    let mut optional = Budgeted::optional(record.into(), BudgetPriority::Tool);
    optional.source = Some(expected_source.clone());
    let image = Budgeted::optional(
        SectionContent::Other(ContentItem::InputImage {
            image: codex_protocol::models::ImageReference::Inline {
                image_url: "data:image/png;base64,original".to_owned(),
            },
            detail: None,
        }),
        BudgetPriority::Image,
    );
    let native: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "function_call", "call_id": "native", "name": "send_message",
        "arguments": "opaque arguments ".repeat(REPEATS),
        "encrypted_function_args": ["opaque-original"],
    }))
    .unwrap();
    let context = ComposedContext {
        sections: vec![
            SectionOutput {
                id: "conversation_transcript",
                delivery: SectionDelivery::UserContent(vec![required, short.clone(), optional]),
            },
            SectionOutput {
                id: "transcript_images",
                delivery: SectionDelivery::UserContent(vec![image.clone()]),
            },
            SectionOutput {
                id: "conversation_transcript",
                delivery: SectionDelivery::Message(Budgeted::optional(
                    Box::new(native.clone().into()),
                    BudgetPriority::Tool,
                )),
            },
        ],
        truncations: Vec::new(),
    };
    assert!(
        context
            .clone()
            .prepare_elastic_entries(&ElasticBudgetConfig::default())
            .is_ok()
    );
    let limits = ElasticSectionLimits::new(ENTRY_TOKENS, ENTRY_TOKENS).unwrap();
    let config = crate::budget::tests::complete_minimum_budget(ElasticBudgetConfig {
        sections: [
            (ElasticSection::TranscriptTools, limits),
            (ElasticSection::TranscriptMessages, limits),
        ]
        .into(),
        ..Default::default()
    });
    let prepared = context.prepare_elastic_entries(&config).unwrap();
    let SectionDelivery::UserContent(items) = &prepared.context.sections[0].delivery else {
        panic!("transcript");
    };
    assert_eq!(items[0].retention, Retention::Required);
    assert!(
        section_content_tokens(&items[0].content) + content_framing_tokens(/*item_count*/ 1)
            <= ENTRY_TOKENS
    );
    assert!(
        matches!(&items[0].content, SectionContent::Other(ContentItem::InputText { text }) if text.contains("<truncated"))
    );
    assert_eq!(items[1], short);
    let capped = items.last().unwrap();
    let SectionContent::Transcript(record) = &capped.content else {
        panic!("JSON record");
    };
    assert!(record.text().contains("<truncated"));
    expected_record["text"] = record.text().into();
    assert_eq!(serde_json::to_value(record).unwrap(), expected_record);
    assert!(
        section_content_tokens(&capped.content) + content_framing_tokens(/*item_count*/ 1)
            <= ENTRY_TOKENS
    );
    expected_source.complete = false;
    assert_eq!(items[0].source, Some(expected_source.clone()));
    assert_eq!(capped.source, Some(expected_source));
    assert!(prepared.context.sections[1].delivery == SectionDelivery::UserContent(vec![image]));
    assert!(
        prepared.context.sections.last().unwrap().delivery
            == SectionDelivery::Message(Budgeted::optional(
                Box::new(native.into()),
                BudgetPriority::Tool
            ))
    );
}

#[test]
fn exact_actions_and_permissions_stay_whole_while_supporting_context_is_capped() {
    const ENTRY_TOKENS: usize = 128;
    const REPEATS: usize = 1_000;
    let json = serde_json::json!({"tool": "write_record", "arguments": {
        "description": "exact action payload ".repeat(REPEATS)
    }})
    .to_string();
    let supporting = "supporting context ".repeat(REPEATS);
    let permissions = "denied path ".repeat(REPEATS);
    let (permission_start, permission_end) =
        <crate::PermissionContext as codex_context_fragments::ContextualUserFragment>::type_markers(
        );
    for kind in [
        crate::PlannedActionKind::Command { json: json.clone() },
        crate::PlannedActionKind::TerminalInput { json: json.clone() },
        crate::PlannedActionKind::Network {
            json: json.clone(),
            has_trigger: true,
        },
    ] {
        let mut collected = crate::CollectedContext {
            sections: vec![
                crate::ContextSection::PermissionContext {
                    items: vec![
                        permission_start.into(),
                        permissions.clone(),
                        permission_end.into(),
                    ],
                },
                crate::ContextSection::SenderUserMessages {
                    items: vec![supporting.clone()],
                },
                crate::ContextSection::PlannedAction(crate::PlannedAction {
                    kind,
                    reason: Some(supporting.clone()),
                    tool_descriptions: crate::GuardianToolDescriptions::new(
                        Some(&supporting),
                        /*connector*/ None,
                    ),
                }),
            ],
        };
        crate::preparation::prepare_sections(&mut collected, crate::ContextBudgeting::Elastic)
            .unwrap();
        let context = collected
            .compose_prepared(
                crate::ContextPresentation::SyncFull {
                    session_id: "review",
                },
                crate::PreparedTranscript {
                    items: Vec::new(),
                    omission_note: None,
                    truncations: Vec::new(),
                },
            )
            .unwrap();
        let limits = ElasticSectionLimits::new(ENTRY_TOKENS, ENTRY_TOKENS)
            .and_then(|limits| limits.with_max_total_section_tokens(ENTRY_TOKENS))
            .unwrap();
        let config = crate::budget::tests::complete_minimum_budget(ElasticBudgetConfig {
            sections: [
                ElasticSection::PlannedAction,
                ElasticSection::PermissionContext,
                ElasticSection::SenderUserMessages,
            ]
            .map(|section| (section, limits))
            .into(),
            ..Default::default()
        });
        let prepared = context.prepare_elastic_entries(&config).unwrap();
        let SectionDelivery::UserContent(items) =
            &prepared.context.sections.last().unwrap().delivery
        else {
            panic!("planned action");
        };
        let expected = ContentItem::InputText {
            text: format!("{json}\n"),
        };
        let permission_tokens = section_content_tokens(&permissions.clone().into());
        for item in items {
            if !item.protected {
                assert!(
                    section_content_tokens(&item.content)
                        + content_framing_tokens(/*item_count*/ 1)
                        <= ENTRY_TOKENS
                );
            }
        }
        assert!(
            prepared
                .clone()
                .enforce_budget(
                    crate::RequestBudget {
                        max_input_tokens: crate::budget::content_tokens(&expected)
                            + permission_tokens
                            - 1,
                        existing_context_tokens: 0,
                    },
                    "Evidence omitted.".to_owned(),
                )
                .is_err()
        );
        let budget = crate::RequestBudget {
            max_input_tokens: prepared.context.estimated_tokens() + ENTRY_TOKENS,
            existing_context_tokens: 0,
        };
        let messages = prepared
            .enforce_budget(budget, "Evidence omitted.".to_owned())
            .unwrap()
            .into_messages();
        assert!(messages.iter().any(|message| matches!(message,
            ResponseItem::Message { content, .. } if content.contains(&expected))));
        assert!(
            !serde_json::to_string(&messages)
                .unwrap()
                .contains(&supporting)
        );
        for (start, end, body) in [
            (permission_start, permission_end, permissions.as_str()),
            (
                ">>> SENDER EVIDENCE CONTEXT START\nHost:",
                ">>> SENDER EVIDENCE CONTEXT END\n",
                "supporting context",
            ),
        ] {
            assert!(messages.iter().any(|message| matches!(message,
                ResponseItem::Message { content, .. } if
                    content.iter().any(|part| matches!(part, ContentItem::InputText { text } if text.starts_with(start))) &&
                    content.iter().any(|part| matches!(part, ContentItem::InputText { text } if text == end)) &&
                    content.iter().any(|part| matches!(part, ContentItem::InputText { text } if text.contains(body))))));
        }
    }
}
