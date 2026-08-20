use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use pretty_assertions::assert_eq;

use super::*;

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
fn unified_exec_shell_mode_respects_feature_and_policy_gates() {
    let exe = std::env::current_exe().expect("current exe path");
    let shell = exe.clone();
    let mut features = Features::with_defaults();
    features.enable(Feature::ShellTool);
    features.enable(Feature::ShellZshFork);
    let mode = UnifiedExecShellMode::for_session(
        &features,
        ToolUserShellType::Zsh,
        Some(&shell),
        Some(&exe),
    );
    if cfg!(unix) {
        assert!(matches!(mode, UnifiedExecShellMode::ZshFork(_)));
    } else {
        assert_eq!(mode, UnifiedExecShellMode::Direct);
    }

    features.disable(Feature::ShellZshFork);
    assert_eq!(
        UnifiedExecShellMode::for_session(
            &features,
            ToolUserShellType::Zsh,
            Some(&shell),
            Some(&exe),
        ),
        UnifiedExecShellMode::Direct
    );

    features.enable(Feature::ShellZshFork);
    features.disable(Feature::UnifiedExecZshFork);
    assert_eq!(
        UnifiedExecShellMode::for_session(
            &features,
            ToolUserShellType::Zsh,
            Some(&shell),
            Some(&exe),
        ),
        UnifiedExecShellMode::Direct
    );

    features.enable(Feature::UnifiedExecZshFork);
    features.disable(Feature::UnifiedExec);
    assert_eq!(
        UnifiedExecShellMode::for_session(
            &features,
            ToolUserShellType::Zsh,
            Some(&shell),
            Some(&exe),
        ),
        UnifiedExecShellMode::Direct
    );

    features.enable(Feature::UnifiedExec);
    features.disable(Feature::ShellTool);
    assert_eq!(
        UnifiedExecShellMode::for_session(
            &features,
            ToolUserShellType::Zsh,
            Some(&shell),
            Some(&exe),
        ),
        UnifiedExecShellMode::Direct
    );
}
