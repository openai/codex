use crate::config_profile::ConfigProfile;
use crate::flags::OPENAI_DEFAULT_MODEL;
use crate::mcp_server_config::McpServerConfig;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::built_in_model_providers;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPermission;
use crate::protocol::SandboxPolicy;
use dirs::home_dir;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Maximum number of bytes of the documentation that will be embedded. Larger
/// files are *silently truncated* to this size so we do not take up too much of
/// the context window.
pub(crate) const PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32 KiB

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Optional override of model selection.
    pub model: String,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Approval policy for executing commands.
    pub approval_policy: AskForApproval,

    pub sandbox_policy: SandboxPolicy,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: bool,

    /// User-provided instructions from instructions.md.
    pub instructions: Option<String>,

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

    /// The directory that should be treated as the current working directory
    /// for the session. All relative paths inside the business-logic layer are
    /// resolved against this path.
    pub cwd: PathBuf,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Combined provider map (defaults merged with user-defined overrides).
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: usize,

    /// Directory containing all Codex state (defaults to `~/.codex` but can be
    /// overridden by the `CODEX_HOME` environment variable).
    pub codex_home: PathBuf,

    /// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
    pub history: History,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: UriBasedFileOpener,

    /// Collection of settings that are specific to the TUI.
    pub tui: Tui,
}

/// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct History {
    /// If true, history entries will not be written to disk.
    pub persistence: HistoryPersistence,

    /// If set, the maximum size of the history file in bytes.
    /// TODO(mbolin): Not currently honored.
    pub max_bytes: Option<usize>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    /// Save all history entries to disk.
    #[default]
    SaveAll,
    /// Do not write history to disk.
    None,
}

/// Collection of settings that are specific to the TUI.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Tui {
    /// By default, mouse capture is enabled in the TUI so that it is possible
    /// to scroll the conversation history with a mouse. This comes at the cost
    /// of not being able to use the mouse to select text in the TUI.
    /// (Most terminals support a modifier key to allow this. For example,
    /// text selection works in iTerm if you hold down the `Option` key while
    /// clicking and dragging.)
    ///
    /// Setting this option to `true` disables mouse capture, so scrolling with
    /// the mouse is not possible, though the keyboard shortcuts e.g. `b` and
    /// `space` still work. This allows the user to select text in the TUI
    /// using the mouse without needing to hold down a modifier key.
    pub disable_mouse_capture: bool,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum UriBasedFileOpener {
    #[serde(rename = "vscode")]
    VsCode,

    #[serde(rename = "vscode-insiders")]
    VsCodeInsiders,

    #[serde(rename = "windsurf")]
    Windsurf,

    #[serde(rename = "cursor")]
    Cursor,

    /// Option to disable the URI-based file opener.
    #[serde(rename = "none")]
    None,
}

impl UriBasedFileOpener {
    pub fn get_scheme(&self) -> Option<&str> {
        match self {
            UriBasedFileOpener::VsCode => Some("vscode"),
            UriBasedFileOpener::VsCodeInsiders => Some("vscode-insiders"),
            UriBasedFileOpener::Windsurf => Some("windsurf"),
            UriBasedFileOpener::Cursor => Some("cursor"),
            UriBasedFileOpener::None => None,
        }
    }
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConfigToml {
    /// Optional override of model selection.
    pub model: Option<String>,

    /// Provider to use from the model_providers map.
    pub model_provider: Option<String>,

    /// Default approval policy for executing commands.
    pub approval_policy: Option<AskForApproval>,

    // The `default` attribute ensures that the field is treated as `None` when
    // the key is omitted from the TOML. Without it, Serde treats the field as
    // required because we supply a custom deserializer.
    #[serde(default, deserialize_with = "deserialize_sandbox_permissions")]
    pub sandbox_permissions: Option<Vec<SandboxPermission>>,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: Option<bool>,

    /// Optional external command to spawn for end-user notifications.
    #[serde(default)]
    pub notify: Option<Vec<String>>,

    /// System instructions.
    pub instructions: Option<String>,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// User-defined provider entries that extend/override the built-in list.
    #[serde(default)]
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: Option<usize>,

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
}

impl ConfigToml {
    /// Attempt to parse the file at `~/.codex/config.toml`. If it does not
    /// exist, return a default config. Though if it exists and cannot be
    /// parsed, report that to the user and force them to fix it.
    fn load_from_toml(codex_home: &Path) -> std::io::Result<Self> {
        let config_toml_path = codex_home.join("config.toml");
        match std::fs::read_to_string(&config_toml_path) {
            Ok(contents) => toml::from_str::<Self>(&contents).map_err(|e| {
                tracing::error!("Failed to parse config.toml: {e}");
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("config.toml not found, using defaults");
                Ok(Self::default())
            }
            Err(e) => {
                tracing::error!("Failed to read config.toml: {e}");
                Err(e)
            }
        }
    }
}

fn deserialize_sandbox_permissions<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<SandboxPermission>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let permissions: Option<Vec<String>> = Option::deserialize(deserializer)?;

