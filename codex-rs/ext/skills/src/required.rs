//! Checks source-scoped requirements against skills discovered before inference.

use codex_extension_api::ExtensionData;

use crate::step_state::SkillsStepState;

/// Skill names that must be supplied by one selected environment before inference.
pub struct EnvironmentSkillRequirements<'a> {
    pub environment_id: &'a str,
    pub skill_names: &'a [String],
}

/// Validates requirements against the policy-checked catalog prepared for this step.
pub fn validate_required_skills<'a>(
    step_store: &ExtensionData,
    environments: impl IntoIterator<Item = EnvironmentSkillRequirements<'a>>,
) -> Result<(), String> {
    let mut environments = environments.into_iter().peekable();
    if environments.peek().is_none() {
        return Ok(());
    }
    let step_state = step_store.get::<SkillsStepState>().ok_or_else(|| {
        "Required skills cannot be validated because the skills extension is unavailable"
            .to_string()
    })?;
    for EnvironmentSkillRequirements {
        environment_id,
        skill_names,
    } in environments
    {
        for name in skill_names {
            let available = step_state.catalog.merged.entries.iter().any(|entry| {
                entry.enabled
                    && entry.name == name.as_str()
                    && (entry.main_prompt.environment_id() == Some(environment_id)
                        || entry.sources.iter().any(|source| {
                            source.enabled
                                && source.main_prompt.environment_id() == Some(environment_id)
                        }))
            });
            if !available {
                return Err(format!(
                    "Required skill {name:?} from environment {environment_id:?} is unavailable"
                ));
            }
        }
    }
    Ok(())
}
