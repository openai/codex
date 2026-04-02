use crate::features::features_schema;
use crate::model_provider::ModelProviderInfo;
use crate::permissions::PermissionsToml;
use codex_config::AppToolApproval;
use codex_config::McpServerConfig;
use codex_config::RawMcpServerConfig;
use codex_config::SkillsConfig;
use codex_protocol::config_types::AltScreenMode;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::config_types::Verbosity;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WebSearchToolConfig;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::RealtimeConversationVersion as RealtimeWsVersion;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::ObjectValidation;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;

const fn default_enabled() -> bool {
    true
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AuthCredentialsStoreMode {
    #[default]
    File,
    Keyring,
    Auto,
    Ephemeral,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OAuthCredentialsStoreMode {
    #[default]
    Auto,
    File,
    Keyring,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ConfigProfile {
    pub model: Option<String>,
    pub service_tier: Option<ServiceTier>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub plan_mode_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub model_catalog_json: Option<AbsolutePathBuf>,
    pub personality: Option<Personality>,
    pub chatgpt_base_url: Option<String>,
    pub model_instructions_file: Option<AbsolutePathBuf>,
    pub js_repl_node_path: Option<AbsolutePathBuf>,
    pub js_repl_node_module_dirs: Option<Vec<AbsolutePathBuf>>,
    pub zsh_path: Option<AbsolutePathBuf>,
    #[schemars(skip)]
    pub experimental_instructions_file: Option<AbsolutePathBuf>,
    pub experimental_compact_prompt_file: Option<AbsolutePathBuf>,
    pub include_apply_patch_tool: Option<bool>,
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub experimental_use_freeform_apply_patch: Option<bool>,
    pub tools_view_image: Option<bool>,
    pub tools: Option<ToolsToml>,
    pub web_search: Option<WebSearchMode>,
    pub analytics: Option<AnalyticsConfigToml>,
    #[serde(default)]
    pub windows: Option<WindowsToml>,
    #[serde(default)]
    #[schemars(schema_with = "features_schema")]
    pub features: Option<FeaturesToml>,
    pub oss_provider: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
pub struct FeaturesToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsSandboxModeToml {
    Elevated,
    Unelevated,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct WindowsToml {
    pub sandbox: Option<WindowsSandboxModeToml>,
    pub sandbox_private_desktop: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, JsonSchema)]
pub enum UriBasedFileOpener {
    #[serde(rename = "vscode")]
    VsCode,
    #[serde(rename = "vscode-insiders")]
    VsCodeInsiders,
    #[serde(rename = "windsurf")]
    Windsurf,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "none")]
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct History {
    pub persistence: HistoryPersistence,
    pub max_bytes: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    #[default]
    SaveAll,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AnalyticsConfigToml {
    pub enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct FeedbackConfigToml {
    pub enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolSuggestDiscoverableType {
    Connector,
    Plugin,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestDiscoverable {
    #[serde(rename = "type")]
    pub kind: ToolSuggestDiscoverableType,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestConfig {
    #[serde(default)]
    pub discoverables: Vec<ToolSuggestDiscoverable>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct MemoriesToml {
    pub no_memories_if_mcp_or_web_search: Option<bool>,
    pub generate_memories: Option<bool>,
    pub use_memories: Option<bool>,
    pub max_raw_memories_for_consolidation: Option<usize>,
    pub max_unused_days: Option<i64>,
    pub max_rollout_age_days: Option<i64>,
    pub max_rollouts_per_startup: Option<usize>,
    pub min_rollout_idle_hours: Option<i64>,
    pub extract_model: Option<String>,
    pub consolidation_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppsDefaultConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(
        default = "default_enabled",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub destructive_enabled: bool,
    #[serde(
        default = "default_enabled",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub open_world_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AppToolApproval>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppToolsConfig {
    #[serde(default, flatten)]
    pub tools: HashMap<String, AppToolConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tools_approval_mode: Option<AppToolApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tools_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<AppToolsConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppsConfigToml {
    #[serde(default, rename = "_default", skip_serializing_if = "Option::is_none")]
    pub default: Option<AppsDefaultConfig>,
    #[serde(default, flatten)]
    pub apps: HashMap<String, AppConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OtelHttpProtocol {
    Binary,
    Json,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct OtelTlsConfig {
    pub ca_certificate: Option<AbsolutePathBuf>,
    pub client_certificate: Option<AbsolutePathBuf>,
    pub client_private_key: Option<AbsolutePathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum OtelExporterKind {
    None,
    Statsig,
    OtlpHttp {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
        #[serde(default)]
        tls: Option<OtelTlsConfig>,
    },
    OtlpGrpc {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        tls: Option<OtelTlsConfig>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct OtelConfigToml {
    pub log_user_prompt: Option<bool>,
    pub environment: Option<String>,
    pub exporter: Option<OtelExporterKind>,
    pub trace_exporter: Option<OtelExporterKind>,
    pub metrics_exporter: Option<OtelExporterKind>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Notifications {
    Enabled(bool),
    Custom(Vec<String>),
}

impl Default for Notifications {
    fn default() -> Self {
        Self::Enabled(true)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum NotificationMethod {
    #[default]
    Auto,
    Osc9,
    Bel,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ModelAvailabilityNuxConfig {
    #[serde(default, flatten)]
    pub shown_count: HashMap<String, u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Tui {
    #[serde(default)]
    pub notifications: Notifications,
    #[serde(default)]
    pub notification_method: NotificationMethod,
    #[serde(default = "default_true")]
    pub animations: bool,
    #[serde(default = "default_true")]
    pub show_tooltips: bool,
    #[serde(default)]
    pub alternate_screen: AltScreenMode,
    #[serde(default)]
    pub status_line: Option<Vec<String>>,
    #[serde(default)]
    pub terminal_title: Option<Vec<String>>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub model_availability_nux: ModelAvailabilityNuxConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
pub struct Notice {
    pub hide_full_access_warning: Option<bool>,
    pub hide_world_writable_warning: Option<bool>,
    pub hide_rate_limit_model_nudge: Option<bool>,
    pub hide_gpt5_1_migration_prompt: Option<bool>,
    #[serde(rename = "hide_gpt-5.1-codex-max_migration_prompt")]
    pub hide_gpt_5_1_codex_max_migration_prompt: Option<bool>,
    #[serde(default)]
    pub model_migrations: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PluginConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<AbsolutePathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ShellEnvironmentPolicyInherit {
    Core,
    #[default]
    All,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShellEnvironmentPolicyToml {
    pub inherit: Option<ShellEnvironmentPolicyInherit>,
    pub ignore_default_excludes: Option<bool>,
    pub exclude: Option<Vec<String>>,
    pub r#set: Option<HashMap<String, String>>,
    pub include_only: Option<Vec<String>>,
    pub experimental_use_profile: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ProjectConfig {
    pub trust_level: Option<TrustLevel>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeWsMode {
    #[default]
    Conversational,
    Transcription,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct RealtimeToml {
    pub version: Option<RealtimeWsVersion>,
    #[serde(rename = "type")]
    pub session_type: Option<RealtimeWsMode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct RealtimeAudioToml {
    pub microphone: Option<String>,
    pub speaker: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CustomModelToml {
    pub name: String,
    pub model: String,
    pub model_context_window: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolsToml {
    pub web_search: Option<WebSearchToolConfig>,
    #[serde(default)]
    pub view_image: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AgentsToml {
    #[schemars(range(min = 1))]
    pub max_threads: Option<usize>,
    #[schemars(range(min = 1))]
    pub max_depth: Option<i32>,
    #[schemars(range(min = 1))]
    pub job_max_runtime_seconds: Option<u64>,
    #[serde(default, flatten)]
    pub roles: BTreeMap<String, AgentRoleToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AgentRoleToml {
    pub description: Option<String>,
    pub config_file: Option<AbsolutePathBuf>,
    pub nickname_candidates: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GhostSnapshotToml {
    #[serde(alias = "ignore_untracked_files_over_bytes")]
    pub ignore_large_untracked_files: Option<i64>,
    #[serde(alias = "large_untracked_dir_warning_threshold")]
    pub ignore_large_untracked_dirs: Option<i64>,
    pub disable_warnings: Option<bool>,
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ConfigToml {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub model_provider: Option<String>,
    pub model_context_window: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(default)]
    pub shell_environment_policy: ShellEnvironmentPolicyToml,
    pub allow_login_shell: Option<bool>,
    pub sandbox_mode: Option<SandboxMode>,
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,
    pub default_permissions: Option<String>,
    #[serde(default)]
    pub permissions: Option<PermissionsToml>,
    #[serde(default)]
    pub notify: Option<Vec<String>>,
    pub instructions: Option<String>,
    #[serde(default)]
    pub developer_instructions: Option<String>,
    pub model_instructions_file: Option<AbsolutePathBuf>,
    pub compact_prompt: Option<String>,
    pub commit_attribution: Option<String>,
    #[serde(default)]
    pub forced_chatgpt_workspace_id: Option<String>,
    #[serde(default)]
    pub forced_login_method: Option<ForcedLoginMethod>,
    #[serde(default)]
    pub cli_auth_credentials_store: Option<AuthCredentialsStoreMode>,
    #[serde(default)]
    #[schemars(schema_with = "mcp_servers_schema")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub mcp_oauth_credentials_store: Option<OAuthCredentialsStoreMode>,
    pub mcp_oauth_callback_port: Option<u16>,
    pub mcp_oauth_callback_url: Option<String>,
    #[serde(default)]
    pub model_providers: HashMap<String, ModelProviderInfo>,
    #[serde(default)]
    pub custom_models: Vec<CustomModelToml>,
    pub project_doc_max_bytes: Option<usize>,
    pub project_doc_fallback_filenames: Option<Vec<String>>,
    pub tool_output_token_limit: Option<usize>,
    pub background_terminal_max_timeout: Option<u64>,
    pub js_repl_node_path: Option<AbsolutePathBuf>,
    pub js_repl_node_module_dirs: Option<Vec<AbsolutePathBuf>>,
    pub zsh_path: Option<AbsolutePathBuf>,
    pub profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,
    #[serde(default)]
    pub history: Option<History>,
    pub sqlite_home: Option<AbsolutePathBuf>,
    pub log_dir: Option<AbsolutePathBuf>,
    pub file_opener: Option<UriBasedFileOpener>,
    pub tui: Option<Tui>,
    pub hide_agent_reasoning: Option<bool>,
    pub show_raw_agent_reasoning: Option<bool>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub plan_mode_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub model_supports_reasoning_summaries: Option<bool>,
    pub model_catalog_json: Option<AbsolutePathBuf>,
    pub personality: Option<Personality>,
    pub service_tier: Option<ServiceTier>,
    pub chatgpt_base_url: Option<String>,
    pub openai_base_url: Option<String>,
    #[serde(default)]
    pub audio: Option<RealtimeAudioToml>,
    pub experimental_realtime_ws_base_url: Option<String>,
    pub experimental_realtime_ws_model: Option<String>,
    #[serde(default)]
    pub realtime: Option<RealtimeToml>,
    pub experimental_realtime_ws_backend_prompt: Option<String>,
    pub experimental_realtime_ws_startup_context: Option<String>,
    pub experimental_realtime_start_instructions: Option<String>,
    pub projects: Option<HashMap<String, ProjectConfig>>,
    pub web_search: Option<WebSearchMode>,
    pub tools: Option<ToolsToml>,
    pub tool_suggest: Option<ToolSuggestConfig>,
    pub agents: Option<AgentsToml>,
    pub memories: Option<MemoriesToml>,
    pub skills: Option<SkillsConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    #[schemars(schema_with = "features_schema")]
    pub features: Option<FeaturesToml>,
    pub suppress_unstable_features_warning: Option<bool>,
    #[serde(default)]
    pub ghost_snapshot: Option<GhostSnapshotToml>,
    #[serde(default)]
    pub project_root_markers: Option<Vec<String>>,
    pub check_for_update_on_startup: Option<bool>,
    pub disable_paste_burst: Option<bool>,
    pub analytics: Option<AnalyticsConfigToml>,
    pub feedback: Option<FeedbackConfigToml>,
    #[serde(default)]
    pub apps: Option<AppsConfigToml>,
    pub otel: Option<OtelConfigToml>,
    #[serde(default)]
    pub windows: Option<WindowsToml>,
    pub windows_wsl_setup_acknowledged: Option<bool>,
    pub notice: Option<Notice>,
    #[schemars(skip)]
    pub experimental_instructions_file: Option<AbsolutePathBuf>,
    pub experimental_compact_prompt_file: Option<AbsolutePathBuf>,
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub experimental_use_freeform_apply_patch: Option<bool>,
    pub oss_provider: Option<String>,
}

/// Schema for the `[mcp_servers]` map using the raw input shape.
pub fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let validation = ObjectValidation {
        additional_properties: Some(Box::new(schema_gen.subschema_for::<RawMcpServerConfig>())),
        ..Default::default()
    };
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}
