use super::ErasedWorldStateSection;
use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;

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
    ErasedWorldStateSection::render_diff(*after, previous)
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
    ErasedWorldStateSection::snapshot(section)
        .expect("world-state section snapshot should serialize to a non-null value")
}
