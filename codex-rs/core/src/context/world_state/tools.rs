use super::PreviousSectionState;
use super::WorldStateContextFragment;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::environment_context::push_xml_escaped_text;
use codex_extension_api::RenderedWorldStateFragment;
use codex_protocol::protocol::TOOLS_CLOSE_TAG;
use codex_protocol::protocol::TOOLS_OPEN_TAG;
use std::collections::BTreeMap;

const MAX_RENDERED_FRAGMENT_BYTES: usize = 4 * 1024;
const MAX_NAMESPACE_DESCRIPTION_CHARS: usize = 250;
const OMITTED_LINE_RESERVE_BYTES: usize = 64;

/// Deferred tool namespaces visible to the model for one sampling step.
#[derive(Debug, Default)]
pub(crate) struct ToolsState {
    deferred_namespaces: BTreeMap<String, String>,
}

impl ToolsState {
    pub(crate) fn new(deferred_namespaces: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            deferred_namespaces: deferred_namespaces
                .into_iter()
                .map(|(namespace, description)| {
                    let description = description
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .chars()
                        .take(MAX_NAMESPACE_DESCRIPTION_CHARS)
                        .collect();
                    (namespace, description)
                })
                .collect(),
        }
    }
}

impl WorldStateSection for ToolsState {
    const ID: &'static str = "tools";
    // Object-valued entries let RFC 7386 patches add and remove namespaces individually.
    type Snapshot = BTreeMap<String, String>;

    fn snapshot(&self) -> Self::Snapshot {
        self.deferred_namespaces.clone()
    }

    fn should_persist(&self) -> bool {
        !self.deferred_namespaces.is_empty()
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let current = self.snapshot();
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &current)
            || self.deferred_namespaces.is_empty()
                && matches!(
                    previous,
                    PreviousSectionState::Absent | PreviousSectionState::Unknown
                )
        {
            return None;
        }

        let body = match previous {
            PreviousSectionState::Absent | PreviousSectionState::Unknown => {
                render_namespace_groups(
                    &[("Deferred tool namespaces", &self.deferred_namespaces)],
                    self.deferred_namespaces.is_empty(),
                )
            }
            PreviousSectionState::Known(previous) => {
                let added = self
                    .deferred_namespaces
                    .iter()
                    .filter(|(namespace, description)| {
                        previous.get(*namespace) != Some(*description)
                    })
                    .map(|(namespace, description)| (namespace.clone(), description.clone()))
                    .collect();
                let removed = previous
                    .iter()
                    .filter(|(namespace, _)| !self.deferred_namespaces.contains_key(*namespace))
                    .map(|(namespace, description)| (namespace.clone(), description.clone()))
                    .collect();
                render_namespace_groups(
                    &[
                        ("Added deferred tool namespaces", &added),
                        ("Removed deferred tool namespaces", &removed),
                    ],
                    self.deferred_namespaces.is_empty(),
                )
            }
        };
        Some(Box::new(WorldStateContextFragment(
            RenderedWorldStateFragment::new("developer", (TOOLS_OPEN_TAG, TOOLS_CLOSE_TAG), body),
        )))
    }
}

fn render_namespace_groups(
    groups: &[(&'static str, &BTreeMap<String, String>)],
    current_is_empty: bool,
) -> String {
    let body_budget =
        MAX_RENDERED_FRAGMENT_BYTES.saturating_sub(TOOLS_OPEN_TAG.len() + TOOLS_CLOSE_TAG.len());
    let empty_state = current_is_empty.then_some("No deferred tool namespaces remain.\n");
    let fixed_bytes = 1
        + groups
            .iter()
            .filter(|(_, namespaces)| !namespaces.is_empty())
            .map(|(label, _)| label.len() + ":\n".len() + OMITTED_LINE_RESERVE_BYTES)
            .sum::<usize>()
        + empty_state.map_or(0, str::len);
    let mut remaining_entry_bytes = body_budget.saturating_sub(fixed_bytes);
    let mut rendered = "\n".to_string();

    for (label, namespaces) in groups {
        if namespaces.is_empty() {
            continue;
        }
        rendered.push_str(label);
        rendered.push_str(":\n");
        let mut omitted = 0usize;
        for (namespace, description) in *namespaces {
            let entry = rendered_namespace(namespace, description);
            if entry.len() <= remaining_entry_bytes {
                remaining_entry_bytes -= entry.len();
                rendered.push_str(&entry);
            } else {
                omitted += 1;
            }
        }
        if omitted > 0 {
            rendered.push_str("... ");
            rendered.push_str(&omitted.to_string());
            rendered.push_str(" additional namespaces omitted.\n");
        }
    }
    if let Some(empty_state) = empty_state {
        rendered.push_str(empty_state);
    }
    rendered
}

fn rendered_namespace(namespace: &str, description: &str) -> String {
    let mut rendered = "- ".to_string();
    push_xml_escaped_text(&mut rendered, namespace);
    if !description.is_empty() {
        rendered.push_str(": ");
        push_xml_escaped_text(&mut rendered, description);
    }
    rendered.push('\n');
    rendered
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
