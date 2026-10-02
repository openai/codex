use super::ErasedWorldStateSection;
use super::PreviousSectionState;
use super::WorldState;
use super::WorldStateSection;
use super::WorldStateSnapshot;
use super::WorldStateUpdate;
use super::WorldStateUpdateContent;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ResponseItem;

pub(super) fn render_section_cases<'a, S: WorldStateSection>(
    cases: &[(PreviousSectionState<'a, S>, PreviousSectionState<'a, S>)],
) -> String {
    cases
        .iter()
        .map(|(before, after)| {
            let rendered = render_diff(before, after);
            let role = rendered.as_ref().map_or_else(String::new, |fragment| {
                format!(" (role - {})", fragment.role())
            });
            let content = rendered
                .as_ref()
                .map_or_else(|| "None".to_string(), |fragment| fragment.render());
            format!(
                "{} -> {}{role}\n{content}",
                render_state(before),
                render_state(after),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_state<S: WorldStateSection>(state: &PreviousSectionState<'_, S>) -> String {
    match state {
        PreviousSectionState::Absent => "Absent".to_string(),
        PreviousSectionState::Unknown => "Unknown".to_string(),
        PreviousSectionState::Known(section) => render_snapshot(*section),
    }
}

fn render_diff<S: WorldStateSection>(
    before: &PreviousSectionState<'_, S>,
    after: &PreviousSectionState<'_, S>,
) -> Option<Box<dyn ContextualUserFragment>> {
    let PreviousSectionState::Known(after) = after else {
        return None;
    };
    let previous_snapshot;
    let previous = match before {
        PreviousSectionState::Absent => PreviousSectionState::Absent,
        PreviousSectionState::Unknown => PreviousSectionState::Unknown,
        PreviousSectionState::Known(before) => {
            previous_snapshot = snapshot_value(*before);
            PreviousSectionState::Known(&previous_snapshot)
        }
    };
    let mut fragments = expect_fragments(ErasedWorldStateSection::render_diff(*after, previous).1);
    assert!(fragments.len() <= 1, "expected at most one fragment");
    fragments.pop()
}

fn render_snapshot<S: WorldStateSection>(section: &S) -> String {
    serde_json::to_string(&sort_json(snapshot_value(section)))
        .expect("world-state section snapshot should serialize")
}

fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sort_json).collect())
        }
        serde_json::Value::Object(values) => {
            let mut values = values.into_iter().collect::<Vec<_>>();
            values.sort_by(|(left, _), (right, _)| left.cmp(right));
            serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, sort_json(value)))
                    .collect(),
            )
        }
        value => value,
    }
}

fn snapshot_value<S: WorldStateSection>(section: &S) -> serde_json::Value {
    ErasedWorldStateSection::render_diff(section, PreviousSectionState::Absent)
        .0
        .expect("world-state section snapshot should serialize to a non-null value")
}

/// Keeps text-only section assertions explicit when a section gains item output.
pub(crate) fn expect_fragments(
    updates: Vec<WorldStateUpdate>,
) -> Vec<Box<dyn ContextualUserFragment>> {
    updates
        .into_iter()
        .map(|update| expect_fragment(update.content))
        .collect()
}

fn expect_fragment(content: WorldStateUpdateContent) -> Box<dyn ContextualUserFragment> {
    match content {
        WorldStateUpdateContent::Fragment(fragment) => fragment,
        WorldStateUpdateContent::Item(item) => panic!("expected a context fragment, got {item:?}"),
    }
}

/// Renders a section that is expected to emit at most one contextual fragment.
pub(crate) trait FragmentSectionTestExt: WorldStateSection {
    fn render_fragment_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> (
        Option<Self::Snapshot>,
        Option<Box<dyn ContextualUserFragment>>,
    ) {
        let (snapshot, updates) = self.render_diff(previous);
        let mut fragments = expect_fragments(updates);
        assert!(fragments.len() <= 1, "expected at most one fragment");
        (snapshot, fragments.pop())
    }
}

impl<S: WorldStateSection> FragmentSectionTestExt for S {}

impl WorldState {
    pub(crate) fn render_full_fragments(
        &self,
    ) -> (WorldStateSnapshot, Vec<Box<dyn ContextualUserFragment>>) {
        let (snapshot, updates) = self.render_full();
        let (prefix, context) = super::split_prefix_updates(updates);
        assert!(
            prefix.is_empty(),
            "expected fragments, got prefix {prefix:?}"
        );
        (snapshot, expect_fragments(context))
    }

    pub(crate) fn render_history_fragment_diff<'a>(
        &self,
        previous: Option<&WorldStateSnapshot>,
        items: impl IntoIterator<Item = &'a ResponseItem> + Clone,
    ) -> (WorldStateSnapshot, Vec<Box<dyn ContextualUserFragment>>) {
        let (snapshot, updates) = self.render_history_diff(previous, items);
        (snapshot, expect_fragments(updates))
    }
}
