//! Complete runtime configuration snapshot for building an agent.
//!
//! This module provides the `Config` struct which is a complete runtime snapshot
//! containing all resolved configuration needed to build and run an agent.
//!
//! ## Relationship with other types
//!
//! - `AppConfig`: JSON file format, supports profile switching
//! - `ConfigManager`: Loading, caching, runtime switching
//! - `Config`: Complete runtime snapshot with resolved values
//!
//! ## Usage
//!
//! ```no_run
//! use cocode_config::{ConfigManager, ConfigOverrides};
//!
//! # fn example() -> Result<(), cocode_config::error::ConfigError> {
//! let manager = ConfigManager::from_default()?;
//! let config = manager.build_config(ConfigOverrides::default())?;
//!
//! // Access main model
//! if let Some(main) = config.main_model_info() {
//!     println!("Main model: {} ({:?})", main.display_name_or_slug(), main.context_window);
//! }
//!
//! // Access role-specific model
//! use cocode_protocol::model::ModelRole;
//! if let Some(fast) = config.model_for_role(ModelRole::Fast) {
//!     println!("Fast model: {}", fast.display_name_or_slug());
//! }
//! # Ok(())
//! # }
//! ```

use crate::json_config::LoggingConfig;
use cocode_protocol::AttachmentConfig;
use cocode_protocol::CompactConfig;
use cocode_protocol::Features;
use cocode_protocol::ModelInfo;
use cocode_protocol::PathConfig;
use cocode_protocol::PlanModeConfig;
use cocode_protocol::ProviderInfo;
use cocode_protocol::SandboxMode;
use cocode_protocol::ToolConfig;
use cocode_protocol::model::ModelRole;
use cocode_protocol::model::ModelRoles;
use cocode_protocol::model::ModelSpec;
use std::collections::HashMap;
use std::path::PathBuf;

/// Complete runtime configuration snapshot for building an agent.
///
/// This struct contains all the resolved configuration needed to build
/// and run an agent. It is created from `ConfigManager::build_config()`.
///
/// ## ModelRoles support
///
/// Supports all 6 roles (Main, Fast, Vision, Review, Plan, Explore).
/// Use `model_for_role()` to get resolved info for a specific role.
///
/// ## Fields
///
/// The Config struct is organized into logical sections:
/// - **Model & Provider**: Complete ModelRoles support and resolved providers
/// - **Paths**: Working directory and cocode home
/// - **Instructions**: User instructions from AGENTS.md
/// - **Features**: Centralized feature flags
/// - **Session**: Logging and profile settings
/// - **Sandbox**: Filesystem access control
#[derive(Debug, Clone)]
pub struct Config {
    // ============================================================
    // 1. Model & Provider
    // ============================================================
    /// Role-based model configuration (all 6 roles).
    pub models: ModelRoles,

    /// All available providers (resolved with API keys).
    pub providers: HashMap<String, ProviderInfo>,

    /// Cached resolved model info for each configured role.
    pub(crate) resolved_models: HashMap<ModelRole, ModelInfo>,

    // ============================================================
    // 2. Paths
    // ============================================================
    /// Current working directory for the session.
    pub cwd: PathBuf,

    /// Cocode home directory (default: ~/.cocode).
    pub cocode_home: PathBuf,

    // ============================================================
    // 3. Instructions
    // ============================================================
    /// User instructions from AGENTS.md.
    pub user_instructions: Option<String>,

    // ============================================================
    // 4. Features
    // ============================================================
    /// Centralized feature flags (resolved).
    pub features: Features,

    // ============================================================
    // 5. Session
    // ============================================================
    /// Logging configuration.
    pub logging: Option<LoggingConfig>,

    /// Active profile name.
    pub active_profile: Option<String>,

    /// Session is ephemeral (not persisted).
    pub ephemeral: bool,

    // ============================================================
    // 6. Sandbox
    // ============================================================
    /// Sandbox mode for filesystem access.
    pub sandbox_mode: SandboxMode,

    /// Writable roots for sandbox (when WorkspaceWrite).
    pub writable_roots: Vec<PathBuf>,

