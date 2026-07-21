use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SkillMetadataBudget;
use codex_core_skills::build_available_skills;
use codex_core_skills::render::SkillRenderSideEffects;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde_json::json;

use crate::catalog::SkillCatalog;
use crate::fragments::AvailableSkillsInstructions;
use crate::render::SkillCatalogRenderPolicy;
use crate::render::available_skills_fragment;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
pub(crate) const HOST_SKILLS_WORLD_STATE_ID: &str = "host_skills";
const MAX_HOST_SKILLS_METADATA_CHARS: usize = 8_000;
const MAX_HOST_SKILLS_METADATA_TOKENS: usize = 4_000;
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";
const HIDDEN_EXECUTOR_SKILLS_BODY: &str = "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_HOST_SKILLS_BODY: &str =
    "\n## Host skills update\nNo host skills are currently available.\n";
const HIDDEN_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";

pub(crate) fn executor_skills_world_state_section(
    catalog: &SkillCatalog,
    include_instructions: bool,
    include_skills_usage_instructions: bool,
) -> WorldStateSectionContribution {
    let body = if include_instructions {
        available_skills_fragment(
            catalog,
            include_skills_usage_instructions,
            SkillCatalogRenderPolicy::ExtensionCompatible,
        )
        .map(|fragment| fragment.body())
    } else {
        None
    };
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
                Some(body) => body,
                None if previous_is_absent => return None,
                None if !include_instructions => HIDDEN_EXECUTOR_SKILLS_BODY,
                None => NO_EXECUTOR_SKILLS_BODY,
            };
            Some(RenderedWorldStateFragment::new(
                "developer",
                (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
                body,
            ))
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
) -> WorldStateSectionContribution {
    let outcome = host_snapshot.outcome();
    let metadata_budget = match metadata_budget {
        SkillMetadataBudget::Tokens(limit) => {
            SkillMetadataBudget::Tokens(limit.min(MAX_HOST_SKILLS_METADATA_TOKENS))
        }
        SkillMetadataBudget::Characters(limit) => {
            SkillMetadataBudget::Characters(limit.min(MAX_HOST_SKILLS_METADATA_CHARS))
        }
    };
    let available = if include_instructions {
        build_available_skills(outcome, metadata_budget, SkillRenderSideEffects::None)
    } else {
        None
    };
    let body = available.map(|available| {
        AvailableSkillsInstructions::from_available_skills(
            available,
            include_skills_usage_instructions,
        )
        .body()
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

            let body = match body.as_deref() {
                Some(body) => body,
                None if previous_is_absent => return None,
                None if !include_instructions => HIDDEN_HOST_SKILLS_BODY,
                None => NO_HOST_SKILLS_BODY,
            };
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
