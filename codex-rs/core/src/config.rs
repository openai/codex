use crate::config_loader::LoadedConfigLayers;
pub use crate::config_loader::load_config_as_toml;
use crate::config_loader::load_config_layers_with_overrides;
use crate::config_loader::merge_toml_values;
use crate::config_profile::ConfigProfile;
use crate::config_types::DEFAULT_OTEL_ENVIRONMENT;
use crate::config_types::History;
use crate::config_types::McpServerConfig;
use crate::config_types::McpServerTransportConfig;
use crate::config_types::Notice;
use crate::config_types::Notifications;
use crate::config_types::OtelConfig;
use crate::config_types::OtelConfigToml;
use crate::config_types::OtelExporterKind;
use crate::config_types::ReasoningSummaryFormat;
use crate::config_types::SandboxWorkspaceWrite;
use crate::config_types::ShellEnvironmentPolicy;
use crate::config_types::ShellEnvironmentPolicyToml;
use crate::config_types::Tui;
use crate::config_types::UriBasedFileOpener;
use crate::features::Feature;
use crate::features::FeatureOverrides;
use crate::features::Features;
use crate::features::FeaturesToml;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::model_family::ModelFamily;
use crate::model_family::derive_default_model_family;
use crate::model_family::find_family_for_model;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::built_in_model_providers;
use crate::openai_model_info::get_model_info;
use crate::project_doc::DEFAULT_PROJECT_DOC_FILENAME;
use crate::project_doc::LOCAL_PROJECT_DOC_FILENAME;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use anyhow::Context;
use codex_app_server_protocol::Tools;
use codex_app_server_protocol::UserSavedConfig;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::Verbosity;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use dirs::home_dir;
use dunce::canonicalize;
use serde::Deserialize;
use similar::DiffableStr;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use tempfile::NamedTempFile;
use toml::Value as TomlValue;
use toml_edit::Array as TomlArray;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;

#[cfg(target_os = "windows")]
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-5";
#[cfg(not(target_os = "windows"))]
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-5-codex";
const OPENAI_DEFAULT_REVIEW_MODEL: &str = "gpt-5-codex";
pub const GPT_5_CODEX_MEDIUM_MODEL: &str = "gpt-5-codex";

/// Maximum number of bytes of the documentation that will be embedded. Larger
/// files are *silently truncated* to this size so we do not take up too much of
/// the context window.
pub(crate) const PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32 KiB

pub(crate) const CONFIG_TOML_FILE: &str = "config.toml";

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Optional override of model selection.
    pub model: String,

    /// Model used specifically for review sessions. Defaults to "gpt-5-codex".
    pub review_model: String,

    pub model_family: ModelFamily,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<i64>,

    /// Maximum number of output tokens.
    pub model_max_output_tokens: Option<i64>,

    /// Token usage threshold triggering auto-compaction of conversation history.
    pub model_auto_compact_token_limit: Option<i64>,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Approval policy for executing commands.
    pub approval_policy: AskForApproval,

    pub sandbox_policy: SandboxPolicy,

    /// True if the user passed in an override or set a value in config.toml
    /// for either of approval_policy or sandbox_mode.
    pub did_user_set_custom_approval_policy_or_sandbox_mode: bool,

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

    /// The directory that should be treated as the current working directory
    /// for the session. All relative paths inside the business-logic layer are
    /// resolved against this path.
    pub cwd: PathBuf,

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

    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: String,

    /// When set, restricts ChatGPT login to a specific workspace identifier.
    pub forced_chatgpt_workspace_id: Option<String>,

    /// When set, restricts the login mechanism users may use.
    pub forced_login_method: Option<ForcedLoginMethod>,

    /// Include an experimental plan tool that the model can use to update its current plan and status of each step.
    pub include_plan_tool: bool,

    /// Include the `apply_patch` tool for models that benefit from invoking
    /// file edits as a structured tool call. When unset, this falls back to the
    /// model family's default preference.
    pub include_apply_patch_tool: bool,

    pub tools_web_search_request: bool,

    pub use_experimental_streamable_shell_tool: bool,

    /// If set to `true`, used only the experimental unified exec tool.
    pub use_experimental_unified_exec_tool: bool,

    /// If set to `true`, use the experimental official Rust MCP client.
    /// https://github.com/modelcontextprotocol/rust-sdk
    pub use_experimental_use_rmcp_client: bool,

    /// Include the `view_image` tool that lets the agent attach a local image path to context.
    pub include_view_image_tool: bool,

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

    /// When true, disables burst-paste detection for typed input entirely.
    /// All characters are inserted as they are received, and no buffering
    /// or placeholder replacement will occur for fast keypress bursts.
    pub disable_paste_burst: bool,

    /// OTEL configuration (exporter type, endpoint, headers, etc.).
    pub otel: crate::config_types::OtelConfig,
}

