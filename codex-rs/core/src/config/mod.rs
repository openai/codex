use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::ConfigRequirementsToml;
use crate::config_loader::ConstrainedWithSource;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::McpServerIdentity;
use crate::config_loader::McpServerRequirement;
use crate::config_loader::ResidencyRequirement;
use crate::config_loader::Sourced;
use crate::config_loader::load_config_layers_state;
use crate::config_loader::project_trust_key;
use crate::memories::memory_root;
use crate::path_utils::normalize_for_native_workdir;
use crate::project_doc::DEFAULT_PROJECT_DOC_FILENAME;
use crate::project_doc::LOCAL_PROJECT_DOC_FILENAME;
use crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS;
use crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS;
use crate::windows_sandbox::WindowsSandboxLevelExt;
use crate::windows_sandbox::resolve_windows_sandbox_mode;
use crate::windows_sandbox::resolve_windows_sandbox_private_desktop;
use codex_config::config_toml::ConfigToml;
use codex_config::config_toml::ProjectConfig;
use codex_config::config_toml::RealtimeAudioConfig;
use codex_config::config_toml::RealtimeConfig;
use codex_config::config_toml::validate_model_providers;
use codex_config::profile_toml::ConfigProfile;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::AuthCredentialsStoreMode;
use codex_config::types::DEFAULT_OTEL_ENVIRONMENT;
use codex_config::types::History;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerDisabledReason;
use codex_config::types::McpServerTransportConfig;
use codex_config::types::MemoriesConfig;
use codex_config::types::ModelAvailabilityNuxConfig;
use codex_config::types::Notice;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_config::types::OtelConfig;
use codex_config::types::OtelConfigToml;
use codex_config::types::OtelExporterKind;
use codex_config::types::ShellEnvironmentPolicy;
use codex_config::types::ToolSuggestConfig;
use codex_config::types::ToolSuggestDiscoverable;
use codex_config::types::TuiNotificationSettings;
use codex_config::types::UriBasedFileOpener;
use codex_config::types::WindowsSandboxModeToml;
use codex_features::Feature;
use codex_features::FeatureConfigSource;
use codex_features::FeatureOverrides;
use codex_features::FeatureToml;
use codex_features::Features;
use codex_features::FeaturesToml;
use codex_features::MultiAgentV2ConfigToml;
use codex_login::AuthManagerConfig;
use codex_mcp::McpConfig;
use codex_model_provider_info::LEGACY_OLLAMA_CHAT_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_CHAT_PROVIDER_REMOVED_ERROR;
use codex_model_provider_info::built_in_model_providers;
use codex_models_manager::ModelsManagerConfig;
use codex_protocol::config_types::AltScreenMode;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::config_types::Verbosity;
use codex_protocol::config_types::WebSearchConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
#[cfg(test)]
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use crate::config::permissions::compile_permission_profile;
use crate::config::permissions::get_readable_roots_required_for_codex_runtime;
use crate::config::permissions::network_proxy_config_from_profile_network;
use codex_network_proxy::NetworkProxyConfig;
use toml::Value as TomlValue;
use toml_edit::DocumentMut;

pub(crate) mod agent_roles;
pub mod edit;
mod managed_features;
mod network_proxy_spec;
mod permissions;
#[cfg(test)]
mod schema;
pub(crate) mod service;
pub use codex_config::Constrained;
pub use codex_config::ConstraintError;
pub use codex_config::ConstraintResult;
pub use codex_network_proxy::NetworkProxyAuditMetadata;
pub use codex_sandboxing::system_bwrap_warning;
pub use managed_features::ManagedFeatures;
pub use network_proxy_spec::NetworkProxySpec;
pub use network_proxy_spec::StartedNetworkProxy;
pub(crate) use permissions::resolve_permission_profile;
pub use service::ConfigService;
pub use service::ConfigServiceError;

pub use codex_git_utils::GhostSnapshotConfig;

/// Maximum number of bytes of the documentation that will be embedded. Larger
/// files are *silently truncated* to this size so we do not take up too much of
/// the context window.
pub(crate) const PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32 KiB
pub(crate) const DEFAULT_AGENT_MAX_THREADS: Option<usize> = Some(6);
pub(crate) const DEFAULT_AGENT_MAX_DEPTH: i32 = 1;
pub(crate) const DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS: Option<u64> = None;
const LOCAL_DEV_BUILD_VERSION: &str = "0.0.0";

pub const CONFIG_TOML_FILE: &str = "config.toml";

fn resolve_sqlite_home_env(resolved_cwd: &Path) -> Option<PathBuf> {
    let raw = std::env::var(codex_state::SQLITE_HOME_ENV).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(resolved_cwd.join(path))
    }
}

fn resolve_cli_auth_credentials_store_mode(
    configured: AuthCredentialsStoreMode,
    package_version: &str,
) -> AuthCredentialsStoreMode {
    match (package_version, configured) {
        (
            LOCAL_DEV_BUILD_VERSION,
            AuthCredentialsStoreMode::Keyring | AuthCredentialsStoreMode::Auto,
        ) => AuthCredentialsStoreMode::File,
        (_, mode) => mode,
    }
}

fn resolve_mcp_oauth_credentials_store_mode(
    configured: OAuthCredentialsStoreMode,
    package_version: &str,
) -> OAuthCredentialsStoreMode {
    match (package_version, configured) {
        (
            LOCAL_DEV_BUILD_VERSION,
            OAuthCredentialsStoreMode::Keyring | OAuthCredentialsStoreMode::Auto,
        ) => OAuthCredentialsStoreMode::File,
        (_, mode) => mode,
    }
}

#[cfg(test)]
pub(crate) async fn test_config() -> Config {
    let codex_home = tempfile::tempdir().expect("create temp dir");
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("temp dir should resolve"),
    )
    .await
    .expect("load default test config")
}

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Permissions {
    /// Approval policy for executing commands.
    pub approval_policy: Constrained<AskForApproval>,
    /// Effective sandbox policy used for shell/unified exec.
    pub sandbox_policy: Constrained<SandboxPolicy>,
    /// Effective filesystem sandbox policy, including entries that cannot yet
    /// be fully represented by the legacy [`SandboxPolicy`] projection.
    pub file_system_sandbox_policy: FileSystemSandboxPolicy,
    /// Effective network sandbox policy split out from the legacy
    /// [`SandboxPolicy`] projection.
    pub network_sandbox_policy: NetworkSandboxPolicy,
    /// Effective network configuration applied to all spawned processes.
    pub network: Option<NetworkProxySpec>,
    /// Whether the model may request a login shell for shell-based tools.
    /// Default to `true`
    ///
    /// If `true`, the model may request a login shell (`login = true`), and
    /// omitting `login` defaults to using a login shell.
    /// If `false`, the model can never use a login shell: `login = true`
    /// requests are rejected, and omitting `login` defaults to a non-login
    /// shell.
    pub allow_login_shell: bool,
    /// Policy used to build process environments for shell/unified exec.
    pub shell_environment_policy: ShellEnvironmentPolicy,
    /// Effective Windows sandbox mode derived from `[windows].sandbox` or
    /// legacy feature keys.
    pub windows_sandbox_mode: Option<WindowsSandboxModeToml>,
    /// Whether the final Windows sandboxed child should run on a private desktop.
    pub windows_sandbox_private_desktop: bool,
}

