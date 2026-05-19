use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShellCommandBackendConfig {
    Classic,
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

pub fn shell_command_backend_for_features(features: &Features) -> ShellCommandBackendConfig {
    if features.enabled(Feature::ShellTool) && features.enabled(Feature::ShellZshFork) {
        ShellCommandBackendConfig::ZshFork
    } else {
        ShellCommandBackendConfig::Classic
    }
}

pub fn shell_type_for_model_and_features(
    model_info: &ModelInfo,
    features: &Features,
) -> ConfigShellToolType {
    let unified_exec_enabled = features.enabled(Feature::UnifiedExec);
    let model_shell_type = match model_info.shell_type {
        ConfigShellToolType::UnifiedExec if !unified_exec_enabled => {
            ConfigShellToolType::ShellCommand
        }
        ConfigShellToolType::Default | ConfigShellToolType::Local => {
            ConfigShellToolType::ShellCommand
        }
        other => other,
    };

    if !features.enabled(Feature::ShellTool) {
        ConfigShellToolType::Disabled
    } else if features.enabled(Feature::ShellZshFork) {
        ConfigShellToolType::ShellCommand
    } else if unified_exec_enabled {
        if codex_utils_pty::conpty_supported() {
            ConfigShellToolType::UnifiedExec
        } else {
            ConfigShellToolType::ShellCommand
        }
    } else {
        model_shell_type
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
        shell_command_backend: ShellCommandBackendConfig,
        user_shell_type: ToolUserShellType,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
    ) -> Self {
        if cfg!(unix)
            && shell_command_backend == ShellCommandBackendConfig::ZshFork
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
