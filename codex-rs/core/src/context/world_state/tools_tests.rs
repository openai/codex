use super::MAX_NAMESPACE_DESCRIPTION_CHARS;
use super::MAX_RENDERED_FRAGMENT_BYTES;
use super::ToolsState;
use crate::context::world_state::PreviousSectionState;
use crate::context::world_state::test_support::FragmentSectionTestExt as _;
use crate::context::world_state::test_support::render_section_cases;
use codex_extension_api::ExtensionMetrics;
use codex_otel::THREAD_TOOLS_FRAGMENT_BYTES_METRIC;
use codex_otel::THREAD_TOOLS_METRIC_BUCKETS;
use codex_otel::THREAD_TOOLS_NAMESPACES_TOTAL_METRIC;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

#[test]
fn snapshots_description_truncation() {
    let description = "Search project documents by title, owner, date, or status. Read matching documents with their summaries, source links, and recent activity. Create documents, update their contents, and organize results into collections for follow-up.";
    let tools = ToolsState::new(
        (0..20).map(|index| {
            (
                format!("project_{index:02}_documents"),
                description.to_string(),
            )
        }),
        Arc::new(RecordingMetrics::default()),
    );

    insta::assert_snapshot!(render_section_cases(&[(
        PreviousSectionState::Absent,
        PreviousSectionState::Known(&tools),
    )]));
}

#[test]
fn snapshots_namespace_omission() {
    let tools = ToolsState::new(
        (0..80).map(|index| {
            (
                format!("mcp__codex_apps__workspace_{index:02}_project_documents_and_search"),
                "Search project documents.".to_string(),
            )
        }),
        Arc::new(RecordingMetrics::default()),
    );

    insta::assert_snapshot!(render_section_cases(&[(
        PreviousSectionState::Absent,
        PreviousSectionState::Known(&tools),
    )]));
}

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
        .render_fragment_diff(PreviousSectionState::Absent)
        .1
        .expect("tools state should render")
        .render();

    assert_eq!(
        rendered,
        "<tools>\nDeferred tool namespaces:\n- app: control the Codex App\n- gmail: access your Google Gmail Account & labels\n- hotline\n</tools>"
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

    let (snapshot, fragment) = tools.render_fragment_diff(PreviousSectionState::Known(&previous));
    let rendered = fragment.expect("tools state delta should render").render();

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
            .render_fragment_diff(PreviousSectionState::Known(&snapshot.unwrap()))
            .1
            .is_none()
    );
    assert!(metrics.take().is_empty());
}

#[test]
fn caps_namespace_descriptions_by_character_count() {
    let exact_description = "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS);
    let capped_description = format!("{}...", "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS - 3));
    let tools = ToolsState::new(
        [
            ("exact".to_string(), exact_description.clone()),
            (
                "over".to_string(),
                "🦀".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS + 1),
            ),
        ],
        Arc::new(RecordingMetrics::default()),
    );

    let (snapshot, fragment) = tools.render_fragment_diff(PreviousSectionState::Absent);
    assert_eq!(
        snapshot.unwrap(),
        BTreeMap::from([
            ("exact".to_string(), exact_description.clone()),
            ("over".to_string(), capped_description.clone()),
        ])
    );
    assert_eq!(
        fragment.expect("tools state should render").render(),
        format!(
            "<tools>\nDeferred tool namespaces:\n- exact: {exact_description}\n- over: {capped_description}\n</tools>"
        )
    );
}

#[test]
fn records_fragment_metrics_after_description_normalization_and_truncation() {
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
        .map(|name| format!("- {name}: {}界...\n", "界&<>'\"".repeat(41)))
        .collect::<String>();
    // Before the byte cap, after first-line/250-character description normalization.
    let before = format!("<tools>\nDeferred tool namespaces:\n{normalized_entries}</tools>");
    let tools = ToolsState::new(namespaces, metrics.clone());
    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Absent)
        .1
        .expect("tools state should render")
        .render();

    assert!(rendered.len() <= MAX_RENDERED_FRAGMENT_BYTES);
    let included = rendered
        .lines()
        .filter(|line| line.starts_with("- "))
        .count();
    assert_eq!(included, 100);
    assert_eq!(
        metrics.take(),
        expected_metrics("snapshot", (100, &before), (included, &rendered))
    );
}