impl Permissions {
    #[cfg(test)]
    pub(crate) fn runtime_permission_profile(&self) -> PermissionProfile {
        PermissionProfile::from_runtime_permissions(
            &self.file_system_sandbox_policy,
            self.network_sandbox_policy,
        )
    }
}

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Provenance for how this [`Config`] was derived (merged layers + enforced
    /// requirements).
    pub config_layer_stack: ConfigLayerStack,

    /// Warnings collected during config load that should be shown on startup.
    pub startup_warnings: Vec<String>,

    /// Optional override of model selection.
    pub model: Option<String>,

    /// Effective service tier preference for new turns (`fast` or `flex`).
    pub service_tier: Option<ServiceTier>,

    /// Model used specifically for review sessions.
    pub review_model: Option<String>,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<i64>,

    /// Token usage threshold triggering auto-compaction of conversation history.
    pub model_auto_compact_token_limit: Option<i64>,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Optionally specify the personality of the model
    pub personality: Option<Personality>,

    /// Effective permission configuration for shell tool execution.
    pub permissions: Permissions,

    /// Configures who approval requests are routed to for review once they have
    /// been escalated. This does not disable separate safety checks such as
    /// ARC.
    pub approvals_reviewer: ApprovalsReviewer,

    /// enforce_residency means web traffic cannot be routed outside of a
    /// particular geography. HTTP clients should direct their requests
    /// using backend-specific headers or URLs to enforce this.
    pub enforce_residency: Constrained<Option<ResidencyRequirement>>,

    /// When `true`, `AgentReasoning` events emitted by the backend will be
    /// suppressed from the frontend output. This can reduce visual noise when
    /// users are only interested in the final agent responses.
    pub hide_agent_reasoning: bool,

    /// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
    /// Defaults to `false`.
    pub show_raw_agent_reasoning: bool,

    /// User-provided instructions from AGENTS.md.
    pub user_instructions: Option<String>,

    /// Path to the global AGENTS file loaded into `user_instructions`.
    pub user_instructions_path: Option<AbsolutePathBuf>,

    /// Base instructions override.
    pub base_instructions: Option<String>,

    /// Developer instructions override injected as a separate message.
    pub developer_instructions: Option<String>,

    /// Guardian-specific tenant policy config override from requirements.toml.
    /// This is inserted into the fixed guardian prompt template under the
    /// `# Policy Configuration` section rather than replacing the whole
    /// guardian developer prompt.
    pub guardian_policy_config: Option<String>,

    /// Whether to inject the `<permissions instructions>` developer block.
    pub include_permissions_instructions: bool,

    /// Whether to inject the `<apps_instructions>` developer block.
    pub include_apps_instructions: bool,

    /// Whether to inject the `<environment_context>` user block.
    pub include_environment_context: bool,

    /// Compact prompt override.
    pub compact_prompt: Option<String>,

    /// Optional commit attribution text for commit message co-author trailers.
    ///
    /// - `None`: use default attribution (`Codex <noreply@openai.com>`)
    /// - `Some("")` or whitespace-only: disable commit attribution
    /// - `Some("...")`: use the provided attribution text verbatim
    pub commit_attribution: Option<String>,

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

    /// TUI notification settings, including enabled events, delivery method, and focus condition.
    pub tui_notifications: TuiNotificationSettings,

    /// Enable ASCII animations and shimmer effects in the TUI.
    pub animations: bool,

    /// Show startup tooltips in the TUI welcome screen.
    pub show_tooltips: bool,

    /// Persisted startup availability NUX state for model tooltips.
    pub model_availability_nux: ModelAvailabilityNuxConfig,

    /// Start the TUI in the specified collaboration mode (plan/default).

    /// Controls whether the TUI uses the terminal's alternate screen buffer.
    ///
    /// This is the same `tui.alternate_screen` value from `config.toml`.
    /// - `auto` (default): Disable alternate screen in Zellij, enable elsewhere.
    /// - `always`: Always use alternate screen (original behavior).
    /// - `never`: Never use alternate screen (inline mode, preserves scrollback).
    pub tui_alternate_screen: AltScreenMode,

    /// Ordered list of status line item identifiers for the TUI.
    ///
    /// When unset, the TUI defaults to: `model-with-reasoning` and `current-dir`.
    pub tui_status_line: Option<Vec<String>>,

    /// Ordered list of terminal title item identifiers for the TUI.
    ///
    /// When unset, the TUI defaults to: `project` and `spinner`.
    pub tui_terminal_title: Option<Vec<String>>,

    /// Syntax highlighting theme override (kebab-case name).
    pub tui_theme: Option<String>,

    /// The absolute directory that should be treated as the current working
    /// directory for the session. All relative paths inside the business-logic
    /// layer are resolved against this path.
    pub cwd: AbsolutePathBuf,

    /// Preferred store for CLI auth credentials.
    /// file (default): Use a file in the Codex home directory.
    /// keyring: Use an OS-specific keyring service.
    /// auto: Use the OS-specific keyring service if available, otherwise use a file.
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    pub mcp_servers: Constrained<HashMap<String, McpServerConfig>>,

    /// Preferred store for MCP OAuth credentials.
    /// keyring: Use an OS-specific keyring service.
    ///          Credentials stored in the keyring will only be readable by Codex unless the user explicitly grants access via OS-level keyring access.
    ///          https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs#L2
    /// file: CODEX_HOME/.credentials.json
    ///       This file will be readable to Codex and other applications running as the same user.
    /// auto (default): keyring if available, otherwise file.
    pub mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode,

    /// Optional fixed port to use for the local HTTP callback server used during MCP OAuth login.
    ///
    /// When unset, Codex will bind to an ephemeral port chosen by the OS.
    pub mcp_oauth_callback_port: Option<u16>,

    /// Optional redirect URI to use during MCP OAuth login.
    ///
    /// When set, this URI is used in the OAuth authorization request instead
    /// of the local listener address. The local callback listener still binds
    /// to 127.0.0.1 (using `mcp_oauth_callback_port` when provided).
    pub mcp_oauth_callback_url: Option<String>,

    /// Combined provider map (defaults plus user-defined providers).
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: usize,

    /// Additional filenames to try when looking for project-level docs.
    pub project_doc_fallback_filenames: Vec<String>,

    /// Token budget applied when storing tool/function outputs in the context manager.
    pub tool_output_token_limit: Option<usize>,

    /// Maximum number of agent threads that can be open concurrently.
    pub agent_max_threads: Option<usize>,
    /// Maximum runtime in seconds for agent job workers before they are failed.
    pub agent_job_max_runtime_seconds: Option<u64>,

    /// Maximum nesting depth allowed for spawned agent threads.
    pub agent_max_depth: i32,

    /// User-defined role declarations keyed by role name.
    pub agent_roles: BTreeMap<String, AgentRoleConfig>,

    /// Memories subsystem settings.
    pub memories: MemoriesConfig,

    /// Directory containing all Codex state (defaults to `~/.codex` but can be
    /// overridden by the `CODEX_HOME` environment variable).
    pub codex_home: AbsolutePathBuf,

    /// Directory where Codex stores the SQLite state DB.
    pub sqlite_home: PathBuf,

    /// Directory where Codex writes log files (defaults to `$CODEX_HOME/log`).
    pub log_dir: PathBuf,

    /// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
    pub history: History,

    /// When true, session is not persisted on disk. Default to `false`
    pub ephemeral: bool,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: UriBasedFileOpener,

    /// Path to the current Codex executable. This cannot be set in the config
    /// file: it must be set in code via [`ConfigOverrides`].
    pub codex_self_exe: Option<PathBuf>,

    /// Path to the `codex-linux-sandbox` executable. This must be set if
    /// [`codex_sandboxing::SandboxType::LinuxSeccomp`] is used. Note that this
    /// cannot be set in the config file: it must be set in code via
    /// [`ConfigOverrides`].
    ///
    /// When this program is invoked, arg0 will be set to `codex-linux-sandbox`.
    pub codex_linux_sandbox_exe: Option<PathBuf>,

    /// Path to the `codex-execve-wrapper` executable used for shell
    /// escalation. This cannot be set in the config file: it must be set in
    /// code via [`ConfigOverrides`].
    pub main_execve_wrapper_exe: Option<PathBuf>,

    /// Optional absolute path to the Node runtime used by `js_repl`.
    pub js_repl_node_path: Option<PathBuf>,

    /// Ordered list of directories to search for Node modules in `js_repl`.
    pub js_repl_node_module_dirs: Vec<PathBuf>,

    /// Optional absolute path to patched zsh used by zsh-exec-bridge-backed shell execution.
    pub zsh_path: Option<PathBuf>,

    /// Value to use for `reasoning.effort` when making a request using the
    /// Responses API.
    pub model_reasoning_effort: Option<ReasoningEffort>,
    /// Optional Plan-mode-specific reasoning effort override used by the TUI.
    ///
    /// When unset, Plan mode uses the built-in Plan preset default (currently
    /// `medium`). When explicitly set (including `none`), this overrides the
    /// Plan preset. The `none` value means "no reasoning" (not "inherit the
    /// global default").
    pub plan_mode_reasoning_effort: Option<ReasoningEffort>,

    /// Optional value to use for `reasoning.summary` when making a request
    /// using the Responses API. When unset, the model catalog default is used.
    pub model_reasoning_summary: Option<ReasoningSummary>,

    /// Optional override to force-enable reasoning summaries for the configured model.
    pub model_supports_reasoning_summaries: Option<bool>,

    /// Optional full model catalog loaded from `model_catalog_json`.
    /// When set, this replaces the bundled catalog for the current process.
    pub model_catalog: Option<ModelsResponse>,

    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: String,

    /// Machine-local realtime audio device preferences used by realtime voice.
    pub realtime_audio: RealtimeAudioConfig,

    /// Experimental / do not use. Overrides only the realtime conversation
    /// websocket transport base URL (the `Op::RealtimeConversation`
    /// `/v1/realtime`
    /// connection) without changing normal provider HTTP requests.
    pub experimental_realtime_ws_base_url: Option<String>,
    /// Experimental / do not use. Selects the realtime websocket model/snapshot
    /// used for the `Op::RealtimeConversation` connection.
    pub experimental_realtime_ws_model: Option<String>,
    /// Experimental / do not use. Realtime websocket session selection.
    /// `version` controls v1/v2 and `type` controls conversational/transcription.
    pub realtime: RealtimeConfig,
    /// Experimental / do not use. Overrides only the realtime conversation
    /// websocket transport instructions (the `Op::RealtimeConversation`
    /// `/ws` session.update instructions) without changing normal prompts.
    pub experimental_realtime_ws_backend_prompt: Option<String>,
    /// Experimental / do not use. Replaces the synthesized realtime startup
    /// context appended to websocket session instructions. An empty string
    /// disables startup context injection entirely.
    pub experimental_realtime_ws_startup_context: Option<String>,
    /// Experimental / do not use. Replaces the built-in realtime start
    /// instructions inserted into developer messages when realtime becomes
    /// active.
    pub experimental_realtime_start_instructions: Option<String>,
    /// When set, restricts ChatGPT login to a specific workspace identifier.
    pub forced_chatgpt_workspace_id: Option<String>,

    /// When set, restricts the login mechanism users may use.
    pub forced_login_method: Option<ForcedLoginMethod>,

    /// Include the `apply_patch` tool for models that benefit from invoking
    /// file edits as a structured tool call. When unset, this falls back to the
    /// model info's default preference.
    pub include_apply_patch_tool: bool,

    /// Explicit or feature-derived web search mode.
    pub web_search_mode: Constrained<WebSearchMode>,

    /// Additional parameters for the web search tool when it is enabled.
    pub web_search_config: Option<WebSearchConfig>,

    /// If set to `true`, used only the experimental unified exec tool.
    pub use_experimental_unified_exec_tool: bool,

    /// Maximum poll window for background terminal output (`write_stdin`), in milliseconds.
    /// Default: `300000` (5 minutes).
    pub background_terminal_max_timeout: u64,

    /// Settings for ghost snapshots (used for undo).
    pub ghost_snapshot: GhostSnapshotConfig,

    /// Settings specific to the task-path-based multi-agent tool surface.
    pub multi_agent_v2: MultiAgentV2Config,

    /// Centralized feature flags; source of truth for feature gating.
    pub features: ManagedFeatures,

    /// When `true`, suppress warnings about unstable (under development) features.
    pub suppress_unstable_features_warning: bool,

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

    /// When `false`, disables analytics across Codex product surfaces in this machine.
    /// Voluntarily left as Optional because the default value might depend on the client.
    pub analytics_enabled: Option<bool>,

    /// When `false`, disables feedback collection across Codex product surfaces.
    /// Defaults to `true`.
    pub feedback_enabled: bool,

    /// Configured discoverable tools for tool suggestions.
    pub tool_suggest: ToolSuggestConfig,

    /// OTEL configuration (exporter type, endpoint, headers, etc.).
    pub otel: codex_config::types::OtelConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiAgentV2Config {
    pub usage_hint_enabled: bool,
    pub usage_hint_text: Option<String>,
    pub hide_spawn_agent_metadata: bool,
}

impl Default for MultiAgentV2Config {
    fn default() -> Self {
        Self {
            usage_hint_enabled: true,
            usage_hint_text: None,
            hide_spawn_agent_metadata: false,
        }
    }
}

impl AuthManagerConfig for Config {
    fn codex_home(&self) -> PathBuf {
        self.codex_home.to_path_buf()
    }

    fn cli_auth_credentials_store_mode(&self) -> AuthCredentialsStoreMode {
        self.cli_auth_credentials_store_mode
    }

    fn forced_chatgpt_workspace_id(&self) -> Option<String> {
        self.forced_chatgpt_workspace_id.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigBuilder {
    codex_home: Option<PathBuf>,
    cli_overrides: Option<Vec<(String, TomlValue)>>,
    harness_overrides: Option<ConfigOverrides>,
    loader_overrides: Option<LoaderOverrides>,
    cloud_requirements: CloudRequirementsLoader,
    fallback_cwd: Option<PathBuf>,
}

impl ConfigBuilder {
    pub fn codex_home(mut self, codex_home: PathBuf) -> Self {
        self.codex_home = Some(codex_home);
        self
    }

    pub fn cli_overrides(mut self, cli_overrides: Vec<(String, TomlValue)>) -> Self {
        self.cli_overrides = Some(cli_overrides);
        self
    }

    pub fn harness_overrides(mut self, harness_overrides: ConfigOverrides) -> Self {
        self.harness_overrides = Some(harness_overrides);
        self
    }

    pub fn loader_overrides(mut self, loader_overrides: LoaderOverrides) -> Self {
        self.loader_overrides = Some(loader_overrides);
        self
    }

    pub fn cloud_requirements(mut self, cloud_requirements: CloudRequirementsLoader) -> Self {
        self.cloud_requirements = cloud_requirements;
        self
    }

    pub fn fallback_cwd(mut self, fallback_cwd: Option<PathBuf>) -> Self {
        self.fallback_cwd = fallback_cwd;
        self
    }

    pub async fn build(self) -> std::io::Result<Config> {
        let Self {
            codex_home,
            cli_overrides,
            harness_overrides,
            loader_overrides,
            cloud_requirements,
            fallback_cwd,
        } = self;
        let codex_home = match codex_home {
            Some(codex_home) => AbsolutePathBuf::from_absolute_path(codex_home)?,
            None => find_codex_home()?,
        };
        let cli_overrides = cli_overrides.unwrap_or_default();
        let mut harness_overrides = harness_overrides.unwrap_or_default();
        let loader_overrides = loader_overrides.unwrap_or_default();
        let cwd_override = harness_overrides.cwd.as_deref().or(fallback_cwd.as_deref());
        let cwd = match cwd_override {
            Some(path) => AbsolutePathBuf::relative_to_current_dir(path)?,
            None => AbsolutePathBuf::current_dir()?,
        };
        harness_overrides.cwd = Some(cwd.to_path_buf());
        let config_layer_stack = load_config_layers_state(
            &codex_home,
            Some(cwd),
            &cli_overrides,
            loader_overrides,
            cloud_requirements,
        )
        .await?;
        let merged_toml = config_layer_stack.effective_config();

        // Note that each layer in ConfigLayerStack should have resolved
        // relative paths to absolute paths based on the parent folder of the
        // respective config file, so we should be safe to deserialize without
        // AbsolutePathBufGuard here.
        let config_toml: ConfigToml = match merged_toml.try_into() {
            Ok(config_toml) => config_toml,
            Err(err) => {
                if let Some(config_error) =
                    crate::config_loader::first_layer_config_error(&config_layer_stack).await
                {
                    return Err(crate::config_loader::io_error_from_config_error(
                        std::io::ErrorKind::InvalidData,
                        config_error,
                        Some(err),
                    ));
                }
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, err));
            }
        };
        Config::load_config_with_layer_stack(
            config_toml,
            harness_overrides,
            codex_home,
            config_layer_stack,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) fn without_managed_config_for_tests() -> Self {
        Self::default().loader_overrides(LoaderOverrides::without_managed_config_for_tests())
    }
}

impl Config {
    pub fn to_models_manager_config(&self) -> ModelsManagerConfig {
        ModelsManagerConfig {
            model_context_window: self.model_context_window,
            model_auto_compact_token_limit: self.model_auto_compact_token_limit,
            tool_output_token_limit: self.tool_output_token_limit,
            base_instructions: self.base_instructions.clone(),
            personality_enabled: self.features.enabled(Feature::Personality),
            model_supports_reasoning_summaries: self.model_supports_reasoning_summaries,
            model_catalog: self.model_catalog.clone(),
        }
    }

    pub async fn to_mcp_config(
        &self,
        plugins_manager: &crate::plugins::PluginsManager,
    ) -> McpConfig {
        let loaded_plugins = plugins_manager.plugins_for_config(self).await;
        let mut configured_mcp_servers = self.mcp_servers.get().clone();
        for (name, plugin_server) in loaded_plugins.effective_mcp_servers() {
            configured_mcp_servers.entry(name).or_insert(plugin_server);
        }

        McpConfig {
            chatgpt_base_url: self.chatgpt_base_url.clone(),
            codex_home: self.codex_home.to_path_buf(),
            mcp_oauth_credentials_store_mode: self.mcp_oauth_credentials_store_mode,
            mcp_oauth_callback_port: self.mcp_oauth_callback_port,
            mcp_oauth_callback_url: self.mcp_oauth_callback_url.clone(),
            skill_mcp_dependency_install_enabled: self
                .features
                .enabled(Feature::SkillMcpDependencyInstall),
            approval_policy: self.permissions.approval_policy.clone(),
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            use_legacy_landlock: self.features.use_legacy_landlock(),
            apps_enabled: self.features.enabled(Feature::Apps),
            configured_mcp_servers,
            plugin_capability_summaries: loaded_plugins.capability_summaries().to_vec(),
        }
    }

    /// This is the preferred way to create an instance of [Config].
    pub async fn load_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> std::io::Result<Self> {
        ConfigBuilder::default()
            .cli_overrides(cli_overrides)
            .build()
            .await
    }

    /// Load a default configuration when user config files are invalid.
    pub async fn load_default_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> std::io::Result<Self> {
        let codex_home = find_codex_home()?;
        Self::load_default_with_cli_overrides_for_codex_home(
            codex_home.to_path_buf(),
            cli_overrides,
        )
        .await
    }

    /// Load a default configuration for a specific Codex home without reading
    /// user, project, or system config layers.
    pub async fn load_default_with_cli_overrides_for_codex_home(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> std::io::Result<Self> {
        let mut merged = toml::Value::try_from(ConfigToml::default()).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to serialize default config: {e}"),
            )
        })?;
        let cli_layer = crate::config_loader::build_cli_overrides_layer(&cli_overrides);
        crate::config_loader::merge_toml_values(&mut merged, &cli_layer);
        let codex_home = AbsolutePathBuf::from_absolute_path_checked(codex_home)?;
        let config_toml = deserialize_config_toml_with_base(merged, &codex_home)?;
        Self::load_config_with_layer_stack(
            config_toml,
            ConfigOverrides::default(),
            codex_home,
            ConfigLayerStack::default(),
        )
        .await
    }

    /// This is a secondary way of creating [Config], which is appropriate when
    /// the harness is meant to be used with a specific configuration that
    /// ignores user settings. For example, the `codex exec` subcommand is
    /// designed to use [AskForApproval::Never] exclusively.
    ///
    /// Further, [ConfigOverrides] contains some options that are not supported
    /// in [ConfigToml], such as `cwd`, `codex_self_exe`, `codex_linux_sandbox_exe`, and
    /// `main_execve_wrapper_exe`.
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

/// DEPRECATED: Use [Config::load_with_cli_overrides()] instead because working
/// with [ConfigToml] directly means that [ConfigRequirements] have not been
/// applied yet, which risks failing to enforce required constraints.
pub async fn load_config_as_toml_with_cli_overrides(
    codex_home: &Path,
    cwd: Option<&AbsolutePathBuf>,
    cli_overrides: Vec<(String, TomlValue)>,
) -> std::io::Result<ConfigToml> {
    let config_layer_stack = load_config_layers_state(
        codex_home,
        cwd.cloned(),
        &cli_overrides,
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let merged_toml = config_layer_stack.effective_config();
    let cfg = deserialize_config_toml_with_base(merged_toml, codex_home).map_err(|e| {
        tracing::error!("Failed to deserialize overridden config: {e}");
        e
    })?;

    Ok(cfg)
}

pub(crate) fn deserialize_config_toml_with_base(
    root_value: TomlValue,
    config_base_dir: &Path,
) -> std::io::Result<ConfigToml> {
    // This guard ensures that any relative paths that is deserialized into an
    // [AbsolutePathBuf] is resolved against `config_base_dir`.
    let _guard = AbsolutePathBufGuard::new(config_base_dir);
    root_value
        .try_into()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn load_catalog_json(path: &AbsolutePathBuf) -> std::io::Result<ModelsResponse> {
    let file_contents = std::fs::read_to_string(path)?;
    let catalog = serde_json::from_str::<ModelsResponse>(&file_contents).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "failed to parse model_catalog_json path `{}` as JSON: {err}",
                path.display()
            ),
        )
    })?;
    if catalog.models.is_empty() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "model_catalog_json path `{}` must contain at least one model",
                path.display()
            ),
        ));
    }
    Ok(catalog)
}

