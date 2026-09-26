use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde::Serialize;
use serde_json::json;

use crate::render::SkillRenderReport;
use crate::world_state_catalogs::CatalogBudgetAllocation;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
pub(crate) const CLOUD_SKILLS_WORLD_STATE_ID: &str = "cloud_skills";
pub(crate) const HOST_SKILLS_WORLD_STATE_ID: &str = "host_skills";
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";
const HIDDEN_EXECUTOR_SKILLS_BODY: &str = "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_CLOUD_SKILLS_BODY: &str =
    "\n## Cloud skills update\nNo cloud skills are currently available.\n";
const HIDDEN_CLOUD_SKILLS_BODY: &str = "\n## Cloud skills update\nCloud skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_HOST_SKILLS_BODY: &str =
    "\n## Host skills update\nNo host skills are currently available.\n";
const HIDDEN_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const OMITTED_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are available but omitted from the model-visible skills list because the skills context budget was exceeded.\n";

pub(crate) type CatalogRenderCallback = Box<dyn Fn() + Send + Sync>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSnapshot {
    body: Option<String>,
    include_instructions: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allocation: Option<CatalogBudgetAllocation>,
}

pub(crate) fn executor_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    previous_snapshot: Option<&serde_json::Value>,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    // Remember display history, not execution authority. Discovery still determines `body`
    // on every step, including when no selected-environment skills are available.
    let last_available_fingerprint = previous_snapshot
        .filter(|_| body.is_none() && include_instructions)
        .and_then(|previous| {
            previous
                .get("body")
                .and_then(serde_json::Value::as_str)
                .map(|body| blake3::hash(body.as_bytes()).to_hex().to_string())
                .or_else(|| {
                    previous
                        .get("lastAvailableFingerprint")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        });
    let snapshot = json!({
        "body": body,
        "includeInstructions": include_instructions,
        "lastAvailableFingerprint": last_available_fingerprint,
    });
    let retained_body = body.clone();
    let contribution = WorldStateSectionContribution::new(
        SKILLS_WORLD_STATE_ID,
        snapshot,
        move |previous| {
            if let PreviousWorldStateSection::Known(previous) = &previous
                && previous.get("includeInstructions").and_then(serde_json::Value::as_bool)
                    == Some(include_instructions)
            {
                if previous.get("body").and_then(serde_json::Value::as_str) == body.as_deref() {
                    return None;
                }
                if let Some(body) = body.as_deref()
                    && previous.get("lastAvailableFingerprint").and_then(serde_json::Value::as_str)
                        == Some(blake3::hash(body.as_bytes()).to_hex().as_str())
                {
                    return Some(RenderedWorldStateFragment::new(
                        "developer",
                        (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                        "\n## Skills update\nThe previously listed selected-environment skills are available again.\n",
                    ));
                }
            }

            let body = match body.as_deref() {
                Some(body) => body,
                None if matches!(previous, PreviousWorldStateSection::Absent) => return None,
                None if !include_instructions => HIDDEN_EXECUTOR_SKILLS_BODY,
                None => NO_EXECUTOR_SKILLS_BODY,
            };
            on_render();
            Some(RenderedWorldStateFragment::new(
                "developer",
                (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                body,
            ))
        },
    )
    .with_legacy_matcher(|role, text| {
        role == "developer"
            && text.trim_start().starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
            && text.trim_end().ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
    });
    // A short availability update is valid only while the actual catalog remains in history.
    match retained_body {
        Some(body) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&body)
        }),
        None => contribution,
    }
}

pub(crate) fn cloud_skills_world_state_section(
    body: Option<String>,
    include_instructions: bool,
    enabled: bool,
    allocation: CatalogBudgetAllocation,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    skills_world_state_section(
        CLOUD_SKILLS_WORLD_STATE_ID,
        CatalogSnapshot {
            body,
            include_instructions,
            enabled: Some(enabled),
            allocation: Some(allocation),
        },
        NO_CLOUD_SKILLS_BODY,
        if enabled {
            HIDDEN_CLOUD_SKILLS_BODY
        } else {
            NO_CLOUD_SKILLS_BODY
        },
        on_render,
    )
}

fn skills_world_state_section(
    id: &'static str,
    state: CatalogSnapshot,
    no_skills_body: &'static str,
    hidden_skills_body: &'static str,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    let snapshot = json!(&state);
    let CatalogSnapshot {
        body,
        include_instructions,
        enabled,
        ..
    } = state;
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
        CatalogSnapshot {
            body,
            include_instructions,
            enabled: None,
            allocation: None,
        },
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
