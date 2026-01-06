use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigProfile;
use codex_core::config::ConfigToml;
use codex_core::config::Constrained;
use codex_core::config::FeatureOverrides;
use codex_core::config::ProjectConfig;
use codex_core::config::SandboxPolicyResolution;
use codex_core::config::types::DEFAULT_OTEL_ENVIRONMENT;
use codex_core::config::types::History;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::Notice;
use codex_core::config::types::Notifications;
use codex_core::config::types::OtelConfig;
use codex_core::config::types::OtelConfigToml;
use codex_core::config::types::OtelExporterKind;
use codex_core::config::types::ScrollInputMode;
use codex_core::config::types::ShellEnvironmentPolicy;
use codex_core::config::types::UriBasedFileOpener;
use codex_core::config_loader::ConfigLayerStack;
use codex_core::config_loader::ConfigRequirements;
use codex_core::features::Feature;
use codex_core::features::Features;
use codex_core::model_provider_info::ModelProviderInfo;
use codex_core::model_provider_info::built_in_model_providers;
use codex_core::project_doc::DEFAULT_PROJECT_DOC_FILENAME;
use codex_core::project_doc::LOCAL_PROJECT_DOC_FILENAME;
use codex_git::GhostSnapshotConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;

use crate::config_types::AuthCredentialsStoreMode;
use crate::config_types::ForcedLoginMethod;
use crate::config_types::OAuthCredentialsStoreMode;
use crate::config_types::ReasoningSummary;
use crate::config_types::Verbosity;
use crate::openai_models::ReasoningEffort;
use crate::openai_models::ReasoningSummaryFormat;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Provenance for how this [`Config`] was derived (merged layers + enforced
    /// requirements).
    pub config_layer_stack: ConfigLayerStack,

    /// Optional override of model selection.
    pub model: Option<String>,

    /// Model used specifically for review sessions. Defaults to "gpt-5.1-codex-max".
    pub review_model: String,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<i64>,

    /// Token usage threshold triggering auto-compaction of conversation history.
    pub model_auto_compact_token_limit: Option<i64>,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Approval policy for executing commands.
    pub approval_policy: Constrained<AskForApproval>,

    pub sandbox_policy: Constrained<SandboxPolicy>,

    /// True if the user passed in an override or set a value in config.toml
    /// for either of approval_policy or sandbox_mode.
    pub did_user_set_custom_approval_policy_or_sandbox_mode: bool,

    /// On Windows, indicates that a previously configured workspace-write sandbox
    /// was coerced to read-only because native auto mode is unsupported.
    pub forced_auto_mode_downgraded_on_windows: bool,

    pub shell_environment_policy: ShellEnvironmentPolicy,

    /// When `true`, `AgentReasoning` events emitted by the backend will be
    /// suppressed from the frontend output. This can reduce visual noise when
    /// users are only interested in the final agent responses.
    pub hide_agent_reasoning: bool,

    /// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
    /// Defaults to `false`.
    pub show_raw_agent_reasoning: bool,

    /// User-provided instructions from AGENTS.md.
    pub user_instructions: Option<String>,

    /// Base instructions override.
    pub base_instructions: Option<String>,

    /// Developer instructions override injected as a separate message.
    pub developer_instructions: Option<String>,

    /// Compact prompt override.
    pub compact_prompt: Option<String>,

    /// Optional external notifier command. When set, Codex will spawn this
    /// program after each completed *turn* (i.e. when the agent finishes
    /// processing a user submission). The value must be the full command
    /// broken into argv tokens **without** the trailing JSON argument - Codex
    /// appends one extra argument containing a JSON payload describing the
    /// event.
    ///
    /// Example `~/.codex/config.toml` snippet:
    ///
    /// ```toml
    /// notify = ["notify-send", "Codex"]
    /// ```
    ///
    /// which will be invoked as:
    ///
    /// ```shell
    /// notify-send Codex '{"type":"agent-turn-complete","turn-id":"12345"}'
    /// ```
    ///
    /// If unset the feature is disabled.
    pub notify: Option<Vec<String>>,

    /// TUI notifications preference. When set, the TUI will send OSC 9 notifications on approvals
    /// and turn completions when not focused.
    pub tui_notifications: Notifications,

    /// Enable ASCII animations and shimmer effects in the TUI.
    pub animations: bool,

    /// Show startup tooltips in the TUI welcome screen.
    pub show_tooltips: bool,

    /// Override the events-per-wheel-tick factor for TUI2 scroll normalization.
    ///
    /// This is the same `tui.scroll_events_per_tick` value from `config.toml`, plumbed through the
    /// merged [`Config`] object (see [`Tui`]) so TUI2 can normalize scroll event density per
    /// terminal.
    pub tui_scroll_events_per_tick: Option<u16>,

    /// Override the number of lines applied per wheel tick in TUI2.
    ///
    /// This is the same `tui.scroll_wheel_lines` value from `config.toml` (see [`Tui`]). TUI2
    /// applies it to wheel-like scroll streams. Trackpad-like scrolling uses a separate
    /// `tui.scroll_trackpad_lines` setting.
    pub tui_scroll_wheel_lines: Option<u16>,

    /// Override the number of lines per tick-equivalent used for trackpad scrolling in TUI2.
    ///
    /// This is the same `tui.scroll_trackpad_lines` value from `config.toml` (see [`Tui`]).
    pub tui_scroll_trackpad_lines: Option<u16>,

    /// Trackpad acceleration: approximate number of events required to gain +1x speed in TUI2.
    ///
    /// This is the same `tui.scroll_trackpad_accel_events` value from `config.toml` (see [`Tui`]).
    pub tui_scroll_trackpad_accel_events: Option<u16>,

    /// Trackpad acceleration: maximum multiplier applied to trackpad-like streams in TUI2.
    ///
    /// This is the same `tui.scroll_trackpad_accel_max` value from `config.toml` (see [`Tui`]).
    pub tui_scroll_trackpad_accel_max: Option<u16>,

    /// Control how TUI2 interprets mouse scroll input (wheel vs trackpad).
    ///
    /// This is the same `tui.scroll_mode` value from `config.toml` (see [`Tui`]).
    pub tui_scroll_mode: ScrollInputMode,

    /// Override the wheel tick detection threshold (ms) for TUI2 auto scroll mode.
    ///
    /// This is the same `tui.scroll_wheel_tick_detect_max_ms` value from `config.toml` (see
    /// [`Tui`]).
    pub tui_scroll_wheel_tick_detect_max_ms: Option<u64>,

    /// Override the wheel-like end-of-stream threshold (ms) for TUI2 auto scroll mode.
    ///
    /// This is the same `tui.scroll_wheel_like_max_duration_ms` value from `config.toml` (see
    /// [`Tui`]).
    pub tui_scroll_wheel_like_max_duration_ms: Option<u64>,

    /// Invert mouse scroll direction for TUI2.
    ///
    /// This is the same `tui.scroll_invert` value from `config.toml` (see [`Tui`]) and is applied
    /// consistently to both mouse wheels and trackpads.
    pub tui_scroll_invert: bool,

    /// The directory that should be treated as the current working directory
    /// for the session. All relative paths inside the business-logic layer are
    /// resolved against this path.
    pub cwd: PathBuf,

    /// Preferred store for CLI auth credentials.
    /// file (default): Use a file in the Codex home directory.
    /// keyring: Use an OS-specific keyring service.
    /// auto: Use the OS-specific keyring service if available, otherwise use a file.
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Preferred store for MCP OAuth credentials.
    /// keyring: Use an OS-specific keyring service.
    ///          Credentials stored in the keyring will only be readable by Codex unless the user explicitly grants access via OS-level keyring access.
    ///          https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs#L2
    /// file: CODEX_HOME/.credentials.json
    ///       This file will be readable to Codex and other applications running as the same user.
    /// auto (default): keyring if available, otherwise file.
    pub mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode,

    /// Combined provider map (defaults merged with user-defined overrides).
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: usize,

    /// Additional filenames to try when looking for project-level docs.
    pub project_doc_fallback_filenames: Vec<String>,

    // todo(aibrahim): this should be used in the override model family
    /// Token budget applied when storing tool/function outputs in the context manager.
    pub tool_output_token_limit: Option<usize>,

    /// Directory containing all Codex state (defaults to `~/.codex` but can be
    /// overridden by the `CODEX_HOME` environment variable).
    pub codex_home: PathBuf,

    /// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
    pub history: History,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: UriBasedFileOpener,

    /// Path to the `codex-linux-sandbox` executable. This must be set if
    /// [`crate::exec::SandboxType::LinuxSeccomp`] is used. Note that this
    /// cannot be set in the config file: it must be set in code via
    /// [`ConfigOverrides`].
    ///
    /// When this program is invoked, arg0 will be set to `codex-linux-sandbox`.
    pub codex_linux_sandbox_exe: Option<PathBuf>,

    /// Value to use for `reasoning.effort` when making a request using the
    /// Responses API.
    pub model_reasoning_effort: Option<ReasoningEffort>,

    /// If not "none", the value to use for `reasoning.summary` when making a
    /// request using the Responses API.
    pub model_reasoning_summary: ReasoningSummary,

    /// Optional override to force-enable reasoning summaries for the configured model.
    pub model_supports_reasoning_summaries: Option<bool>,

    /// Optional override to force reasoning summary format for the configured model.
    pub model_reasoning_summary_format: Option<ReasoningSummaryFormat>,

    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: String,

    /// When set, restricts ChatGPT login to a specific workspace identifier.
    pub forced_chatgpt_workspace_id: Option<String>,

    /// When set, restricts the login mechanism users may use.
    pub forced_login_method: Option<ForcedLoginMethod>,

    /// Include the `apply_patch` tool for models that benefit from invoking
    /// file edits as a structured tool call. When unset, this falls back to the
    /// model family's default preference.
    pub include_apply_patch_tool: bool,

    pub tools_web_search_request: bool,

    /// If set to `true`, used only the experimental unified exec tool.
    pub use_experimental_unified_exec_tool: bool,

    /// Settings for ghost snapshots (used for undo).
    pub ghost_snapshot: GhostSnapshotConfig,

    /// Centralized feature flags; source of truth for feature gating.
    pub features: Features,

    /// The active profile name used to derive this `Config` (if any).
    pub active_profile: Option<String>,

    /// The currently active project config, resolved by checking if cwd:
    /// is (1) part of a git repo, (2) a git worktree, or (3) just using the cwd
    pub active_project: ProjectConfig,

    /// Tracks whether the Windows onboarding screen has been acknowledged.
    pub windows_wsl_setup_acknowledged: bool,

    /// Collection of various notices we show the user
    pub notices: Notice,

    /// When `true`, checks for Codex updates on startup and surfaces update prompts.
    /// Set to `false` only if your Codex updates are centrally managed.
    /// Defaults to `true`.
    pub check_for_update_on_startup: bool,

    /// When true, disables burst-paste detection for typed input entirely.
    /// All characters are inserted as they are received, and no buffering
    /// or placeholder replacement will occur for fast keypress bursts.
    pub disable_paste_burst: bool,

    /// OTEL configuration (exporter type, endpoint, headers, etc.).
    pub otel: OtelConfig,
}

