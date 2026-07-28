use std::sync::Arc;

use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::build_available_skills;
use codex_core_skills::render::SkillRenderSideEffects;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde_json::json;

use crate::fragments::AvailableSkillsInstructions;
use crate::render::SkillMetadataBudget;
use crate::render_observability::CatalogSurface;
use crate::render_observability::record_catalog_metrics;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
pub(crate) const HOST_SKILLS_WORLD_STATE_ID: &str = "host_skills";
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";
const HIDDEN_EXECUTOR_SKILLS_BODY: &str = "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_HOST_SKILLS_BODY: &str =
    "\n## Host skills update\nNo host skills are currently available.\n";
const HIDDEN_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const OMITTED_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are available but omitted from the model-visible skills list because the skills context budget was exceeded.\n";

pub(crate) type HostSkillsWarningEmitter = Arc<dyn Fn(String) + Send + Sync>;

pub(crate) fn executor_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
) -> WorldStateSectionContribution {
    let snapshot = json!({
        "body": body,
        "includeInstructions": include_instructions,
    });
    let retained_body = body.clone();

    let contribution =
        WorldStateSectionContribution::new(SKILLS_WORLD_STATE_ID, snapshot, move |previous| {
            let previous_is_absent = matches!(&previous, PreviousWorldStateSection::Absent);
            if let PreviousWorldStateSection::Known(previous) = &previous {
                let previous_body = previous.get("body").and_then(serde_json::Value::as_str);
                let previous_include_instructions = previous
                    .get("includeInstructions")
                    .and_then(serde_json::Value::as_bool);
                if previous_body == body.as_deref()
                    && previous_include_instructions == Some(include_instructions)
                {
                    return None;
                }
            }

            let body = match body.as_deref() {
                Some(body) => Some(body),
                None if previous_is_absent => None,
                None if !include_instructions => Some(HIDDEN_EXECUTOR_SKILLS_BODY),
                None => Some(NO_EXECUTOR_SKILLS_BODY),
            };
            body.map(|body| {
                RenderedWorldStateFragment::new(
                    "developer",
                    (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                    body,
                )
            })
        })
        .with_legacy_matcher(|role, text| {
            role == "developer"
                && text.trim_start().starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
                && text.trim_end().ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
        });
    match retained_body {
        Some(body) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&body)
        }),
        None => contribution,
    }
}

pub(crate) fn host_skills_world_state_section(
    host_snapshot: &HostSkillsSnapshot,
    include_instructions: bool,
    include_skills_usage_instructions: bool,
    metadata_budget: SkillMetadataBudget,
    extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    warning_emitter: HostSkillsWarningEmitter,
) -> WorldStateSectionContribution {
    let outcome = host_snapshot.outcome();
    let metadata_budget = match metadata_budget {
        SkillMetadataBudget::Tokens(limit) => codex_core_skills::SkillMetadataBudget::Tokens(limit),
        SkillMetadataBudget::Characters(limit) => {
            codex_core_skills::SkillMetadataBudget::Characters(limit)
        }
    };
    let available = if include_instructions {
        build_available_skills(outcome, metadata_budget, SkillRenderSideEffects::None)
    } else {
        None
    };
    let warning_message = available
        .as_ref()
        .and_then(|available| available.warning_message.clone());
    let render_metrics = include_instructions.then(|| {
        available
            .as_ref()
            .map(|available| {
                let report = &available.report;
                (
                    report.total_count,
                    report.included_count,
                    report.omitted_count,
                    report.truncated_description_chars,
                )
            })
            .unwrap_or_default()
    });
    let body = available.map(|available| {
        AvailableSkillsInstructions::from_available_skills(
            available,
            include_skills_usage_instructions,
        )
        .body()
    });
    rendered_host_skills_world_state_section(
        body,
        include_instructions,
        render_metrics,
        extension_metrics,
        warning_message,
        warning_emitter,
    )
}

pub(crate) fn rendered_host_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    render_metrics: Option<(usize, usize, usize, usize)>,
    extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    warning_message: Option<String>,
    warning_emitter: HostSkillsWarningEmitter,
) -> WorldStateSectionContribution {
    let body = body.or_else(|| {
        render_metrics
            .is_some_and(|(total_count, included_count, omitted_count, _)| {
                total_count > 0 && included_count == 0 && omitted_count > 0
            })
            .then(|| OMITTED_HOST_SKILLS_BODY.to_string())
    });
    let snapshot = json!({
        "body": body,
        "includeInstructions": include_instructions,
    });
    let retained_fragment = body
        .as_ref()
        .map(|body| format!("{SKILLS_INSTRUCTIONS_OPEN_TAG}{body}{SKILLS_INSTRUCTIONS_CLOSE_TAG}"));

    let contribution =
        WorldStateSectionContribution::new(HOST_SKILLS_WORLD_STATE_ID, snapshot, move |previous| {
            let previous_is_absent = matches!(&previous, PreviousWorldStateSection::Absent);
            if let PreviousWorldStateSection::Known(previous) = &previous {
                let previous_body = previous.get("body").and_then(serde_json::Value::as_str);
                let previous_include_instructions = previous
                    .get("includeInstructions")
                    .and_then(serde_json::Value::as_bool);
                if previous_body == body.as_deref()
                    && previous_include_instructions == Some(include_instructions)
                {
                    return None;
                }
            }

            if let Some((total_count, included_count, omitted_count, truncated_description_chars)) =
                render_metrics
            {
                record_catalog_metrics(
                    extension_metrics.as_deref(),
                    CatalogSurface::HostWorldState,
                    total_count,
                    included_count,
                    omitted_count,
                    truncated_description_chars,
                );
            }
            let body = match body.as_deref() {
                Some(body) => body,
                None if previous_is_absent => return None,
                None if !include_instructions => HIDDEN_HOST_SKILLS_BODY,
                None => NO_HOST_SKILLS_BODY,
            };
            if let Some(message) = warning_message.as_ref() {
                warning_emitter(message.clone());
            }
            Some(RenderedWorldStateFragment::new(
                "developer",
                (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                body,
            ))
        });
    match retained_fragment {
        Some(fragment) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&fragment)
        }),
        None => contribution,
    }
}