    match permissions {
        Some(raw_permissions) => {
            let base_path = find_codex_home().map_err(serde::de::Error::custom)?;

            let converted = raw_permissions
                .into_iter()
                .map(|raw| {
                    parse_sandbox_permission_with_base_path(&raw, base_path.clone())
                        .map_err(serde::de::Error::custom)
                })
                .collect::<Result<Vec<_>, D::Error>>()?;

            Ok(Some(converted))
        }
        None => Ok(None),
    }
}

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub disable_response_storage: Option<bool>,
    pub model_provider: Option<String>,
    pub config_profile: Option<String>,
}

impl Config {
    /// Load configuration, optionally applying overrides (CLI flags). Merges
    /// ~/.codex/config.toml, ~/.codex/instructions.md, embedded defaults, and
    /// any values provided in `overrides` (highest precedence).
    pub fn load_with_overrides(overrides: ConfigOverrides) -> std::io::Result<Self> {
        // Resolve the directory that stores Codex state (e.g. ~/.codex or the
        // value of $CODEX_HOME) so we can embed it into the resulting
        // `Config` instance.
        let codex_home = find_codex_home()?;

        let cfg: ConfigToml = ConfigToml::load_from_toml(&codex_home)?;
        tracing::warn!("Config parsed from config.toml: {cfg:?}");

        Self::load_from_base_config_with_overrides(cfg, overrides, codex_home)
    }

    /// Meant to be used exclusively for tests: `load_with_overrides()` should
    /// be used in all other cases.
    pub fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        codex_home: PathBuf,
    ) -> std::io::Result<Self> {
        let instructions = Self::load_instructions(Some(&codex_home));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            cwd,
            approval_policy,
            sandbox_policy,
            disable_response_storage,
            model_provider,
            config_profile: config_profile_key,
        } = overrides;

        let config_profile = match config_profile_key.or(cfg.profile) {
            Some(key) => cfg
                .profiles
                .get(&key)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("config profile `{key}` not found"),
                    )
                })?
                .clone(),
            None => ConfigProfile::default(),
        };

        let sandbox_policy = match sandbox_policy {
            Some(sandbox_policy) => sandbox_policy,
            None => {
                // Derive a SandboxPolicy from the permissions in the config.
                match cfg.sandbox_permissions {
                    // Note this means the user can explicitly set permissions
                    // to the empty list in the config file, granting it no
                    // permissions whatsoever.
                    Some(permissions) => SandboxPolicy::from(permissions),
                    // Default to read only rather than completely locked down.
                    None => SandboxPolicy::new_read_only_policy(),
                }
            }
        };

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

        let history = cfg.history.unwrap_or_default();

        let config = Self {
            model: model
                .or(config_profile.model)
                .or(cfg.model)
                .unwrap_or_else(default_model),
            model_provider_id,
            model_provider,
            cwd: resolved_cwd,
            approval_policy: approval_policy
                .or(config_profile.approval_policy)
                .or(cfg.approval_policy)
                .unwrap_or_else(AskForApproval::default),
            sandbox_policy,
            disable_response_storage: disable_response_storage
                .or(config_profile.disable_response_storage)
                .or(cfg.disable_response_storage)
                .unwrap_or(false),
            notify: cfg.notify,
            instructions,
            mcp_servers: cfg.mcp_servers,
            model_providers,
            project_doc_max_bytes: cfg.project_doc_max_bytes.unwrap_or(PROJECT_DOC_MAX_BYTES),
            codex_home,
            history,
            file_opener: cfg.file_opener.unwrap_or(UriBasedFileOpener::VsCode),
            tui: cfg.tui.unwrap_or_default(),
        };
        Ok(config)
    }

    fn load_instructions(codex_dir: Option<&Path>) -> Option<String> {
        let mut p = match codex_dir {
            Some(p) => p.to_path_buf(),
            None => return None,
        };

        p.push("instructions.md");
        std::fs::read_to_string(&p).ok().and_then(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
    }
}

