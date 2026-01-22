use crate::config::CollaborationModeOverride;
use crate::config::CollaborationModesConfig;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_PAIR_PROGRAMMING: &str =
    include_str!("../../templates/collaboration_mode/pair_programming.md");
const COLLABORATION_MODE_EXECUTE: &str =
    include_str!("../../templates/collaboration_mode/execute.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    builtin_collaboration_mode_presets_with_overrides(None)
}

pub(super) fn builtin_collaboration_mode_presets_with_overrides(
    overrides: Option<&CollaborationModesConfig>,
) -> Vec<CollaborationMode> {
    vec![
        apply_override(plan_preset(), overrides.and_then(|modes| modes.plan.as_ref())),
        apply_override(
            pair_programming_preset(),
            overrides.and_then(|modes| modes.pair_programming.as_ref()),
        ),
        apply_override(
            execute_preset(),
            overrides.and_then(|modes| modes.execute.as_ref()),
        ),
    ]
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    builtin_collaboration_mode_presets()
}

fn plan_preset() -> CollaborationMode {
    CollaborationMode::Plan(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: Some(COLLABORATION_MODE_PLAN.to_string()),
    })
}

fn pair_programming_preset() -> CollaborationMode {
    CollaborationMode::PairProgramming(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: Some(COLLABORATION_MODE_PAIR_PROGRAMMING.to_string()),
    })
}

fn execute_preset() -> CollaborationMode {
    CollaborationMode::Execute(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::XHigh),
        developer_instructions: Some(COLLABORATION_MODE_EXECUTE.to_string()),
    })
}

fn apply_override(
    preset: CollaborationMode,
    override_cfg: Option<&CollaborationModeOverride>,
) -> CollaborationMode {
    let Some(override_cfg) = override_cfg else {
        return preset;
    };

    let (variant, settings) = match preset {
        CollaborationMode::Plan(settings) => (ModeVariant::Plan, settings),
        CollaborationMode::PairProgramming(settings) => (ModeVariant::PairProgramming, settings),
        CollaborationMode::Execute(settings) => (ModeVariant::Execute, settings),
        CollaborationMode::Custom(settings) => (ModeVariant::Custom, settings),
    };

    let merged = Settings {
        model: override_cfg.model.clone().unwrap_or(settings.model),
        reasoning_effort: override_cfg.reasoning_effort.or(settings.reasoning_effort),
        developer_instructions: override_cfg
            .developer_instructions
            .clone()
            .or(settings.developer_instructions),
    };

    match variant {
        ModeVariant::Plan => CollaborationMode::Plan(merged),
        ModeVariant::PairProgramming => CollaborationMode::PairProgramming(merged),
        ModeVariant::Execute => CollaborationMode::Execute(merged),
        ModeVariant::Custom => CollaborationMode::Custom(merged),
    }
}

enum ModeVariant {
    Plan,
    PairProgramming,
    Execute,
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply_to_presets() {
        let overrides = CollaborationModesConfig {
            plan: Some(CollaborationModeOverride {
                model: Some("override-model".to_string()),
                reasoning_effort: Some(ReasoningEffort::Low),
                developer_instructions: Some("override plan".to_string()),
            }),
            ..Default::default()
        };

        let presets = builtin_collaboration_mode_presets_with_overrides(Some(&overrides));
        let plan = presets
            .into_iter()
            .find(|preset| matches!(preset, CollaborationMode::Plan(_)))
            .expect("plan preset");

        match plan {
            CollaborationMode::Plan(settings) => {
                assert_eq!(settings.model, "override-model");
                assert_eq!(settings.reasoning_effort, Some(ReasoningEffort::Low));
                assert_eq!(
                    settings.developer_instructions.as_deref(),
                    Some("override plan")
                );
            }
            _ => unreachable!("expected plan preset"),
        }
    }
}
