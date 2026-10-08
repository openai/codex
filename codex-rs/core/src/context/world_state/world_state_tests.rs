use super::*;
use codex_context_fragments::AnnotatedContent;
use codex_protocol::models::ContentItemKind;
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

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let current = self.clone();
        let fragment: Option<Box<dyn ContextualUserFragment>> = match previous {
            PreviousSectionState::Known(previous) if self.value != previous.value => {
                Some(Box::new(TestFragment(self.value.clone())))
            }
            PreviousSectionState::Unknown => Some(Box::new(TestFragment("unknown".to_string()))),
            PreviousSectionState::Absent | PreviousSectionState::Known(_) => None,
        };
        (
            Some(current),
            WorldStateUpdate::optional_boxed_fragment(fragment),
        )
    }
}

struct TestFragment(String);

impl ContextualUserFragment for TestFragment {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("generic.test".to_string())
    }

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

#[test]
fn world_state_hash_normalizes_crlf_line_endings() {
    assert_eq!(
        WorldStateHash::from_fragment(&TestFragment("line one\r\nline two".to_string())),
        WorldStateHash::from_fragment(&TestFragment("line one\nline two".to_string())),
    );
}

#[test]
fn world_state_json_hash_ignores_object_key_order() {
    let first: Value = serde_json::from_str(
        r#"{"type":"function","parameters":{"type":"object","properties":{"query":{"type":"string"}}}}"#,
    )
    .unwrap();
    let reordered: Value = serde_json::from_str(
        r#"{"parameters":{"properties":{"query":{"type":"string"}},"type":"object"},"type":"function"}"#,
    )
    .unwrap();
    assert_eq!(
        WorldStateHash::from_json(&first),
        WorldStateHash::from_json(&reordered),
    );
}

struct DuplicateTestSection;

impl WorldStateSection for DuplicateTestSection {
    const ID: &'static str = "test";
    type Snapshot = ();

    fn render_diff(
        &self,
        _previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        (Some(()), Vec::new())
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
        serde_json::to_value(world_state.render_full().0).expect("serialize world-state snapshot"),
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

    let rendered = current
        .render_history_fragment_diff(Some(&previous.render_full().0), &[])
        .1;

    assert_eq!(
        vec!["after"],
        rendered
            .into_iter()
            .map(|fragment| fragment.body())
            .collect::<Vec<_>>()
    );
}

#[test]
fn extension_owned_section_uses_its_snapshot_and_renderer() {
    let mut world_state = WorldState::default();
    world_state.add_extension_section(WorldStateSectionContribution::new(
        "extension_test",
        |previous| {
            (
                Some(json!({"value": "after", "optional": null})),
                match previous {
                    PreviousWorldStateSection::Known(previous)
                        if previous == &json!({"value": "before"}) =>
                    {
                        Some(RenderedWorldStateFragment::new(
                            "developer",
                            ("<extension_test>", "</extension_test>"),
                            "after",
                        ))
                    }
                    PreviousWorldStateSection::Absent
                    | PreviousWorldStateSection::Unknown
                    | PreviousWorldStateSection::Known(_) => None,
                },
            )
        },
    ));
    let previous = WorldStateSnapshot {
        sections: BTreeMap::from([("extension_test".to_string(), json!({"value": "before"}))]),
    };

    let rendered = world_state
        .render_history_fragment_diff(Some(&previous), &[])
        .1;

    assert_eq!(
        serde_json::to_value(world_state.render_full().0).expect("serialize world-state snapshot"),
        json!({"extension_test": {"value": "after"}})
    );
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].role(), "developer");
    assert_eq!(
        rendered[0].render(),
        "<extension_test>after</extension_test>"
    );
}

#[test]
fn extension_owned_section_uses_its_stable_id_as_content_kind_feature() {
    let mut world_state = WorldState::default();
    world_state.add_extension_section(WorldStateSectionContribution::new("extension_test", |_| {
        (
            Some(json!({"value": "after"})),
            Some(
                RenderedWorldStateFragment::new(
                    "developer",
                    ("<extension_test>", "</extension_test>"),
                    "after",
                )
                .with_metadata(codex_protocol::models::ContentItemMetadata::skills()),
            ),
        )
    }));

    let rendered = world_state
        .render_history_fragment_diff(Some(&WorldStateSnapshot::default()), &[])
        .1;

    assert_eq!(
        rendered
            .into_iter()
            .map(|fragment| fragment.render_fragment().into_parts())
            .collect::<Vec<_>>(),
        vec![(
            "developer",
            AnnotatedContent::text(
                "<extension_test>after</extension_test>",
                ContentItemKind("extension_test.instructions".to_string()),
                codex_protocol::models::ContentItemMetadata::skills(),
            ),
        )]
    );
}

#[test]
fn missing_retained_fragment_is_rendered_again() {
    let mut world_state = WorldState::default();
    world_state.add_extension_section(
        WorldStateSectionContribution::new("extension_test", |previous| {
            (
                Some(json!({"body": "current catalog"})),
                match previous {
                    PreviousWorldStateSection::Absent => Some(RenderedWorldStateFragment::new(
                        "developer",
                        ("<extension_test>", "</extension_test>"),
                        "current catalog",
                    )),
                    PreviousWorldStateSection::Unknown | PreviousWorldStateSection::Known(_) => {
                        None
                    }
                },
            )
        })
        .with_retained_fragment_matcher(|role, text| {
            role == "developer" && text.contains("current catalog")
        }),
    );
    let previous = world_state.render_full().0;
    let retained = ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: "<extension_test>current catalog</extension_test>".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };

    assert_eq!(
        world_state
            .render_history_fragment_diff(Some(&previous), &[])
            .1
            .into_iter()
            .map(|fragment| fragment.body())
            .collect::<Vec<_>>(),
        vec!["current catalog"]
    );
    assert!(
        world_state
            .render_history_fragment_diff(Some(&previous), &[retained])
            .1
            .is_empty()
    );
}

