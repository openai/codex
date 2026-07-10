use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationPolicyConfig;
use pretty_assertions::assert_eq;

use super::*;

fn model_with_shell_type(shell_type: ConfigShellToolType) -> ModelInfo {
    ModelInfo {
        slug: "test-model".to_string(),
        display_name: "Test Model".to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: Vec::new(),
        shell_type,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 0,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        availability_nux: None,
        upgrade: None,
        base_instructions: String::new(),
        model_messages: None,
        include_skills_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: Default::default(),
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::tokens(/*limit*/ 1024),
        supports_parallel_tool_calls: true,
        supports_image_detail_original: false,
        context_window: None,
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: codex_protocol::openai_models::default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        auto_review_model_override: None,
        tool_mode: None,
        multi_agent_version: None,
    }
}

fn shell_features() -> Features {
    let mut features = Features::with_defaults();
    features.enable(Feature::ShellTool);
    features.disable(Feature::ShellZshFork);
    features.disable(Feature::UnifiedExec);
    features.disable(Feature::UnifiedExecZshFork);
    features
}

#[test]
fn shell_type_is_derived_from_model_and_feature_gates() {
    let model = model_with_shell_type(ConfigShellToolType::UnifiedExec);
    let mut features = shell_features();
    assert_eq!(
        shell_type_for_model_and_features(&model, &features),
        ConfigShellToolType::ShellCommand
    );

    features.enable(Feature::UnifiedExec);
    let expected_unified_exec = if codex_utils_pty::conpty_supported() {
        ConfigShellToolType::UnifiedExec
    } else {
        ConfigShellToolType::ShellCommand
    };
    assert_eq!(
        shell_type_for_model_and_features(&model, &features),
        expected_unified_exec
    );

    features.enable(Feature::ShellZshFork);
    assert_eq!(
        shell_type_for_model_and_features(&model, &features),
        ConfigShellToolType::ShellCommand
    );

    features.enable(Feature::UnifiedExecZshFork);
    assert_eq!(
        shell_type_for_model_and_features(&model, &features),
        expected_unified_exec
    );

    features.disable(Feature::ShellTool);
    assert_eq!(
        shell_type_for_model_and_features(&model, &features),
        ConfigShellToolType::Disabled
    );
}

#[test]
fn shell_command_backend_requires_both_shell_tool_and_zsh_fork() {
    let mut features = shell_features();
    assert_eq!(
        shell_command_backend_for_features(&features),
        ShellCommandBackendConfig::Classic
    );

    features.enable(Feature::ShellZshFork);
    assert_eq!(
        shell_command_backend_for_features(&features),
        ShellCommandBackendConfig::ZshFork
    );

    features.disable(Feature::ShellTool);
    assert_eq!(
        shell_command_backend_for_features(&features),
        ShellCommandBackendConfig::Classic
    );
}

#[test]
fn unified_exec_feature_mode_follows_composition_dependencies() {
    let mut features = shell_features();
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::Disabled
    );

    features.enable(Feature::UnifiedExec);
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::Direct
    );

    features.enable(Feature::UnifiedExecZshFork);
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::Direct
    );

    features.enable(Feature::ShellZshFork);
    features.disable(Feature::UnifiedExecZshFork);
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::Disabled
    );

    features.enable(Feature::UnifiedExecZshFork);
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::ZshFork
    );

    features.disable(Feature::ShellTool);
    assert_eq!(
        unified_exec_feature_mode_for_features(&features),
        UnifiedExecFeatureMode::Disabled
    );
}

#[test]
fn request_user_input_modes_follow_default_mode_feature() {
    let mut features = Features::with_defaults();
    features.disable(Feature::DefaultModeRequestUserInput);
    assert_eq!(
        request_user_input_available_modes(&features),
        vec![ModeKind::Plan]
    );

    features.enable(Feature::DefaultModeRequestUserInput);
    assert_eq!(
        request_user_input_available_modes(&features),
        vec![ModeKind::Default, ModeKind::Plan]
    );
}

#[test]
fn unified_exec_shell_mode_uses_zsh_fork_only_when_all_inputs_match() {
    let exe = std::env::current_exe().expect("current exe path");
    let shell = exe.clone();

    let mode = UnifiedExecShellMode::for_session(
        UnifiedExecFeatureMode::ZshFork,
        ToolUserShellType::Zsh,
        Some(&shell),
        Some(&exe),
    );
    if cfg!(unix) {
        assert!(matches!(mode, UnifiedExecShellMode::ZshFork(_)));
    } else {
        assert_eq!(mode, UnifiedExecShellMode::Direct);
    }

    assert_eq!(
        UnifiedExecShellMode::for_session(
            UnifiedExecFeatureMode::Direct,
            ToolUserShellType::Zsh,
            Some(&shell),
            Some(&exe),
        ),
        UnifiedExecShellMode::Direct
    );
}