impl Config {
    pub async fn load_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
        overrides: ConfigOverrides,
    ) -> std::io::Result<Self> {
        let codex_home = find_codex_home()?;

        let root_value = load_resolved_config(
            &codex_home,
            cli_overrides,
            crate::config_loader::LoaderOverrides::default(),
        )
        .await?;

        let cfg: ConfigToml = root_value.try_into().map_err(|e| {
            tracing::error!("Failed to deserialize overridden config: {e}");
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        Self::load_from_base_config_with_overrides(cfg, overrides, codex_home)
    }
}

pub async fn load_config_as_toml_with_cli_overrides(
    codex_home: &Path,
    cli_overrides: Vec<(String, TomlValue)>,
) -> std::io::Result<ConfigToml> {
    let root_value = load_resolved_config(
        codex_home,
        cli_overrides,
        crate::config_loader::LoaderOverrides::default(),
    )
    .await?;

    let cfg: ConfigToml = root_value.try_into().map_err(|e| {
        tracing::error!("Failed to deserialize overridden config: {e}");
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;

    Ok(cfg)
}

async fn load_resolved_config(
    codex_home: &Path,
    cli_overrides: Vec<(String, TomlValue)>,
    overrides: crate::config_loader::LoaderOverrides,
) -> std::io::Result<TomlValue> {
    let layers = load_config_layers_with_overrides(codex_home, overrides).await?;
    Ok(apply_overlays(layers, cli_overrides))
}

fn apply_overlays(
    layers: LoadedConfigLayers,
    cli_overrides: Vec<(String, TomlValue)>,
) -> TomlValue {
    let LoadedConfigLayers {
        mut base,
        managed_config,
        managed_preferences,
    } = layers;

    for (path, value) in cli_overrides.into_iter() {
        apply_toml_override(&mut base, &path, value);
    }

    for overlay in [managed_config, managed_preferences].into_iter().flatten() {
        merge_toml_values(&mut base, &overlay);
    }

    base
}

pub async fn load_global_mcp_servers(
    codex_home: &Path,
) -> std::io::Result<BTreeMap<String, McpServerConfig>> {
    let root_value = load_config_as_toml(codex_home).await?;
    let Some(servers_value) = root_value.get("mcp_servers") else {
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

pub fn write_global_mcp_servers(
    codex_home: &Path,
    servers: &BTreeMap<String, McpServerConfig>,
) -> std::io::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let mut doc = match std::fs::read_to_string(&config_path) {
        Ok(contents) => contents
            .parse::<DocumentMut>()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(e),
    };

    doc.as_table_mut().remove("mcp_servers");

    if !servers.is_empty() {
        let mut table = TomlTable::new();
        table.set_implicit(true);
        doc["mcp_servers"] = TomlItem::Table(table);

        for (name, config) in servers {
            let mut entry = TomlTable::new();
            entry.set_implicit(false);
            match &config.transport {
                McpServerTransportConfig::Stdio {
                    command,
                    args,
                    env,
                    env_vars,
                    cwd,
                } => {
                    entry["command"] = toml_edit::value(command.clone());

                    if !args.is_empty() {
                        let mut args_array = TomlArray::new();
                        for arg in args {
                            args_array.push(arg.clone());
                        }
                        entry["args"] = TomlItem::Value(args_array.into());
                    }

                    if let Some(env) = env
                        && !env.is_empty()
                    {
                        let mut env_table = TomlTable::new();
                        env_table.set_implicit(false);
                        let mut pairs: Vec<_> = env.iter().collect();
                        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                        for (key, value) in pairs {
                            env_table.insert(key, toml_edit::value(value.clone()));
                        }
                        entry["env"] = TomlItem::Table(env_table);
                    }

                    if !env_vars.is_empty() {
                        entry["env_vars"] =
                            TomlItem::Value(env_vars.iter().collect::<TomlArray>().into());
                    }

                    if let Some(cwd) = cwd {
                        entry["cwd"] = toml_edit::value(cwd.to_string_lossy().to_string());
                    }
                }
                McpServerTransportConfig::StreamableHttp {
                    url,
                    bearer_token_env_var,
                    http_headers,
                    env_http_headers,
                } => {
                    entry["url"] = toml_edit::value(url.clone());
                    if let Some(env_var) = bearer_token_env_var {
                        entry["bearer_token_env_var"] = toml_edit::value(env_var.clone());
                    }
                    if let Some(headers) = http_headers
                        && !headers.is_empty()
                    {
                        let mut table = TomlTable::new();
                        table.set_implicit(false);
                        let mut pairs: Vec<_> = headers.iter().collect();
                        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                        for (key, value) in pairs {
                            table.insert(key, toml_edit::value(value.clone()));
                        }
                        entry["http_headers"] = TomlItem::Table(table);
                    }
                    if let Some(headers) = env_http_headers
                        && !headers.is_empty()
                    {
                        let mut table = TomlTable::new();
                        table.set_implicit(false);
                        let mut pairs: Vec<_> = headers.iter().collect();
                        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                        for (key, value) in pairs {
                            table.insert(key, toml_edit::value(value.clone()));
                        }
                        entry["env_http_headers"] = TomlItem::Table(table);
                    }
                }
            }

            if !config.enabled {
                entry["enabled"] = toml_edit::value(false);
            }

            if let Some(timeout) = config.startup_timeout_sec {
                entry["startup_timeout_sec"] = toml_edit::value(timeout.as_secs_f64());
            }

            if let Some(timeout) = config.tool_timeout_sec {
                entry["tool_timeout_sec"] = toml_edit::value(timeout.as_secs_f64());
            }

            if let Some(enabled_tools) = &config.enabled_tools {
                entry["enabled_tools"] =
                    TomlItem::Value(enabled_tools.iter().collect::<TomlArray>().into());
            }

            if let Some(disabled_tools) = &config.disabled_tools {
                entry["disabled_tools"] =
                    TomlItem::Value(disabled_tools.iter().collect::<TomlArray>().into());
            }

            doc["mcp_servers"][name.as_str()] = TomlItem::Table(entry);
        }
    }

    std::fs::create_dir_all(codex_home)?;
    let tmp_file = NamedTempFile::new_in(codex_home)?;
    std::fs::write(tmp_file.path(), doc.to_string())?;
    tmp_file.persist(config_path).map_err(|err| err.error)?;

    Ok(())
}

fn set_project_trusted_inner(doc: &mut DocumentMut, project_path: &Path) -> anyhow::Result<()> {
    // Ensure we render a human-friendly structure:
    //
    // [projects]
    // [projects."/path/to/project"]
    // trust_level = "trusted"
    //
    // rather than inline tables like:
    //
    // [projects]
    // "/path/to/project" = { trust_level = "trusted" }
    let project_key = project_path.to_string_lossy().to_string();

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
    proj_tbl["trust_level"] = toml_edit::value("trusted");
    Ok(())
}

/// Patch `CODEX_HOME/config.toml` project state.
/// Use with caution.
pub fn set_project_trusted(codex_home: &Path, project_path: &Path) -> anyhow::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    // Parse existing config if present; otherwise start a new document.
    let mut doc = match std::fs::read_to_string(config_path.clone()) {
        Ok(s) => s.parse::<DocumentMut>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(e.into()),
    };

    set_project_trusted_inner(&mut doc, project_path)?;

    // ensure codex_home exists
    std::fs::create_dir_all(codex_home)?;

    // create a tmp_file
    let tmp_file = NamedTempFile::new_in(codex_home)?;
    std::fs::write(tmp_file.path(), doc.to_string())?;

    // atomically move the tmp file into config.toml
    tmp_file.persist(config_path)?;

    Ok(())
}

/// Persist the acknowledgement flag for the Windows onboarding screen.
pub fn set_windows_wsl_setup_acknowledged(
    codex_home: &Path,
    acknowledged: bool,
) -> anyhow::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let mut doc = match std::fs::read_to_string(config_path.clone()) {
        Ok(s) => s.parse::<DocumentMut>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(e.into()),
    };

    doc["windows_wsl_setup_acknowledged"] = toml_edit::value(acknowledged);

    std::fs::create_dir_all(codex_home)?;

    let tmp_file = NamedTempFile::new_in(codex_home)?;
    std::fs::write(tmp_file.path(), doc.to_string())?;
    tmp_file.persist(config_path)?;

    Ok(())
}

/// Persist the acknowledgement flag for the full access warning prompt.
pub fn set_hide_full_access_warning(codex_home: &Path, acknowledged: bool) -> anyhow::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let mut doc = match std::fs::read_to_string(config_path.clone()) {
        Ok(s) => s.parse::<DocumentMut>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(e.into()),
    };

    let notices_table = load_or_create_top_level_table(&mut doc, Notice::TABLE_KEY)?;

    notices_table["hide_full_access_warning"] = toml_edit::value(acknowledged);

    std::fs::create_dir_all(codex_home)?;
    let tmp_file = NamedTempFile::new_in(codex_home)?;
    std::fs::write(tmp_file.path(), doc.to_string())?;
    tmp_file.persist(config_path)?;

    Ok(())
}

fn load_or_create_top_level_table<'a>(
    doc: &'a mut DocumentMut,
    key: &str,
) -> anyhow::Result<&'a mut toml_edit::Table> {
    let mut created_table = false;

    let root = doc.as_table_mut();
    let needs_table =
        !root.contains_key(key) || root.get(key).and_then(|item| item.as_table()).is_none();
    if needs_table {
        root.insert(key, toml_edit::table());
        created_table = true;
    }

    let Some(table) = doc[key].as_table_mut() else {
        return Err(anyhow::anyhow!(format!(
            "table [{key}] missing after initialization"
        )));
    };

    if created_table {
        table.set_implicit(true);
    }

    Ok(table)
}

fn ensure_profile_table<'a>(
    doc: &'a mut DocumentMut,
    profile_name: &str,
) -> anyhow::Result<&'a mut toml_edit::Table> {
    let mut created_profiles_table = false;
    {
        let root = doc.as_table_mut();
        let needs_table = !root.contains_key("profiles")
            || root
                .get("profiles")
                .and_then(|item| item.as_table())
                .is_none();
        if needs_table {
            root.insert("profiles", toml_edit::table());
            created_profiles_table = true;
        }
    }

    let Some(profiles_table) = doc["profiles"].as_table_mut() else {
        return Err(anyhow::anyhow!(
            "profiles table missing after initialization"
        ));
    };

    if created_profiles_table {
        profiles_table.set_implicit(true);
    }

    let needs_profile_table = !profiles_table.contains_key(profile_name)
        || profiles_table
            .get(profile_name)
            .and_then(|item| item.as_table())
            .is_none();
    if needs_profile_table {
        profiles_table.insert(profile_name, toml_edit::table());
    }

    let Some(profile_table) = profiles_table
        .get_mut(profile_name)
        .and_then(|item| item.as_table_mut())
    else {
        return Err(anyhow::anyhow!(format!(
            "profile table missing for {profile_name}"
        )));
    };

    profile_table.set_implicit(false);
    Ok(profile_table)
}

// TODO(jif) refactor config persistence.
pub async fn persist_model_selection(
    codex_home: &Path,
    active_profile: Option<&str>,
    model: &str,
    effort: Option<ReasoningEffort>,
) -> anyhow::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let serialized = match tokio::fs::read_to_string(&config_path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };

    let mut doc = if serialized.is_empty() {
        DocumentMut::new()
    } else {
        serialized.parse::<DocumentMut>()?
    };

    if let Some(profile_name) = active_profile {
        let profile_table = ensure_profile_table(&mut doc, profile_name)?;
        profile_table["model"] = toml_edit::value(model);
        match effort {
            Some(effort) => {
                profile_table["model_reasoning_effort"] = toml_edit::value(effort.to_string());
            }
            None => {
                profile_table.remove("model_reasoning_effort");
            }
        }
    } else {
        let table = doc.as_table_mut();
        table["model"] = toml_edit::value(model);
        match effort {
            Some(effort) => {
                table["model_reasoning_effort"] = toml_edit::value(effort.to_string());
            }
            None => {
                table.remove("model_reasoning_effort");
            }
        }
    }

    // TODO(jif) refactor the home creation
    tokio::fs::create_dir_all(codex_home)
        .await
        .with_context(|| {
            format!(
                "failed to create Codex home directory at {}",
                codex_home.display()
            )
        })?;

    tokio::fs::write(&config_path, doc.to_string())
        .await
        .with_context(|| format!("failed to persist config.toml at {}", config_path.display()))?;

    Ok(())
}