fn load_model_catalog(
    model_catalog_json: Option<AbsolutePathBuf>,
) -> std::io::Result<Option<ModelsResponse>> {
    model_catalog_json
        .map(|path| load_catalog_json(&path))
        .transpose()
}

fn filter_mcp_servers_by_requirements(
    mcp_servers: &mut HashMap<String, McpServerConfig>,
    mcp_requirements: Option<&Sourced<BTreeMap<String, McpServerRequirement>>>,
) {
    let Some(allowlist) = mcp_requirements else {
        return;
    };

    let source = allowlist.source.clone();
    for (name, server) in mcp_servers.iter_mut() {
        let allowed = allowlist
            .value
            .get(name)
            .is_some_and(|requirement| mcp_server_matches_requirement(requirement, server));
        if allowed {
            server.disabled_reason = None;
        } else {
            server.enabled = false;
            server.disabled_reason = Some(McpServerDisabledReason::Requirements {
                source: source.clone(),
            });
        }
    }
}

fn constrain_mcp_servers(
    mcp_servers: HashMap<String, McpServerConfig>,
    mcp_requirements: Option<&Sourced<BTreeMap<String, McpServerRequirement>>>,
) -> ConstraintResult<Constrained<HashMap<String, McpServerConfig>>> {
    if mcp_requirements.is_none() {
        return Ok(Constrained::allow_any(mcp_servers));
    }

    let mcp_requirements = mcp_requirements.cloned();
    Constrained::normalized(mcp_servers, move |mut servers| {
        filter_mcp_servers_by_requirements(&mut servers, mcp_requirements.as_ref());
        servers
    })
}

