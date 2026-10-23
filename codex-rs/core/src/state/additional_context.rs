use std::collections::BTreeMap;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextUserFragment;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, AdditionalContextEntry>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
    ) -> Vec<ResponseItem> {
        let fragments = values
            .iter()
            .filter(|(key, entry)| {
                self.values.get(*key).is_none_or(|previous| {
                    previous.kind != entry.kind || previous.value != entry.value
                })
            })
            .map(|(key, entry)| match entry.kind {
                AdditionalContextKind::Untrusted => ContextualUserFragment::into(
                    AdditionalContextUserFragment::new(key.clone(), entry.value.clone()),
                ),
                AdditionalContextKind::Application => ContextualUserFragment::into(
                    AdditionalContextDeveloperFragment::new(key.clone(), entry.value.clone()),
                ),
            })
            .collect();
        self.values = values;
        fragments
    }
}
