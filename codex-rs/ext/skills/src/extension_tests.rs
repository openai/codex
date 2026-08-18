use std::sync::Mutex;

use codex_extension_api::ExtensionMetrics;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use pretty_assertions::assert_eq;

use super::*;

#[derive(Default)]
struct RecordingMetrics {
    samples: Mutex<Vec<(String, i64)>>,
}

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, name: &str, _inc: i64, _tags: &[(&str, &str)]) {
        panic!("unexpected counter: {name}");
    }

    fn histogram(&self, name: &str, value: i64, _tags: &[(&str, &str)]) {
        self.samples
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((name.to_string(), value));
    }
}

#[test]
fn empty_catalog_records_zero_metrics_without_a_fragment() {
    let metrics = RecordingMetrics::default();

    let rendered = render_catalog(
        Some(&metrics),
        CatalogSurface::ThreadContext,
        &SkillCatalog::default(),
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(8_000),
    );

    assert!(rendered.fragment.is_none());
    assert_eq!(rendered.warning_message, None);

    assert_eq!(
        *metrics
            .samples
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            (THREAD_SKILLS_ENABLED_TOTAL_METRIC.to_string(), 0),
            (THREAD_SKILLS_KEPT_TOTAL_METRIC.to_string(), 0),
            (THREAD_SKILLS_TRUNCATED_METRIC.to_string(), 0),
            (
                THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC.to_string(),
                0,
            ),
        ]
    );
}