fn apply_requirement_constrained_value<T>(
    field_name: &'static str,
    configured_value: T,
    constrained_value: &mut ConstrainedWithSource<T>,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<()>
where
    T: Clone + std::fmt::Debug + Send + Sync,
{
    if let Err(err) = constrained_value.set(configured_value) {
        let fallback_value = constrained_value.get().clone();
        tracing::warn!(
            error = %err,
            ?fallback_value,
            requirement_source = ?constrained_value.source,
            "configured value is disallowed by requirements; falling back to required value for {field_name}"
        );
        let message = format!(
            "Configured value for `{field_name}` is disallowed by requirements; falling back to required value {fallback_value:?}. Details: {err}"
        );
        startup_warnings.push(message);

        constrained_value.set(fallback_value).map_err(|fallback_err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "configured value for `{field_name}` is disallowed by requirements ({err}); fallback to a requirement-compliant value also failed ({fallback_err})"
                ),
            )
        })?;
    }

    Ok(())
}

fn mcp_server_matches_requirement(
    requirement: &McpServerRequirement,
    server: &McpServerConfig,
) -> bool {
    match &requirement.identity {
        McpServerIdentity::Command {
            command: want_command,
        } => matches!(
            &server.transport,
            McpServerTransportConfig::Stdio { command: got_command, .. }
                if got_command == want_command
        ),
        McpServerIdentity::Url { url: want_url } => matches!(
            &server.transport,
            McpServerTransportConfig::StreamableHttp { url: got_url, .. }
                if got_url == want_url
        ),
    }
}

