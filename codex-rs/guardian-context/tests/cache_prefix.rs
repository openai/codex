//! Exercises cache-prefix stability through the public collection and composition API.

use codex_context_fragments::ContextualUserFragment;
use codex_guardian_context::ActionPresentation;
use codex_guardian_context::ContextPresentation;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::PreviousReviews;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
use codex_guardian_context::TranscriptFormat;
use codex_guardian_context::TrustedSkills;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::default_registry;
use codex_history::RetainedContext;
use codex_history::RetainedInputSource;
use codex_history::RetainedUserMessage;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
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
                );
            }
            let reviews =
                PreviousReviews::try_from_fragments(vec![codex_guardian_context::PreviousReview {
                    id: codex_protocol::ResponseItemId::new("review"),
                    fragment: format!("Host-attested decision {generation}: denied."),
                }])
                .unwrap();
            let tool = TrustedTool {
                server: format!("server-{generation}"),
                connector_id: None,
                source: "user configuration".to_owned(),
            };
            let skills = TrustedSkills {
                paths: vec![format!("/skills/skill-{generation}/SKILL.md")],
            };
            let action = PlannedAction {
                json: format!(r#"{{"tool":"inspect","path":"file-{generation}"}}"#),
                tool_descriptions: None,
                kind: PlannedActionKind::Command,
                reason: None,
            };
            let collected = default_registry()
                .prepare(&SectionInput {
                    target: ContextTarget::Async,
                    history: &history,
                    transcript: &profile.transcript,
                    root_conversation: &[],
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
            let transcript = profile.prepare_transcript(
                collected.transcript_entries(),
                /*entry_number_offset*/ 0,
            );
            let context = collected
                .compose(ContextPresentation::Async, transcript)
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
