use super::*;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Deserialize, Serialize)]
struct TestSection {
    value: String,
    optional: Option<String>,
    array: Vec<Value>,
}

impl WorldStateSection for TestSection {
    const ID: &'static str = "test";
    type Snapshot = Self;

    fn snapshot(&self) -> Self::Snapshot {
        self.clone()
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        match previous {
            PreviousSectionState::Known(previous) if self.value != previous.value => {
                Some(Box::new(TestFragment(self.value.clone())))
            }
            PreviousSectionState::Unknown => Some(Box::new(TestFragment("unknown".to_string()))),
            PreviousSectionState::Absent | PreviousSectionState::Known(_) => None,
        }
    }
}

struct TestFragment(String);

impl ContextualUserFragment for TestFragment {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.0.clone()
    }
}

struct DuplicateTestSection;

impl WorldStateSection for DuplicateTestSection {
    const ID: &'static str = "test";
    type Snapshot = ();

    fn snapshot(&self) -> Self::Snapshot {}

    fn render_diff(
        &self,
        _previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        None
    }
}

#[test]
fn snapshot_uses_stable_section_ids_and_omits_null_fields() {
    let mut world_state = WorldState::default();
    world_state.add_section(TestSection {
        value: "current".to_string(),
        optional: None,
        array: vec![json!({"value": null})],
    });

    assert_eq!(
        serde_json::to_value(world_state.snapshot()).expect("serialize world-state snapshot"),
        json!({"test": {"value": "current", "array": [{"value": null}]}})
    );
}

#[test]
fn render_diff_restores_the_typed_section_snapshot() {
    let mut previous = WorldState::default();
    previous.add_section(TestSection {
        value: "before".to_string(),
        optional: None,
        array: Vec::new(),
    });
    let mut current = WorldState::default();
    current.add_section(TestSection {
        value: "after".to_string(),
        optional: None,
        array: Vec::new(),
    });

    let rendered = current.render_diff(&previous.snapshot());

    assert_eq!(
        vec!["after"],
        rendered
            .into_iter()
            .map(|fragment| fragment.body())
            .collect::<Vec<_>>()
    );
}

#[test]
fn unreadable_section_snapshot_is_treated_as_unknown() {
    let mut current = WorldState::default();
    current.add_section(TestSection {
        value: "current".to_string(),
        optional: None,
        array: Vec::new(),
    });
    let previous = WorldStateSnapshot {
        sections: BTreeMap::from([("test".to_string(), json!({"invalid": true}))]),
    };

    let rendered = current.render_diff(&previous);

    assert_eq!(
        vec!["unknown"],
        rendered
            .into_iter()
            .map(|fragment| fragment.body())
            .collect::<Vec<_>>()
    );
}

#[test]
#[should_panic(expected = "duplicate world-state section ID: test")]
fn duplicate_section_ids_are_rejected() {
    let mut world_state = WorldState::default();
    world_state.add_section(TestSection {
        value: "current".to_string(),
        optional: None,
        array: Vec::new(),
    });

    world_state.add_section(DuplicateTestSection);
}

#[test]
fn snapshot_merge_patch_changes_and_removes_nested_values() {
    let mut previous = WorldStateSnapshot {
        sections: BTreeMap::from([
            (
                "kept".to_string(),
                json!({"same": true, "changed": "before", "removed": true}),
            ),
            ("removed_section".to_string(), json!({"value": true})),
        ]),
    };
    let current = WorldStateSnapshot {
        sections: BTreeMap::from([(
            "kept".to_string(),
            json!({"same": true, "changed": "after"}),
        )]),
    };

    assert_eq!(
        current.merge_patch_from(&previous),
        Some(json!({
            "kept": {"changed": "after", "removed": null},
            "removed_section": null,
        }))
    );
    previous
        .apply_merge_patch(
            &current
                .merge_patch_from(&previous)
                .expect("changed snapshots should produce a patch"),
        )
        .expect("apply world-state merge patch");
    assert_eq!(previous, current);
    assert_eq!(current.merge_patch_from(&current), None);
}
