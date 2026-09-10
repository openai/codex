//! Planned-action prompt framing shared by the two production reviewers.
//! Hosts retain their action serialization and truncation policies; this module
//! only renders already-bounded JSON and reasons, preserving content-item boundaries.

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;

/// Host-prepared action evidence. JSON and optional reason must already be bounded.
#[derive(Clone, PartialEq)]
pub struct PlannedAction {
    pub json: String,
    pub kind: PlannedActionKind,
    pub reason: Option<String>,
    /// Host-rendered, bounded untrusted metadata; the action JSON remains required.
    pub tool_descriptions: Option<String>,
}

// Action JSON and reasons can contain credentials; diagnostics expose only the kind.
impl std::fmt::Debug for PlannedAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedAction")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlannedActionKind {
    Command,
    TerminalInput,
    Network { has_trigger: bool },
}

/// Existing consumer-specific prompt framing, including reusable sync sessions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionPresentation {
    SyncFull,
    SyncDelta,
    Async,
}

impl PlannedAction {
    pub fn render(&self, presentation: ActionPresentation) -> Vec<String> {
        if presentation == ActionPresentation::Async {
            return vec![
                "The Codex agent has requested the following action:\n".to_owned(),
                ">>> APPROVAL REQUEST START\n".to_owned(),
                "Planned action JSON:\n".to_owned(),
                format!("{}\n", self.json),
                ">>> APPROVAL REQUEST END\n".to_owned(),
            ];
        }
        let mut items = Vec::new();
        let mut push_text = |text| items.push(text);
        match self.kind {
            PlannedActionKind::Network { has_trigger } => {
                push_text(">>> APPROVAL REQUEST START\n".to_string());
                push_text("Below is a proposed network access request under review.\n".to_string());
                if has_trigger {
                    push_text(
                    "The network access was triggered by the action in the `trigger` entry. When assessing this request, focus primarily on whether the triggering command is authorised by the user and whether it is within the rules. The user does not need to have explicitly authorised this exact network connection, as long as the network access is a reasonable consequence of the triggering command.\n\n"
                        .to_string(),
                );
                } else {
                    push_text(
                    "No trigger action was captured for this network access request. When performing the assessment, use the retained transcript and network access JSON to evaluate user authorization and risk.\n\n"
                        .to_string(),
                );
                }
                push_text(
                "Assess the exact network access below. Use read-only tool checks when local state matters.\n"
                    .to_string(),
            );
                push_text("Network access JSON:\n".to_string());
            }
            PlannedActionKind::Command | PlannedActionKind::TerminalInput => {
                push_text(
                    match presentation {
                        ActionPresentation::SyncDelta => {
                            "The Codex agent has requested the following next action:\n"
                        }
                        ActionPresentation::SyncFull | ActionPresentation::Async => {
                            "The Codex agent has requested the following action:\n"
                        }
                    }
                    .to_string(),
                );
                push_text(">>> APPROVAL REQUEST START\n".to_string());
                if let Some(reason) = &self.reason {
                    push_text("Retry reason:\n".to_string());
                    push_text(format!("{reason}\n\n"));
                }
                let action_scope = if matches!(self.kind, PlannedActionKind::TerminalInput) {
                    "Assess input to the existing terminal, not a fresh command. The `cwd` field is its launch directory; the terminal's current directory and state may have changed. Use the retained transcript and read-only checks when that state matters.\n"
                } else {
                    "Assess the exact planned action below. Use read-only tool checks when local state matters.\n"
                };
                push_text(action_scope.to_string());
                push_text("Planned action JSON:\n".to_string());
            }
        }
        push_text(format!("{}\n", self.json));
        push_text(">>> APPROVAL REQUEST END\n".to_string());
        items
    }
}

pub(crate) struct PlannedActionSection;

impl SectionContributor for PlannedActionSection {
    fn scope(&self) -> SectionScope {
        SectionScope::Shared
    }

    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        Ok(input
            .planned_action
            .cloned()
            .map(ContextSection::PlannedAction))
    }
}
