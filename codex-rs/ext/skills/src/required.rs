//! Checks source-scoped requirements against skills discovered before inference.

use codex_extension_api::ExtensionData;

use crate::state::ExecutorSkillsStepState;
use crate::state::SkillsThreadState;

/// Skill names that must be supplied by one selected environment before inference.
pub struct EnvironmentSkillRequirements<'a> {
    pub environment_id: &'a str,
    pub skill_names: &'a [String],
}

/// Validates requirements after discovery refreshes the turn's skill snapshots.
pub fn validate_required_skills<'a>(
    thread_store: &ExtensionData,
    turn_store: &ExtensionData,
    environments: impl IntoIterator<Item = EnvironmentSkillRequirements<'a>>,
) -> Result<(), String> {
    let mut environments = environments.into_iter().peekable();
    if environments.peek().is_none() {
        return Ok(());
    }
    let thread_state = thread_store.get::<SkillsThreadState>().ok_or_else(|| {
        "Required skills cannot be validated because the skills extension is unavailable"
            .to_string()
    })?;
    let executor = turn_store.get::<ExecutorSkillsStepState>();
    thread_state.with_cloud_catalog(|cloud| {
        for EnvironmentSkillRequirements {
            environment_id,
            skill_names,
        } in environments
        {
            for name in skill_names {
                let available = executor
                    .iter()
                    .flat_map(|state| &state.0.entries)
                    .chain(cloud.into_iter().flat_map(|catalog| &catalog.entries))
                    .any(|entry| {
                        entry.enabled
                            && entry.name == name.as_str()
                            && entry
                                .main_prompt
                                .environment_path()
                                .is_some_and(|(id, _)| id == environment_id)
                    });
                if !available {
                    return Err(format!(
                        "Required skill {name:?} from environment {environment_id:?} is unavailable"
                    ));
                }
            }
        }
        Ok(())
    })
}
