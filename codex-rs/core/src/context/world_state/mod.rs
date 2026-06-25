mod agents_md;
mod environment;

use crate::context::ContextualUserFragment;
use indexmap::IndexMap;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

pub(crate) use agents_md::AgentsMdState;
pub(crate) use environment::EnvironmentsState;

trait ErasedWorldStateSection: Send + Sync {
    fn snapshot(&self) -> Option<Value>;

    fn render_diff(&self, previous: Option<&Value>) -> Option<Box<dyn ContextualUserFragment>>;
}

impl<S: WorldStateSection> ErasedWorldStateSection for S {
    fn snapshot(&self) -> Option<Value> {
        let mut snapshot = match serde_json::to_value(WorldStateSection::snapshot(self)) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                tracing::error!(
                    section_id = S::ID,
                    %err,
                    "failed to serialize world-state section snapshot"
                );
                return None;
            }
        };
        remove_null_object_fields(&mut snapshot);
        if snapshot.is_null() {
            tracing::error!(
                section_id = S::ID,
                "world-state section snapshot cannot be null"
            );
            return None;
        }
        Some(snapshot)
    }

    fn render_diff(&self, previous: Option<&Value>) -> Option<Box<dyn ContextualUserFragment>> {
        let previous = previous.and_then(|previous| {
            serde_json::from_value::<S::Snapshot>(previous.clone())
                .inspect_err(|err| {
                    tracing::warn!(
                        section_id = S::ID,
                        %err,
                        "failed to restore world-state section snapshot"
                    );
                })
                .ok()
        });
        WorldStateSection::render_diff(self, previous.as_ref())
    }
}

/// A typed portion of the state visible to the model.
///
/// Implementations own how their current state is rendered relative to an
/// earlier snapshot of the same section. `ID` is persisted in rollouts and
/// must remain stable. `Snapshot` should contain only the comparison data
/// needed to decide what the model must be told next, and must not serialize
/// to null because merge-patch nulls represent deletion.
pub(crate) trait WorldStateSection: Send + Sync + 'static {
    const ID: &'static str;
    type Snapshot: DeserializeOwned + Serialize;

    fn snapshot(&self) -> Self::Snapshot;

    fn render_diff(
        &self,
        previous: Option<&Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>>;
}

/// Live model-visible state, keyed by the same stable section IDs used in rollouts.
#[derive(Default)]
pub(crate) struct WorldState {
    sections: IndexMap<&'static str, Box<dyn ErasedWorldStateSection>>,
}

/// Compact comparison state for each model-visible world-state section.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct WorldStateSnapshot {
    sections: BTreeMap<String, Value>,
}

impl WorldStateSnapshot {
    pub(crate) fn into_value(self) -> Value {
        Value::Object(self.sections.into_iter().collect())
    }

    /// Returns the RFC 7386 merge patch that advances `previous` to `self`.
    pub(crate) fn merge_patch_from(&self, previous: &Self) -> Option<Value> {
        let previous = Value::Object(previous.sections.clone().into_iter().collect());
        let current = Value::Object(self.sections.clone().into_iter().collect());
        create_merge_patch(&previous, &current)
    }

    pub(crate) fn apply_merge_patch(&mut self, patch: &Value) -> serde_json::Result<()> {
        let mut current = self.clone().into_value();
        apply_merge_patch_value(&mut current, patch);
        *self = serde_json::from_value(current)?;
        Ok(())
    }
}

impl fmt::Debug for WorldState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldState")
            .field("section_count", &self.sections.len())
            .finish()
    }
}

impl WorldState {
    pub(crate) fn add_section<S: WorldStateSection>(&mut self, section: S) {
        let id = S::ID;
        assert!(
            !self.sections.contains_key(id),
            "duplicate world-state section ID: {id}"
        );
        self.sections.insert(id, Box::new(section));
    }

    pub(crate) fn snapshot(&self) -> WorldStateSnapshot {
        WorldStateSnapshot {
            sections: self
                .sections
                .iter()
                .filter_map(|(id, section)| {
                    section
                        .snapshot()
                        .map(|snapshot| ((*id).to_string(), snapshot))
                })
                .collect(),
        }
    }

    pub(crate) fn render_full(&self) -> Vec<Box<dyn ContextualUserFragment>> {
        self.render_diff(&WorldStateSnapshot::default())
    }

    pub(crate) fn render_diff(
        &self,
        previous: &WorldStateSnapshot,
    ) -> Vec<Box<dyn ContextualUserFragment>> {
        self.sections
            .iter()
            .filter_map(|(id, section)| section.render_diff(previous.sections.get(*id)))
            .collect()
    }
}

fn remove_null_object_fields(value: &mut Value) {
    // RFC 7386 reserves object-valued nulls for deletion, but arrays are replaced whole.
    match value {
        Value::Object(values) => {
            values.retain(|_, value| !value.is_null());
            values.values_mut().for_each(remove_null_object_fields);
        }
        Value::Array(_) => {}
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn create_merge_patch(previous: &Value, current: &Value) -> Option<Value> {
    if previous == current {
        return None;
    }

    let Value::Object(current) = current else {
        return Some(current.clone());
    };
    let previous = previous.as_object();
    let mut patch = Map::new();

    if let Some(previous) = previous {
        for key in previous.keys() {
            if !current.contains_key(key) {
                patch.insert(key.clone(), Value::Null);
            }
        }
    }

    for (key, current_value) in current {
        let Some(previous_value) = previous.and_then(|previous| previous.get(key)) else {
            patch.insert(key.clone(), current_value.clone());
            continue;
        };
        if let Some(value_patch) = create_merge_patch(previous_value, current_value) {
            patch.insert(key.clone(), value_patch);
        }
    }

    Some(Value::Object(patch))
}

fn apply_merge_patch_value(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        target.clone_from(patch);
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in patch {
            if value.is_null() {
                target.remove(key);
            } else {
                apply_merge_patch_value(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
#[path = "world_state_tests.rs"]
mod tests;