/// Apply a single dotted-path override onto a TOML value.
fn apply_toml_override(root: &mut TomlValue, path: &str, value: TomlValue) {
    use toml::value::Table;

    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (idx, segment) in segments.iter().enumerate() {
        let is_last = idx == segments.len() - 1;

        if is_last {
            match current {
                TomlValue::Table(table) => {
                    table.insert(segment.to_string(), value);
                }
                _ => {
                    let mut table = Table::new();
                    table.insert(segment.to_string(), value);
                    *current = TomlValue::Table(table);
                }
            }
            return;
        }

        // Traverse or create intermediate object.
        match current {
            TomlValue::Table(table) => {
                current = table
                    .entry(segment.to_string())
                    .or_insert_with(|| TomlValue::Table(Table::new()));
            }
            _ => {
                *current = TomlValue::Table(Table::new());
                if let TomlValue::Table(tbl) = current {
                    current = tbl
                        .entry(segment.to_string())
                        .or_insert_with(|| TomlValue::Table(Table::new()));
                }
            }
        }
    }
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigToml {
    /// Optional override of model selection.
    pub model: Option<String>,
    /// Review model override used by the `/review` feature.
    pub review_model: Option<String>,

    /// Provider to use from the model_providers map.
    pub model_provider: Option<String>,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<i64>,

    /// Maximum number of output tokens.
    pub model_max_output_tokens: Option<i64>,

    /// Token usage threshold triggering auto-compaction of conversation history.
    pub model_auto_compact_token_limit: Option<i64>,

    /// Default approval policy for executing commands.
    pub approval_policy: Option<AskForApproval>,

    #[serde(default)]
    pub shell_environment_policy: ShellEnvironmentPolicyToml,

    /// Sandbox mode to use.
    pub sandbox_mode: Option<SandboxMode>,

    /// Sandbox configuration to apply if `sandbox` is `WorkspaceWrite`.
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,

    /// Optional external command to spawn for end-user notifications.
    #[serde(default)]
    pub notify: Option<Vec<String>>,

    /// System instructions.
    pub instructions: Option<String>,

    /// When set, restricts ChatGPT login to a specific workspace identifier.
    #[serde(default)]
    pub forced_chatgpt_workspace_id: Option<String>,

    /// When set, restricts the login mechanism users may use.
    #[serde(default)]
    pub forced_login_method: Option<ForcedLoginMethod>,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Preferred backend for storing MCP OAuth credentials.
    /// keyring: Use an OS-specific keyring service.
    ///          https://github.com/openai/codex/blob/main/codex-rs/rmcp-client/src/oauth.rs#L2
    /// file: Use a file in the Codex home directory.
    /// auto (default): Use the OS-specific keyring service if available, otherwise use a file.
    #[serde(default)]
    pub mcp_oauth_credentials_store: Option<OAuthCredentialsStoreMode>,

    /// User-defined provider entries that extend/override the built-in list.
    #[serde(default)]
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: Option<usize>,

    /// Ordered list of fallback filenames to look for when AGENTS.md is missing.
    pub project_doc_fallback_filenames: Option<Vec<String>>,

    /// Profile to use from the `profiles` map.
    pub profile: Option<String>,

    /// Named profiles to facilitate switching between different configurations.
    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,

    /// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
    #[serde(default)]
    pub history: Option<History>,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: Option<UriBasedFileOpener>,

    /// Collection of settings that are specific to the TUI.
    pub tui: Option<Tui>,

    /// When set to `true`, `AgentReasoning` events will be hidden from the
    /// UI/output. Defaults to `false`.
    pub hide_agent_reasoning: Option<bool>,

    /// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
    /// Defaults to `false`.
    pub show_raw_agent_reasoning: Option<bool>,

    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Override to force-enable reasoning summaries for the configured model.
    pub model_supports_reasoning_summaries: Option<bool>,

    /// Override to force reasoning summary format for the configured model.
    pub model_reasoning_summary_format: Option<ReasoningSummaryFormat>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: Option<String>,

    pub projects: Option<HashMap<String, ProjectConfig>>,

    /// Nested tools section for feature toggles
    pub tools: Option<ToolsToml>,

    /// Centralized feature flags (new). Prefer this over individual toggles.
    #[serde(default)]
    pub features: Option<FeaturesToml>,

    /// When true, disables burst-paste detection for typed input entirely.
    /// All characters are inserted as they are received, and no buffering
    /// or placeholder replacement will occur for fast keypress bursts.
    pub disable_paste_burst: Option<bool>,

    /// OTEL configuration.
    pub otel: Option<crate::config_types::OtelConfigToml>,

    /// Tracks whether the Windows onboarding screen has been acknowledged.
    pub windows_wsl_setup_acknowledged: Option<bool>,

    /// Collection of in-product notices (different from notifications)
    /// See [`crate::config_types::Notices`] for more details
    pub notice: Option<Notice>,

    /// Legacy, now use features
    pub experimental_instructions_file: Option<PathBuf>,
    pub experimental_use_exec_command_tool: Option<bool>,
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub experimental_use_rmcp_client: Option<bool>,
    pub experimental_use_freeform_apply_patch: Option<bool>,
}

impl From<ConfigToml> for UserSavedConfig {
    fn from(config_toml: ConfigToml) -> Self {
        let profiles = config_toml
            .profiles
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect();

        Self {
            approval_policy: config_toml.approval_policy,
            sandbox_mode: config_toml.sandbox_mode,
            sandbox_settings: config_toml.sandbox_workspace_write.map(From::from),
            forced_chatgpt_workspace_id: config_toml.forced_chatgpt_workspace_id,
            forced_login_method: config_toml.forced_login_method,
            model: config_toml.model,
            model_reasoning_effort: config_toml.model_reasoning_effort,
            model_reasoning_summary: config_toml.model_reasoning_summary,
            model_verbosity: config_toml.model_verbosity,
            tools: config_toml.tools.map(From::from),
            profile: config_toml.profile,
            profiles,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub trust_level: Option<String>,
}

impl ProjectConfig {
    pub fn is_trusted(&self) -> bool {
        match &self.trust_level {
            Some(trust_level) => trust_level == "trusted",
            None => false,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ToolsToml {
    #[serde(default, alias = "web_search_request")]
    pub web_search: Option<bool>,

    /// Enable the `view_image` tool that lets the agent attach local images.
    #[serde(default)]
    pub view_image: Option<bool>,
}

impl From<ToolsToml> for Tools {
    fn from(tools_toml: ToolsToml) -> Self {
        Self {
            web_search: tools_toml.web_search,
            view_image: tools_toml.view_image,
        }
    }
}

impl ConfigToml {
    /// Derive the effective sandbox policy from the configuration.
    fn derive_sandbox_policy(
        &self,
        sandbox_mode_override: Option<SandboxMode>,
        resolved_cwd: &Path,
    ) -> SandboxPolicy {
        let resolved_sandbox_mode = sandbox_mode_override
            .or(self.sandbox_mode)
            .or_else(|| {
                // if no sandbox_mode is set, but user has marked directory as trusted, use WorkspaceWrite
                self.get_active_project(resolved_cwd).and_then(|p| {
                    if p.is_trusted() {
                        Some(SandboxMode::WorkspaceWrite)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();
        match resolved_sandbox_mode {
            SandboxMode::ReadOnly => SandboxPolicy::new_read_only_policy(),
            SandboxMode::WorkspaceWrite => match self.sandbox_workspace_write.as_ref() {
                Some(SandboxWorkspaceWrite {
                    writable_roots,
                    network_access,
                    exclude_tmpdir_env_var,
                    exclude_slash_tmp,
                }) => SandboxPolicy::WorkspaceWrite {
                    writable_roots: writable_roots.clone(),
                    network_access: *network_access,
                    exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                    exclude_slash_tmp: *exclude_slash_tmp,
                },
                None => SandboxPolicy::new_workspace_write_policy(),
            },
            SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        }
    }

    /// Resolves the cwd to an existing project, or returns None if ConfigToml
    /// does not contain a project corresponding to cwd or a git repo for cwd
    pub fn get_active_project(&self, resolved_cwd: &Path) -> Option<ProjectConfig> {
        let projects = self.projects.clone().unwrap_or_default();

        if let Some(project_config) = projects.get(&resolved_cwd.to_string_lossy().to_string()) {
            return Some(project_config.clone());
        }

        // If cwd lives inside a git repo/worktree, check whether the root git project
        // (the primary repository working directory) is trusted. This lets
        // worktrees inherit trust from the main project.
        if let Some(repo_root) = resolve_root_git_project_for_trust(resolved_cwd)
            && let Some(project_config_for_root) =
                projects.get(&repo_root.to_string_lossy().to_string_lossy().to_string())
        {
            return Some(project_config_for_root.clone());
        }

        None
    }

    pub fn get_config_profile(
        &self,
        override_profile: Option<String>,
    ) -> Result<ConfigProfile, std::io::Error> {
        let profile = override_profile.or_else(|| self.profile.clone());

        match profile {
            Some(key) => {
                if let Some(profile) = self.profiles.get(key.as_str()) {
                    return Ok(profile.clone());
                }

                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("config profile `{key}` not found"),
                ))
            }
            None => Ok(ConfigProfile::default()),
        }
    }
}

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub review_model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_provider: Option<String>,
    pub config_profile: Option<String>,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub base_instructions: Option<String>,
    pub include_plan_tool: Option<bool>,
    pub include_apply_patch_tool: Option<bool>,
    pub include_view_image_tool: Option<bool>,
    pub show_raw_agent_reasoning: Option<bool>,
    pub tools_web_search_request: Option<bool>,
    /// Additional directories that should be treated as writable roots for this session.
    pub additional_writable_roots: Vec<PathBuf>,
}

impl Config {
    /// Meant to be used exclusively for tests: `load_with_overrides()` should
    /// be used in all other cases.
    pub fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: PathBuf,
    ) -> std::io::Result<Self> {
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
            include_plan_tool: include_plan_tool_override,
            include_apply_patch_tool: include_apply_patch_tool_override,
            include_view_image_tool: include_view_image_tool_override,
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
            include_plan_tool: include_plan_tool_override,
            include_apply_patch_tool: include_apply_patch_tool_override,
            include_view_image_tool: include_view_image_tool_override,
            web_search_request: override_tools_web_search_request,
        };

        let features = Features::from_config(&cfg, &config_profile, feature_overrides);

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
        let additional_writable_roots: Vec<PathBuf> = additional_writable_roots
            .into_iter()
            .map(|path| {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    resolved_cwd.join(path)
                };
                match canonicalize(&absolute) {
                    Ok(canonical) => canonical,
                    Err(_) => absolute,
                }
            })
            .collect();
        let active_project = cfg
            .get_active_project(&resolved_cwd)
            .unwrap_or(ProjectConfig { trust_level: None });

        let mut sandbox_policy = cfg.derive_sandbox_policy(sandbox_mode, &resolved_cwd);
        if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = &mut sandbox_policy {
            for path in additional_writable_roots {
                if !writable_roots.iter().any(|existing| existing == &path) {
                    writable_roots.push(path);
                }
            }
        }
        let mut approval_policy = approval_policy_override
            .or(config_profile.approval_policy)
            .or(cfg.approval_policy)
            .unwrap_or_else(|| {
                if active_project.is_trusted() {
                    // If no explicit approval policy is set, but we trust cwd, default to OnRequest
                    AskForApproval::OnRequest
                } else {
                    AskForApproval::default()
                }
            });
        let did_user_set_custom_approval_policy_or_sandbox_mode = approval_policy_override
            .is_some()
            || config_profile.approval_policy.is_some()
            || cfg.approval_policy.is_some()
            // TODO(#3034): profile.sandbox_mode is not implemented
            || sandbox_mode.is_some()
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

        let include_plan_tool_flag = features.enabled(Feature::PlanTool);
        let include_apply_patch_tool_flag = features.enabled(Feature::ApplyPatchFreeform);
        let include_view_image_tool_flag = features.enabled(Feature::ViewImageTool);
        let tools_web_search_request = features.enabled(Feature::WebSearchRequest);
        let use_experimental_streamable_shell_tool = features.enabled(Feature::StreamableShell);
        let use_experimental_unified_exec_tool = features.enabled(Feature::UnifiedExec);
        let use_experimental_use_rmcp_client = features.enabled(Feature::RmcpClient);

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

        let model = model
            .or(config_profile.model)
            .or(cfg.model)
            .unwrap_or_else(default_model);

        let mut model_family =
            find_family_for_model(&model).unwrap_or_else(|| derive_default_model_family(&model));

        if let Some(supports_reasoning_summaries) = cfg.model_supports_reasoning_summaries {
            model_family.supports_reasoning_summaries = supports_reasoning_summaries;
        }
        if let Some(model_reasoning_summary_format) = cfg.model_reasoning_summary_format {
            model_family.reasoning_summary_format = model_reasoning_summary_format;
        }

        let openai_model_info = get_model_info(&model_family);
        let model_context_window = cfg
            .model_context_window
            .or_else(|| openai_model_info.as_ref().map(|info| info.context_window));
        let model_max_output_tokens = cfg.model_max_output_tokens.or_else(|| {
            openai_model_info
                .as_ref()
                .map(|info| info.max_output_tokens)
        });
        let model_auto_compact_token_limit = cfg.model_auto_compact_token_limit.or_else(|| {
            openai_model_info
                .as_ref()
                .and_then(|info| info.auto_compact_token_limit)
        });

        // Load base instructions override from a file if specified. If the
        // path is relative, resolve it against the effective cwd so the
        // behaviour matches other path-like config values.
        let experimental_instructions_path = config_profile
            .experimental_instructions_file
            .as_ref()
            .or(cfg.experimental_instructions_file.as_ref());
        let file_base_instructions =
            Self::get_base_instructions(experimental_instructions_path, &resolved_cwd)?;
        let base_instructions = base_instructions.or(file_base_instructions);

        // Default review model when not set in config; allow CLI override to take precedence.
        let review_model = override_review_model
            .or(cfg.review_model)
            .unwrap_or_else(default_review_model);

        if features.enabled(Feature::ApproveAll) {
            approval_policy = AskForApproval::OnRequest;
        }

        let config = Self {
            model,
            review_model,
            model_family,
            model_context_window,
            model_max_output_tokens,
            model_auto_compact_token_limit,
            model_provider_id,
            model_provider,
            cwd: resolved_cwd,
            approval_policy,
            sandbox_policy,
            did_user_set_custom_approval_policy_or_sandbox_mode,
            shell_environment_policy,
            notify: cfg.notify,
            user_instructions,
            base_instructions,
            mcp_servers: cfg.mcp_servers,
            // The config.toml omits "_mode" because it's a config file. However, "_mode"
            // is important in code to differentiate the mode from the store implementation.
            mcp_oauth_credentials_store_mode: cfg.mcp_oauth_credentials_store.unwrap_or_default(),
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
            codex_home,
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
            model_verbosity: config_profile.model_verbosity.or(cfg.model_verbosity),
            chatgpt_base_url: config_profile
                .chatgpt_base_url
                .or(cfg.chatgpt_base_url)
                .unwrap_or("https://chatgpt.com/backend-api/".to_string()),
            forced_chatgpt_workspace_id,
            forced_login_method,
            include_plan_tool: include_plan_tool_flag,
            include_apply_patch_tool: include_apply_patch_tool_flag,
            tools_web_search_request,
            use_experimental_streamable_shell_tool,
            use_experimental_unified_exec_tool,
            use_experimental_use_rmcp_client,
            include_view_image_tool: include_view_image_tool_flag,
            features,
            active_profile: active_profile_name,
            active_project,
            windows_wsl_setup_acknowledged: cfg.windows_wsl_setup_acknowledged.unwrap_or(false),
            notices: cfg.notice.unwrap_or_default(),
            disable_paste_burst: cfg.disable_paste_burst.unwrap_or(false),
            tui_notifications: cfg
                .tui
                .as_ref()
                .map(|t| t.notifications.clone())
                .unwrap_or_default(),
            otel: {
                let t: OtelConfigToml = cfg.otel.unwrap_or_default();
                let log_user_prompt = t.log_user_prompt.unwrap_or(false);
                let environment = t
                    .environment
                    .unwrap_or(DEFAULT_OTEL_ENVIRONMENT.to_string());
                let exporter = t.exporter.unwrap_or(OtelExporterKind::None);
                OtelConfig {
                    log_user_prompt,
                    environment,
                    exporter,
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

    fn get_base_instructions(
        path: Option<&PathBuf>,
        cwd: &Path,
    ) -> std::io::Result<Option<String>> {
        let p = match path.as_ref() {
            None => return Ok(None),
            Some(p) => p,
        };

        // Resolve relative paths against the provided cwd to make CLI
        // overrides consistent regardless of where the process was launched
        // from.
        let full_path = if p.is_relative() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };

        let contents = std::fs::read_to_string(&full_path).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "failed to read experimental instructions file {}: {e}",
                    full_path.display()
                ),
            )
        })?;

        let s = contents.trim().to_string();
        if s.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "experimental instructions file is empty: {}",
                    full_path.display()
                ),
            ))
        } else {
            Ok(Some(s))
        }
    }
}

