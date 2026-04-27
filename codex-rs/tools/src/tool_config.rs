use crate::can_request_original_image_detail;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::WebSearchConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
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

#[derive(Debug, Clone)]
pub struct ToolsConfig {
    pub available_models: Vec<ModelPreset>,
    pub shell_type: ConfigShellToolType,
    pub shell_command_backend: ShellCommandBackendConfig,
    pub unified_exec_shell_mode: UnifiedExecShellMode,
    pub has_environment: bool,
    pub allow_login_shell: bool,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_mode: Option<WebSearchMode>,
    pub web_search_config: Option<WebSearchConfig>,
    pub web_search_tool_type: WebSearchToolType,
    pub image_gen_tool: bool,
    pub search_tool: bool,
    pub tool_suggest: bool,
    pub exec_permission_approvals_enabled: bool,
    pub request_permissions_tool_enabled: bool,
    pub code_mode_enabled: bool,
    pub code_mode_only_enabled: bool,
    pub can_request_original_image_detail: bool,
    pub collab_tools: bool,
    pub goal_tools: bool,
    pub multi_agent_v2: bool,
    pub hide_spawn_agent_metadata: bool,
    pub spawn_agent_usage_hint: bool,
    pub spawn_agent_usage_hint_text: Option<String>,
    pub max_concurrent_threads_per_session: Option<usize>,
    pub default_mode_request_user_input: bool,
    pub experimental_supported_tools: Vec<String>,
    pub agent_jobs_tools: bool,
    pub agent_jobs_worker_tools: bool,
    pub agent_type_description: String,
}

pub struct ToolsConfigParams<'a> {
    pub model_info: &'a ModelInfo,
    pub available_models: &'a [ModelPreset],
    pub features: &'a Features,
    pub image_generation_tool_auth_allowed: bool,
    pub web_search_mode: Option<WebSearchMode>,
    pub session_source: SessionSource,
    pub permission_profile: &'a PermissionProfile,
    pub windows_sandbox_level: WindowsSandboxLevel,
}

impl ToolsConfig {
    pub fn new(params: &ToolsConfigParams<'_>) -> Self {
        let ToolsConfigParams {
            model_info,
            available_models,
            features,
            image_generation_tool_auth_allowed,
            web_search_mode,
            session_source,
            permission_profile,
            windows_sandbox_level,
        } = params;
        let include_apply_patch_tool = features.enabled(Feature::ApplyPatchFreeform);
        let include_code_mode = features.enabled(Feature::CodeMode);
        let include_code_mode_only = include_code_mode && features.enabled(Feature::CodeModeOnly);
        let include_collab_tools = features.enabled(Feature::Collab);
        let include_goal_tools = features.enabled(Feature::Goals);
        let include_multi_agent_v2 = features.enabled(Feature::MultiAgentV2);
        let include_agent_jobs = features.enabled(Feature::SpawnCsv);
        let include_default_mode_request_user_input =
            features.enabled(Feature::DefaultModeRequestUserInput);
        let include_search_tool =
            model_info.supports_search_tool && features.enabled(Feature::ToolSearch);
        let include_tool_suggest = features.enabled(Feature::ToolSuggest)
            && features.enabled(Feature::Apps)
            && features.enabled(Feature::Plugins);
        let include_original_image_detail = can_request_original_image_detail(model_info);
        // API-key auth bypasses Codex backend entitlement/tool normalization, so
        // callers must confirm ChatGPT auth before exposing the built-in tool.
        let include_image_gen_tool = *image_generation_tool_auth_allowed
            && features.enabled(Feature::ImageGeneration)
            && supports_image_generation(model_info);
        let exec_permission_approvals_enabled = features.enabled(Feature::ExecPermissionApprovals);
        let request_permissions_tool_enabled = features.enabled(Feature::RequestPermissionsTool);
        let shell_command_backend =
            if features.enabled(Feature::ShellTool) && features.enabled(Feature::ShellZshFork) {
                ShellCommandBackendConfig::ZshFork
            } else {
                ShellCommandBackendConfig::Classic
            };
        let unified_exec_allowed = unified_exec_allowed_in_environment(
            cfg!(target_os = "windows"),
            permission_profile,
            *windows_sandbox_level,
        );
        let shell_type = if !features.enabled(Feature::ShellTool) {
            ConfigShellToolType::Disabled
        } else if features.enabled(Feature::ShellZshFork) {
            ConfigShellToolType::ShellCommand
        } else if features.enabled(Feature::UnifiedExec) && unified_exec_allowed {
            if codex_utils_pty::conpty_supported() {
                ConfigShellToolType::UnifiedExec
            } else {
                ConfigShellToolType::ShellCommand
            }
        } else if model_info.shell_type == ConfigShellToolType::UnifiedExec && !unified_exec_allowed
        {
            ConfigShellToolType::ShellCommand
        } else {
            model_info.shell_type
        };

        let apply_patch_tool_type = match model_info.apply_patch_tool_type {
            Some(ApplyPatchToolType::Freeform) => Some(ApplyPatchToolType::Freeform),
            Some(ApplyPatchToolType::Function) => Some(ApplyPatchToolType::Function),
            None => include_apply_patch_tool.then_some(ApplyPatchToolType::Freeform),
        };

        let agent_jobs_worker_tools = include_agent_jobs
            && matches!(
                session_source,
                SessionSource::SubAgent(SubAgentSource::Other(label))
                    if label.starts_with("agent_job:")
            );

        Self {
            available_models: available_models.to_vec(),
            shell_type,
            shell_command_backend,
            unified_exec_shell_mode: UnifiedExecShellMode::Direct,
            has_environment: true,
            allow_login_shell: true,
            apply_patch_tool_type,
            web_search_mode: *web_search_mode,
            web_search_config: None,
            web_search_tool_type: model_info.web_search_tool_type,
            image_gen_tool: include_image_gen_tool,
            search_tool: include_search_tool,
            tool_suggest: include_tool_suggest,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
            code_mode_enabled: include_code_mode,
            code_mode_only_enabled: include_code_mode_only,
            can_request_original_image_detail: include_original_image_detail,
            collab_tools: include_collab_tools,
            goal_tools: include_goal_tools,
            multi_agent_v2: include_multi_agent_v2,
            hide_spawn_agent_metadata: false,
            spawn_agent_usage_hint: true,
            spawn_agent_usage_hint_text: None,
            max_concurrent_threads_per_session: None,
            default_mode_request_user_input: include_default_mode_request_user_input,
            experimental_supported_tools: model_info.experimental_supported_tools.clone(),
            agent_jobs_tools: include_agent_jobs,
            agent_jobs_worker_tools,
            agent_type_description: String::new(),
        }
    }

