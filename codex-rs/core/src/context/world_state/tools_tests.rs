use super::MAX_NAMESPACE_DESCRIPTION_CHARS;
use super::MAX_RENDERED_FRAGMENT_BYTES;
use super::ToolsState;
use crate::context::world_state::PreviousSectionState;
use crate::context::world_state::WorldStateSection;
use codex_extension_api::ExtensionMetrics;
use codex_otel::THREAD_TOOLS_FRAGMENT_BYTES_METRIC;
use codex_otel::THREAD_TOOLS_NAMESPACES_TOTAL_METRIC;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

#[test]
fn renders_first_line_of_namespace_descriptions() {
    let tools = ToolsState::new(
        [
            (
                "app".to_string(),
                "  control the Codex App  \nAdditional instructions.".to_string(),
            ),
            (
                "gmail".to_string(),
                "access your Google Gmail Account & labels".to_string(),
            ),
            ("hotline".to_string(), String::new()),
        ],
        Arc::new(RecordingMetrics::default()),
    );

    let rendered = tools
        .render_diff(PreviousSectionState::Absent)
        .expect("tools state should render")
        .render();

    assert_eq!(
        rendered,
        "<tools>\nDeferred tool namespaces:\n- app: control the Codex App\n- gmail: access your Google Gmail Account &amp; labels\n- hotline\n</tools>"
    );
}

#[test]
fn renders_added_removed_and_updated_namespace_descriptions() {
    let metrics = Arc::new(RecordingMetrics::default());
    let tools = ToolsState::new(
        [
            ("app".to_string(), "control the Codex App".to_string()),
            ("kept".to_string(), "unchanged".to_string()),
            (
                "gmail".to_string(),
                "access your Google Gmail Account".to_string(),
            ),
        ],
        metrics.clone(),
    );
    let previous = BTreeMap::from([
        ("kept".to_string(), "unchanged".to_string()),
        ("gmail".to_string(), "old Gmail description".to_string()),
        (
            "hotline".to_string(),
            "access hotline information".to_string(),
        ),
    ]);

    let rendered = tools
        .render_diff(PreviousSectionState::Known(&previous))
        .expect("tools state delta should render")
        .render();

    assert_eq!(
        rendered,
        "<tools>\nAdded deferred tool namespaces:\n- app: control the Codex App\n- gmail: access your Google Gmail Account\nRemoved deferred tool namespaces:\n- hotline: access hotline information\n</tools>"
    );

    // Delta samples count the emitted additions/removals, excluding unchanged entries.
    assert_eq!(
        metrics.take(),
        expected_metrics("delta", (3, &rendered), (3, &rendered))
    );
    assert!(
        tools
            .render_diff(PreviousSectionState::Known(&tools.snapshot()))
            .is_none()
    );
    assert!(metrics.take().is_empty());
}

#[test]
fn caps_namespace_descriptions_by_character_count() {
    let description = "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS + 1);
    let tools = ToolsState::new(
        [("app".to_string(), description)],
        Arc::new(RecordingMetrics::default()),
    );

    assert_eq!(
        tools.snapshot(),
        BTreeMap::from([(
            "app".to_string(),
            "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS)
        )])
    );
}

#[test]
fn caps_rendered_tools_fragment_after_xml_escaping() {
    let metrics = Arc::new(RecordingMetrics::default());
    let namespaces = (0..100)
        .map(|index| {
            (
                format!("namespace_{index}"),
                format!("{}\nsecond line", "界&<>'\"".repeat(100)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let normalized_entries = namespaces
        .keys()
        .map(|name| {
            format!(
                "- {name}: {}界&amp;&lt;&gt;\n",
                "界&amp;&lt;&gt;&apos;&quot;".repeat(41)
            )
        })
        .collect::<String>();
    // Before the byte cap, after first-line/250-character description normalization.
    let before = format!("<tools>\nDeferred tool namespaces:\n{normalized_entries}</tools>");
    let tools = ToolsState::new(namespaces, metrics.clone());
    let rendered = tools
        .render_diff(PreviousSectionState::Absent)
        .expect("tools state should render")
        .render();

    assert!(rendered.len() <= MAX_RENDERED_FRAGMENT_BYTES);
    assert!(rendered.contains(" additional namespaces omitted.\n"));
    let included = rendered
        .lines()
        .filter(|line| line.starts_with("- "))
        .count();
    assert_eq!(
        metrics.take(),
        expected_metrics("snapshot", (100, &before), (included, &rendered))
    );
}

#[derive(Default)]
struct RecordingMetrics {
    samples: Mutex<BTreeMap<(String, String, String), i64>>,
}

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, name: &str, _inc: i64, _tags: &[(&str, &str)]) {
        panic!("unexpected counter: {name}");
    }

    fn histogram_with_boundaries(
        &self,
        name: &str,
        value: i64,
        _boundaries: &[f64],
        tags: &[(&str, &str)],
    ) {
        self.histogram(name, value, tags);
    }

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        let [("stage", stage), ("kind", kind)] = tags else {
            panic!("unexpected tags: {tags:?}")
        };
        self.samples.lock().expect("metric samples lock").insert(
            (name.to_string(), (*stage).to_string(), (*kind).to_string()),
            value,
        );
    }
}

impl RecordingMetrics {
    fn take(&self) -> BTreeMap<(String, String, String), i64> {
        std::mem::take(&mut *self.samples.lock().expect("metric samples lock"))
    }
}

fn expected_metrics(
    kind: &str,
    before: (usize, &str),
    after: (usize, &str),
) -> BTreeMap<(String, String, String), i64> {
    [("before", before), ("after", after)]
        .into_iter()
        .flat_map(|(stage, (count, text))| {
            [
                (THREAD_TOOLS_NAMESPACES_TOTAL_METRIC, count),
                (THREAD_TOOLS_FRAGMENT_BYTES_METRIC, text.len()),
            ]
            .map(|(name, value)| {
                (
                    (name.to_string(), stage.to_string(), kind.to_string()),
                    value as i64,
                )
            })
        })
        .collect()
}