pub async fn load_global_mcp_servers(
    codex_home: &Path,
) -> std::io::Result<BTreeMap<String, McpServerConfig>> {
    // In general, Config::load_with_cli_overrides() should be used to load the
    // full config with requirements.toml applied, but in this case, we need
    // access to the raw TOML in order to warn the user about deprecated fields.
    //
    // Note that a more precise way to do this would be to audit the individual
    // config layers for deprecated fields rather than reporting on the merged
    // result.
    let cli_overrides = Vec::<(String, TomlValue)>::new();
    // There is no cwd/project context for this query, so this will not include
    // MCP servers defined in in-repo .codex/ folders.
    let cwd: Option<AbsolutePathBuf> = None;
    let config_layer_stack = load_config_layers_state(
        codex_home,
        cwd,
        &cli_overrides,
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;
    let merged_toml = config_layer_stack.effective_config();
    let Some(servers_value) = merged_toml.get("mcp_servers") else {
        return Ok(BTreeMap::new());
    };

    ensure_no_inline_bearer_tokens(servers_value)?;

    servers_value
        .clone()
        .try_into()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// We briefly allowed plain text bearer_token fields in MCP server configs.
/// We want to warn people who recently added these fields but can remove this after a few months.
fn ensure_no_inline_bearer_tokens(value: &TomlValue) -> std::io::Result<()> {
    let Some(servers_table) = value.as_table() else {
        return Ok(());
    };

    for (server_name, server_value) in servers_table {
        if let Some(server_table) = server_value.as_table()
            && server_table.contains_key("bearer_token")
        {
            let message = format!(
                "mcp_servers.{server_name} uses unsupported `bearer_token`; set `bearer_token_env_var`."
            );
            return Err(std::io::Error::new(ErrorKind::InvalidData, message));
        }
    }

    Ok(())
}

pub(crate) fn set_project_trust_level_inner(
    doc: &mut DocumentMut,
    project_path: &Path,
    trust_level: TrustLevel,
) -> anyhow::Result<()> {
    // Ensure we render a human-friendly structure:
    //
    // [projects]
    // [projects."/path/to/project"]
    // trust_level = "trusted" or "untrusted"
    //
    // rather than inline tables like:
    //
    // [projects]
    // "/path/to/project" = { trust_level = "trusted" }
    let project_key = project_trust_key(project_path);

    // Ensure top-level `projects` exists as a non-inline, explicit table. If it
    // exists but was previously represented as a non-table (e.g., inline),
    // replace it with an explicit table.
    {
        let root = doc.as_table_mut();
        // If `projects` exists but isn't a standard table (e.g., it's an inline table),
        // convert it to an explicit table while preserving existing entries.
        let existing_projects = root.get("projects").cloned();
        if existing_projects.as_ref().is_none_or(|i| !i.is_table()) {
            let mut projects_tbl = toml_edit::Table::new();
            projects_tbl.set_implicit(true);

            // If there was an existing inline table, migrate its entries to explicit tables.
            if let Some(inline_tbl) = existing_projects.as_ref().and_then(|i| i.as_inline_table()) {
                for (k, v) in inline_tbl.iter() {
                    if let Some(inner_tbl) = v.as_inline_table() {
                        let new_tbl = inner_tbl.clone().into_table();
                        projects_tbl.insert(k, toml_edit::Item::Table(new_tbl));
                    }
                }
            }

            root.insert("projects", toml_edit::Item::Table(projects_tbl));
        }
    }
    let Some(projects_tbl) = doc["projects"].as_table_mut() else {
        return Err(anyhow::anyhow!(
            "projects table missing after initialization"
        ));
    };

    // Ensure the per-project entry is its own explicit table. If it exists but
    // is not a table (e.g., an inline table), replace it with an explicit table.
    let needs_proj_table = !projects_tbl.contains_key(project_key.as_str())
        || projects_tbl
            .get(project_key.as_str())
            .and_then(|i| i.as_table())
            .is_none();
    if needs_proj_table {
        projects_tbl.insert(project_key.as_str(), toml_edit::table());
    }
    let Some(proj_tbl) = projects_tbl
        .get_mut(project_key.as_str())
        .and_then(|i| i.as_table_mut())
    else {
        return Err(anyhow::anyhow!("project table missing for {project_key}"));
    };
    proj_tbl.set_implicit(false);
    proj_tbl["trust_level"] = toml_edit::value(trust_level.to_string());
    Ok(())
}

/// Patch `CODEX_HOME/config.toml` project state to set trust level.
/// Use with caution.
pub fn set_project_trust_level(
    codex_home: &Path,
    project_path: &Path,
    trust_level: TrustLevel,
) -> anyhow::Result<()> {
    use crate::config::edit::ConfigEditsBuilder;

    ConfigEditsBuilder::new(codex_home)
        .set_project_trust_level(project_path, trust_level)
        .apply_blocking()
}

/// Save the default OSS provider preference to config.toml
pub fn set_default_oss_provider(codex_home: &Path, provider: &str) -> std::io::Result<()> {
    codex_config::config_toml::validate_oss_provider(provider)?;
    use toml_edit::value;

    let edits = [ConfigEdit::SetPath {
        segments: vec!["oss_provider".to_string()],
        value: value(provider),
    }];

    ConfigEditsBuilder::new(codex_home)
        .with_edits(edits)
        .apply_blocking()
        .map_err(|err| std::io::Error::other(format!("failed to persist config.toml: {err}")))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentRoleConfig {
    /// Human-facing role documentation used in spawn tool guidance.
    /// Required for loaded user-defined roles after deprecated/new metadata precedence resolves.
    pub description: Option<String>,
    /// Path to a role-specific config layer.
    pub config_file: Option<PathBuf>,
    /// Candidate nicknames for agents spawned with this role.
    pub nickname_candidates: Option<Vec<String>>,
}

fn resolve_tool_suggest_config(config_toml: &ConfigToml) -> ToolSuggestConfig {
    let discoverables = config_toml
        .tool_suggest
        .as_ref()
        .into_iter()
        .flat_map(|tool_suggest| tool_suggest.discoverables.iter())
        .filter_map(|discoverable| {
            let trimmed = discoverable.id.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(ToolSuggestDiscoverable {
                    kind: discoverable.kind,
                    id: trimmed.to_string(),
                })
            }
        })
        .collect();

    ToolSuggestConfig { discoverables }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionConfigSyntax {
    Legacy,
    Profiles,
}

#[derive(Debug, Deserialize, Default)]
struct PermissionSelectionToml {
    default_permissions: Option<String>,
    sandbox_mode: Option<SandboxMode>,
}

fn resolve_permission_config_syntax(
    config_layer_stack: &ConfigLayerStack,
    cfg: &ConfigToml,
    sandbox_mode_override: Option<SandboxMode>,
    profile_sandbox_mode: Option<SandboxMode>,
) -> Option<PermissionConfigSyntax> {
    if sandbox_mode_override.is_some() || profile_sandbox_mode.is_some() {
        return Some(PermissionConfigSyntax::Legacy);
    }

    let mut selection = None;
    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        let Ok(layer_selection) = layer.config.clone().try_into::<PermissionSelectionToml>() else {
            continue;
        };

        if layer_selection.sandbox_mode.is_some() {
            selection = Some(PermissionConfigSyntax::Legacy);
        }
        if layer_selection.default_permissions.is_some() {
            selection = Some(PermissionConfigSyntax::Profiles);
        }
    }

    selection.or_else(|| {
        if cfg.default_permissions.is_some() {
            Some(PermissionConfigSyntax::Profiles)
        } else if cfg.sandbox_mode.is_some() {
            Some(PermissionConfigSyntax::Legacy)
        } else {
            None
        }
    })
}

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_provider: Option<String>,
    pub service_tier: Option<Option<ServiceTier>>,
    pub config_profile: Option<String>,
    pub codex_self_exe: Option<PathBuf>,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub main_execve_wrapper_exe: Option<PathBuf>,
    pub js_repl_node_path: Option<PathBuf>,
    pub js_repl_node_module_dirs: Option<Vec<PathBuf>>,
    pub zsh_path: Option<PathBuf>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub personality: Option<Personality>,
    pub compact_prompt: Option<String>,
    pub include_apply_patch_tool: Option<bool>,
    pub show_raw_agent_reasoning: Option<bool>,
    pub tools_web_search_request: Option<bool>,
    pub ephemeral: Option<bool>,
    /// Additional directories that should be treated as writable roots for this session.
    pub additional_writable_roots: Vec<PathBuf>,
}

/// Resolves the OSS provider from CLI override, profile config, or global config.
/// Returns `None` if no provider is configured at any level.
pub fn resolve_oss_provider(
    explicit_provider: Option<&str>,
    config_toml: &ConfigToml,
    config_profile: Option<String>,
) -> Option<String> {
    if let Some(provider) = explicit_provider {
        // Explicit provider specified (e.g., via --local-provider)
        Some(provider.to_string())
    } else {
        // Check profile config first, then global config
        let profile = config_toml.get_config_profile(config_profile).ok();
        if let Some(profile) = &profile {
            // Check if profile has an oss provider
            if let Some(profile_oss_provider) = &profile.oss_provider {
                Some(profile_oss_provider.clone())
            }
            // If not then check if the toml has an oss provider
            else {
                config_toml.oss_provider.clone()
            }
        } else {
            config_toml.oss_provider.clone()
        }
    }
}

/// Resolve the web search mode from explicit config and feature flags.
fn resolve_web_search_mode(
    config_toml: &ConfigToml,
    config_profile: &ConfigProfile,
    features: &Features,
) -> Option<WebSearchMode> {
    if let Some(mode) = config_profile.web_search.or(config_toml.web_search) {
        return Some(mode);
    }
    if features.enabled(Feature::WebSearchCached) {
        return Some(WebSearchMode::Cached);
    }
    if features.enabled(Feature::WebSearchRequest) {
        return Some(WebSearchMode::Live);
    }
    None
}

fn resolve_web_search_config(
    config_toml: &ConfigToml,
    config_profile: &ConfigProfile,
) -> Option<WebSearchConfig> {
    let base = config_toml
        .tools
        .as_ref()
        .and_then(|tools| tools.web_search.as_ref());
    let profile = config_profile
        .tools
        .as_ref()
        .and_then(|tools| tools.web_search.as_ref());

    match (base, profile) {
        (None, None) => None,
        (Some(base), None) => Some(base.clone().into()),
        (None, Some(profile)) => Some(profile.clone().into()),
        (Some(base), Some(profile)) => Some(base.merge(profile).into()),
    }
}

fn resolve_multi_agent_v2_config(
    config_toml: &ConfigToml,
    config_profile: &ConfigProfile,
) -> MultiAgentV2Config {
    let base = multi_agent_v2_toml_config(config_toml.features.as_ref());
    let profile = multi_agent_v2_toml_config(config_profile.features.as_ref());
    let default = MultiAgentV2Config::default();

    let usage_hint_enabled = profile
        .and_then(|config| config.usage_hint_enabled)
        .or_else(|| base.and_then(|config| config.usage_hint_enabled))
        .unwrap_or(default.usage_hint_enabled);
    let usage_hint_text = profile
        .and_then(|config| config.usage_hint_text.as_ref())
        .or_else(|| base.and_then(|config| config.usage_hint_text.as_ref()))
        .cloned()
        .or(default.usage_hint_text);
    let hide_spawn_agent_metadata = profile
        .and_then(|config| config.hide_spawn_agent_metadata)
        .or_else(|| base.and_then(|config| config.hide_spawn_agent_metadata))
        .unwrap_or(default.hide_spawn_agent_metadata);

    MultiAgentV2Config {
        usage_hint_enabled,
        usage_hint_text,
        hide_spawn_agent_metadata,
    }
}

fn multi_agent_v2_toml_config(features: Option<&FeaturesToml>) -> Option<&MultiAgentV2ConfigToml> {
    match features?.multi_agent_v2.as_ref()? {
        FeatureToml::Enabled(_) => None,
        FeatureToml::Config(config) => Some(config),
    }
}

pub(crate) fn resolve_web_search_mode_for_turn(
    web_search_mode: &Constrained<WebSearchMode>,
    sandbox_policy: &SandboxPolicy,
) -> WebSearchMode {
    let preferred = web_search_mode.value();

    if matches!(sandbox_policy, SandboxPolicy::DangerFullAccess)
        && preferred != WebSearchMode::Disabled
    {
        for mode in [
            WebSearchMode::Live,
            WebSearchMode::Cached,
            WebSearchMode::Disabled,
        ] {
            if web_search_mode.can_set(&mode).is_ok() {
                return mode;
            }
        }
    } else {
        if web_search_mode.can_set(&preferred).is_ok() {
            return preferred;
        }
        for mode in [
            WebSearchMode::Cached,
            WebSearchMode::Live,
            WebSearchMode::Disabled,
        ] {
            if web_search_mode.can_set(&mode).is_ok() {
                return mode;
            }
        }
    }

    WebSearchMode::Disabled
}

impl Config {
    #[cfg(test)]
    async fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: AbsolutePathBuf,
    ) -> std::io::Result<Self> {
        // Note this ignores requirements.toml enforcement for tests.
        let config_layer_stack = ConfigLayerStack::default();
        Self::load_config_with_layer_stack(cfg, overrides, codex_home, config_layer_stack).await
    }

    pub(crate) async fn load_config_with_layer_stack(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: AbsolutePathBuf,
        config_layer_stack: ConfigLayerStack,
    ) -> std::io::Result<Self> {
        // Keep the large config-construction future off small test thread stacks.
        Box::pin(async move {
        validate_model_providers(&cfg.model_providers)
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
        // Ensure that every field of ConfigRequirements is applied to the final
        // Config.
        let ConfigRequirements {
            approval_policy: mut constrained_approval_policy,
            approvals_reviewer: mut constrained_approvals_reviewer,
            sandbox_policy: mut constrained_sandbox_policy,
            web_search_mode: mut constrained_web_search_mode,
            feature_requirements,
            mcp_servers,
            exec_policy: _,
            enforce_residency,
            network: network_requirements,
        } = config_layer_stack.requirements().clone();

        let (user_instructions, user_instructions_path) =
            Self::load_instructions(Some(&codex_home))
                .map(|loaded| (Some(loaded.contents), Some(loaded.path)))
                .unwrap_or((None, None));
        let mut startup_warnings = Vec::new();

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            review_model: override_review_model,
            cwd,
            approval_policy: approval_policy_override,
            approvals_reviewer: approvals_reviewer_override,
            sandbox_mode,
            model_provider,
            service_tier: service_tier_override,
            config_profile: config_profile_key,
            codex_self_exe,
            codex_linux_sandbox_exe,
            main_execve_wrapper_exe,
            js_repl_node_path: js_repl_node_path_override,
            js_repl_node_module_dirs: js_repl_node_module_dirs_override,
            zsh_path: zsh_path_override,
            base_instructions,
            developer_instructions,
            personality,
            compact_prompt,
            include_apply_patch_tool: include_apply_patch_tool_override,
            show_raw_agent_reasoning,
            tools_web_search_request: override_tools_web_search_request,
            ephemeral,
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
        let tool_suggest = resolve_tool_suggest_config(&cfg);
        let feature_overrides = FeatureOverrides {
            include_apply_patch_tool: include_apply_patch_tool_override,
            web_search_request: override_tools_web_search_request,
        };

        let configured_features = Features::from_sources(
            FeatureConfigSource {
                features: cfg.features.as_ref(),
                include_apply_patch_tool: None,
                experimental_use_freeform_apply_patch: cfg.experimental_use_freeform_apply_patch,
                experimental_use_unified_exec_tool: cfg.experimental_use_unified_exec_tool,
            },
            FeatureConfigSource {
                features: config_profile.features.as_ref(),
                include_apply_patch_tool: config_profile.include_apply_patch_tool,
                experimental_use_freeform_apply_patch: config_profile
                    .experimental_use_freeform_apply_patch,
                experimental_use_unified_exec_tool: config_profile
                    .experimental_use_unified_exec_tool,
            },
            feature_overrides,
        );
        let features = ManagedFeatures::from_configured(configured_features, feature_requirements)?;
        let windows_sandbox_mode = resolve_windows_sandbox_mode(&cfg, &config_profile);
        let windows_sandbox_private_desktop =
            resolve_windows_sandbox_private_desktop(&cfg, &config_profile);
        let resolved_cwd = AbsolutePathBuf::try_from(normalize_for_native_workdir({
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
        }))?;
        let mut additional_writable_roots: Vec<AbsolutePathBuf> = additional_writable_roots
            .into_iter()
            .map(|path| AbsolutePathBuf::resolve_path_against_base(path, resolved_cwd.as_path()))
            .collect();
        let active_project = cfg
            .get_active_project(resolved_cwd.as_path())
            .await
            .unwrap_or(ProjectConfig { trust_level: None });
        let permission_config_syntax = resolve_permission_config_syntax(
            &config_layer_stack,
            &cfg,
            sandbox_mode,
            config_profile.sandbox_mode,
        );
        let has_permission_profiles = cfg
            .permissions
            .as_ref()
            .is_some_and(|profiles| !profiles.is_empty());
        if has_permission_profiles
            && !matches!(
                permission_config_syntax,
                Some(PermissionConfigSyntax::Legacy)
            )
            && cfg.default_permissions.is_none()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "config defines `[permissions]` profiles but does not set `default_permissions`",
            ));
        }

        let windows_sandbox_level = match windows_sandbox_mode {
            Some(WindowsSandboxModeToml::Elevated) => WindowsSandboxLevel::Elevated,
            Some(WindowsSandboxModeToml::Unelevated) => WindowsSandboxLevel::RestrictedToken,
            None => WindowsSandboxLevel::from_features(&features),
        };
        let memories_root = memory_root(&codex_home);
        std::fs::create_dir_all(&memories_root)?;
        if !additional_writable_roots
            .iter()
            .any(|existing| existing == &memories_root)
        {
            additional_writable_roots.push(memories_root);
        }

        let profiles_are_active = matches!(
            permission_config_syntax,
            Some(PermissionConfigSyntax::Profiles)
        ) || (permission_config_syntax.is_none()
            && has_permission_profiles);
        let (
            configured_network_proxy_config,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
        ) = if profiles_are_active {
            let permissions = cfg.permissions.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "default_permissions requires a `[permissions]` table",
                )
            })?;
            let default_permissions = cfg.default_permissions.as_deref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "default_permissions requires a named permissions profile",
                )
            })?;
            let profile = resolve_permission_profile(permissions, default_permissions)?;
            let configured_network_proxy_config =
                network_proxy_config_from_profile_network(profile.network.as_ref());
            let (mut file_system_sandbox_policy, network_sandbox_policy) =
                compile_permission_profile(
                    permissions,
                    default_permissions,
                    &mut startup_warnings,
                )?;
            let mut sandbox_policy = file_system_sandbox_policy
                .to_legacy_sandbox_policy(network_sandbox_policy, resolved_cwd.as_path())?;
            if matches!(sandbox_policy, SandboxPolicy::WorkspaceWrite { .. }) {
                file_system_sandbox_policy = file_system_sandbox_policy
                    .with_additional_writable_roots(
                        resolved_cwd.as_path(),
                        &additional_writable_roots,
                    );
                sandbox_policy = file_system_sandbox_policy
                    .to_legacy_sandbox_policy(network_sandbox_policy, resolved_cwd.as_path())?;
            }
            (
                configured_network_proxy_config,
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            )
        } else {
            let configured_network_proxy_config = NetworkProxyConfig::default();
            let mut sandbox_policy = cfg
                .derive_sandbox_policy(
                    sandbox_mode,
                    config_profile.sandbox_mode,
                    windows_sandbox_level,
                    resolved_cwd.as_path(),
                    Some(&constrained_sandbox_policy),
                )
                .await;
            if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = &mut sandbox_policy {
                for path in &additional_writable_roots {
                    if !writable_roots.iter().any(|existing| existing == path) {
                        writable_roots.push(path.clone());
                    }
                }
            }
            let file_system_sandbox_policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &sandbox_policy,
                resolved_cwd.as_path(),
            );
            let network_sandbox_policy = NetworkSandboxPolicy::from(&sandbox_policy);
            (
                configured_network_proxy_config,
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            )
        };
        let approval_policy_was_explicit = approval_policy_override.is_some()
            || config_profile.approval_policy.is_some()
            || cfg.approval_policy.is_some();
        let mut approval_policy = approval_policy_override
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
        if !approval_policy_was_explicit
            && let Err(err) = constrained_approval_policy.can_set(&approval_policy)
        {
            tracing::warn!(
                error = %err,
                "default approval policy is disallowed by requirements; falling back to required default"
            );
            approval_policy = constrained_approval_policy.value();
        }
        let approvals_reviewer_was_explicit = approvals_reviewer_override.is_some()
            || config_profile.approvals_reviewer.is_some()
            || cfg.approvals_reviewer.is_some();
        let mut approvals_reviewer = approvals_reviewer_override
            .or(config_profile.approvals_reviewer)
            .or(cfg.approvals_reviewer)
            .unwrap_or(ApprovalsReviewer::User);
        if !approvals_reviewer_was_explicit
            && let Err(err) = constrained_approvals_reviewer.can_set(&approvals_reviewer)
        {
            tracing::warn!(
                error = %err,
                "default approvals reviewer is disallowed by requirements; falling back to required default"
            );
            approvals_reviewer = constrained_approvals_reviewer.value();
        }
        let web_search_mode = resolve_web_search_mode(&cfg, &config_profile, &features)
            .unwrap_or(WebSearchMode::Cached);
        let web_search_config = resolve_web_search_config(&cfg, &config_profile);
        let multi_agent_v2 = resolve_multi_agent_v2_config(&cfg, &config_profile);

        let agent_roles =
            agent_roles::load_agent_roles(&cfg, &config_layer_stack, &mut startup_warnings)?;

        let openai_base_url = cfg
            .openai_base_url
            .clone()
            .filter(|value| !value.is_empty());

        let mut model_providers = built_in_model_providers(openai_base_url);
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
                let message = if model_provider_id == LEGACY_OLLAMA_CHAT_PROVIDER_ID {
                    OLLAMA_CHAT_PROVIDER_REMOVED_ERROR.to_string()
                } else {
                    format!("Model provider `{model_provider_id}` not found")
                };
                std::io::Error::new(std::io::ErrorKind::NotFound, message)
            })?
            .clone();

        let shell_environment_policy = cfg.shell_environment_policy.into();
        let allow_login_shell = cfg.allow_login_shell.unwrap_or(true);

        let history = cfg.history.unwrap_or_default();

        let agent_max_threads = cfg
            .agents
            .as_ref()
            .and_then(|agents| agents.max_threads)
            .or(DEFAULT_AGENT_MAX_THREADS);
        if agent_max_threads == Some(0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "agents.max_threads must be at least 1",
            ));
        }
        let agent_max_depth = cfg
            .agents
            .as_ref()
            .and_then(|agents| agents.max_depth)
            .unwrap_or(DEFAULT_AGENT_MAX_DEPTH);
        if agent_max_depth < 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "agents.max_depth must be at least 1",
            ));
        }
        let agent_job_max_runtime_seconds = cfg
            .agents
            .as_ref()
            .and_then(|agents| agents.job_max_runtime_seconds)
            .or(DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS);
        if agent_job_max_runtime_seconds == Some(0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "agents.job_max_runtime_seconds must be at least 1",
            ));
        }
        if let Some(max_runtime_seconds) = agent_job_max_runtime_seconds
            && max_runtime_seconds > i64::MAX as u64
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "agents.job_max_runtime_seconds must fit within a 64-bit signed integer",
            ));
        }
        let background_terminal_max_timeout = cfg
            .background_terminal_max_timeout
            .unwrap_or(DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS)
            .max(MIN_EMPTY_YIELD_TIME_MS);

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
        let service_tier = service_tier_override
            .unwrap_or_else(|| config_profile.service_tier.or(cfg.service_tier));
        let service_tier = match service_tier {
            Some(ServiceTier::Fast) if features.enabled(Feature::FastMode) => {
                Some(ServiceTier::Fast)
            }
            Some(ServiceTier::Flex) => Some(ServiceTier::Flex),
            _ => None,
        };

        let compact_prompt = compact_prompt.or(cfg.compact_prompt).and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let commit_attribution = cfg.commit_attribution;

        // Load base instructions override from a file if specified. If the
        // path is relative, resolve it against the effective cwd so the
        // behaviour matches other path-like config values.
        let model_instructions_path = config_profile
            .model_instructions_file
            .as_ref()
            .or(cfg.model_instructions_file.as_ref());
        let file_base_instructions =
            Self::try_read_non_empty_file(model_instructions_path, "model instructions file")?;
        let base_instructions = base_instructions.or(file_base_instructions);
        let developer_instructions = developer_instructions.or(cfg.developer_instructions);
        let include_permissions_instructions = config_profile
            .include_permissions_instructions
            .or(cfg.include_permissions_instructions)
            .unwrap_or(true);
        let include_apps_instructions = config_profile
            .include_apps_instructions
            .or(cfg.include_apps_instructions)
            .unwrap_or(true);
        let include_environment_context = config_profile
            .include_environment_context
            .or(cfg.include_environment_context)
            .unwrap_or(true);
        let guardian_policy_config =
            guardian_policy_config_from_requirements(config_layer_stack.requirements_toml());
        let personality = personality
            .or(config_profile.personality)
            .or(cfg.personality)
            .or_else(|| {
                features
                    .enabled(Feature::Personality)
                    .then_some(Personality::Pragmatic)
            });

        let experimental_compact_prompt_path = config_profile
            .experimental_compact_prompt_file
            .as_ref()
            .or(cfg.experimental_compact_prompt_file.as_ref());
        let file_compact_prompt = Self::try_read_non_empty_file(
            experimental_compact_prompt_path,
            "experimental compact prompt file",
        )?;
        let compact_prompt = compact_prompt.or(file_compact_prompt);
        let js_repl_node_path = js_repl_node_path_override
            .or(config_profile.js_repl_node_path.map(Into::into))
            .or(cfg.js_repl_node_path.map(Into::into));
        let js_repl_node_module_dirs = js_repl_node_module_dirs_override
            .or_else(|| {
                config_profile
                    .js_repl_node_module_dirs
                    .map(|dirs| dirs.into_iter().map(Into::into).collect::<Vec<PathBuf>>())
            })
            .or_else(|| {
                cfg.js_repl_node_module_dirs
                    .map(|dirs| dirs.into_iter().map(Into::into).collect::<Vec<PathBuf>>())
            })
            .unwrap_or_default();
        let zsh_path = zsh_path_override
            .or(config_profile.zsh_path.map(Into::into))
            .or(cfg.zsh_path.map(Into::into));

        let review_model = override_review_model.or(cfg.review_model);

        let check_for_update_on_startup = cfg.check_for_update_on_startup.unwrap_or(true);
        let model_catalog = load_model_catalog(
            config_profile
                .model_catalog_json
                .clone()
                .or(cfg.model_catalog_json.clone()),
        )?;

        let log_dir = cfg
            .log_dir
            .as_ref()
            .map(AbsolutePathBuf::to_path_buf)
            .unwrap_or_else(|| codex_home.join("log").to_path_buf());
        let sqlite_home = cfg
            .sqlite_home
            .as_ref()
            .map(AbsolutePathBuf::to_path_buf)
            .or_else(|| resolve_sqlite_home_env(&resolved_cwd))
            .unwrap_or_else(|| codex_home.to_path_buf());
        let original_sandbox_policy = sandbox_policy.clone();

        apply_requirement_constrained_value(
            "approval_policy",
            approval_policy,
            &mut constrained_approval_policy,
            &mut startup_warnings,
        )?;
        apply_requirement_constrained_value(
            "approvals_reviewer",
            approvals_reviewer,
            &mut constrained_approvals_reviewer,
            &mut startup_warnings,
        )?;
        apply_requirement_constrained_value(
            "sandbox_mode",
            sandbox_policy,
            &mut constrained_sandbox_policy,
            &mut startup_warnings,
        )?;
        apply_requirement_constrained_value(
            "web_search_mode",
            web_search_mode,
            &mut constrained_web_search_mode,
            &mut startup_warnings,
        )?;

        let mcp_servers = constrain_mcp_servers(cfg.mcp_servers.clone(), mcp_servers.as_ref())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?;

        let (network_requirements, network_requirements_source) = match network_requirements {
            Some(Sourced { value, source }) => (Some(value), Some(source)),
            None => (None, None),
        };
        let has_network_requirements = network_requirements.is_some();
        let network = NetworkProxySpec::from_config_and_constraints(
            configured_network_proxy_config,
            network_requirements,
            constrained_sandbox_policy.get(),
        )
        .map_err(|err| {
            if let Some(source) = network_requirements_source.as_ref() {
                std::io::Error::new(
                    err.kind(),
                    format!("failed to build managed network proxy from {source}: {err}"),
                )
            } else {
                err
            }
        })?;
        let network = if has_network_requirements {
            Some(network)
        } else {
            network.enabled().then_some(network)
        };
        let helper_readable_roots = get_readable_roots_required_for_codex_runtime(
            &codex_home,
            zsh_path.as_ref(),
            main_execve_wrapper_exe.as_ref(),
        );
        let effective_sandbox_policy = constrained_sandbox_policy.value.get().clone();
        let effective_file_system_sandbox_policy =
            if effective_sandbox_policy == original_sandbox_policy {
                file_system_sandbox_policy
            } else {
                FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                    &effective_sandbox_policy,
                    resolved_cwd.as_path(),
                )
            };
        let effective_file_system_sandbox_policy = effective_file_system_sandbox_policy
            .with_additional_readable_roots(resolved_cwd.as_path(), &helper_readable_roots);
        let effective_network_sandbox_policy =
            if effective_sandbox_policy == original_sandbox_policy {
                network_sandbox_policy
            } else {
                NetworkSandboxPolicy::from(&effective_sandbox_policy)
            };
        let config = Self {
            model,
            service_tier,
            review_model,
            model_context_window: cfg.model_context_window,
            model_auto_compact_token_limit: cfg.model_auto_compact_token_limit,
            model_provider_id,
            model_provider,
            cwd: resolved_cwd,
            startup_warnings,
            permissions: Permissions {
                approval_policy: constrained_approval_policy.value,
                sandbox_policy: constrained_sandbox_policy.value,
                file_system_sandbox_policy: effective_file_system_sandbox_policy,
                network_sandbox_policy: effective_network_sandbox_policy,
                network,
                allow_login_shell,
                shell_environment_policy,
                windows_sandbox_mode,
                windows_sandbox_private_desktop,
            },
            approvals_reviewer: constrained_approvals_reviewer.value(),
            enforce_residency: enforce_residency.value,
            notify: cfg.notify,
            user_instructions,
            user_instructions_path,
            base_instructions,
            personality,
            developer_instructions,
            compact_prompt,
            commit_attribution,
            include_permissions_instructions,
            include_apps_instructions,
            include_environment_context,
            // The config.toml omits "_mode" because it's a config file. However, "_mode"
            // is important in code to differentiate the mode from the store implementation.
            cli_auth_credentials_store_mode: resolve_cli_auth_credentials_store_mode(
                cfg.cli_auth_credentials_store.unwrap_or_default(),
                env!("CARGO_PKG_VERSION"),
            ),
            mcp_servers,
            // The config.toml omits "_mode" because it's a config file. However, "_mode"
            // is important in code to differentiate the mode from the store implementation.
            mcp_oauth_credentials_store_mode: resolve_mcp_oauth_credentials_store_mode(
                cfg.mcp_oauth_credentials_store.unwrap_or_default(),
                env!("CARGO_PKG_VERSION"),
            ),
            mcp_oauth_callback_port: cfg.mcp_oauth_callback_port,
            mcp_oauth_callback_url: cfg.mcp_oauth_callback_url.clone(),
            model_providers,
            project_doc_max_bytes: cfg.project_doc_max_bytes.unwrap_or(PROJECT_DOC_MAX_BYTES),
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
            agent_max_threads,
            agent_max_depth,
            agent_roles,
            memories: cfg.memories.unwrap_or_default().into(),
            agent_job_max_runtime_seconds,
            codex_home,
            sqlite_home,
            log_dir,
            config_layer_stack,
            history,
            ephemeral: ephemeral.unwrap_or_default(),
            file_opener: cfg.file_opener.unwrap_or(UriBasedFileOpener::VsCode),
            codex_self_exe,
            codex_linux_sandbox_exe,
            main_execve_wrapper_exe,
            js_repl_node_path,
            js_repl_node_module_dirs,
            zsh_path,

            hide_agent_reasoning: cfg.hide_agent_reasoning.unwrap_or(false),
            show_raw_agent_reasoning: cfg
                .show_raw_agent_reasoning
                .or(show_raw_agent_reasoning)
                .unwrap_or(false),
            guardian_policy_config,
            model_reasoning_effort: config_profile
                .model_reasoning_effort
                .or(cfg.model_reasoning_effort),
            plan_mode_reasoning_effort: config_profile
                .plan_mode_reasoning_effort
                .or(cfg.plan_mode_reasoning_effort),
            model_reasoning_summary: config_profile
                .model_reasoning_summary
                .or(cfg.model_reasoning_summary),
            model_supports_reasoning_summaries: cfg.model_supports_reasoning_summaries,
            model_catalog,
            model_verbosity: config_profile.model_verbosity.or(cfg.model_verbosity),
            chatgpt_base_url: config_profile
                .chatgpt_base_url
                .or(cfg.chatgpt_base_url)
                .unwrap_or("https://chatgpt.com/backend-api/".to_string()),
            realtime_audio: cfg
                .audio
                .map_or_else(RealtimeAudioConfig::default, |audio| RealtimeAudioConfig {
                    microphone: audio.microphone,
                    speaker: audio.speaker,
                }),
            experimental_realtime_ws_base_url: cfg.experimental_realtime_ws_base_url,
            experimental_realtime_ws_model: cfg.experimental_realtime_ws_model,
            realtime: cfg
                .realtime
                .map_or_else(RealtimeConfig::default, |realtime| {
                    let defaults = RealtimeConfig::default();
                    RealtimeConfig {
                        version: realtime.version.unwrap_or(defaults.version),
                        session_type: realtime.session_type.unwrap_or(defaults.session_type),
                        transport: realtime.transport.unwrap_or(defaults.transport),
                        voice: realtime.voice,
                    }
                }),
            experimental_realtime_ws_backend_prompt: cfg.experimental_realtime_ws_backend_prompt,
            experimental_realtime_ws_startup_context: cfg.experimental_realtime_ws_startup_context,
            experimental_realtime_start_instructions: cfg.experimental_realtime_start_instructions,
            forced_chatgpt_workspace_id,
            forced_login_method,
            include_apply_patch_tool: include_apply_patch_tool_flag,
            web_search_mode: constrained_web_search_mode.value,
            web_search_config,
            use_experimental_unified_exec_tool,
            background_terminal_max_timeout,
            ghost_snapshot,
            multi_agent_v2,
            features,
            suppress_unstable_features_warning: cfg
                .suppress_unstable_features_warning
                .unwrap_or(false),
            active_profile: active_profile_name,
            active_project,
            windows_wsl_setup_acknowledged: cfg.windows_wsl_setup_acknowledged.unwrap_or(false),
            notices: cfg.notice.unwrap_or_default(),
            check_for_update_on_startup,
            disable_paste_burst: cfg.disable_paste_burst.unwrap_or(false),
            analytics_enabled: config_profile
                .analytics
                .as_ref()
                .and_then(|a| a.enabled)
                .or(cfg.analytics.as_ref().and_then(|a| a.enabled)),
            feedback_enabled: cfg
                .feedback
                .as_ref()
                .and_then(|feedback| feedback.enabled)
                .unwrap_or(true),
            tool_suggest,
            tui_notifications: cfg
                .tui
                .as_ref()
                .map(|t| t.notification_settings.clone())
                .unwrap_or_default(),
            animations: cfg.tui.as_ref().map(|t| t.animations).unwrap_or(true),
            show_tooltips: cfg.tui.as_ref().map(|t| t.show_tooltips).unwrap_or(true),
            model_availability_nux: cfg
                .tui
                .as_ref()
                .map(|t| t.model_availability_nux.clone())
                .unwrap_or_default(),
            tui_alternate_screen: cfg
                .tui
                .as_ref()
                .map(|t| t.alternate_screen)
                .unwrap_or_default(),
            tui_status_line: cfg.tui.as_ref().and_then(|t| t.status_line.clone()),
            tui_terminal_title: cfg.tui.as_ref().and_then(|t| t.terminal_title.clone()),
            tui_theme: cfg.tui.as_ref().and_then(|t| t.theme.clone()),
            otel: {
                let t: OtelConfigToml = cfg.otel.unwrap_or_default();
                let log_user_prompt = t.log_user_prompt.unwrap_or(false);
                let environment = t
                    .environment
                    .unwrap_or(DEFAULT_OTEL_ENVIRONMENT.to_string());
                let exporter = t.exporter.unwrap_or(OtelExporterKind::None);
                let trace_exporter = t.trace_exporter.unwrap_or_else(|| exporter.clone());
                let metrics_exporter = t.metrics_exporter.unwrap_or(OtelExporterKind::Statsig);
                OtelConfig {
                    log_user_prompt,
                    environment,
                    exporter,
                    trace_exporter,
                    metrics_exporter,
                }
            },
        };
        Ok(config)
        })
        .await
    }

    fn load_instructions(codex_dir: Option<&AbsolutePathBuf>) -> Option<LoadedUserInstructions> {
        let base = codex_dir?;
        for candidate in [LOCAL_PROJECT_DOC_FILENAME, DEFAULT_PROJECT_DOC_FILENAME] {
            let path = base.join(candidate);
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let trimmed = contents.trim();
                if !trimmed.is_empty() {
                    return Some(LoadedUserInstructions {
                        contents: trimmed.to_string(),
                        path,
                    });
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

    pub fn set_windows_sandbox_enabled(&mut self, value: bool) {
        self.permissions.windows_sandbox_mode = if value {
            Some(WindowsSandboxModeToml::Unelevated)
        } else if matches!(
            self.permissions.windows_sandbox_mode,
            Some(WindowsSandboxModeToml::Unelevated)
        ) {
            None
        } else {
            self.permissions.windows_sandbox_mode
        };
    }

    pub fn set_windows_elevated_sandbox_enabled(&mut self, value: bool) {
        self.permissions.windows_sandbox_mode = if value {
            Some(WindowsSandboxModeToml::Elevated)
        } else if matches!(
            self.permissions.windows_sandbox_mode,
            Some(WindowsSandboxModeToml::Elevated)
        ) {
            None
        } else {
            self.permissions.windows_sandbox_mode
        };
    }

    pub fn managed_network_requirements_enabled(&self) -> bool {
        !matches!(
            self.permissions.sandbox_policy.get(),
            SandboxPolicy::DangerFullAccess
        ) && self
            .config_layer_stack
            .requirements_toml()
            .network
            .is_some()
    }

    pub fn bundled_skills_enabled(&self) -> bool {
        crate::manager::bundled_skills_enabled_from_stack(&self.config_layer_stack)
    }
}

struct LoadedUserInstructions {
    contents: String,
    path: AbsolutePathBuf,
}

pub(crate) fn uses_deprecated_instructions_file(config_layer_stack: &ConfigLayerStack) -> bool {
    config_layer_stack
        .layers_high_to_low()
        .into_iter()
        .any(|layer| toml_uses_deprecated_instructions_file(&layer.config))
}

fn guardian_policy_config_from_requirements(
    requirements_toml: &ConfigRequirementsToml,
) -> Option<String> {
    requirements_toml
        .guardian_policy_config
        .as_deref()
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
}

fn toml_uses_deprecated_instructions_file(value: &TomlValue) -> bool {
    let Some(table) = value.as_table() else {
        return false;
    };
    if table.contains_key("experimental_instructions_file") {
        return true;
    }
    let Some(profiles) = table.get("profiles").and_then(TomlValue::as_table) else {
        return false;
    };
    profiles.values().any(|profile| {
        profile.as_table().is_some_and(|profile_table| {
            profile_table.contains_key("experimental_instructions_file")
        })
    })
}

/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value must exist and be a directory. The
///   value will be canonicalized and this function will Err otherwise.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub fn find_codex_home() -> std::io::Result<AbsolutePathBuf> {
    codex_utils_home_dir::find_codex_home()
}

/// Returns the path to the folder where Codex logs are stored. Does not verify
/// that the directory exists.
pub fn log_dir(cfg: &Config) -> std::io::Result<PathBuf> {
    Ok(cfg.log_dir.clone())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
