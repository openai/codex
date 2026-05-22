use std::collections::BTreeMap;

use crate::context::AdditionalContextFragment;
use codex_protocol::protocol::AdditionalContextEntry;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, AdditionalContextEntry>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
    ) -> Vec<AdditionalContextFragment> {
        let fragments = values
            .iter()
            .filter(|(key, value)| self.values.get(*key) != Some(*value))
            .map(|(key, entry)| {
                AdditionalContextFragment::new(key.clone(), entry.value.clone(), entry.is_untrusted)
            })
            .collect();
        self.values = values;
        fragments
    }
}
