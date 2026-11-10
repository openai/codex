//! Exercises cache-prefix and evidence integrity through public collection and composition.

use codex_context_fragments::ContextualUserFragment;
use codex_guardian_context::ActionPresentation;
use codex_guardian_context::ContextBudgeting;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::ElasticBudgetConfig;
use codex_guardian_context::ElasticSection;
use codex_guardian_context::ElasticSectionLimits;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::PreviousReviews;
use codex_guardian_context::RequestBudget;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
use codex_guardian_context::TranscriptFormat;
use codex_guardian_context::TrustedSkills;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::TrustedToolSource;
use codex_guardian_context::collect_verified_answer_candidates;
use codex_guardian_context::default_registry;
use codex_guardian_context::prepare_context;
use codex_guardian_context::render_original_verified_answer;
use codex_history::RetainedContext;
use codex_history::RetainedContextEvent;
use codex_history::RetainedInputSource;
use codex_history::RetainedUserMessage;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_protocol_test_support::uuid_v7_item_id_generator_for_test;
use pretty_assertions::assert_eq;

struct History {
    items: Vec<ResponseItem>,
    retained: Option<RetainedContext>,
}

impl SectionHistory for History {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.items.iter())
    }

    fn retained_context(&self) -> Option<&RetainedContext> {
        self.retained.as_ref()
    }
}

