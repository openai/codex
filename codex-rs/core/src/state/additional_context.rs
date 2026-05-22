use std::collections::BTreeMap;

use crate::context::AdditionalContextFragment;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, String>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, String>,
    ) -> Vec<AdditionalContextFragment> {
        let fragments = values
            .iter()
            .filter(|(key, value)| self.values.get(*key) != Some(*value))
            .map(|(key, value)| AdditionalContextFragment::new(key.clone(), value.clone()))
            .collect();
        self.values = values;
        fragments
    }
}