#[test]
fn retains_all_names_before_sharing_description_bytes() {
    let tools = ToolsState::new(
        (0..100).map(|index| {
            (
                format!("namespace_{index}"),
                "&".repeat(MAX_NAMESPACE_DESCRIPTION_CHARS),
            )
        }),
        Arc::new(RecordingMetrics::default()),
    );

    let (snapshot, fragment) = tools.render_fragment_diff(PreviousSectionState::Absent);
    let rendered = fragment.expect("tools state should render").render();

    let entries = snapshot
        .unwrap()
        .keys()
        .enumerate()
        .map(|(index, namespace)| {
            // Name-only rows leave 2,564 bytes: 200 separator bytes, 300 ellipsis
            // bytes, and 20 ampersands per namespace, plus one for the first 64.
            let description = "&".repeat(if index < 64 { 21 } else { 20 });
            format!("- {namespace}: {description}...\n")
        })
        .collect::<String>();

    assert_eq!(
        (rendered.len(), rendered),
        (
            MAX_RENDERED_FRAGMENT_BYTES,
            format!("<tools>\nDeferred tool namespaces:\n{entries}</tools>")
        )
    );
}

#[test]
fn retains_names_that_fit_without_reserving_an_omission_notice() {
    let namespaces = (0..80)
        .map(|index| format!("service_{index:02}_{}", "x".repeat(/*n*/ 36)))
        .collect::<Vec<_>>();
    let tools = ToolsState::new(
        namespaces
            .iter()
            .map(|namespace| (namespace.clone(), String::new())),
        Arc::new(RecordingMetrics::default()),
    );
    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Absent)
        .1
        .expect("tools state should render")
        .render();
    let entries = namespaces
        .iter()
        .map(|namespace| format!("- {namespace}\n"))
        .collect::<String>();

    assert_eq!(
        (rendered.len(), rendered),
        (
            4042,
            format!("<tools>\nDeferred tool namespaces:\n{entries}</tools>")
        )
    );
}

#[test]
fn preserves_raw_names_and_complete_unicode_at_the_budget_boundary() {
    // Name-only rows leave 14 bytes. Each description gets five bytes, leaving
    // two for its UTF-8 prefix after reserving the ellipsis. The second prefix
    // cannot fit a crab, so it renders only the ellipsis.
    let namespace = format!("a{}", "<".repeat(/*n*/ 4032));
    let tools = ToolsState::new(
        [
            (namespace.clone(), "&🦀x".to_string()),
            ("z".to_string(), "🦀&y".to_string()),
        ],
        Arc::new(RecordingMetrics::default()),
    );

    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Absent)
        .1
        .expect("tools state should render")
        .render();

    assert_eq!(
        (rendered.len(), rendered),
        (
            MAX_RENDERED_FRAGMENT_BYTES - 3,
            format!("<tools>\nDeferred tool namespaces:\n- {namespace}: &...\n- z: ...\n</tools>")
        )
    );
}

#[test]
fn redistributes_description_space_after_short_and_empty_descriptions() {
    // The names leave 12 bytes: three for `: X`, nine for `: abcd...`.
    let namespace = "a".repeat(/*n*/ 4031);
    let tools = ToolsState::new(
        [
            (namespace.clone(), "abcdefgh".to_string()),
            ("b".to_string(), "X".to_string()),
            ("c".to_string(), String::new()),
        ],
        Arc::new(RecordingMetrics::default()),
    );

    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Absent)
        .1
        .expect("tools state should render")
        .render();

    assert_eq!(
        rendered,
        format!(
            "<tools>\nDeferred tool namespaces:\n- {namespace}: abcd...\n- b: X\n- c\n</tools>"
        )
    );
}

#[test]
fn reserves_names_and_shares_descriptions_across_added_and_removed_groups() {
    // Both groups' names leave 15 bytes, shared between descriptions and ellipses.
    let namespace = "a".repeat(/*n*/ 3992);
    let tools = ToolsState::new(
        [(namespace.clone(), "AAAAAAAA".to_string())],
        Arc::new(RecordingMetrics::default()),
    );
    let previous = BTreeMap::from([("z".to_string(), "ZZZZZZZZ".to_string())]);

    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Known(&previous))
        .1
        .expect("tools state delta should render")
        .render();

    assert_eq!(
        rendered,
        format!(
            "<tools>\nAdded deferred tool namespaces:\n- {namespace}: AAA...\nRemoved deferred tool namespaces:\n- z: ZZ...\n</tools>"
        )
    );
}

