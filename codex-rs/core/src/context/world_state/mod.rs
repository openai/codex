mod collaboration_mode;
mod environment;
mod model;
mod multi_agent_mode;
mod permissions;
mod personality;
mod realtime;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use indexmap::IndexMap;
use std::any::Any;
use std::any::TypeId;
use std::fmt;

pub(crate) use collaboration_mode::CollaborationModeState;
pub(crate) use environment::EnvironmentsState;
pub(crate) use model::ModelState;
pub(crate) use multi_agent_mode::MultiAgentModeState;
pub(crate) use permissions::PermissionsState;
pub(crate) use personality::PersonalityState;
pub(crate) use realtime::RealtimeState;

fn developer_message(text: String) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText { text }],
        phase: None,
        metadata: None,
    }
}

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

impl fmt::Debug for WorldState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorldState")
            .field("section_count", &self.sections.len())
            .finish()
    }
}

impl WorldState {
    pub(crate) fn add_section<S: WorldStateSection>(&mut self, section: S) {
        self.sections.insert(TypeId::of::<S>(), Box::new(section));
    }

    pub(crate) fn render_full(&self) -> Vec<ResponseItem> {
        self.render_diff(&Self::default())
    }

    pub(crate) fn render_diff(&self, previous: &Self) -> Vec<ResponseItem> {
        let section_items = self
            .sections
            .iter()
            .filter_map(|(type_id, section)| {
                let previous = previous
                    .sections
                    .get(type_id)
                    .map(|section| section.as_any());
                section.render_diff(previous)
            })
            .collect::<Vec<_>>();
        let mut items = Vec::with_capacity(section_items.len());
        for item in section_items {
            match (items.last_mut(), item) {
                (
                    Some(ResponseItem::Message {
                        id: None,
                        role: previous_role,
                        content: previous_content,
                        phase: None,
                        metadata: None,
                    }),
                    ResponseItem::Message {
                        id: None,
                        role,
                        content,
                        phase: None,
                        metadata: None,
                    },
                ) if *previous_role == role => previous_content.extend(content),
                (_, item) => items.push(item),
            }
        }
        items
    }
}
