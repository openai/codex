use std::collections::BTreeMap;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextUserFragment;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::AdditionalContextEntry;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, AdditionalContextEntry>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
    ) -> Vec<ResponseInputItem> {
        let fragments = values
            .iter()
            .filter(|(key, value)| self.values.get(*key) != Some(*value))
            .map(|(key, entry)| {
                if entry.is_untrusted {
                    AdditionalContextUserFragment::new(key.clone(), entry.value.clone())
                        .into_input_item()
                } else {
                    AdditionalContextDeveloperFragment::new(key.clone(), entry.value.clone())
                        .into_input_item()
                }
            })
            .collect();
        self.values = values;
        fragments
    }
}
