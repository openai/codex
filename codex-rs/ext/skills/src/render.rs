use codex_utils_string::take_bytes_at_char_boundary;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;
use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SkillRenderReport;
use codex_core_skills::build_available_skills;
use codex_core_skills::default_skill_metadata_budget;
use codex_core_skills::render::SkillCatalogMode;
use codex_extension_api::ContextualUserFragment;
use codex_otel::SessionTelemetry;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_utils_string::approx_token_count;

const MAX_AVAILABLE_SKILLS_BYTES: usize = 8_000;
const MAX_AVAILABLE_SKILLS_TOKENS: usize = 10_000;
const MAX_MAIN_PROMPT_BYTES: usize = 8_000;
pub(crate) const MAX_SKILL_NAME_BYTES: usize = 256;
pub(crate) const MAX_SKILL_PATH_BYTES: usize = 1_024;

pub(crate) fn available_skills_fragment(
    catalog: &SkillCatalog,
    host_snapshot: Option<&HostSkillsSnapshot>,
    model_context_window: Option<i64>,
    session_telemetry: Option<&SessionTelemetry>,
) -> Option<(AvailableSkillsInstructions, Option<String>)> {
    let visible_entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.prompt_visible)
        .collect::<Vec<_>>();
    let host_available = host_snapshot.and_then(|host_snapshot| {
        build_available_skills(
            host_snapshot.outcome(),
            default_skill_metadata_budget(model_context_window),
        )
    });
    if host_snapshot.is_some() {
        record_skill_render_metrics(
            session_telemetry,
            host_available.as_ref().map(|available| &available.report),
        );
    }

    if visible_entries
        .iter()
        .any(|entry| entry.authority.kind == SkillSourceKind::Host)
        && let Some(available) = host_available
    {
        let external_lines = bounded_skill_lines(
            visible_entries
                .iter()
                .copied()
                .filter(|entry| entry.authority.kind != SkillSourceKind::Host),
        );
        let warning = available.warning_message.clone();
        return Some((bounded_mixed_fragment(available, external_lines), warning));
    }

    let mode = if visible_entries
        .iter()
        .any(|entry| entry.authority.kind != SkillSourceKind::Host)
    {
        SkillCatalogMode::Mixed
    } else {
        SkillCatalogMode::HostOnly
    };
    let skill_lines = bounded_skill_lines(visible_entries).into_lines();
    (!skill_lines.is_empty()).then(|| {
        (
            AvailableSkillsInstructions::from_skill_lines(skill_lines, mode),
            None,
        )
    })
}

struct BoundedSkillLines {
    lines: Vec<String>,
    omitted: usize,
}

impl BoundedSkillLines {
    fn into_lines(mut self) -> Vec<String> {
        if let Some(line) = omitted_skills_line(self.omitted) {
            self.lines.push(line);
        }
        self.lines
    }
}

fn bounded_skill_lines<'a>(
    entries: impl IntoIterator<Item = &'a SkillCatalogEntry>,
) -> BoundedSkillLines {
    let mut total_bytes = 0usize;
    let mut omitted = 0usize;
    let mut skill_lines = Vec::new();

    for entry in entries {
        let description = entry
            .short_description
            .as_deref()
            .unwrap_or(entry.description.as_str());
        let line = render_skill_line(entry, description);
        let next_bytes = total_bytes.saturating_add(line.len());
        if next_bytes > MAX_AVAILABLE_SKILLS_BYTES {
            omitted = omitted.saturating_add(1);
            continue;
        }
        total_bytes = next_bytes;
        skill_lines.push(line);
    }

    BoundedSkillLines {
        lines: skill_lines,
        omitted,
    }
}

fn bounded_mixed_fragment(
    available: codex_core_skills::AvailableSkills,
    mut external: BoundedSkillLines,
) -> AvailableSkillsInstructions {
    loop {
        let mut additional_lines = external.lines.clone();
        if let Some(line) = omitted_skills_line(external.omitted) {
            additional_lines.push(line);
        }
        let fragment = AvailableSkillsInstructions::from_available_skills_with_additional_lines(
            available.clone(),
            additional_lines,
        );
        if approx_token_count(&fragment.render()) <= MAX_AVAILABLE_SKILLS_TOKENS {
            return fragment;
        }
        if external.lines.pop().is_none() {
            return AvailableSkillsInstructions::from_available_skills_with_additional_lines(
                available,
                Vec::new(),
            );
        }
        external.omitted = external.omitted.saturating_add(1);
    }
}

fn omitted_skills_line(omitted: usize) -> Option<String> {
    (omitted > 0).then(|| {
        let skill_word = if omitted == 1 { "skill" } else { "skills" };
        format!("- {omitted} additional {skill_word} omitted from this bounded skills list.")
    })
}

fn record_skill_render_metrics(
    session_telemetry: Option<&SessionTelemetry>,
    report: Option<&SkillRenderReport>,
) {
    let Some(session_telemetry) = session_telemetry else {
        return;
    };
    let (total_count, included_count, truncated, truncated_description_chars) =
        report.map_or((0, 0, 0, 0), |report| {
            (
                i64::try_from(report.total_count).unwrap_or(i64::MAX),
                i64::try_from(report.included_count).unwrap_or(i64::MAX),
                i64::from(report.omitted_count > 0),
                i64::try_from(report.truncated_description_chars).unwrap_or(i64::MAX),
            )
        });
    session_telemetry.histogram(THREAD_SKILLS_ENABLED_TOTAL_METRIC, total_count, &[]);
    session_telemetry.histogram(THREAD_SKILLS_KEPT_TOTAL_METRIC, included_count, &[]);
    session_telemetry.histogram(THREAD_SKILLS_TRUNCATED_METRIC, truncated, &[]);
    session_telemetry.histogram(
        THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC,
        truncated_description_chars,
        &[],
    );
}

fn render_skill_line(entry: &SkillCatalogEntry, description: &str) -> String {
    let locator_kind = match &entry.authority.kind {
        SkillSourceKind::Host => "file",
        SkillSourceKind::Executor => "environment resource",
        SkillSourceKind::Orchestrator => "orchestrator resource",
        SkillSourceKind::Custom(_) => "custom resource",
    };
    let name = entry.name.as_str();
    let path = entry.rendered_path();
    if description.is_empty() {
        format!("- {name}: ({locator_kind}: {path})")
    } else {
        format!("- {name}: {description} ({locator_kind}: {path})")
    }
}

pub(crate) fn truncate_main_prompt_contents(contents: &str) -> (String, bool) {
    truncate_utf8_to_bytes(contents, MAX_MAIN_PROMPT_BYTES)
}

pub(crate) fn truncate_utf8_to_bytes(contents: &str, max_bytes: usize) -> (String, bool) {
    let truncated = take_bytes_at_char_boundary(contents, max_bytes);
    (truncated.to_string(), truncated.len() < contents.len())
}