fn default_model() -> String {
    OPENAI_DEFAULT_MODEL.to_string()
}

/// Returns the path to the Codex configuration directory, which can be
/// specified by the `CODEX_HOME` environment variable. If not set, defaults to
/// `~/.codex`.
///
/// - If `CODEX_HOME` is set, the value will be canonicalized and this
///   function will Err if the path does not exist.
/// - If `CODEX_HOME` is not set, this function does not verify that the
///   directory exists.
fn find_codex_home() -> std::io::Result<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val).canonicalize();
        }
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

pub fn parse_sandbox_permission_with_base_path(
    raw: &str,
    base_path: PathBuf,
) -> std::io::Result<SandboxPermission> {
    use SandboxPermission::*;

    if let Some(path) = raw.strip_prefix("disk-write-folder=") {
        return if path.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--sandbox-permission disk-write-folder=<PATH> requires a non-empty PATH",
            ))
        } else {
            use path_absolutize::*;

            let file = PathBuf::from(path);
            let absolute_path = if file.is_relative() {
                file.absolutize_from(base_path)
            } else {
                file.absolutize()
            }
            .map(|path| path.into_owned())?;
            Ok(DiskWriteFolder {
                folder: absolute_path,
            })
        };
    }

    match raw {
        "disk-full-read-access" => Ok(DiskFullReadAccess),
        "disk-write-platform-user-temp-folder" => Ok(DiskWritePlatformUserTempFolder),
        "disk-write-platform-global-temp-folder" => Ok(DiskWritePlatformGlobalTempFolder),
        "disk-write-cwd" => Ok(DiskWriteCwd),
        "disk-full-write-access" => Ok(DiskFullWriteAccess),
        "network-full-access" => Ok(NetworkFullAccess),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "`{raw}` is not a recognised permission.\nRun with `--help` to see the accepted values."
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    /// Verify that the `sandbox_permissions` field on `ConfigToml` correctly
    /// differentiates between a value that is completely absent in the
    /// provided TOML (i.e. `None`) and one that is explicitly specified as an
    /// empty array (i.e. `Some(vec![])`). This ensures that downstream logic
    /// that treats these two cases differently (default read-only policy vs a
    /// fully locked-down sandbox) continues to function.
    #[test]
    fn test_sandbox_permissions_none_vs_empty_vec() {
        // Case 1: `sandbox_permissions` key is *absent* from the TOML source.
        let toml_source_without_key = "";
        let cfg_without_key: ConfigToml = toml::from_str(toml_source_without_key)
            .expect("TOML deserialization without key should succeed");
        assert!(cfg_without_key.sandbox_permissions.is_none());

        // Case 2: `sandbox_permissions` is present but set to an *empty array*.
        let toml_source_with_empty = "sandbox_permissions = []";
        let cfg_with_empty: ConfigToml = toml::from_str(toml_source_with_empty)
            .expect("TOML deserialization with empty array should succeed");
        assert_eq!(Some(vec![]), cfg_with_empty.sandbox_permissions);

        // Case 3: `sandbox_permissions` contains a non-empty list of valid values.
        let toml_source_with_values = r#"
            sandbox_permissions = ["disk-full-read-access", "network-full-access"]
        "#;
        let cfg_with_values: ConfigToml = toml::from_str(toml_source_with_values)
            .expect("TOML deserialization with valid permissions should succeed");

        assert_eq!(
            Some(vec![
                SandboxPermission::DiskFullReadAccess,
                SandboxPermission::NetworkFullAccess
            ]),
            cfg_with_values.sandbox_permissions
        );
    }

    #[test]
    fn test_toml_parsing() {
        let history_with_persistence = r#"
[history]
persistence = "save-all"
"#;
        let history_with_persistence_cfg: ConfigToml =
            toml::from_str::<ConfigToml>(history_with_persistence)
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

        let history_no_persistence_cfg: ConfigToml =
            toml::from_str::<ConfigToml>(history_no_persistence)
                .expect("TOML deserialization should succeed");
        assert_eq!(
            Some(History {
                persistence: HistoryPersistence::None,
                max_bytes: None,
            }),
            history_no_persistence_cfg.history
        );
    }

    /// Deserializing a TOML string containing an *invalid* permission should
    /// fail with a helpful error rather than silently defaulting or
    /// succeeding.
    #[test]
    fn test_sandbox_permissions_illegal_value() {
        let toml_bad = r#"sandbox_permissions = ["not-a-real-permission"]"#;

        let err = toml::from_str::<ConfigToml>(toml_bad)
            .expect_err("Deserialization should fail for invalid permission");

        // Make sure the error message contains the invalid value so users have
        // useful feedback.
        let msg = err.to_string();
        assert!(msg.contains("not-a-real-permission"));
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
approval_policy = "unless-allow-listed"
sandbox_permissions = ["disk-full-read-access"]
disable_response_storage = false

# Can be used to determine which profile to use if not specified by
# `ConfigOverrides`.
profile = "gpt3"

[model_providers.openai-chat-completions]
name = "OpenAI using Chat Completions"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"

[profiles.o3]
model = "o3"
model_provider = "openai"
approval_policy = "never"

[profiles.gpt3]
model = "gpt-3.5-turbo"
model_provider = "openai-chat-completions"

[profiles.zdr]
model = "o3"
model_provider = "openai"
approval_policy = "on-failure"
disable_response_storage = true
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
            base_url: "https://api.openai.com/v1".to_string(),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: crate::WireApi::Chat,
            env_key_instructions: None,
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
    ///    (or in the config file itelf)
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
                model_provider_id: "openai".to_string(),
                model_provider: fixture.openai_provider.clone(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                disable_response_storage: false,
                instructions: None,
                notify: None,
                cwd: fixture.cwd(),
                mcp_servers: HashMap::new(),
                model_providers: fixture.model_provider_map.clone(),
                project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
                codex_home: fixture.codex_home(),
                history: History::default(),
                file_opener: UriBasedFileOpener::VsCode,
                tui: Tui::default(),
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
            model_provider_id: "openai-chat-completions".to_string(),
            model_provider: fixture.openai_chat_completions_provider.clone(),
            approval_policy: AskForApproval::UnlessAllowListed,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            disable_response_storage: false,
            instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            codex_home: fixture.codex_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            tui: Tui::default(),
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
            model_provider_id: "openai".to_string(),
            model_provider: fixture.openai_provider.clone(),
            approval_policy: AskForApproval::OnFailure,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            disable_response_storage: true,
            instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            codex_home: fixture.codex_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            tui: Tui::default(),
        };

        assert_eq!(expected_zdr_profile_config, zdr_profile_config);

        Ok(())
    }
}
