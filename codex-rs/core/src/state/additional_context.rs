//! Deduplicates the active client context using its selected model-visible representation.

use std::collections::BTreeMap;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextToolOutput;
use crate::context::AdditionalContextUserFragment;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;

pub(crate) enum AdditionalContextFormat {
    Messages,
    ToolOutputs,
}

#[derive(Debug, Clone, Eq)]
enum StoredAdditionalContext {
    Message(AdditionalContextEntry),
    ToolOutput(AdditionalContextToolOutput),
}

impl PartialEq for StoredAdditionalContext {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Message(left), Self::Message(right)) => {
                left.kind == right.kind && left.value == right.value
            }
            (Self::ToolOutput(left), Self::ToolOutput(right)) => left == right,
            (Self::Message(_), Self::ToolOutput(_)) | (Self::ToolOutput(_), Self::Message(_)) => {
                false
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, StoredAdditionalContext>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
        format: AdditionalContextFormat,
    ) -> Vec<ResponseItem> {
        let values =
            values
                .into_iter()
                .map(|(key, entry)| {
                    let value = match (&format, entry.kind) {
                        (
                            AdditionalContextFormat::ToolOutputs,
                            AdditionalContextKind::Untrusted,
                        ) => StoredAdditionalContext::ToolOutput(AdditionalContextToolOutput::new(
                            &key,
                            entry.name,
                            &entry.value,
                        )),
                        (AdditionalContextFormat::Messages, AdditionalContextKind::Untrusted)
                        | (AdditionalContextFormat::Messages, AdditionalContextKind::Application)
                        | (
                            AdditionalContextFormat::ToolOutputs,
                            AdditionalContextKind::Application,
                        ) => StoredAdditionalContext::Message(entry),
                    };
                    (key, value)
                })
                .collect::<BTreeMap<_, _>>();
        let fragments = values
            .iter()
            .filter(|(key, value)| self.values.get(*key) != Some(*value))
            .map(|(key, entry)| match entry {
                StoredAdditionalContext::ToolOutput(output) => output.clone().into(),
                StoredAdditionalContext::Message(entry) => match entry.kind {
                    AdditionalContextKind::Untrusted => ContextualUserFragment::into(
                        AdditionalContextUserFragment::new(key.clone(), entry.value.clone()),
                    ),
                    AdditionalContextKind::Application => ContextualUserFragment::into(
                        AdditionalContextDeveloperFragment::new(key.clone(), entry.value.clone()),
                    ),
                },
            })
            .collect();
        self.values = values;
        fragments
    }
}