#[test]
fn extension_snapshot_updates_preserve_skipped_state() {
    let previous = WorldStateSnapshot {
        sections: BTreeMap::from([("extension_test".to_string(), json!({"published": true}))]),
    };
    let mut removed = WorldState::default();
    removed.add_extension_section(WorldStateSectionContribution::new("extension_test", |_| {
        (Some(Value::Null), None)
    }));
    let (snapshot, fragments) = removed.render_history_fragment_diff(Some(&previous), &[]);
    assert!(fragments.is_empty());
    assert_eq!(snapshot, WorldStateSnapshot::default());
    assert_eq!(
        snapshot.merge_patch_from(&previous).unwrap()["extension_test"],
        Value::Null
    );

    let mut invalidated = WorldState::default();
    invalidated.add_extension_section(
        WorldStateSectionContribution::new("extension_test", |previous| {
            assert!(matches!(previous, PreviousWorldStateSection::Absent));
            (None, None)
        })
        .with_retained_fragment_matcher(|_, _| false),
    );
    let (snapshot, fragments) = invalidated.render_history_fragment_diff(Some(&previous), &[]);
    assert!(fragments.is_empty());
    assert_eq!(snapshot, previous);
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

    let rendered = current.render_history_fragment_diff(Some(&previous), &[]).1;

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
        current.merge_patch_from(&previous).map(Value::Object),
        Some(json!({
            "kept": {"changed": "after", "removed": null},
            "removed_section": null,
        }))
    );
    let patch = current
        .merge_patch_from(&previous)
        .expect("changed snapshots should produce a patch");
    previous.apply_merge_patch(&patch);
    assert_eq!(previous, current);
    assert_eq!(current.merge_patch_from(&current), None);
}

#[derive(Clone)]
struct MixedOutputSection(u64);

impl WorldStateSection for MixedOutputSection {
    const ID: &'static str = "mixed_output";
    type Snapshot = u64;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        if matches!(previous, PreviousSectionState::Known(previous) if *previous == self.0) {
            return (None, Vec::new());
        }
        let updates = vec![
            WorldStateUpdate::fragment(TestFragment("before tools".to_string())),
            WorldStateUpdate {
                placement: Placement::Prefix,
                content: WorldStateUpdateContent::Item(Box::new(ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_string(),
                    tools: vec![
                        json!({"type": "function", "name": "example", "description": self.0.to_string()}),
                    ],
                })),
            },
            WorldStateUpdate {
                placement: Placement::Prefix,
                ..WorldStateUpdate::fragment(TestFragment("prefix instructions".to_string()))
            },
            WorldStateUpdate {
                placement: Placement::Standalone,
                content: WorldStateUpdateContent::Item(Box::new(ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_string(),
                    tools: vec![json!({"type": "function", "name": "context_tool"})],
                })),
            },
            WorldStateUpdate::fragment(TestFragment("after tools".to_string())),
        ];
        (Some(self.0), updates)
    }
}

#[test]
fn mixed_output_preserves_order_and_restores_its_snapshot() {
    use crate::context_manager::updates::merge_world_state_updates;

    let mut before = WorldState::default();
    before.add_section(MixedOutputSection(1));
    let (previous, _) = before.render_full();
    let mut current = WorldState::default();
    current.add_section(MixedOutputSection(2));
    let (snapshot, updates) = current.render_history_diff(Some(&previous), &[]);
    let expected = vec![
        ContextualUserFragment::into(TestFragment("before tools".to_string())),
        ResponseItem::AdditionalTools {
            id: None,
            role: "developer".to_string(),
            tools: vec![json!({"type": "function", "name": "example", "description": "2"})],
        },
        ContextualUserFragment::into(TestFragment("prefix instructions".to_string())),
        ResponseItem::AdditionalTools {
            id: None,
            role: "developer".to_string(),
            tools: vec![json!({"type": "function", "name": "context_tool"})],
        },
        ContextualUserFragment::into(TestFragment("after tools".to_string())),
    ];
    assert_eq!(merge_world_state_updates(updates), expected);
    assert_eq!(
        serde_json::to_value(&snapshot).unwrap(),
        json!({"mixed_output": 2})
    );
    let mut restored = previous;
    restored.apply_merge_patch(&snapshot.merge_patch_from(&restored).unwrap());
    assert_eq!(restored, snapshot);
    let (unchanged, updates) = current.render_history_diff(Some(&restored), &[]);
    assert_eq!(unchanged, snapshot);
    assert!(updates.is_empty());
    let (full_snapshot, updates) = current.render_full();
    assert_eq!(full_snapshot, snapshot);
    assert_eq!(
        updates
            .iter()
            .map(|update| matches!(update.placement, Placement::Prefix))
            .collect::<Vec<_>>(),
        vec![false, true, true, false, false],
    );
    let (prefix, context) = split_prefix_updates(updates);
    assert_eq!(prefix, vec![expected[1].clone(), expected[2].clone()]);
    assert_eq!(
        context
            .into_iter()
            .map(|update| match update.content {
                WorldStateUpdateContent::Fragment(fragment) => fragment.into_boxed_response_item(),
                WorldStateUpdateContent::Item(item) => *item,
            })
            .collect::<Vec<_>>(),
        vec![
            expected[0].clone(),
            expected[3].clone(),
            expected[4].clone()
        ],
    );
}
