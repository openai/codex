use super::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Deserialize, Serialize)]
struct TestSection {
    value: String,
    optional: Option<String>,
}

impl WorldStateSection for TestSection {
    const ID: &'static str = "test";
    type Snapshot = Self;

    fn snapshot(&self) -> Self::Snapshot {
        self.clone()
    }

    fn render_diff(
        &self,
        _previous: Option<&Self::Snapshot>,
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
    });

    assert_eq!(
        serde_json::to_value(world_state.snapshot()).expect("serialize world-state snapshot"),
        json!({"test": {"value": "current"}})
    );
}