#[test]
fn omits_whole_names_per_group_only_after_dropping_every_description() {
    let retained_namespace = "b".repeat(/*n*/ 3870);
    let tools = ToolsState::new(
        [
            ("a".repeat(/*n*/ 4000), "oversized namespace".to_string()),
            (retained_namespace.clone(), "large namespace".to_string()),
            (
                "c".repeat(/*n*/ 100),
                "another omitted namespace".to_string(),
            ),
            ("d".to_string(), "short namespace".to_string()),
        ],
        Arc::new(RecordingMetrics::default()),
    );
    let previous = BTreeMap::from([
        (
            "y".repeat(/*n*/ 100),
            "omitted removed namespace".to_string(),
        ),
        ("z".to_string(), "short removed namespace".to_string()),
    ]);

    let rendered = tools
        .render_fragment_diff(PreviousSectionState::Known(&previous))
        .1
        .expect("tools state delta should render")
        .render();

    assert_eq!(
        rendered,
        format!(
            "<tools>\nAdded deferred tool namespaces:\n- {retained_namespace}\n- d\n... 2 additional namespaces omitted.\nRemoved deferred tool namespaces:\n- z\n... 1 additional namespaces omitted.\n</tools>"
        )
    );
}

#[test]
fn reserves_the_empty_state_notice_when_all_namespaces_are_removed() {
    // The names and empty-state notice leave 15 bytes for descriptions and ellipses.
    let namespace = "a".repeat(/*n*/ 3988);
    let previous = BTreeMap::from([
        (namespace.clone(), "long namespace".to_string()),
        ("z".to_string(), "short namespace".to_string()),
    ]);

    let rendered = ToolsState::new([], Arc::new(RecordingMetrics::default()))
        .render_fragment_diff(PreviousSectionState::Known(&previous))
        .1
        .expect("final tools state delta should render")
        .render();

    assert_eq!(
        (rendered.len(), rendered),
        (
            MAX_RENDERED_FRAGMENT_BYTES,
            format!(
                "<tools>\nRemoved deferred tool namespaces:\n- {namespace}: lon...\n- z: sh...\nNo deferred tool namespaces remain.\n</tools>"
            )
        )
    );
}

#[derive(Default)]
struct RecordingMetrics {
    samples: Mutex<BTreeMap<(String, String, String), RecordedHistogram>>,
}

#[derive(Debug, PartialEq)]
struct RecordedHistogram {
    value: i64,
    boundaries: Vec<f64>,
}

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, name: &str, _inc: i64, _tags: &[(&str, &str)]) {
        panic!("unexpected counter: {name}");
    }

    fn histogram_with_boundaries(
        &self,
        name: &str,
        value: i64,
        boundaries: &[f64],
        tags: &[(&str, &str)],
    ) {
        let [("stage", stage), ("kind", kind)] = tags else {
            panic!("unexpected tags: {tags:?}")
        };
        self.samples.lock().expect("metric samples lock").insert(
            (name.to_string(), (*stage).to_string(), (*kind).to_string()),
            RecordedHistogram {
                value,
                boundaries: boundaries.to_vec(),
            },
        );
    }

    fn histogram(&self, name: &str, _value: i64, _tags: &[(&str, &str)]) {
        panic!("expected explicit boundaries for {name}");
    }
}

impl RecordingMetrics {
    fn take(&self) -> BTreeMap<(String, String, String), RecordedHistogram> {
        std::mem::take(&mut *self.samples.lock().expect("metric samples lock"))
    }
}

fn expected_metrics(
    kind: &str,
    before: (usize, &str),
    after: (usize, &str),
) -> BTreeMap<(String, String, String), RecordedHistogram> {
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
                    RecordedHistogram {
                        value: value as i64,
                        boundaries: THREAD_TOOLS_METRIC_BUCKETS.to_vec(),
                    },
                )
            })
        })
        .collect()
}
