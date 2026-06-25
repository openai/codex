use codex_extension_api::ContextualUserFragment;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde_json::json;

use crate::catalog::SkillCatalog;
use crate::render::available_skills_fragment;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";

pub(crate) fn executor_skills_world_state_section(
    catalog: &SkillCatalog,
    include_instructions: bool,
) -> WorldStateSectionContribution {
    let body = if include_instructions {
        available_skills_fragment(catalog).map(|fragment| fragment.body())
    } else {
        None
    };
    let snapshot = json!({"body": body});

    WorldStateSectionContribution::new(SKILLS_WORLD_STATE_ID, snapshot, move |previous| {
        let previous_is_absent = matches!(&previous, PreviousWorldStateSection::Absent);
        let previous_is_known = matches!(&previous, PreviousWorldStateSection::Known(_));
        let previous_body = match &previous {
            PreviousWorldStateSection::Known(previous) => {
                previous.get("body").and_then(serde_json::Value::as_str)
            }
            PreviousWorldStateSection::Absent | PreviousWorldStateSection::Unknown => None,
        };
        if previous_is_known && previous_body == body.as_deref() {
            return None;
        }

        let body = match body.as_deref() {
            Some(body) => body,
            None if previous_is_absent => return None,
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
    })
}
