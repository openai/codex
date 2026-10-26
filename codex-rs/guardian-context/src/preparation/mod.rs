//! Prepares synchronous request evidence before final aggregate admission.
//! Collection keeps captured sources intact. This owner applies text and image policy;
//! delivery checks repeat against current reviewer history after compaction.

pub(crate) mod root;

use crate::CollectedContext;
use crate::ContextSection;
use crate::GuardianRootMessage;
use crate::RetainedInstructionItem;
use crate::retained_instructions::RetainedInstructionKind;
use crate::retained_instructions::USER_OMISSION;
use codex_protocol::protocol::TruncationPolicy;

pub(crate) const MAX_INSTRUCTION_TOKENS: usize = 900;

/// Applies synchronous section policy to the already bounded collection snapshot.
/// The host prepares its borrowed transcript selection before consuming the collection.
pub(crate) fn prepare_sync_sections(context: &mut CollectedContext) {
    {
        let max_bytes = TruncationPolicy::Tokens(MAX_INSTRUCTION_TOKENS).byte_budget();
        for section in &mut context.sections {
            if let ContextSection::RetainedUserInstructions { items } = section {
                let assistant_notice = GuardianRootMessage::IncompleteAssistantContext.render();
                let mut user_omitted = false;
                let mut assistant_omitted = false;
                items.retain(|item| {
                    if item.kind == RetainedInstructionKind::UserInstruction
                        && item.content.len() > max_bytes
                    {
                        user_omitted = true;
                        return false;
                    }
                    if item.kind == RetainedInstructionKind::AssistantContext
                        && !assistant_fits(item.original_assistant_bytes, item.content.len())
                    {
                        assistant_omitted = true;
                        return false;
                    }
                    if item.kind == RetainedInstructionKind::HostNotice {
                        if item.content == USER_OMISSION {
                            user_omitted = true;
                            return false;
                        }
                        if item.content == assistant_notice {
                            assistant_omitted = true;
                            return false;
                        }
                    }
                    true
                });
                let index = usize::from(
                    items
                        .first()
                        .is_some_and(|item| item.kind == RetainedInstructionKind::Framing),
                );
                if user_omitted {
                    items.insert(
                        index,
                        RetainedInstructionItem {
                            content: USER_OMISSION.to_owned(),
                            original_assistant_bytes: 0,
                            kind: RetainedInstructionKind::HostNotice,
                            source: None,
                        },
                    );
                }
                if assistant_omitted {
                    items.insert(
                        index,
                        RetainedInstructionItem {
                            content: assistant_notice,
                            original_assistant_bytes: 0,
                            kind: RetainedInstructionKind::HostNotice,
                            source: None,
                        },
                    );
                }
            }
        }
    }
    {
        for section in &mut context.sections {
            if let ContextSection::TrustedUserAnswers { items } = section {
                let mut omitted = false;
                items.retain(|item| {
                    let keep = item.answer_bytes.is_none_or(answer_fits);
                    omitted |= !keep;
                    keep
                });
                let notice = GuardianRootMessage::IncompleteVerifiedAnswers.render();
                if omitted && !items.iter().any(|item| item.text == notice) {
                    // Framing precedes the same whole-record omission notice as before.
                    let index = usize::from(items.first().is_some_and(|item| {
                        item.text == crate::authorization::TRUSTED_ANSWERS_START
                    }));
                    items.insert(index, notice.into());
                }
            }
        }
    }
    for section in &mut context.sections {
        if let ContextSection::NodeReplEvidence(evidence) = section
            && let crate::node_repl::NodeReplEvidenceContent::Collected {
                responses,
                omitted_responses,
                mode,
            } = &evidence.content
        {
            let snapshot = crate::NodeReplContext {
                responses,
                omitted_responses: *omitted_responses,
                mode: *mode,
            };
            let items = snapshot.render_inputs();
            evidence.content = crate::node_repl::NodeReplEvidenceContent::Prepared(items);
        }
    }
}

/// The original role-labeled message reserves the same 32 bytes before order framing.
pub(crate) fn assistant_fits(original_bytes: usize, rendered_bytes: usize) -> bool {
    let maximum = TruncationPolicy::Tokens(MAX_INSTRUCTION_TOKENS).byte_budget();
    original_bytes + 32 <= maximum && rendered_bytes <= maximum
}

const MAX_ANSWER_TOKENS: usize = 900;

pub(crate) fn answer_fits(bytes: usize) -> bool {
    bytes != 0 && bytes <= TruncationPolicy::Tokens(MAX_ANSWER_TOKENS).byte_budget()
}
