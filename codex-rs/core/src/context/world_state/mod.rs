mod environment;

use crate::context::ContextualUserFragment;
use indexmap::IndexMap;
use std::any::Any;
use std::any::TypeId;
use std::fmt;

pub(crate) use environment::EnvironmentsState;

trait ErasedWorldStateSection: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn render_diff(&self, previous: Option<&dyn Any>) -> Option<Box<dyn ContextualUserFragment>>;
}

impl<S: WorldStateSection> ErasedWorldStateSection for S {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn render_diff(&self, previous: Option<&dyn Any>) -> Option<Box<dyn ContextualUserFragment>> {
        let previous = match previous {
            Some(previous) => {
                let Some(previous) = previous.downcast_ref::<S>() else {
                    unreachable!("world-state section type must match its type ID");
                };
                Some(previous)
            }
            None => None,
        };
        WorldStateSection::render_diff(self, previous)
    }
}

/// A typed portion of the state visible to the model.
///
/// Implementations own how their current state is rendered relative to an
/// earlier value of the same section type. A missing previous value requests
/// the section's complete current representation.
pub(crate) trait WorldStateSection: Any + Send + Sync {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>>;
}

/// A snapshot of the model-visible world with one section per concrete type.
#[derive(Default)]
pub(crate) struct WorldState {
    sections: IndexMap<TypeId, Box<dyn ErasedWorldStateSection>>,
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
        self.sections.insert(TypeId::of::<S>(), Box::new(section));
    }

    pub(crate) fn render_full(&self) -> Vec<Box<dyn ContextualUserFragment>> {
        self.render_diff(&Self::default())
    }

    pub(crate) fn render_diff(&self, previous: &Self) -> Vec<Box<dyn ContextualUserFragment>> {
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
