use serde::Deserialize;

const LUNA_MODEL: &str = "gpt-5.6-luna";
const TERRA_MODEL: &str = "gpt-5.6-terra";
const SOL_MODEL: &str = "gpt-5.6-sol";
const MAX_TARGET_MODEL_BYTES: usize = 128;
const MAX_DELEGATION_INSTRUCTION_BYTES: usize = 2_048;

/// Model requested for work governed by a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillModel {
    Luna,
}

/// Bounded instructions for delegating skill-governed work to a cheaper model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillModelDelegationInstruction(String);

impl SkillModelDelegationInstruction {
    /// Builds bounded instructions only when the indexed skill requests an available lower-tier model.
    pub fn from_skill_model(
        skill_model: SkillModel,
        skill_name: &str,
        current_model: &str,
        available_models: &[String],
    ) -> Option<Self> {
        if skill_name
            .chars()
            .any(|character| matches!(character, '`' | '<' | '>'))
        {
            return None;
        }

        let provider_prefix = [SOL_MODEL, TERRA_MODEL]
            .into_iter()
            .find_map(|parent_model| {
                current_model.strip_suffix(parent_model).filter(|prefix| {
                    prefix.is_empty() || prefix.ends_with('.') || prefix.ends_with('/')
                })
            })?;
        let target_model = format!("{provider_prefix}{LUNA_MODEL}");
        let target_model = available_models
            .iter()
            .find(|model| model.as_str() == target_model && is_safe_model_identifier(model))?;
        let skill_model = match skill_model {
            SkillModel::Luna => "luna",
        };
        let instruction = format!(
            "<skill_model_delegation>\n\
For this invocation only, skill `{skill_name}` requests `model: {skill_model}`. \
If the user prohibits delegation or subagents, or the work depends on an image or audio \
attachment, work locally. Otherwise, delegate only self-contained text-based skill work \
and use `spawn_agent` \
exactly once. Set `model` to `{target_model}`, set `fork_turns` to `\"none\"`, and choose a \
unique `task_name`. Omit `agent_type` and `reasoning_effort`.\n\
Give the child only this skill's work and necessary context; never include this block and tell \
it not to spawn agents. Delegate the whole request only if the skill fully covers it; otherwise, \
retain ownership of the remaining work and final answer. Wait for the result. If spawning \
fails, continue locally. If waiting fails, retry waiting without duplicating the \
child's work.\n\
Ignore this instruction in child agents and on later turns.\n\
</skill_model_delegation>"
        );

        (instruction.len() <= MAX_DELEGATION_INSTRUCTION_BYTES).then_some(Self(instruction))
    }

    /// Returns the bounded model-visible delegation instructions.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_safe_model_identifier(model: &str) -> bool {
    model.len() <= MAX_TARGET_MODEL_BYTES
        && model.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '/' | '_' | '-')
        })
}

#[cfg(test)]
#[path = "model_delegation_tests.rs"]
mod tests;
