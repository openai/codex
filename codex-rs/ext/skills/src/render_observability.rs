use codex_extension_api::ExtensionMetrics;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;

use crate::render::SkillMetadataBudget;
use crate::render::SkillRenderReport;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatalogSurface {
    ThreadContext,
    ExecutorWorldState,
    HostWorldState,
    TurnInput,
}

impl CatalogSurface {
    fn as_str(self) -> &'static str {
        match self {
            Self::ThreadContext => "thread_context",
            Self::ExecutorWorldState => "executor_world_state",
            Self::HostWorldState => "host_world_state",
            Self::TurnInput => "turn_input",
        }
    }
}

pub(crate) fn record_catalog_render(
    extension_metrics: Option<&dyn ExtensionMetrics>,
    catalog_surface: CatalogSurface,
    budget: SkillMetadataBudget,
    report: &SkillRenderReport,
) {
    record_catalog_metrics(
        extension_metrics,
        catalog_surface,
        report.total_count,
        report.included_count,
        report.omitted_count,
        report.truncated_description_chars,
    );

    trace_catalog_budget_pressure(budget, report);
}

pub(crate) fn trace_catalog_budget_pressure(
    budget: SkillMetadataBudget,
    report: &SkillRenderReport,
) {
    if report.omitted_count > 0 || report.truncated_description_chars > 0 {
        tracing::info!(
            budget_limit = budget.limit(),
            total_skills = report.total_count,
            included_skills = report.included_count,
            omitted_skills = report.omitted_count,
            truncated_description_chars_per_skill = report.average_truncated_description_chars(),
            truncated_skill_descriptions = report.truncated_description_count,
            "truncated skill metadata to fit skills context budget"
        );
    }
}

pub(crate) fn record_catalog_metrics(
    extension_metrics: Option<&dyn ExtensionMetrics>,
    catalog_surface: CatalogSurface,
    total_count: usize,
    included_count: usize,
    omitted_count: usize,
    truncated_description_chars: usize,
) {
    let Some(extension_metrics) = extension_metrics else {
        return;
    };
    let samples = [
        (
            THREAD_SKILLS_ENABLED_TOTAL_METRIC,
            i64::try_from(total_count).unwrap_or(i64::MAX),
        ),
        (
            THREAD_SKILLS_KEPT_TOTAL_METRIC,
            i64::try_from(included_count).unwrap_or(i64::MAX),
        ),
        (
            THREAD_SKILLS_TRUNCATED_METRIC,
            if omitted_count > 0 { 1 } else { 0 },
        ),
        (
            THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC,
            i64::try_from(truncated_description_chars).unwrap_or(i64::MAX),
        ),
    ];
    let tags = [("catalog_surface", catalog_surface.as_str())];
    for (name, value) in samples {
        extension_metrics.histogram(name, value, &tags);
    }
}

#[cfg(test)]
#[path = "render_observability_tests.rs"]
mod tests;
