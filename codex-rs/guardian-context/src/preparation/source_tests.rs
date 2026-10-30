#![cfg(test)]

//! Checks that elastic preparation keeps captured root and REPL evidence.

use super::*;
use crate::GuardianRootMessage;
use crate::NodeReplContext;
use crate::NodeReplResponse;
use crate::NodeReplReviewEvidenceMode;
use crate::RootConversationCandidates;
use crate::RootMessageCandidate;
use codex_history::RetainedContextOrder;
use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[test]
fn elastic_root_preparation_preserves_captured_messages() {
    const ROOT_RECORDS: usize = 20;
    const TEXT_REPEATS: usize = 1_000;
    const LEGACY_SOURCE_TOKENS: usize = 900;
    let text = "root evidence ".repeat(TEXT_REPEATS);
    let legacy = codex_history::RetainedUserMessage {
        turn_id: "legacy-turn".to_owned(),
        message_id: None,
        text: text.clone(),
        complete: false,
        origin: codex_history::UserInputOrigin::User,
        phase: None,
    };
    let retained = codex_history::RetainedUserMessage {
        text: crate::truncate_text(&text, LEGACY_SOURCE_TOKENS),
        ..legacy.clone()
    };
    let mut input = RootConversationCandidates {
        user_messages: (0..ROOT_RECORDS)
            .map(|index| RootMessageCandidate {
                order: Some(RetainedContextOrder::Local(u64::try_from(index).unwrap())),
                message: GuardianRootMessage::User(text.clone()),
            })
            .collect(),
        assistant_messages: Vec::new(),
        retained_assistants: Vec::new(),
        delivered_messages: Vec::new(),
        retained_user_sources: vec![retained.clone()],
        handoff_boundaries: Vec::new(),
        latest_user_turn_id: None,
        missing_root_instructions: true,
        missing_assistant_context: false,
        missing_verified_answers: false,
    };
    input.user_messages[0].message = GuardianRootMessage::User(retained.text.clone());
    let fixed = root::prepare_root_context(&input, vec![legacy.clone()], ContextBudgeting::Fixed);
    let elastic = root::prepare_root_context(&input, vec![legacy], ContextBudgeting::Elastic);
    let mut expected = vec![GuardianRootMessage::User(text); ROOT_RECORDS];
    expected[0] = GuardianRootMessage::User(retained.text);
    expected.insert(
        /*index*/ 0,
        GuardianRootMessage::IncompleteRootInstructions,
    );
    expected.insert(/*index*/ 0, GuardianRootMessage::RetainedContextScope);
    assert_eq!(elastic.messages, expected);
    assert!(fixed.messages.len() < elastic.messages.len());
}

#[test]
fn elastic_repl_rendering_keeps_stored_text_images_and_omissions() {
    const TEXT_BYTES: usize = 32_001;
    const STORAGE_OMISSIONS: u64 = 7;
    let text = "x".repeat(TEXT_BYTES);
    let image = UserInput::Image {
        image: ImageReference::Inline {
            image_url: "data:image/png;base64,original".to_owned(),
        },
        detail: None,
    };
    let responses = [Arc::new(NodeReplResponse {
        sequence: 1,
        provenance: "source".to_owned(),
        items: vec![
            UserInput::Text {
                text: text.clone(),
                text_elements: Vec::new(),
            },
            image.clone(),
        ],
    })];
    for mode in [
        NodeReplReviewEvidenceMode::TextOnly,
        NodeReplReviewEvidenceMode::Multimodal,
    ] {
        let [fixed, elastic] =
            [ContextBudgeting::Fixed, ContextBudgeting::Elastic].map(|budgeting| {
                NodeReplContext {
                    responses: &responses,
                    omitted_responses: STORAGE_OMISSIONS,
                    mode,
                    budgeting,
                }
                .render_inputs()
            });
        let rendered = serde_json::to_string(&elastic).unwrap();
        assert!(!serde_json::to_string(&fixed).unwrap().contains(&text));
        assert!(rendered.contains(&text));
        assert!(rendered.contains(&format!("node_repl_responses=\\\"{STORAGE_OMISSIONS}\\\"")));
        assert_eq!(
            elastic.contains(&image),
            mode == NodeReplReviewEvidenceMode::Multimodal
        );
    }
}