impl Config {
    /// This is the preferred way to create an instance of [Config].
    pub async fn load_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> std::io::Result<Self> {
        ConfigBuilder::default()
            .cli_overrides(cli_overrides)
            .build()
            .await
    }

    /// This is a secondary way of creating [Config], which is appropriate when
    /// the harness is meant to be used with a specific configuration that
    /// ignores user settings. For example, the `codex exec` subcommand is
    /// designed to use [AskForApproval::Never] exclusively.
    ///
    /// Further, [ConfigOverrides] contains some options that are not supported
    /// in [ConfigToml], such as `cwd` and `codex_linux_sandbox_exe`.
    pub async fn load_with_cli_overrides_and_harness_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
        harness_overrides: ConfigOverrides,
    ) -> std::io::Result<Self> {
        ConfigBuilder::default()
            .cli_overrides(cli_overrides)
            .harness_overrides(harness_overrides)
            .build()
            .await
    }
}

impl Config {
    #[cfg(test)]
    fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: PathBuf,
    ) -> std::io::Result<Self> {
        // Note this ignores requirements.toml enforcement for tests.
        let config_layer_stack = ConfigLayerStack::default();
        Self::load_config_with_layer_stack(cfg, overrides, codex_home, config_layer_stack)
    }

    fn load_config_with_layer_stack(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: PathBuf,
        config_layer_stack: ConfigLayerStack,
    ) -> std::io::Result<Self> {
        let requirements = config_layer_stack.requirements().clone();
        let user_instructions = Self::load_instructions(Some(&codex_home));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            review_model: override_review_model,
            cwd,
            approval_policy: approval_policy_override,
            sandbox_mode,
            model_provider,
            config_profile: config_profile_key,
            codex_linux_sandbox_exe,
            base_instructions,
            developer_instructions,
            compact_prompt,
            include_apply_patch_tool: include_apply_patch_tool_override,
            show_raw_agent_reasoning,
            tools_web_search_request: override_tools_web_search_request,
            additional_writable_roots,
        } = overrides;

        let active_profile_name = config_profile_key
            .as_ref()
            .or(cfg.profile.as_ref())
            .cloned();
        let config_profile = match active_profile_name.as_ref() {
            Some(key) => cfg
                .profiles
                .get(key)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("config profile `{key}` not found"),
                    )
                })?
                .clone(),
            None => ConfigProfile::default(),
        };

        let feature_overrides = FeatureOverrides {
            include_apply_patch_tool: include_apply_patch_tool_override,
            web_search_request: override_tools_web_search_request,
        };

        let features = Features::from_config(&cfg, &config_profile, feature_overrides);
        #[cfg(target_os = "windows")]
        {
            // Base flag controls sandbox on/off; elevated only applies when base is enabled.
            let sandbox_enabled = features.enabled(Feature::WindowsSandbox);
            codex_core::safety::set_windows_sandbox_enabled(sandbox_enabled);
            let elevated_enabled =
                sandbox_enabled && features.enabled(Feature::WindowsSandboxElevated);
            codex_core::safety::set_windows_elevated_sandbox_enabled(elevated_enabled);
        }

        let resolved_cwd = {
            use std::env;

            match cwd {
                None => {
                    tracing::info!("cwd not set, using current dir");
                    env::current_dir()?
                }
                Some(p) if p.is_absolute() => p,
                Some(p) => {
                    // Resolve relative path against the current working directory.
                    tracing::info!("cwd is relative, resolving against current dir");
                    let mut current = env::current_dir()?;
                    current.push(p);
                    current
                }
            }
        };
        let additional_writable_roots: Vec<AbsolutePathBuf> = additional_writable_roots
            .into_iter()
            .map(|path| AbsolutePathBuf::resolve_path_against_base(path, &resolved_cwd))
            .collect::<Result<Vec<_>, _>>()?;
        let active_project = cfg
            .get_active_project(&resolved_cwd)
            .unwrap_or(ProjectConfig { trust_level: None });

        let SandboxPolicyResolution {
            policy: mut sandbox_policy,
            forced_auto_mode_downgraded_on_windows,
        } = cfg.derive_sandbox_policy(sandbox_mode, config_profile.sandbox_mode, &resolved_cwd);
        if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = &mut sandbox_policy {
            for path in additional_writable_roots {
                if !writable_roots.iter().any(|existing| existing == &path) {
                    writable_roots.push(path);
                }
            }
        }
        let approval_policy = approval_policy_override
            .or(config_profile.approval_policy)
            .or(cfg.approval_policy)
            .unwrap_or_else(|| {
                if active_project.is_trusted() {
                    AskForApproval::OnRequest
                } else if active_project.is_untrusted() {
                    AskForApproval::UnlessTrusted
                } else {
                    AskForApproval::default()
                }
            });
        // TODO(dylan): We should be able to leverage ConfigLayerStack so that
        // we can reliably check this at every config level.
        let did_user_set_custom_approval_policy_or_sandbox_mode = approval_policy_override
            .is_some()
            || config_profile.approval_policy.is_some()
            || cfg.approval_policy.is_some()
            || sandbox_mode.is_some()
            || config_profile.sandbox_mode.is_some()
            || cfg.sandbox_mode.is_some();

        let mut model_providers = built_in_model_providers();
        // Merge user-defined providers into the built-in list.
        for (key, provider) in cfg.model_providers.into_iter() {
            model_providers.entry(key).or_insert(provider);
        }

        let model_provider_id = model_provider
            .or(config_profile.model_provider)
            .or(cfg.model_provider)
            .unwrap_or_else(|| "openai".to_string());
        let model_provider = model_providers
            .get(&model_provider_id)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Model provider `{model_provider_id}` not found"),
                )
            })?
            .clone();

        let shell_environment_policy = cfg.shell_environment_policy.into();

        let history = cfg.history.unwrap_or_default();

        let ghost_snapshot = {
            let mut config = GhostSnapshotConfig::default();
            if let Some(ghost_snapshot) = cfg.ghost_snapshot.as_ref()
                && let Some(ignore_over_bytes) = ghost_snapshot.ignore_large_untracked_files
            {
                config.ignore_large_untracked_files = if ignore_over_bytes > 0 {
                    Some(ignore_over_bytes)
                } else {
                    None
                };
            }
            if let Some(ghost_snapshot) = cfg.ghost_snapshot.as_ref()
                && let Some(threshold) = ghost_snapshot.ignore_large_untracked_dirs
            {
                config.ignore_large_untracked_dirs =
                    if threshold > 0 { Some(threshold) } else { None };
            }
            if let Some(ghost_snapshot) = cfg.ghost_snapshot.as_ref()
                && let Some(disable_warnings) = ghost_snapshot.disable_warnings
            {
                config.disable_warnings = disable_warnings;
            }
            config
        };

        let include_apply_patch_tool_flag = features.enabled(Feature::ApplyPatchFreeform);
        let tools_web_search_request = features.enabled(Feature::WebSearchRequest);
        let use_experimental_unified_exec_tool = features.enabled(Feature::UnifiedExec);

        let forced_chatgpt_workspace_id =
            cfg.forced_chatgpt_workspace_id.as_ref().and_then(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

        let forced_login_method = cfg.forced_login_method;

        let model = model.or(config_profile.model).or(cfg.model);

        let compact_prompt = compact_prompt.or(cfg.compact_prompt).and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        // Load base instructions override from a file if specified. If the
        // path is relative, resolve it against the effective cwd so the
        // behaviour matches other path-like config values.
        let experimental_instructions_path = config_profile
            .experimental_instructions_file
            .as_ref()
            .or(cfg.experimental_instructions_file.as_ref());
        let file_base_instructions = Self::try_read_non_empty_file(
            experimental_instructions_path,
            "experimental instructions file",
        )?;
        let base_instructions = base_instructions.or(file_base_instructions);
        let developer_instructions = developer_instructions.or(cfg.developer_instructions);

        let experimental_compact_prompt_path = config_profile
            .experimental_compact_prompt_file
            .as_ref()
            .or(cfg.experimental_compact_prompt_file.as_ref());
        let file_compact_prompt = Self::try_read_non_empty_file(
            experimental_compact_prompt_path,
            "experimental compact prompt file",
        )?;
        let compact_prompt = compact_prompt.or(file_compact_prompt);

        // Default review model when not set in config; allow CLI override to take precedence.
        let review_model = override_review_model
            .or(cfg.review_model)
            .unwrap_or_else(|| "gpt-5.1-codex-max".to_string());

        let check_for_update_on_startup = cfg.check_for_update_on_startup.unwrap_or(true);

        // Ensure that every field of ConfigRequirements is applied to the final
        // Config.
        let ConfigRequirements {
            approval_policy: mut constrained_approval_policy,
            sandbox_policy: mut constrained_sandbox_policy,
        } = requirements;

        constrained_approval_policy
            .set(approval_policy)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?;
        constrained_sandbox_policy
            .set(sandbox_policy)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?;

        let config = Self {
            model,
            review_model,
            model_context_window: cfg.model_context_window,
            model_auto_compact_token_limit: cfg.model_auto_compact_token_limit,
            model_provider_id,
            model_provider,
            cwd: resolved_cwd,
            approval_policy: constrained_approval_policy,
            sandbox_policy: constrained_sandbox_policy,
            did_user_set_custom_approval_policy_or_sandbox_mode,
            forced_auto_mode_downgraded_on_windows,
            shell_environment_policy,
            notify: cfg.notify,
            user_instructions,
            base_instructions,
            developer_instructions,
            compact_prompt,
            // The config.toml omits "_mode" because it's a config file. However, "_mode"
            // is important in code to differentiate the mode from the store implementation.
            cli_auth_credentials_store_mode: cfg.cli_auth_credentials_store.unwrap_or_default(),
            mcp_servers: cfg.mcp_servers,
            // The config.toml omits "_mode" because it's a config file. However, "_mode"
            // is important in code to differentiate the mode from the store implementation.
            mcp_oauth_credentials_store_mode: cfg.mcp_oauth_credentials_store.unwrap_or_default(),
            model_providers,
            project_doc_max_bytes: cfg.project_doc_max_bytes.unwrap_or(32 * 1024), // 32 KiB
            project_doc_fallback_filenames: cfg
                .project_doc_fallback_filenames
                .unwrap_or_default()
                .into_iter()
                .filter_map(|name| {
                    let trimmed = name.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
                .collect(),
            tool_output_token_limit: cfg.tool_output_token_limit,
            codex_home,
            config_layer_stack,
            history,
            file_opener: cfg.file_opener.unwrap_or(UriBasedFileOpener::VsCode),
            codex_linux_sandbox_exe,

            hide_agent_reasoning: cfg.hide_agent_reasoning.unwrap_or(false),
            show_raw_agent_reasoning: cfg
                .show_raw_agent_reasoning
                .or(show_raw_agent_reasoning)
                .unwrap_or(false),
            model_reasoning_effort: config_profile
                .model_reasoning_effort
                .or(cfg.model_reasoning_effort),
            model_reasoning_summary: config_profile
                .model_reasoning_summary
                .or(cfg.model_reasoning_summary)
                .unwrap_or_default(),
            model_supports_reasoning_summaries: cfg.model_supports_reasoning_summaries,
            model_reasoning_summary_format: cfg.model_reasoning_summary_format.clone(),
            model_verbosity: config_profile.model_verbosity.or(cfg.model_verbosity),
            chatgpt_base_url: config_profile
                .chatgpt_base_url
                .or(cfg.chatgpt_base_url)
                .unwrap_or("https://chatgpt.com/backend-api/".to_string()),
            forced_chatgpt_workspace_id,
            forced_login_method,
            include_apply_patch_tool: include_apply_patch_tool_flag,
            tools_web_search_request,
            use_experimental_unified_exec_tool,
            ghost_snapshot,
            features,
            active_profile: active_profile_name,
            active_project,
            windows_wsl_setup_acknowledged: cfg.windows_wsl_setup_acknowledged.unwrap_or(false),
            notices: cfg.notice.unwrap_or_default(),
            check_for_update_on_startup,
            disable_paste_burst: cfg.disable_paste_burst.unwrap_or(false),
            tui_notifications: cfg
                .tui
                .as_ref()
                .map(|t| t.notifications.clone())
                .unwrap_or_default(),
            animations: cfg.tui.as_ref().map(|t| t.animations).unwrap_or(true),
            show_tooltips: cfg.tui.as_ref().map(|t| t.show_tooltips).unwrap_or(true),
            tui_scroll_events_per_tick: cfg.tui.as_ref().and_then(|t| t.scroll_events_per_tick),
            tui_scroll_wheel_lines: cfg.tui.as_ref().and_then(|t| t.scroll_wheel_lines),
            tui_scroll_trackpad_lines: cfg.tui.as_ref().and_then(|t| t.scroll_trackpad_lines),
            tui_scroll_trackpad_accel_events: cfg
                .tui
                .as_ref()
                .and_then(|t| t.scroll_trackpad_accel_events),
            tui_scroll_trackpad_accel_max: cfg
                .tui
                .as_ref()
                .and_then(|t| t.scroll_trackpad_accel_max),
            tui_scroll_mode: cfg.tui.as_ref().map(|t| t.scroll_mode).unwrap_or_default(),
            tui_scroll_wheel_tick_detect_max_ms: cfg
                .tui
                .as_ref()
                .and_then(|t| t.scroll_wheel_tick_detect_max_ms),
            tui_scroll_wheel_like_max_duration_ms: cfg
                .tui
                .as_ref()
                .and_then(|t| t.scroll_wheel_like_max_duration_ms),
            tui_scroll_invert: cfg.tui.as_ref().map(|t| t.scroll_invert).unwrap_or(false),
            otel: {
                let t: OtelConfigToml = cfg.otel.unwrap_or_default();
                let log_user_prompt = t.log_user_prompt.unwrap_or(false);
                let environment = t
                    .environment
                    .unwrap_or(DEFAULT_OTEL_ENVIRONMENT.to_string());
                let exporter = t.exporter.unwrap_or(OtelExporterKind::None);
                let trace_exporter = t.trace_exporter.unwrap_or_else(|| exporter.clone());
                OtelConfig {
                    log_user_prompt,
                    environment,
                    exporter,
                    trace_exporter,
                }
            },
        };
        Ok(config)
    }

    fn load_instructions(codex_dir: Option<&Path>) -> Option<String> {
        let base = codex_dir?;
        for candidate in [LOCAL_PROJECT_DOC_FILENAME, DEFAULT_PROJECT_DOC_FILENAME] {
            let mut path = base.to_path_buf();
            path.push(candidate);
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let trimmed = contents.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    /// If `path` is `Some`, attempts to read the file at the given path and
    /// returns its contents as a trimmed `String`. If the file is empty, or
    /// is `Some` but cannot be read, returns an `Err`.
    fn try_read_non_empty_file(
        path: Option<&AbsolutePathBuf>,
        context: &str,
    ) -> std::io::Result<Option<String>> {
        let Some(path) = path else {
            return Ok(None);
        };

        let contents = std::fs::read_to_string(path).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("failed to read {context} {}: {e}", path.display()),
            )
        })?;

        let s = contents.trim().to_string();
        if s.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{context} is empty: {}", path.display()),
            ))
        } else {
            Ok(Some(s))
        }
    }

    pub fn set_windows_sandbox_globally(&mut self, value: bool) {
        codex_core::safety::set_windows_sandbox_enabled(value);
        if value {
            self.features.enable(Feature::WindowsSandbox);
        } else {
            self.features.disable(Feature::WindowsSandbox);
        }
        self.forced_auto_mode_downgraded_on_windows = !value;
    }
}