fn default_model() -> String {
    OPENAI_DEFAULT_MODEL.to_string()
}

fn default_review_model() -> String {
    OPENAI_DEFAULT_REVIEW_MODEL.to_string()
}

/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value will be canonicalized and this
///   function will Err if the path does not exist.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
pub fn find_codex_home() -> std::io::Result<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    if let Ok(val) = std::env::var("CODEX_HOME")
        && !val.is_empty()
    {
        return PathBuf::from(val).canonicalize();
    }

    let mut p = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    p.push(".codex");
    Ok(p)
}

/// Returns the path to the folder where Codex logs are stored. Does not verify
/// that the directory exists.
pub fn log_dir(cfg: &Config) -> std::io::Result<PathBuf> {
    let mut p = cfg.codex_home.clone();
    p.push("log");
    Ok(p)
}

#[cfg(test)]
mod tests {
    use crate::config_types::HistoryPersistence;
    use crate::config_types::Notifications;
    use crate::features::Feature;

    use super::*;
    use pretty_assertions::assert_eq;

    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_toml_parsing() {
        let history_with_persistence = r#"
[history]
persistence = "save-all"
"#;
        let history_with_persistence_cfg = toml::from_str::<ConfigToml>(history_with_persistence)
            .expect("TOML deserialization should succeed");
        assert_eq!(
            Some(History {
                persistence: HistoryPersistence::SaveAll,
                max_bytes: None,
            }),
            history_with_persistence_cfg.history
        );

        let history_no_persistence = r#"
[history]
persistence = "none"
"#;

        let history_no_persistence_cfg = toml::from_str::<ConfigToml>(history_no_persistence)
            .expect("TOML deserialization should succeed");
        assert_eq!(
            Some(History {
                persistence: HistoryPersistence::None,
                max_bytes: None,
            }),
            history_no_persistence_cfg.history
        );
    }

    #[test]
    fn tui_config_missing_notifications_field_defaults_to_disabled() {
        let cfg = r#"
[tui]
"#;

        let parsed = toml::from_str::<ConfigToml>(cfg)
            .expect("TUI config without notifications should succeed");
        let tui = parsed.tui.expect("config should include tui section");

        assert_eq!(tui.notifications, Notifications::Enabled(false));
    }

    #[test]
    fn test_sandbox_config_parsing() {
        let sandbox_full_access = r#"
sandbox_mode = "danger-full-access"

[sandbox_workspace_write]
network_access = false  # This should be ignored.
"#;
        let sandbox_full_access_cfg = toml::from_str::<ConfigToml>(sandbox_full_access)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::DangerFullAccess,
            sandbox_full_access_cfg
                .derive_sandbox_policy(sandbox_mode_override, &PathBuf::from("/tmp/test"))
        );