    // ============================================================
    // 7. Tool Execution
    // ============================================================
    /// Tool execution configuration.
    pub tool_config: ToolConfig,

    // ============================================================
    // 8. Compaction
    // ============================================================
    /// Compaction and session memory configuration.
    pub compact_config: CompactConfig,

    // ============================================================
    // 9. Plan Mode
    // ============================================================
    /// Plan mode configuration.
    pub plan_config: PlanModeConfig,

    // ============================================================
    // 11. Attachments
    // ============================================================
    /// Attachment configuration.
    pub attachment_config: AttachmentConfig,

    // ============================================================
    // 12. Extended Paths
    // ============================================================
    /// Extended path configuration.
    pub path_config: PathConfig,
}

impl Config {
    /// Get resolved model info for a specific role.
    ///
    /// Falls back to Main role if the specific role is not configured.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cocode_config::{ConfigManager, ConfigOverrides};
    /// use cocode_protocol::model::ModelRole;
    ///
    /// # fn example() -> Result<(), cocode_config::error::ConfigError> {
    /// let manager = ConfigManager::from_default()?;
    /// let config = manager.build_config(ConfigOverrides::default())?;
    ///
    /// // Get fast model (falls back to main if not configured)
    /// if let Some(fast) = config.model_for_role(ModelRole::Fast) {
    ///     println!("Using model: {}", fast.display_name_or_slug());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn model_for_role(&self, role: ModelRole) -> Option<&ModelInfo> {
        self.resolved_models
            .get(&role)
            .or_else(|| self.resolved_models.get(&ModelRole::Main))
    }

    /// Get provider info for a specific role.
    ///
    /// Looks up the provider based on the model spec for the given role.
    pub fn provider_for_role(&self, role: ModelRole) -> Option<&ProviderInfo> {
        let spec = self.models.get(role)?;
        self.providers.get(&spec.provider)
    }

    /// Get the main model spec.
    pub fn main_model(&self) -> Option<&ModelSpec> {
        self.models.main()
    }

    /// Get resolved info for main model.
    pub fn main_model_info(&self) -> Option<&ModelInfo> {
        self.model_for_role(ModelRole::Main)
    }

    /// Get model spec for a role (with fallback to main).
    pub fn model_spec_for_role(&self, role: ModelRole) -> Option<&ModelSpec> {
        self.models.get(role)
    }

    /// Get provider info by name.
    pub fn provider(&self, name: &str) -> Option<&ProviderInfo> {
        self.providers.get(name)
    }

    /// Get all configured role-model pairs.
    pub fn configured_roles(&self) -> Vec<(ModelRole, &ModelInfo)> {
        self.resolved_models
            .iter()
            .map(|(role, info)| (*role, info))
            .collect()
    }

    /// Check if a specific feature is enabled.
    pub fn is_feature_enabled(&self, feature: cocode_protocol::Feature) -> bool {
        self.features.enabled(feature)
    }

    /// Check if sandbox allows write operations.
    pub fn allows_write(&self) -> bool {
        self.sandbox_mode.allows_write()
    }

    /// Check if a path is writable under current sandbox mode.
    pub fn is_path_writable(&self, path: &std::path::Path) -> bool {
        match self.sandbox_mode {
            SandboxMode::ReadOnly => false,
            SandboxMode::FullAccess => true,
            SandboxMode::WorkspaceWrite => {
                // Check if path is under any writable root
                self.writable_roots
                    .iter()
                    .any(|root| path.starts_with(root))
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            models: ModelRoles::default(),
            providers: HashMap::new(),
            resolved_models: HashMap::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cocode_home: crate::loader::default_config_dir(),
            user_instructions: None,
            features: Features::with_defaults(),
            logging: None,
            active_profile: None,
            ephemeral: false,
            sandbox_mode: SandboxMode::default(),
            writable_roots: Vec::new(),
            tool_config: ToolConfig::default(),
            compact_config: CompactConfig::default(),
            plan_config: PlanModeConfig::default(),
            attachment_config: AttachmentConfig::default(),
            path_config: PathConfig::default(),
        }
    }
}