    pub fn with_agent_type_description(mut self, agent_type_description: String) -> Self {
        self.agent_type_description = agent_type_description;
        self
    }

    pub fn with_spawn_agent_usage_hint(mut self, spawn_agent_usage_hint: bool) -> Self {
        self.spawn_agent_usage_hint = spawn_agent_usage_hint;
        self
    }

    pub fn with_spawn_agent_usage_hint_text(
        mut self,
        spawn_agent_usage_hint_text: Option<String>,
    ) -> Self {
        self.spawn_agent_usage_hint_text = spawn_agent_usage_hint_text;
        self
    }

    pub fn with_hide_spawn_agent_metadata(mut self, hide_spawn_agent_metadata: bool) -> Self {
        self.hide_spawn_agent_metadata = hide_spawn_agent_metadata;
        self
    }

    pub fn with_goal_tools_allowed(mut self, allowed: bool) -> Self {
        self.goal_tools = self.goal_tools && allowed;
        self
    }

    pub fn with_max_concurrent_threads_per_session(
        mut self,
        max_concurrent_threads_per_session: Option<usize>,
    ) -> Self {
        self.max_concurrent_threads_per_session = max_concurrent_threads_per_session;
        self
    }

    pub fn with_allow_login_shell(mut self, allow_login_shell: bool) -> Self {
        self.allow_login_shell = allow_login_shell;
        self
    }

    pub fn with_has_environment(mut self, has_environment: bool) -> Self {
        self.has_environment = has_environment;
        self
    }

    pub fn with_unified_exec_shell_mode(
        mut self,
        unified_exec_shell_mode: UnifiedExecShellMode,
    ) -> Self {
        self.unified_exec_shell_mode = unified_exec_shell_mode;
        self
    }

    pub fn with_unified_exec_shell_mode_for_session(
        mut self,
        user_shell_type: ToolUserShellType,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
    ) -> Self {
        self.unified_exec_shell_mode = UnifiedExecShellMode::for_session(
            self.shell_command_backend,
            user_shell_type,
            shell_zsh_path,
            main_execve_wrapper_exe,
        );
        self
    }

    pub fn with_web_search_config(mut self, web_search_config: Option<WebSearchConfig>) -> Self {
        self.web_search_config = web_search_config;
        self
    }

    pub fn for_code_mode_nested_tools(&self) -> Self {
        let mut nested = self.clone();
        nested.code_mode_enabled = false;
        nested.code_mode_only_enabled = false;
        nested
    }
}

fn supports_image_generation(model_info: &ModelInfo) -> bool {
    model_info.input_modalities.contains(&InputModality::Image)
}

fn unified_exec_allowed_in_environment(
    is_windows: bool,
    permission_profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
) -> bool {
    let managed_sandbox_required = match permission_profile {
        PermissionProfile::Managed {
            file_system,
            network,
        } => !file_system.to_sandbox_policy().has_full_disk_write_access() || !network.is_enabled(),
        PermissionProfile::Disabled | PermissionProfile::External { .. } => false,
    };
    !(is_windows
        && windows_sandbox_level != WindowsSandboxLevel::Disabled
        && managed_sandbox_required)
}

#[cfg(test)]
#[path = "tool_config_tests.rs"]
mod tests;
