mod environment;

use codex_protocol::models::ResponseItem;
use indexmap::IndexMap;
use std::any::Any;
use std::any::TypeId;

pub(crate) use environment::EnvironmentsState;

trait ErasedWorldStateSection: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn render_diff(&self, previous: Option<&dyn Any>) -> Option<ResponseItem>;
}

impl<S: WorldStateSection> ErasedWorldStateSection for S {
    fn as_any(&self) -> &dyn Any {
        self
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
/// earlier value of the same section type.
pub(crate) trait WorldStateSection: Any + Default + Send + Sync {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem>;
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