        let sandbox_read_only = r#"
sandbox_mode = "read-only"

[sandbox_workspace_write]
network_access = true  # This should be ignored.
"#;

        let sandbox_read_only_cfg = toml::from_str::<ConfigToml>(sandbox_read_only)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::ReadOnly,
            sandbox_read_only_cfg
                .derive_sandbox_policy(sandbox_mode_override, &PathBuf::from("/tmp/test"))
        );

        let sandbox_workspace_write = r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [
    "/my/workspace",
]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true
"#;

        let sandbox_workspace_write_cfg = toml::from_str::<ConfigToml>(sandbox_workspace_write)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![PathBuf::from("/my/workspace")],
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            sandbox_workspace_write_cfg
                .derive_sandbox_policy(sandbox_mode_override, &PathBuf::from("/tmp/test"))
        );

        let sandbox_workspace_write = r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [
    "/my/workspace",
]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true

[projects."/tmp/test"]
trust_level = "trusted"
"#;

        let sandbox_workspace_write_cfg = toml::from_str::<ConfigToml>(sandbox_workspace_write)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![PathBuf::from("/my/workspace")],
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            sandbox_workspace_write_cfg
                .derive_sandbox_policy(sandbox_mode_override, &PathBuf::from("/tmp/test"))
        );
    }

    #[test]
    fn add_dir_override_extends_workspace_writable_roots() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let frontend = temp_dir.path().join("frontend");
        let backend = temp_dir.path().join("backend");
        std::fs::create_dir_all(&frontend)?;
        std::fs::create_dir_all(&backend)?;

        let overrides = ConfigOverrides {
            cwd: Some(frontend),
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            additional_writable_roots: vec![PathBuf::from("../backend"), backend.clone()],
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            overrides,
            temp_dir.path().to_path_buf(),
        )?;

        let expected_backend = canonicalize(&backend).expect("canonicalize backend directory");
        match config.sandbox_policy {
            SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
                assert_eq!(
                    writable_roots
                        .iter()
                        .filter(|root| **root == expected_backend)
                        .count(),
                    1,
                    "expected single writable root entry for {}",
                    expected_backend.display()
                );
            }
            other => panic!("expected workspace-write policy, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn approve_all_feature_forces_on_request_policy() -> std::io::Result<()> {
        let cfg = r#"
[features]
approve_all = true
"#;
        let parsed = toml::from_str::<ConfigToml>(cfg)
            .expect("TOML deserialization should succeed for approve_all feature");
        let temp_dir = TempDir::new()?;
        let config = Config::load_from_base_config_with_overrides(
            parsed,
            ConfigOverrides::default(),
            temp_dir.path().to_path_buf(),
        )?;

        assert!(config.features.enabled(Feature::ApproveAll));
        assert_eq!(config.approval_policy, AskForApproval::OnRequest);
        Ok(())
    }

    #[test]
    fn config_defaults_to_auto_oauth_store_mode() -> std::io::Result<()> {
        let codex_home = TempDir::new()?;
        let cfg = ConfigToml::default();

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert_eq!(
            config.mcp_oauth_credentials_store_mode,
            OAuthCredentialsStoreMode::Auto,
        );

        Ok(())
    }

    #[test]
    fn profile_legacy_toggles_override_base() -> std::io::Result<()> {
        let codex_home = TempDir::new()?;
        let mut profiles = HashMap::new();
        profiles.insert(
            "work".to_string(),
            ConfigProfile {
                include_plan_tool: Some(true),
                include_view_image_tool: Some(false),
                ..Default::default()
            },
        );
        let cfg = ConfigToml {
            profiles,
            profile: Some("work".to_string()),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert!(config.features.enabled(Feature::PlanTool));
        assert!(!config.features.enabled(Feature::ViewImageTool));
        assert!(config.include_plan_tool);
        assert!(!config.include_view_image_tool);

        Ok(())
    }

    #[test]
    fn feature_table_overrides_legacy_flags() -> std::io::Result<()> {
        let codex_home = TempDir::new()?;
        let mut entries = BTreeMap::new();
        entries.insert("plan_tool".to_string(), false);
        entries.insert("apply_patch_freeform".to_string(), false);
        let cfg = ConfigToml {
            features: Some(crate::features::FeaturesToml { entries }),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert!(!config.features.enabled(Feature::PlanTool));
        assert!(!config.features.enabled(Feature::ApplyPatchFreeform));
        assert!(!config.include_plan_tool);
        assert!(!config.include_apply_patch_tool);

        Ok(())
    }

    #[test]
    fn legacy_toggles_map_to_features() -> std::io::Result<()> {
        let codex_home = TempDir::new()?;
        let cfg = ConfigToml {
            experimental_use_exec_command_tool: Some(true),
            experimental_use_unified_exec_tool: Some(true),
            experimental_use_rmcp_client: Some(true),
            experimental_use_freeform_apply_patch: Some(true),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert!(config.features.enabled(Feature::ApplyPatchFreeform));
        assert!(config.features.enabled(Feature::StreamableShell));
        assert!(config.features.enabled(Feature::UnifiedExec));
        assert!(config.features.enabled(Feature::RmcpClient));

        assert!(config.include_apply_patch_tool);
        assert!(config.use_experimental_streamable_shell_tool);
        assert!(config.use_experimental_unified_exec_tool);
        assert!(config.use_experimental_use_rmcp_client);

        Ok(())
    }

    #[test]
    fn config_honors_explicit_file_oauth_store_mode() -> std::io::Result<()> {
        let codex_home = TempDir::new()?;
        let cfg = ConfigToml {
            mcp_oauth_credentials_store: Some(OAuthCredentialsStoreMode::File),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert_eq!(
            config.mcp_oauth_credentials_store_mode,
            OAuthCredentialsStoreMode::File,
        );

        Ok(())
    }

    #[tokio::test]
    async fn managed_config_overrides_oauth_store_mode() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let managed_path = codex_home.path().join("managed_config.toml");
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        std::fs::write(&config_path, "mcp_oauth_credentials_store = \"file\"\n")?;
        std::fs::write(&managed_path, "mcp_oauth_credentials_store = \"keyring\"\n")?;

        let overrides = crate::config_loader::LoaderOverrides {
            managed_config_path: Some(managed_path.clone()),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let root_value = load_resolved_config(codex_home.path(), Vec::new(), overrides).await?;
        let cfg: ConfigToml = root_value.try_into().map_err(|e| {
            tracing::error!("Failed to deserialize overridden config: {e}");
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        assert_eq!(
            cfg.mcp_oauth_credentials_store,
            Some(OAuthCredentialsStoreMode::Keyring),
        );

        let final_config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;
        assert_eq!(
            final_config.mcp_oauth_credentials_store_mode,
            OAuthCredentialsStoreMode::Keyring,
        );

        Ok(())
    }

    #[tokio::test]
    async fn load_global_mcp_servers_returns_empty_if_missing() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = load_global_mcp_servers(codex_home.path()).await?;
        assert!(servers.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_round_trips_entries() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let mut servers = BTreeMap::new();
        servers.insert(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                startup_timeout_sec: Some(Duration::from_secs(3)),
                tool_timeout_sec: Some(Duration::from_secs(5)),
                enabled_tools: None,
                disabled_tools: None,
            },
        );

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        assert_eq!(loaded.len(), 1);
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                assert_eq!(command, "echo");
                assert_eq!(args, &vec!["hello".to_string()]);
                assert!(env.is_none());
                assert!(env_vars.is_empty());
                assert!(cwd.is_none());
            }
            other => panic!("unexpected transport {other:?}"),
        }
        assert_eq!(docs.startup_timeout_sec, Some(Duration::from_secs(3)));
        assert_eq!(docs.tool_timeout_sec, Some(Duration::from_secs(5)));
        assert!(docs.enabled);

        let empty = BTreeMap::new();
        write_global_mcp_servers(codex_home.path(), &empty)?;
        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        assert!(loaded.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn managed_config_wins_over_cli_overrides() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let managed_path = codex_home.path().join("managed_config.toml");

        std::fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            "model = \"base\"\n",
        )?;
        std::fs::write(&managed_path, "model = \"managed_config\"\n")?;

        let overrides = crate::config_loader::LoaderOverrides {
            managed_config_path: Some(managed_path),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let root_value = load_resolved_config(
            codex_home.path(),
            vec![("model".to_string(), TomlValue::String("cli".to_string()))],
            overrides,
        )
        .await?;

        let cfg: ConfigToml = root_value.try_into().map_err(|e| {
            tracing::error!("Failed to deserialize overridden config: {e}");
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        assert_eq!(cfg.model.as_deref(), Some("managed_config"));
        Ok(())
    }

    #[tokio::test]
    async fn load_global_mcp_servers_accepts_legacy_ms_field() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        std::fs::write(
            &config_path,
            r#"
[mcp_servers]
[mcp_servers.docs]
command = "echo"
startup_timeout_ms = 2500
"#,
        )?;

        let servers = load_global_mcp_servers(codex_home.path()).await?;
        let docs = servers.get("docs").expect("docs entry");
        assert_eq!(docs.startup_timeout_sec, Some(Duration::from_millis(2500)));

        Ok(())
    }

    #[tokio::test]
    async fn load_global_mcp_servers_rejects_inline_bearer_token() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        std::fs::write(
            &config_path,
            r#"
[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token = "secret"
"#,
        )?;

        let err = load_global_mcp_servers(codex_home.path())
            .await
            .expect_err("bearer_token entries should be rejected");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("bearer_token"));
        assert!(err.to_string().contains("bearer_token_env_var"));

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_serializes_env_sorted() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "docs-server".to_string(),
                    args: vec!["--verbose".to_string()],
                    env: Some(HashMap::from([
                        ("ZIG_VAR".to_string(), "3".to_string()),
                        ("ALPHA_VAR".to_string(), "1".to_string()),
                    ])),
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert_eq!(
            serialized,
            r#"[mcp_servers.docs]
command = "docs-server"
args = ["--verbose"]

[mcp_servers.docs.env]
ALPHA_VAR = "1"
ZIG_VAR = "3"
"#
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                assert_eq!(command, "docs-server");
                assert_eq!(args, &vec!["--verbose".to_string()]);
                let env = env
                    .as_ref()
                    .expect("env should be preserved for stdio transport");
                assert_eq!(env.get("ALPHA_VAR"), Some(&"1".to_string()));
                assert_eq!(env.get("ZIG_VAR"), Some(&"3".to_string()));
                assert!(env_vars.is_empty());
                assert!(cwd.is_none());
            }
            other => panic!("unexpected transport {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_serializes_env_vars() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "docs-server".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: vec!["ALPHA".to_string(), "BETA".to_string()],
                    cwd: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert!(
            serialized.contains(r#"env_vars = ["ALPHA", "BETA"]"#),
            "serialized config missing env_vars field:\n{serialized}"
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::Stdio { env_vars, .. } => {
                assert_eq!(env_vars, &vec!["ALPHA".to_string(), "BETA".to_string()]);
            }
            other => panic!("unexpected transport {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_serializes_cwd() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let cwd_path = PathBuf::from("/tmp/codex-mcp");
        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "docs-server".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: Some(cwd_path.clone()),
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert!(
            serialized.contains(r#"cwd = "/tmp/codex-mcp""#),
            "serialized config missing cwd field:\n{serialized}"
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::Stdio { cwd, .. } => {
                assert_eq!(cwd.as_deref(), Some(Path::new("/tmp/codex-mcp")));
            }
            other => panic!("unexpected transport {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_streamable_http_serializes_bearer_token() -> anyhow::Result<()>
    {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com/mcp".to_string(),
                    bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: Some(Duration::from_secs(2)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert_eq!(
            serialized,
            r#"[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
startup_timeout_sec = 2.0
"#
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                http_headers,
                env_http_headers,
            } => {
                assert_eq!(url, "https://example.com/mcp");
                assert_eq!(bearer_token_env_var.as_deref(), Some("MCP_TOKEN"));
                assert!(http_headers.is_none());
                assert!(env_http_headers.is_none());
            }
            other => panic!("unexpected transport {other:?}"),
        }
        assert_eq!(docs.startup_timeout_sec, Some(Duration::from_secs(2)));

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_streamable_http_serializes_custom_headers()
    -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com/mcp".to_string(),
                    bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                    http_headers: Some(HashMap::from([("X-Doc".to_string(), "42".to_string())])),
                    env_http_headers: Some(HashMap::from([(
                        "X-Auth".to_string(),
                        "DOCS_AUTH".to_string(),
                    )])),
                },
                enabled: true,
                startup_timeout_sec: Some(Duration::from_secs(2)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);
        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert_eq!(
            serialized,
            r#"[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
startup_timeout_sec = 2.0

[mcp_servers.docs.http_headers]
X-Doc = "42"

[mcp_servers.docs.env_http_headers]
X-Auth = "DOCS_AUTH"
"#
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::StreamableHttp {
                http_headers,
                env_http_headers,
                ..
            } => {
                assert_eq!(
                    http_headers,
                    &Some(HashMap::from([("X-Doc".to_string(), "42".to_string())]))
                );
                assert_eq!(
                    env_http_headers,
                    &Some(HashMap::from([(
                        "X-Auth".to_string(),
                        "DOCS_AUTH".to_string()
                    )]))
                );
            }
            other => panic!("unexpected transport {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_streamable_http_removes_optional_sections()
    -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        let mut servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com/mcp".to_string(),
                    bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                    http_headers: Some(HashMap::from([("X-Doc".to_string(), "42".to_string())])),
                    env_http_headers: Some(HashMap::from([(
                        "X-Auth".to_string(),
                        "DOCS_AUTH".to_string(),
                    )])),
                },
                enabled: true,
                startup_timeout_sec: Some(Duration::from_secs(2)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;
        let serialized_with_optional = std::fs::read_to_string(&config_path)?;
        assert!(serialized_with_optional.contains("bearer_token_env_var = \"MCP_TOKEN\""));
        assert!(serialized_with_optional.contains("[mcp_servers.docs.http_headers]"));
        assert!(serialized_with_optional.contains("[mcp_servers.docs.env_http_headers]"));

        servers.insert(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com/mcp".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        );
        write_global_mcp_servers(codex_home.path(), &servers)?;

        let serialized = std::fs::read_to_string(&config_path)?;
        assert_eq!(
            serialized,
            r#"[mcp_servers.docs]
url = "https://example.com/mcp"
"#
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                http_headers,
                env_http_headers,
            } => {
                assert_eq!(url, "https://example.com/mcp");
                assert!(bearer_token_env_var.is_none());
                assert!(http_headers.is_none());
                assert!(env_http_headers.is_none());
            }
            other => panic!("unexpected transport {other:?}"),
        }

        assert!(docs.startup_timeout_sec.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_streamable_http_isolates_headers_between_servers()
    -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        let servers = BTreeMap::from([
            (
                "docs".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: "https://example.com/mcp".to_string(),
                        bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                        http_headers: Some(HashMap::from([(
                            "X-Doc".to_string(),
                            "42".to_string(),
                        )])),
                        env_http_headers: Some(HashMap::from([(
                            "X-Auth".to_string(),
                            "DOCS_AUTH".to_string(),
                        )])),
                    },
                    enabled: true,
                    startup_timeout_sec: Some(Duration::from_secs(2)),
                    tool_timeout_sec: None,
                    enabled_tools: None,
                    disabled_tools: None,
                },
            ),
            (
                "logs".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: "logs-server".to_string(),
                        args: vec!["--follow".to_string()],
                        env: None,
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    enabled: true,
                    startup_timeout_sec: None,
                    tool_timeout_sec: None,
                    enabled_tools: None,
                    disabled_tools: None,
                },
            ),
        ]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let serialized = std::fs::read_to_string(&config_path)?;
        assert!(
            serialized.contains("[mcp_servers.docs.http_headers]"),
            "serialized config missing docs headers section:\n{serialized}"
        );
        assert!(
            !serialized.contains("[mcp_servers.logs.http_headers]"),
            "serialized config should not add logs headers section:\n{serialized}"
        );
        assert!(
            !serialized.contains("[mcp_servers.logs.env_http_headers]"),
            "serialized config should not add logs env headers section:\n{serialized}"
        );
        assert!(
            !serialized.contains("mcp_servers.logs.bearer_token_env_var"),
            "serialized config should not add bearer token to logs:\n{serialized}"
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        match &docs.transport {
            McpServerTransportConfig::StreamableHttp {
                http_headers,
                env_http_headers,
                ..
            } => {
                assert_eq!(
                    http_headers,
                    &Some(HashMap::from([("X-Doc".to_string(), "42".to_string())]))
                );
                assert_eq!(
                    env_http_headers,
                    &Some(HashMap::from([(
                        "X-Auth".to_string(),
                        "DOCS_AUTH".to_string()
                    )]))
                );
            }
            other => panic!("unexpected transport {other:?}"),
        }
        let logs = loaded.get("logs").expect("logs entry");
        match &logs.transport {
            McpServerTransportConfig::Stdio { env, .. } => {
                assert!(env.is_none());
            }
            other => panic!("unexpected transport {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_serializes_disabled_flag() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "docs-server".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: false,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert!(
            serialized.contains("enabled = false"),
            "serialized config missing disabled flag:\n{serialized}"
        );

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        assert!(!docs.enabled);

        Ok(())
    }

    #[tokio::test]
    async fn write_global_mcp_servers_serializes_tool_filters() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let servers = BTreeMap::from([(
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "docs-server".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: Some(vec!["allowed".to_string()]),
                disabled_tools: Some(vec!["blocked".to_string()]),
            },
        )]);

        write_global_mcp_servers(codex_home.path(), &servers)?;

        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        let serialized = std::fs::read_to_string(&config_path)?;
        assert!(serialized.contains(r#"enabled_tools = ["allowed"]"#));
        assert!(serialized.contains(r#"disabled_tools = ["blocked"]"#));

        let loaded = load_global_mcp_servers(codex_home.path()).await?;
        let docs = loaded.get("docs").expect("docs entry");
        assert_eq!(
            docs.enabled_tools.as_ref(),
            Some(&vec!["allowed".to_string()])
        );
        assert_eq!(
            docs.disabled_tools.as_ref(),
            Some(&vec!["blocked".to_string()])
        );

        Ok(())
    }

    #[tokio::test]
    async fn persist_model_selection_updates_defaults() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        persist_model_selection(
            codex_home.path(),
            None,
            "gpt-5-codex",
            Some(ReasoningEffort::High),
        )
        .await?;

        let serialized =
            tokio::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).await?;
        let parsed: ConfigToml = toml::from_str(&serialized)?;

        assert_eq!(parsed.model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(parsed.model_reasoning_effort, Some(ReasoningEffort::High));

        Ok(())
    }

    #[tokio::test]
    async fn persist_model_selection_overwrites_existing_model() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        tokio::fs::write(
            &config_path,
            r#"
model = "gpt-5-codex"
model_reasoning_effort = "medium"

[profiles.dev]
model = "gpt-4.1"
"#,
        )
        .await?;

        persist_model_selection(
            codex_home.path(),
            None,
            "o4-mini",
            Some(ReasoningEffort::High),
        )
        .await?;

        let serialized = tokio::fs::read_to_string(config_path).await?;
        let parsed: ConfigToml = toml::from_str(&serialized)?;

        assert_eq!(parsed.model.as_deref(), Some("o4-mini"));
        assert_eq!(parsed.model_reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(
            parsed
                .profiles
                .get("dev")
                .and_then(|profile| profile.model.as_deref()),
            Some("gpt-4.1"),
        );

        Ok(())
    }

    #[tokio::test]
    async fn persist_model_selection_updates_profile() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        persist_model_selection(
            codex_home.path(),
            Some("dev"),
            "gpt-5-codex",
            Some(ReasoningEffort::Medium),
        )
        .await?;

        let serialized =
            tokio::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).await?;
        let parsed: ConfigToml = toml::from_str(&serialized)?;
        let profile = parsed
            .profiles
            .get("dev")
            .expect("profile should be created");

        assert_eq!(profile.model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(
            profile.model_reasoning_effort,
            Some(ReasoningEffort::Medium)
        );

        Ok(())
    }

    #[tokio::test]
    async fn persist_model_selection_updates_existing_profile() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);

        tokio::fs::write(
            &config_path,
            r#"
[profiles.dev]
model = "gpt-4"
model_reasoning_effort = "medium"

[profiles.prod]
model = "gpt-5-codex"
"#,
        )
        .await?;

        persist_model_selection(
            codex_home.path(),
            Some("dev"),
            "o4-high",
            Some(ReasoningEffort::Medium),
        )
        .await?;

        let serialized = tokio::fs::read_to_string(config_path).await?;
        let parsed: ConfigToml = toml::from_str(&serialized)?;

        let dev_profile = parsed
            .profiles
            .get("dev")
            .expect("dev profile should survive updates");
        assert_eq!(dev_profile.model.as_deref(), Some("o4-high"));
        assert_eq!(
            dev_profile.model_reasoning_effort,
            Some(ReasoningEffort::Medium)
        );

        assert_eq!(
            parsed
                .profiles
                .get("prod")
                .and_then(|profile| profile.model.as_deref()),
            Some("gpt-5-codex"),
        );

        Ok(())
    }

    struct PrecedenceTestFixture {
        cwd: TempDir,
        codex_home: TempDir,
        cfg: ConfigToml,
        model_provider_map: HashMap<String, ModelProviderInfo>,
        openai_provider: ModelProviderInfo,
        openai_chat_completions_provider: ModelProviderInfo,
    }

    impl PrecedenceTestFixture {
        fn cwd(&self) -> PathBuf {
            self.cwd.path().to_path_buf()
        }

        fn codex_home(&self) -> PathBuf {
            self.codex_home.path().to_path_buf()
        }
    }

    fn create_test_fixture() -> std::io::Result<PrecedenceTestFixture> {
        let toml = r#"
model = "o3"
approval_policy = "untrusted"

# Can be used to determine which profile to use if not specified by
# `ConfigOverrides`.
profile = "gpt3"

[model_providers.openai-chat-completions]
name = "OpenAI using Chat Completions"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"
request_max_retries = 4            # retry failed HTTP requests
stream_max_retries = 10            # retry dropped SSE streams
stream_idle_timeout_ms = 300000    # 5m idle timeout

[profiles.o3]
model = "o3"
model_provider = "openai"
approval_policy = "never"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"

[profiles.gpt3]
model = "gpt-3.5-turbo"
model_provider = "openai-chat-completions"

[profiles.zdr]
model = "o3"
model_provider = "openai"
approval_policy = "on-failure"

[profiles.gpt5]
model = "gpt-5"
model_provider = "openai"
approval_policy = "on-failure"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"
model_verbosity = "high"
"#;

        let cfg: ConfigToml = toml::from_str(toml).expect("TOML deserialization should succeed");

        // Use a temporary directory for the cwd so it does not contain an
        // AGENTS.md file.
        let cwd_temp_dir = TempDir::new().unwrap();
        let cwd = cwd_temp_dir.path().to_path_buf();
        // Make it look like a Git repo so it does not search for AGENTS.md in
        // a parent folder, either.
        std::fs::write(cwd.join(".git"), "gitdir: nowhere")?;

        let codex_home_temp_dir = TempDir::new().unwrap();

        let openai_chat_completions_provider = ModelProviderInfo {
            name: "OpenAI using Chat Completions".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: crate::WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let model_provider_map = {
            let mut model_provider_map = built_in_model_providers();
            model_provider_map.insert(
                "openai-chat-completions".to_string(),
                openai_chat_completions_provider.clone(),
            );
            model_provider_map
        };

        let openai_provider = model_provider_map
            .get("openai")
            .expect("openai provider should exist")
            .clone();

        Ok(PrecedenceTestFixture {
            cwd: cwd_temp_dir,
            codex_home: codex_home_temp_dir,
            cfg,
            model_provider_map,
            openai_provider,
            openai_chat_completions_provider,
        })
    }

    /// Users can specify config values at multiple levels that have the
    /// following precedence:
    ///
    /// 1. custom command-line argument, e.g. `--model o3`
    /// 2. as part of a profile, where the `--profile` is specified via a CLI
    ///    (or in the config file itself)
    /// 3. as an entry in `config.toml`, e.g. `model = "o3"`
    /// 4. the default value for a required field defined in code, e.g.,
    ///    `crate::flags::OPENAI_DEFAULT_MODEL`
    ///
    /// Note that profiles are the recommended way to specify a group of
    /// configuration options together.
    #[test]
    fn test_precedence_fixture_with_o3_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let o3_profile_overrides = ConfigOverrides {
            config_profile: Some("o3".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let o3_profile_config: Config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            o3_profile_overrides,
            fixture.codex_home(),
        )?;
        assert_eq!(
            Config {
                model: "o3".to_string(),
                review_model: OPENAI_DEFAULT_REVIEW_MODEL.to_string(),
                model_family: find_family_for_model("o3").expect("known model slug"),
                model_context_window: Some(200_000),
                model_max_output_tokens: Some(100_000),
                model_auto_compact_token_limit: Some(180_000),
                model_provider_id: "openai".to_string(),
                model_provider: fixture.openai_provider.clone(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                did_user_set_custom_approval_policy_or_sandbox_mode: true,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                user_instructions: None,
                notify: None,
                cwd: fixture.cwd(),
                mcp_servers: HashMap::new(),
                mcp_oauth_credentials_store_mode: Default::default(),
                model_providers: fixture.model_provider_map.clone(),
                project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
                project_doc_fallback_filenames: Vec::new(),
                codex_home: fixture.codex_home(),
                history: History::default(),
                file_opener: UriBasedFileOpener::VsCode,
                codex_linux_sandbox_exe: None,
                hide_agent_reasoning: false,
                show_raw_agent_reasoning: false,
                model_reasoning_effort: Some(ReasoningEffort::High),
                model_reasoning_summary: ReasoningSummary::Detailed,
                model_verbosity: None,
                chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
                base_instructions: None,
                forced_chatgpt_workspace_id: None,
                forced_login_method: None,
                include_plan_tool: false,
                include_apply_patch_tool: false,
                tools_web_search_request: false,
                use_experimental_streamable_shell_tool: false,
                use_experimental_unified_exec_tool: false,
                use_experimental_use_rmcp_client: false,
                include_view_image_tool: true,
                features: Features::with_defaults(),
                active_profile: Some("o3".to_string()),
                active_project: ProjectConfig { trust_level: None },
                windows_wsl_setup_acknowledged: false,
                notices: Default::default(),
                disable_paste_burst: false,
                tui_notifications: Default::default(),
                otel: OtelConfig::default(),
            },
            o3_profile_config
        );
        Ok(())
    }

    #[test]
    fn test_precedence_fixture_with_gpt3_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let gpt3_profile_overrides = ConfigOverrides {
            config_profile: Some("gpt3".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let gpt3_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            gpt3_profile_overrides,
            fixture.codex_home(),
        )?;
        let expected_gpt3_profile_config = Config {
            model: "gpt-3.5-turbo".to_string(),
            review_model: OPENAI_DEFAULT_REVIEW_MODEL.to_string(),
            model_family: find_family_for_model("gpt-3.5-turbo").expect("known model slug"),
            model_context_window: Some(16_385),
            model_max_output_tokens: Some(4_096),
            model_auto_compact_token_limit: Some(14_746),
            model_provider_id: "openai-chat-completions".to_string(),
            model_provider: fixture.openai_chat_completions_provider.clone(),
            approval_policy: AskForApproval::UnlessTrusted,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            did_user_set_custom_approval_policy_or_sandbox_mode: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            mcp_oauth_credentials_store_mode: Default::default(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            project_doc_fallback_filenames: Vec::new(),
            codex_home: fixture.codex_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            codex_linux_sandbox_exe: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: None,
            model_reasoning_summary: ReasoningSummary::default(),
            model_verbosity: None,
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            base_instructions: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            use_experimental_streamable_shell_tool: false,
            use_experimental_unified_exec_tool: false,
            use_experimental_use_rmcp_client: false,
            include_view_image_tool: true,
            features: Features::with_defaults(),
            active_profile: Some("gpt3".to_string()),
            active_project: ProjectConfig { trust_level: None },
            windows_wsl_setup_acknowledged: false,
            notices: Default::default(),
            disable_paste_burst: false,
            tui_notifications: Default::default(),
            otel: OtelConfig::default(),
        };

        assert_eq!(expected_gpt3_profile_config, gpt3_profile_config);

        // Verify that loading without specifying a profile in ConfigOverrides
        // uses the default profile from the config file (which is "gpt3").
        let default_profile_overrides = ConfigOverrides {
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };

        let default_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            default_profile_overrides,
            fixture.codex_home(),
        )?;

        assert_eq!(expected_gpt3_profile_config, default_profile_config);
        Ok(())
    }

    #[test]
    fn test_precedence_fixture_with_zdr_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let zdr_profile_overrides = ConfigOverrides {
            config_profile: Some("zdr".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let zdr_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            zdr_profile_overrides,
            fixture.codex_home(),
        )?;
        let expected_zdr_profile_config = Config {
            model: "o3".to_string(),
            review_model: OPENAI_DEFAULT_REVIEW_MODEL.to_string(),
            model_family: find_family_for_model("o3").expect("known model slug"),
            model_context_window: Some(200_000),
            model_max_output_tokens: Some(100_000),
            model_auto_compact_token_limit: Some(180_000),
            model_provider_id: "openai".to_string(),
            model_provider: fixture.openai_provider.clone(),
            approval_policy: AskForApproval::OnFailure,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            did_user_set_custom_approval_policy_or_sandbox_mode: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            mcp_oauth_credentials_store_mode: Default::default(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            project_doc_fallback_filenames: Vec::new(),
            codex_home: fixture.codex_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            codex_linux_sandbox_exe: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: None,
            model_reasoning_summary: ReasoningSummary::default(),
            model_verbosity: None,
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            base_instructions: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            use_experimental_streamable_shell_tool: false,
            use_experimental_unified_exec_tool: false,
            use_experimental_use_rmcp_client: false,
            include_view_image_tool: true,
            features: Features::with_defaults(),
            active_profile: Some("zdr".to_string()),
            active_project: ProjectConfig { trust_level: None },
            windows_wsl_setup_acknowledged: false,
            notices: Default::default(),
            disable_paste_burst: false,
            tui_notifications: Default::default(),
            otel: OtelConfig::default(),
        };

        assert_eq!(expected_zdr_profile_config, zdr_profile_config);

        Ok(())
    }

    #[test]
    fn test_precedence_fixture_with_gpt5_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let gpt5_profile_overrides = ConfigOverrides {
            config_profile: Some("gpt5".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let gpt5_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            gpt5_profile_overrides,
            fixture.codex_home(),
        )?;
        let expected_gpt5_profile_config = Config {
            model: "gpt-5".to_string(),
            review_model: OPENAI_DEFAULT_REVIEW_MODEL.to_string(),
            model_family: find_family_for_model("gpt-5").expect("known model slug"),
            model_context_window: Some(272_000),
            model_max_output_tokens: Some(128_000),
            model_auto_compact_token_limit: Some(244_800),
            model_provider_id: "openai".to_string(),
            model_provider: fixture.openai_provider.clone(),
            approval_policy: AskForApproval::OnFailure,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            did_user_set_custom_approval_policy_or_sandbox_mode: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            mcp_oauth_credentials_store_mode: Default::default(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            project_doc_fallback_filenames: Vec::new(),
            codex_home: fixture.codex_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            codex_linux_sandbox_exe: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: Some(ReasoningEffort::High),
            model_reasoning_summary: ReasoningSummary::Detailed,
            model_verbosity: Some(Verbosity::High),
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            base_instructions: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            use_experimental_streamable_shell_tool: false,
            use_experimental_unified_exec_tool: false,
            use_experimental_use_rmcp_client: false,
            include_view_image_tool: true,
            features: Features::with_defaults(),
            active_profile: Some("gpt5".to_string()),
            active_project: ProjectConfig { trust_level: None },
            windows_wsl_setup_acknowledged: false,
            notices: Default::default(),
            disable_paste_burst: false,
            tui_notifications: Default::default(),
            otel: OtelConfig::default(),
        };

        assert_eq!(expected_gpt5_profile_config, gpt5_profile_config);

        Ok(())
    }

    #[test]
    fn test_did_user_set_custom_approval_policy_or_sandbox_mode_defaults_no() -> anyhow::Result<()>
    {
        let fixture = create_test_fixture()?;

        let config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            ConfigOverrides {
                ..Default::default()
            },
            fixture.codex_home(),
        )?;

        assert!(config.did_user_set_custom_approval_policy_or_sandbox_mode);

        Ok(())
    }

    #[test]
    fn test_set_project_trusted_writes_explicit_tables() -> anyhow::Result<()> {
        let project_dir = Path::new("/some/path");
        let mut doc = DocumentMut::new();

        set_project_trusted_inner(&mut doc, project_dir)?;

        let contents = doc.to_string();

        let raw_path = project_dir.to_string_lossy();
        let path_str = if raw_path.contains('\\') {
            format!("'{raw_path}'")
        } else {
            format!("\"{raw_path}\"")
        };
        let expected = format!(
            r#"[projects.{path_str}]
trust_level = "trusted"
"#
        );
        assert_eq!(contents, expected);

        Ok(())
    }

    #[test]
    fn test_set_project_trusted_converts_inline_to_explicit() -> anyhow::Result<()> {
        let project_dir = Path::new("/some/path");

        // Seed config.toml with an inline project entry under [projects]
        let raw_path = project_dir.to_string_lossy();
        let path_str = if raw_path.contains('\\') {
            format!("'{raw_path}'")
        } else {
            format!("\"{raw_path}\"")
        };
        // Use a quoted key so backslashes don't require escaping on Windows
        let initial = format!(
            r#"[projects]
{path_str} = {{ trust_level = "untrusted" }}
"#
        );
        let mut doc = initial.parse::<DocumentMut>()?;

        // Run the function; it should convert to explicit tables and set trusted
        set_project_trusted_inner(&mut doc, project_dir)?;

        let contents = doc.to_string();

        // Assert exact output after conversion to explicit table
        let expected = format!(
            r#"[projects]

[projects.{path_str}]
trust_level = "trusted"
"#
        );
        assert_eq!(contents, expected);

        Ok(())
    }

    #[test]
    fn test_set_project_trusted_migrates_top_level_inline_projects_preserving_entries()
    -> anyhow::Result<()> {
        let initial = r#"toplevel = "baz"
projects = { "/Users/mbolin/code/codex4" = { trust_level = "trusted", foo = "bar" } , "/Users/mbolin/code/codex3" = { trust_level = "trusted" } }
model = "foo""#;
        let mut doc = initial.parse::<DocumentMut>()?;

        // Approve a new directory
        let new_project = Path::new("/Users/mbolin/code/codex2");
        set_project_trusted_inner(&mut doc, new_project)?;

        let contents = doc.to_string();

        // Since we created the [projects] table as part of migration, it is kept implicit.
        // Expect explicit per-project tables, preserving prior entries and appending the new one.
        let expected = r#"toplevel = "baz"
model = "foo"

[projects."/Users/mbolin/code/codex4"]
trust_level = "trusted"
foo = "bar"

[projects."/Users/mbolin/code/codex3"]
trust_level = "trusted"

[projects."/Users/mbolin/code/codex2"]
trust_level = "trusted"
"#;
        assert_eq!(contents, expected);

        Ok(())
    }
}

#[cfg(test)]
mod notifications_tests {
    use crate::config_types::Notifications;
    use assert_matches::assert_matches;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct TuiTomlTest {
        notifications: Notifications,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct RootTomlTest {
        tui: TuiTomlTest,
    }

    #[test]
    fn test_tui_notifications_true() {
        let toml = r#"
            [tui]
            notifications = true
        "#;
        let parsed: RootTomlTest = toml::from_str(toml).expect("deserialize notifications=true");
        assert_matches!(parsed.tui.notifications, Notifications::Enabled(true));
    }

    #[test]
    fn test_tui_notifications_custom_array() {
        let toml = r#"
            [tui]
            notifications = ["foo"]
        "#;
        let parsed: RootTomlTest =
            toml::from_str(toml).expect("deserialize notifications=[\"foo\"]");
        assert_matches!(
            parsed.tui.notifications,
            Notifications::Custom(ref v) if v == &vec!["foo".to_string()]
        );
    }
}