fn user_message(texts: Vec<String>) -> ResponseItem {
    ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".to_owned(),
        content: texts
            .into_iter()
            .map(|text| ContentItem::InputText { text })
            .collect(),
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
fn changing_retained_context_and_attestations_preserves_history_before_the_current_action() {
    let instruction = "Inspect staging only. Do not publish.";
    let mut retained = RetainedContext::default();
    retained.record_user_message(
        RetainedUserMessage {
            phase: None,
            origin: codex_history::UserInputOrigin::User,
            turn_id: "turn-1".to_owned(),
            message_id: None,
            text: instruction.to_owned(),
            complete: true,
        },
        RetainedInputSource::Local(None),
        &uuid_v7_item_id_generator_for_test(),
    );
    // Legacy review history and Felix's retained, thread-owned history use the
    // same composer. Retained assistant growth and eviction must also preserve its prefix.
    for (format, retained) in [TranscriptFormat::Line, TranscriptFormat::Json]
        .into_iter()
        .flat_map(|format| {
            [None, Some(retained.clone())]
                .into_iter()
                .map(move |retained| (format, retained))
        })
    {
        let mut history = History {
            items: vec![user_message(vec![instruction.to_owned()])],
            retained,
        };
        let mut profile = ContextProfile::asynchronous();
        profile.transcript_format = format;
        let mut previous_prefix = None;
        for generation in 0..10 {
            let assistant_text = format!("Assistant progress {generation}.");
            if let Some(retained) = history.retained.as_mut() {
                retained.record_assistant_message(
                    RetainedUserMessage {
                        phase: None,
                        origin: codex_history::UserInputOrigin::User,
                        turn_id: "turn-1".to_owned(),
                        message_id: Some(format!("assistant-{generation}")),
                        text: assistant_text.clone(),
                        complete: true,
                    },
                    RetainedInputSource::Local(Some(generation + 1)),
                    &uuid_v7_item_id_generator_for_test(),
                );
            }
            let reviews =
                PreviousReviews::try_from_fragments(vec![codex_guardian_context::PreviousReview {
                    id: uuid_v7_item_id_generator_for_test()("review"),
                    fragment: format!("Host-attested decision {generation}: denied."),
                }])
                .unwrap();
            let tool = TrustedTool {
                server: format!("server-{generation}"),
                connector_id: None,
                source: TrustedToolSource::UserConfiguration("user configuration".to_owned()),
            };
            let skills = TrustedSkills {
                paths: vec![format!("/skills/skill-{generation}/SKILL.md")],
            };
            let action = PlannedAction {
                tool_descriptions: None,
                kind: PlannedActionKind::Command {
                    json: format!(r#"{{"tool":"inspect","path":"file-{generation}"}}"#),
                },
                reason: None,
            };
            let collected = default_registry()
                .prepare(&SectionInput {
                    target: ContextTarget::Async,
                    history: &history,
                    transcript: &profile.transcript,
                    root_conversation: &[],
                    root_conversation_candidates: None,
                    trusted_user_answers: &[],
                    planned_action: Some(&action),
                    permissions: None,
                    previous_reviews: Some(&reviews),
                    trusted_tool: Some(&tool),
                    trusted_skill_paths: &skills.paths,
                    images: None,
                    node_repl: None,
                })
                .unwrap();
            let context = prepare_context(
                std::borrow::Cow::Owned(collected),
                &profile,
                /*entry_number_offset*/ 0,
                ContextPresentation::Async,
                codex_guardian_context::ContextBudgeting::Fixed,
                &[],
            )
            .unwrap();
            let retained_text = context
                .retained_instructions()
                .into_user_inputs()
                .unwrap()
                .into_iter()
                .map(|input| match input {
                    UserInput::Text { text, .. } => text,
                    _ => panic!("retained context must be text"),
                })
                .collect::<Vec<_>>();
            if history.retained.is_some() {
                assert!(
                    retained_text
                        .iter()
                        .any(|text| text.contains(&assistant_text))
                );
                if generation == 9 {
                    assert!(
                        !retained_text
                            .iter()
                            .any(|text| text.contains("Assistant progress 0."))
                    );
                }
            }
            let messages = context.into_messages();
            let (prefix, suffix) = messages.split_first().expect("history prefix");
            let ResponseItem::Message { role, content, .. } = prefix else {
                panic!("history remains untrusted user evidence");
            };
            assert_eq!(role, "user");
            assert!(content.iter().any(|item| matches!(
                item,
                ContentItem::InputText { text } if text == ">>> TRANSCRIPT END\n\n"
            )));
            if let Some(previous) = previous_prefix.replace(prefix.clone()) {
                assert_eq!(prefix, &previous);
            }
            assert_eq!(
                suffix,
                [
                    reviews.into_annotated_message().into_item(),
                    ContextualUserFragment::into(tool),
                    ContextualUserFragment::into(skills),
                    user_message(
                        retained_text
                            .into_iter()
                            .chain(action.render(ActionPresentation::Async))
                            .collect(),
                    ),
                ]
            );
        }
    }
}

#[test]
fn elastic_verified_answers_preserve_question_answer_identity() {
    let answer = VerifiedAnswer {
        turn_id: "turn".to_owned(),
        call_id: "confirm".to_owned(),
        questions: vec![
            VerifiedQuestionAnswer {
                question: format!(
                    "May I delete the production database? {}",
                    "A".repeat(2_500)
                ),
                answer: "No.".to_owned(),
            },
            VerifiedQuestionAnswer {
                question: format!("May I list local files instead? {}", "B".repeat(2_500)),
                answer: "Yes.".to_owned(),
            },
        ],
    };
    let original = render_original_verified_answer(&answer);
    let mut retained = RetainedContext::default();
    retained.record(
        &RetainedContextEvent::VerifiedAnswer {
            answer,
            acceptance_order: Some(7),
        },
        &uuid_v7_item_id_generator_for_test(),
    );
    let trusted_answers = collect_verified_answer_candidates(&retained);
    let root = [GuardianRootMessage::UserInput(original.clone())];
    // The retained record must stand on its own after the raw transcript is compacted.
    let history = History {
        items: Vec::new(),
        retained: Some(retained),
    };
    let profile = ContextProfile::synchronous();
    for entry_tokens in [900, 1_800] {
        let collected = default_registry()
            .prepare(&SectionInput {
                target: ContextTarget::Sync,
                history: &history,
                transcript: &profile.transcript,
                root_conversation: &root,
                root_conversation_candidates: None,
                trusted_user_answers: &trusted_answers,
                planned_action: None,
                permissions: None,
                previous_reviews: None,
                trusted_tool: None,
                trusted_skill_paths: &[],
                images: None,
                node_repl: None,
            })
            .unwrap();
        let limits = ElasticSectionLimits::new(
            /*min_total_section_tokens*/ entry_tokens,
            /*max_section_entry_tokens*/ entry_tokens,
        )
        .unwrap();
        let config = ElasticBudgetConfig {
            sections: [
                (ElasticSection::RootConversationMessages, limits),
                (ElasticSection::TrustedUserAnswers, limits),
                (
                    ElasticSection::TranscriptMessages,
                    // The other section defaults total 9,100 tokens; minimums must sum to 128K.
                    ElasticSectionLimits::new(
                        /*min_total_section_tokens*/ 118_900 - 2 * entry_tokens,
                        /*max_section_entry_tokens*/ 5_000,
                    )
                    .unwrap(),
                ),
            ]
            .into(),
        };
        let messages = prepare_context(
            std::borrow::Cow::Owned(collected),
            &profile,
            /*entry_number_offset*/ 0,
            ContextPresentation::SyncFull {
                session_id: "worker",
            },
            ContextBudgeting::Elastic,
            &[],
        )
        .unwrap()
        .prepare_elastic_entries(&config)
        .unwrap()
        .enforce_budget(
            RequestBudget {
                max_input_tokens: 128_000,
                existing_context_tokens: 0,
            },
            "Evidence omitted.".to_owned(),
        )
        .unwrap()
        .into_messages();
        let omission = GuardianRootMessage::IncompleteVerifiedAnswers.render();
        let evidence = messages
            .into_iter()
            .flat_map(|message| match message {
                ResponseItem::Message { content, .. } => content,
                _ => panic!("text evidence"),
            })
            .filter_map(|item| match item {
                ContentItem::InputText { text }
                    if text.contains("assistant:") || text == omission =>
                {
                    Some(text)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected = if entry_tokens == 900 {
            vec![omission.clone(), omission]
        } else {
            vec![
                original.clone(),
                format!("Retained source order: 7\n{original}"),
            ]
        };
        assert_eq!(evidence, expected);
    }
}