/// Configuration overrides for building a Config.
///
/// These overrides are applied on top of the resolved configuration
/// from ConfigManager.
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Override for specific roles.
    pub models: Option<ModelRoles>,

    /// Override working directory.
    pub cwd: Option<PathBuf>,

    /// Override sandbox mode.
    pub sandbox_mode: Option<SandboxMode>,

    /// Override ephemeral flag.
    pub ephemeral: Option<bool>,

    /// Feature overrides (key -> enabled).
    pub features: HashMap<String, bool>,

    /// Writable roots for sandbox.
    pub writable_roots: Option<Vec<PathBuf>>,

    /// Override user instructions.
    pub user_instructions: Option<String>,

    /// Override tool configuration.
    pub tool_config: Option<ToolConfig>,

    /// Override compaction configuration.
    pub compact_config: Option<CompactConfig>,

    /// Override plan mode configuration.
    pub plan_config: Option<PlanModeConfig>,

    /// Override attachment configuration.
    pub attachment_config: Option<AttachmentConfig>,

    /// Override path configuration.
    pub path_config: Option<PathConfig>,
}

impl ConfigOverrides {
    /// Create new empty overrides.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set model overrides.
    pub fn with_models(mut self, models: ModelRoles) -> Self {
        self.models = Some(models);
        self
    }

    /// Set working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set sandbox mode.
    pub fn with_sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = Some(mode);
        self
    }

    /// Set ephemeral flag.
    pub fn with_ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = Some(ephemeral);
        self
    }

    /// Add a feature override.
    pub fn with_feature(mut self, key: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(key.into(), enabled);
        self
    }

    /// Set writable roots.
    pub fn with_writable_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.writable_roots = Some(roots);
        self
    }

    /// Set user instructions.
    pub fn with_user_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.user_instructions = Some(instructions.into());
        self
    }

    /// Set tool configuration.
    pub fn with_tool_config(mut self, config: ToolConfig) -> Self {
        self.tool_config = Some(config);
        self
    }

    /// Set compaction configuration.
    pub fn with_compact_config(mut self, config: CompactConfig) -> Self {
        self.compact_config = Some(config);
        self
    }

    /// Set plan mode configuration.
    pub fn with_plan_config(mut self, config: PlanModeConfig) -> Self {
        self.plan_config = Some(config);
        self
    }

    /// Set attachment configuration.
    pub fn with_attachment_config(mut self, config: AttachmentConfig) -> Self {
        self.attachment_config = Some(config);
        self
    }

    /// Set path configuration.
    pub fn with_path_config(mut self, config: PathConfig) -> Self {
        self.path_config = Some(config);
        self
    }
}

/// Builder for creating Config instances.
///
/// Use this builder when you need fine-grained control over configuration
/// loading and resolution.
///
/// # Example
///
/// ```no_run
/// use cocode_config::ConfigBuilder;
/// use cocode_protocol::SandboxMode;
///
/// # fn example() -> Result<(), cocode_config::error::ConfigError> {
/// let config = ConfigBuilder::new()
///     .cwd("/my/project")
///     .sandbox_mode(SandboxMode::WorkspaceWrite)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    cocode_home: Option<PathBuf>,
    cwd: Option<PathBuf>,
    profile: Option<String>,
    overrides: ConfigOverrides,
}

impl ConfigBuilder {
    /// Create a new ConfigBuilder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the cocode home directory.
    ///
    /// If not set, uses `COCODE_HOME` environment variable or `~/.cocode`.
    pub fn cocode_home(mut self, path: impl Into<PathBuf>) -> Self {
        self.cocode_home = Some(path.into());
        self
    }

    /// Set the current working directory.
    ///
    /// If not set, uses the current process working directory.
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Set the profile to use.
    ///
    /// This overrides the profile setting in config.json.
    pub fn profile(mut self, name: impl Into<String>) -> Self {
        self.profile = Some(name.into());
        self
    }

    /// Set configuration overrides.
    pub fn overrides(mut self, overrides: ConfigOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// Set sandbox mode.
    pub fn sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.overrides.sandbox_mode = Some(mode);
        self
    }

