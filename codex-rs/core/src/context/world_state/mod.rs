mod environment;
mod environment_support;

use codex_protocol::models::ResponseItem;
use indexmap::IndexMap;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::any::Any;
use std::any::TypeId;
use std::collections::BTreeMap;

pub(crate) use environment::EnvironmentsState;

trait ErasedWorldStateSection: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn name(&self) -> &'static str;

    fn json(&self) -> serde_json::Result<Value>;

    fn render_diff(&self, previous: Option<&dyn Any>) -> Option<ResponseItem>;
}

impl<S: WorldStateSection> ErasedWorldStateSection for S {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        S::NAME
    }

    fn json(&self) -> serde_json::Result<Value> {
        serde_json::to_value(self)
    }

    fn render_diff(&self, previous: Option<&dyn Any>) -> Option<ResponseItem> {
        match previous {
            Some(previous) => {
                let Some(previous) = previous.downcast_ref::<S>() else {
                    unreachable!("world-state section type must match its type ID");
                };
                WorldStateSection::render_diff(self, previous)
            }
            None => WorldStateSection::render_diff(self, &S::default()),
        }
    }
}

/// A typed portion of the state visible to the model.
///
/// Implementations own how their current state is rendered relative to an
/// earlier value of the same section type. `NAME` is the stable key used in
/// persisted JSON and must be registered in `SECTION_REGISTRY`.
pub(crate) trait WorldStateSection:
    Any + Default + DeserializeOwned + Serialize + Send + Sync
{
    const NAME: &'static str;

    fn render_diff(&self, previous: &Self) -> Option<ResponseItem>;
}

type LoadSection = fn(Value) -> serde_json::Result<Box<dyn ErasedWorldStateSection>>;

struct SectionRegistration {
    name: &'static str,
    load: LoadSection,
}

const SECTION_REGISTRY: &[SectionRegistration] = &[
    SectionRegistration {
        name: EnvironmentsState::NAME,
        load: load_section::<EnvironmentsState>,
    },
    #[cfg(test)]
    SectionRegistration {
        name: tests::AlphaSection::NAME,
        load: load_section::<tests::AlphaSection>,
    },
    #[cfg(test)]
    SectionRegistration {
        name: tests::BetaSection::NAME,
        load: load_section::<tests::BetaSection>,
    },
];

fn load_section<S: WorldStateSection>(
    value: Value,
) -> serde_json::Result<Box<dyn ErasedWorldStateSection>> {
    serde_json::from_value::<S>(value)
        .map(|section| Box::new(section) as Box<dyn ErasedWorldStateSection>)
}

/// A snapshot of the model-visible world with one section per concrete type.
#[derive(Default)]
pub(crate) struct WorldState {
    sections: IndexMap<TypeId, Box<dyn ErasedWorldStateSection>>,
}

impl WorldState {
    pub(crate) fn add_section<S: WorldStateSection>(&mut self, section: S) {
        self.sections.insert(TypeId::of::<S>(), Box::new(section));
    }

    pub(crate) fn json_full(&self) -> serde_json::Result<Value> {
        let mut sections = BTreeMap::new();
        for section in self.sections.values() {
            sections.insert(section.name(), section.json()?);
        }
        serde_json::to_value(sections)
    }

    /// Returns the JSON Merge Patch that transforms `previous` into this state.
    pub(crate) fn json_diff(&self, previous: &Self) -> serde_json::Result<Value> {
        Ok(json_merge_patch_diff(
            &previous.json_full()?,
            &self.json_full()?,
        ))
    }

    pub(crate) fn from_json(json: Value) -> serde_json::Result<Self> {
        let stored_sections = serde_json::from_value::<BTreeMap<String, Value>>(json)?;
        let mut world_state = Self::default();
        for (name, value) in stored_sections {
            let Some(registration) = SECTION_REGISTRY
                .iter()
                .find(|registration| registration.name == name)
            else {
                continue;
            };
            let section = (registration.load)(value)?;
            world_state
                .sections
                .insert(section.as_any().type_id(), section);
        }
        Ok(world_state)
    }

    /// Applies a persisted diff, discarding values that are not represented in JSON.
    pub(crate) fn apply_json_diff(&mut self, diff: &Value) -> serde_json::Result<()> {
        let mut json = self.json_full()?;
        apply_json_merge_patch(&mut json, diff);
        *self = Self::from_json(json)?;
        Ok(())
    }

    pub(crate) fn render_full(&self) -> Vec<ResponseItem> {
        self.render_diff(&Self::default())
    }

    pub(crate) fn render_diff(&self, previous: &Self) -> Vec<ResponseItem> {
        self.sections
            .iter()
            .filter_map(|(type_id, section)| {
                let previous = previous
                    .sections
                    .get(type_id)
                    .map(|section| section.as_any());
                section.render_diff(previous)
            })
            .collect()
    }
}

fn json_merge_patch_diff(previous: &Value, current: &Value) -> Value {
    if previous == current {
        return Value::Object(serde_json::Map::new());
    }

    let (Value::Object(previous), Value::Object(current)) = (previous, current) else {
        return current.clone();
    };

    let mut diff = serde_json::Map::new();
    for key in previous.keys() {
        if !current.contains_key(key) {
            diff.insert(key.clone(), Value::Null);
        }
    }
    for (key, current) in current {
        match previous.get(key) {
            Some(previous) if previous == current => {}
            Some(previous) => {
                diff.insert(key.clone(), json_merge_patch_diff(previous, current));
            }
            None => {
                diff.insert(key.clone(), current.clone());
            }
        }
    }
    Value::Object(diff)
}

fn apply_json_merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    let Value::Object(target) = target else {
        unreachable!("target was initialized as an object");
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            apply_json_merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

#[cfg(test)]
#[path = "world_state_tests.rs"]
mod tests;
