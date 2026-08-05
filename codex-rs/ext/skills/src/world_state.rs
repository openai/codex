use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde_json::json;

use crate::render::SkillRenderReport;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
pub(crate) const ORCHESTRATOR_SKILLS_WORLD_STATE_ID: &str = "orchestrator_skills";
pub(crate) const HOST_SKILLS_WORLD_STATE_ID: &str = "host_skills";
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";
const HIDDEN_EXECUTOR_SKILLS_BODY: &str = "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_ORCHESTRATOR_SKILLS_BODY: &str =
    "\n## Orchestrator skills update\nNo orchestrator skills are currently available.\n";
const HIDDEN_ORCHESTRATOR_SKILLS_BODY: &str = "\n## Orchestrator skills update\nOrchestrator skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_HOST_SKILLS_BODY: &str =
    "\n## Host skills update\nNo host skills are currently available.\n";
const HIDDEN_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const OMITTED_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are available but omitted from the model-visible skills list because the skills context budget was exceeded.\n";

pub(crate) type CatalogRenderCallback = Box<dyn Fn() + Send + Sync>;

pub(crate) fn executor_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    skills_world_state_section(
        SKILLS_WORLD_STATE_ID,
        body,
        include_instructions,
        /*enabled*/ None,
        NO_EXECUTOR_SKILLS_BODY,
        HIDDEN_EXECUTOR_SKILLS_BODY,
        on_render,
    )
    .with_legacy_matcher(|role, text| {
        role == "developer"
            && text.trim_start().starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
            && text.trim_end().ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
    })
}

pub(crate) fn orchestrator_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    enabled: bool,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    skills_world_state_section(
        ORCHESTRATOR_SKILLS_WORLD_STATE_ID,
        body,
        include_instructions,
        Some(enabled),
        NO_ORCHESTRATOR_SKILLS_BODY,
        if enabled {
            HIDDEN_ORCHESTRATOR_SKILLS_BODY
        } else {
            NO_ORCHESTRATOR_SKILLS_BODY
        },
        on_render,
    )
}

fn skills_world_state_section(
    id: &'static str,
    body: Option<String>,
    include_instructions: bool,
    enabled: Option<bool>,
    no_skills_body: &'static str,
    hidden_skills_body: &'static str,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    let mut snapshot = json!({
        "body": body,
        "includeInstructions": include_instructions,
    });
    if let Some(enabled) = enabled {
        snapshot["enabled"] = json!(enabled);
    }
    let retained_body = body.clone();

    let contribution = WorldStateSectionContribution::new(id, snapshot, move |previous| {
        if let PreviousWorldStateSection::Known(previous) = &previous {
            let previous_body = previous.get("body").and_then(serde_json::Value::as_str);
            let previous_include_instructions = previous
                .get("includeInstructions")
                .and_then(serde_json::Value::as_bool);
            let previous_enabled = previous.get("enabled").and_then(serde_json::Value::as_bool);
            if previous_body == body.as_deref()
                && previous_include_instructions == Some(include_instructions)
                && previous_enabled == enabled
            {
                return None;
            }
        }

        let body = match body.as_deref() {
            Some(body) => body,
            None if matches!(previous, PreviousWorldStateSection::Absent) => return None,
            None if !include_instructions => hidden_skills_body,
            None => no_skills_body,
        };
        on_render();

        Some(RenderedWorldStateFragment::new(
            "developer",
            (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
            body,
        ))
    });
    match retained_body {
        Some(body) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&body)
        }),
        None => contribution,
    }
}

pub(crate) fn host_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    report: &SkillRenderReport,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    let body = body.or_else(|| {
        (report.included_count == 0 && report.omitted_count > 0)
            .then(|| OMITTED_HOST_SKILLS_BODY.to_string())
    });
    let retained_fragment = body
        .as_ref()
        .map(|body| format!("{SKILLS_INSTRUCTIONS_OPEN_TAG}{body}{SKILLS_INSTRUCTIONS_CLOSE_TAG}"));

    let contribution = skills_world_state_section(
        HOST_SKILLS_WORLD_STATE_ID,
        body,
        include_instructions,
        /*enabled*/ None,
        NO_HOST_SKILLS_BODY,
        HIDDEN_HOST_SKILLS_BODY,
        on_render,
    );
    match retained_fragment {
        Some(fragment) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&fragment)
        }),
        None => contribution,
    }
}