    /// Set ephemeral mode.
    pub fn ephemeral(mut self, ephemeral: bool) -> Self {
        self.overrides.ephemeral = Some(ephemeral);
        self
    }

    /// Add a feature override.
    pub fn feature(mut self, key: impl Into<String>, enabled: bool) -> Self {
        self.overrides.features.insert(key.into(), enabled);
        self
    }

    /// Set tool configuration.
    pub fn tool_config(mut self, config: ToolConfig) -> Self {
        self.overrides.tool_config = Some(config);
        self
    }

    /// Set compaction configuration.
    pub fn compact_config(mut self, config: CompactConfig) -> Self {
        self.overrides.compact_config = Some(config);
        self
    }

    /// Set plan mode configuration.
    pub fn plan_config(mut self, config: PlanModeConfig) -> Self {
        self.overrides.plan_config = Some(config);
        self
    }

    /// Set attachment configuration.
    pub fn attachment_config(mut self, config: AttachmentConfig) -> Self {
        self.overrides.attachment_config = Some(config);
        self
    }

    /// Set path configuration.
    pub fn path_config(mut self, config: PathConfig) -> Self {
        self.overrides.path_config = Some(config);
        self
    }

    /// Build the Config.
    ///
    /// This method:
    /// 1. Determines cocode_home (from builder, env var, or default)
    /// 2. Creates ConfigManager and loads configuration
    /// 3. Applies profile if set
    /// 4. Resolves all configured roles
    /// 5. Loads instructions from AGENTS.md in cwd
    /// 6. Applies overrides
    /// 7. Returns the complete Config
    pub fn build(self) -> Result<Config, crate::error::ConfigError> {
        use crate::ConfigManager;
        use crate::loader::find_cocode_home;

        // 1. Determine cocode_home
        let cocode_home = self.cocode_home.unwrap_or_else(find_cocode_home);

        // 2. Create ConfigManager
        let manager = ConfigManager::from_path(&cocode_home)?;

        // 3. Apply profile if set (update overrides)
        let mut overrides = self.overrides;
        if self.profile.is_some() {
            // Profile is handled by ConfigManager's app_config
            // For now, we pass through to build_config
        }

        // 4. Set cwd in overrides if provided
        if let Some(cwd) = self.cwd {
            overrides.cwd = Some(cwd);
        }

        // 5. Build config from manager
        manager.build_config(overrides)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.models.is_empty());
        assert!(config.providers.is_empty());
        assert!(config.resolved_models.is_empty());
        assert!(config.user_instructions.is_none());
        assert!(!config.ephemeral);
        assert_eq!(config.sandbox_mode, SandboxMode::ReadOnly);
    }

    #[test]
    fn test_config_overrides_builder() {
        let overrides = ConfigOverrides::new()
            .with_cwd("/my/project")
            .with_sandbox_mode(SandboxMode::WorkspaceWrite)
            .with_ephemeral(true)
            .with_feature("subagent", true);

        assert_eq!(overrides.cwd, Some(PathBuf::from("/my/project")));
        assert_eq!(overrides.sandbox_mode, Some(SandboxMode::WorkspaceWrite));
        assert_eq!(overrides.ephemeral, Some(true));
        assert_eq!(overrides.features.get("subagent"), Some(&true));
    }

    #[test]
    fn test_config_builder_new() {
        let builder = ConfigBuilder::new()
            .cocode_home("/custom/home")
            .cwd("/my/project")
            .profile("fast")
            .sandbox_mode(SandboxMode::FullAccess)
            .ephemeral(true);

        assert_eq!(builder.cocode_home, Some(PathBuf::from("/custom/home")));
        assert_eq!(builder.cwd, Some(PathBuf::from("/my/project")));
        assert_eq!(builder.profile, Some("fast".to_string()));
        assert_eq!(
            builder.overrides.sandbox_mode,
            Some(SandboxMode::FullAccess)
        );
        assert_eq!(builder.overrides.ephemeral, Some(true));
    }

    #[test]
    fn test_is_path_writable_read_only() {
        let config = Config {
            sandbox_mode: SandboxMode::ReadOnly,
            ..Default::default()
        };

        assert!(!config.is_path_writable(&PathBuf::from("/any/path")));
    }

    #[test]
    fn test_is_path_writable_full_access() {
        let config = Config {
            sandbox_mode: SandboxMode::FullAccess,
            ..Default::default()
        };

        assert!(config.is_path_writable(&PathBuf::from("/any/path")));
    }

    #[test]
    fn test_is_path_writable_workspace_write() {
        let config = Config {
            sandbox_mode: SandboxMode::WorkspaceWrite,
            writable_roots: vec![PathBuf::from("/workspace")],
            ..Default::default()
        };

        assert!(config.is_path_writable(&PathBuf::from("/workspace/file.txt")));
        assert!(config.is_path_writable(&PathBuf::from("/workspace/sub/dir/file.txt")));
        assert!(!config.is_path_writable(&PathBuf::from("/other/path")));
    }

    #[test]
    fn test_allows_write() {
        assert!(
            !Config {
                sandbox_mode: SandboxMode::ReadOnly,
                ..Default::default()
            }
            .allows_write()
        );

        assert!(
            Config {
                sandbox_mode: SandboxMode::WorkspaceWrite,
                ..Default::default()
            }
            .allows_write()
        );

        assert!(
            Config {
                sandbox_mode: SandboxMode::FullAccess,
                ..Default::default()
            }
            .allows_write()
        );
    }

    #[test]
    fn test_model_for_role_fallback() {
        let main_info = ModelInfo {
            slug: "main-model".to_string(),
            display_name: Some("Main Model".to_string()),
            context_window: Some(128000),
            max_output_tokens: Some(16384),
            ..Default::default()
        };

        let mut resolved_models = HashMap::new();
        resolved_models.insert(ModelRole::Main, main_info.clone());

        let config = Config {
            resolved_models,
            ..Default::default()
        };

        // Main role returns main model
        assert_eq!(
            config.model_for_role(ModelRole::Main).unwrap().slug,
            "main-model"
        );

        // Fast role falls back to main
        assert_eq!(
            config.model_for_role(ModelRole::Fast).unwrap().slug,
            "main-model"
        );

        // Vision role falls back to main
        assert_eq!(
            config.model_for_role(ModelRole::Vision).unwrap().slug,
            "main-model"
        );
    }

    #[test]
    fn test_model_for_role_specific() {
        let main_info = ModelInfo {
            slug: "main-model".to_string(),
            display_name: Some("Main Model".to_string()),
            context_window: Some(128000),
            max_output_tokens: Some(16384),
            ..Default::default()
        };

        let fast_info = ModelInfo {
            slug: "fast-model".to_string(),
            display_name: Some("Fast Model".to_string()),
            ..main_info.clone()
        };

        let mut resolved_models = HashMap::new();
        resolved_models.insert(ModelRole::Main, main_info);
        resolved_models.insert(ModelRole::Fast, fast_info);

        let config = Config {
            resolved_models,
            ..Default::default()
        };

        // Fast role returns specific model
        assert_eq!(
            config.model_for_role(ModelRole::Fast).unwrap().slug,
            "fast-model"
        );

        // Vision still falls back to main
        assert_eq!(
            config.model_for_role(ModelRole::Vision).unwrap().slug,
            "main-model"
        );
    }

    #[test]
    fn test_configured_roles() {
        let main_info = ModelInfo {
            slug: "main-model".to_string(),
            display_name: Some("Main Model".to_string()),
            context_window: Some(128000),
            max_output_tokens: Some(16384),
            ..Default::default()
        };

        let mut resolved_models = HashMap::new();
        resolved_models.insert(ModelRole::Main, main_info.clone());
        resolved_models.insert(
            ModelRole::Fast,
            ModelInfo {
                slug: "fast-model".to_string(),
                ..main_info
            },
        );

        let config = Config {
            resolved_models,
            ..Default::default()
        };

        let roles = config.configured_roles();
        assert_eq!(roles.len(), 2);
    }
}
