//! Diffs the one-level Responses Lite catalog: namespaces of callable tools and built-ins.
//! Missing state starts a fresh catalog; this section does not migrate legacy history.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use crate::context::DeveloperInstructions;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use std::collections::BTreeMap;

/// The exact serialized Responses Lite declarations visible in one captured step.
pub(crate) struct TopLevelToolsState {
    definitions: Vec<Value>,
    hashes: BTreeMap<String, WorldStateHash>,
}

impl TopLevelToolsState {
    #[expect(
        clippy::expect_used,
        reason = "the Responses Lite serializer emits objects with array-valued namespace members"
    )]
    pub(crate) fn new(definitions: Vec<Value>) -> Result<Self> {
        let mut hashes = BTreeMap::new();
        let mut insert_hash = |name: String, definition: &Value| -> Result<()> {
            let hash = WorldStateHash::from_json(definition);
            if hashes.insert(name.clone(), hash).is_some() {
                return Err(CodexErrorDetails::ToolCollision(name).into());
            }
            Ok(())
        };
        for definition in &definitions {
            let name = definition_name(definition);
            let mut header = definition.clone();
            if let Some(tools) = header
                .as_object_mut()
                .expect("serialized tool object")
                .remove("tools")
            {
                for tool in tools.as_array().expect("serialized namespace members") {
                    insert_hash(format!("{name}.{}", definition_name(tool)), tool)?;
                }
            }
            insert_hash(name.to_string(), &header)?;
        }
        Ok(Self {
            definitions,
            hashes,
        })
    }
}

// Input comes from create_tools_json_for_responses_lite: namespaces and their members have
// names; built-ins are identified by type. Namespace members cannot contain namespaces.
#[expect(
    clippy::expect_used,
    reason = "the Responses Lite serializer gives every declaration a name or type"
)]
fn definition_name(definition: &Value) -> &str {
    definition["name"]
        .as_str()
        .unwrap_or_else(|| definition["type"].as_str().expect("serialized tool type"))
}

impl WorldStateSection for TopLevelToolsState {
    const ID: &'static str = "top_level_tools";
    type Snapshot = BTreeMap<String, WorldStateHash>;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let previous = match previous {
            PreviousSectionState::Known(previous) => Some(previous),
            PreviousSectionState::Absent | PreviousSectionState::Unknown => None,
        };
        let changed =
            |name: &str| previous.and_then(|previous| previous.get(name)) != self.hashes.get(name);
        let mut namespace_updates = Vec::new();
        let mut tools = Vec::new();
        for definition in &self.definitions {
            let name = definition_name(definition);
            let Some(members) = definition["tools"].as_array() else {
                if changed(name) {
                    tools.push(definition.clone());
                }
                continue;
            };
            let members = members
                .iter()
                .filter(|tool| changed(&format!("{name}.{}", definition_name(tool))))
                .cloned()
                .collect::<Vec<_>>();
            if !members.is_empty() {
                let mut namespace = definition.clone();
                namespace["tools"] = Value::Array(members);
                tools.push(namespace);
            } else if changed(name) {
                // Namespace declarations require a member. Update metadata as text without
                // repeating unchanged tool definitions.
                let instructions = definition["description"].as_str().unwrap_or_default();
                let text = if instructions.is_empty() {
                    format!("The {name} namespace no longer has additional instructions.")
                } else {
                    format!("Updated instructions for the {name} namespace:\n{instructions}")
                };
                namespace_updates
                    .push(WorldStateUpdate::fragment(DeveloperInstructions::new(text)));
            }
        }
        let mut updates = Vec::new();
        if !tools.is_empty() {
            updates.push(WorldStateUpdate::prefix_item(
                ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_string(),
                    tools,
                },
            ));
        }
        updates.extend(namespace_updates);
        if let Some(previous) = previous {
            let removed = previous
                .keys()
                .filter(|key| !self.hashes.contains_key(*key))
                .cloned()
                .collect::<Vec<_>>();
            if !removed.is_empty() {
                updates.push(WorldStateUpdate::fragment(RemovedTools(removed)).standalone());
            }
        }
        // Persist the empty map too: it is a known empty catalog, not missing state.
        (Some(self.hashes.clone()), updates)
    }
}

struct RemovedTools(Vec<String>);

impl ContextualUserFragment for RemovedTools {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("tools.removed_definition".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let names = self.0.join("\n- ");
        format!("The following tools are no longer available. Do not call them:\n- {names}")
    }
}

#[cfg(test)]
#[path = "top_level_tools_tests.rs"]
mod tests;
