use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnifiedExecFeatureMode {
    /// Unified exec should not be selected by this feature set.
    ///
    Disabled,
    Direct,
    ZshFork,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ToolUserShellType {
    Zsh,
    Bash,
    PowerShell,
    Sh,
    Cmd,
}

pub fn request_user_input_available_modes(features: &Features) -> Vec<ModeKind> {
    TUI_VISIBLE_COLLABORATION_MODES
        .into_iter()
        .filter(|mode| {
            mode.allows_request_user_input()
                || (features.enabled(Feature::DefaultModeRequestUserInput)
                    && *mode == ModeKind::Default)
        })
        .collect()
}

/// Returns the unified-exec mode requested by feature policy, before runtime
/// session inputs such as platform, user shell, and zsh-fork binary paths are
/// resolved.
///
/// Disabling unified exec keeps command execution disabled. The legacy
/// composition flag can still disable zsh-fork interception independently.
pub fn unified_exec_feature_mode_for_features(features: &Features) -> UnifiedExecFeatureMode {
    if !features.enabled(Feature::ShellTool) || !features.enabled(Feature::UnifiedExec) {
        UnifiedExecFeatureMode::Disabled
    } else if features.enabled(Feature::ShellZshFork) {
        if features.enabled(Feature::UnifiedExecZshFork) {
            UnifiedExecFeatureMode::ZshFork
        } else {
            UnifiedExecFeatureMode::Direct
        }
    } else {
        UnifiedExecFeatureMode::Direct
    }
}

pub fn shell_type_for_model_and_features(
    model_info: &ModelInfo,
    features: &Features,
) -> ConfigShellToolType {
    if !features.enabled(Feature::ShellTool)
        || !features.enabled(Feature::UnifiedExec)
        || matches!(model_info.shell_type, ConfigShellToolType::Disabled)
    {
        ConfigShellToolType::Disabled
    } else {
        ConfigShellToolType::UnifiedExec
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UnifiedExecShellMode {
    Direct,
    ZshFork(ZshForkConfig),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ZshForkConfig {
    pub shell_zsh_path: AbsolutePathBuf,
    pub main_execve_wrapper_exe: AbsolutePathBuf,
}

impl UnifiedExecShellMode {
    pub fn for_session(
        feature_mode: UnifiedExecFeatureMode,
        user_shell_type: ToolUserShellType,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
    ) -> Self {
        if cfg!(unix)
            && matches!(feature_mode, UnifiedExecFeatureMode::ZshFork)
            && matches!(user_shell_type, ToolUserShellType::Zsh)
            && let (Some(shell_zsh_path), Some(main_execve_wrapper_exe)) =
                (shell_zsh_path, main_execve_wrapper_exe)
            && let (Ok(shell_zsh_path), Ok(main_execve_wrapper_exe)) = (
                AbsolutePathBuf::try_from(shell_zsh_path.as_path()).inspect_err(|err| {
                    tracing::warn!(
                        "Failed to convert shell_zsh_path `{shell_zsh_path:?}`: {err:?}"
                    )
                }),
                AbsolutePathBuf::try_from(main_execve_wrapper_exe.as_path()).inspect_err(
                    |err| {
                        tracing::warn!(
                            "Failed to convert main_execve_wrapper_exe `{main_execve_wrapper_exe:?}`: {err:?}"
                        )
                    },
                ),
            )
        {
            Self::ZshFork(ZshForkConfig {
                shell_zsh_path,
                main_execve_wrapper_exe,
            })
        } else {
            Self::Direct
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEnvironmentMode {
    None,
    Single,
    Multiple,
}

impl ToolEnvironmentMode {
    pub fn from_count(count: usize) -> Self {
        match count {
            0 => Self::None,
            1 => Self::Single,
            _ => Self::Multiple,
        }
    }

    pub fn has_environment(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(test)]
#[path = "tool_config_tests.rs"]
mod tests;
