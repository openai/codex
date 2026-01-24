//! Multi-provider configuration management.
//!
//! This module provides a layered configuration system for managing multiple
//! LLM providers, models, and profiles. Configuration is stored in JSON and
//! TOML files in the `~/.cocode` directory by default.
//!
//! # Configuration Files
//!
//! - `config.toml`: User-friendly TOML configuration (model, provider, features)
//! - `models.json`: Provider-independent model metadata
//! - `providers.json`: Provider access configuration
//! - `profiles.json`: Named configuration bundles for quick switching
//! - `active.json`: Runtime state (managed by SDK)
//!
//! # Configuration Resolution
//!
//! Values are resolved with the following precedence (highest to lowest):
//! 1. Runtime overrides (API calls, `/model` command)
//! 2. TOML config (`config.toml`)
//! 3. Active state (`active.json`)
//! 4. Default profile
//! 5. Built-in defaults (compiled into binary)
//!
//! # Example
//!
//! ```no_run
//! use cocode_config::ConfigManager;
//! use cocode_config::error::ConfigError;
//!
//! # fn example() -> Result<(), ConfigError> {
//! // Load from default path (~/.cocode)
//! let manager = ConfigManager::from_default()?;
//!
//! // Get current provider/model
//! let (provider, model) = manager.current();
//! println!("Using: {provider}/{model}");
//!
//! // Switch to a different provider/model
//! manager.switch("anthropic", "claude-sonnet-4-20250514")?;
//!
//! // Or switch to a named profile
//! manager.switch_profile("coding")?;
//!
//! // Get resolved model info
//! let info = manager.resolve_model_info("anthropic", "claude-sonnet-4-20250514")?;
//! println!("Context window: {}", info.context_window);
//! # Ok(())
//! # }
//! ```

pub mod builtin;
pub mod error;
pub mod loader;
pub mod manager;
pub mod resolver;
pub mod toml_config;
pub mod types;

// Re-export protocol types (model)
pub use cocode_protocol::Capability;
pub use cocode_protocol::ConfigShellToolType;
pub use cocode_protocol::ModelInfo;
pub use cocode_protocol::ReasoningEffort;
pub use cocode_protocol::TruncationMode;
pub use cocode_protocol::TruncationPolicyConfig;

// Re-export protocol types (features)
pub use cocode_protocol::Feature;
pub use cocode_protocol::FeatureSpec;
pub use cocode_protocol::Features;
pub use cocode_protocol::Stage;
pub use cocode_protocol::all_features;
pub use cocode_protocol::feature_for_key;
pub use cocode_protocol::is_known_feature_key;

// Re-export main config types
pub use loader::ConfigLoader;
pub use loader::LoadedConfig;
pub use manager::ConfigManager;
pub use manager::RuntimeOverrides;
pub use resolver::ConfigResolver;
pub use types::ActiveState;
pub use types::ModelSummary;
pub use types::ModelsFile;
pub use types::ProfileConfig;
pub use types::ProfilesFile;
pub use types::ProviderConfig;
pub use types::ProviderModelConfig;
pub use types::ProviderSummary;
pub use types::ProviderType;
pub use types::ProvidersFile;
pub use types::ResolvedModelInfo;
pub use types::ResolvedProviderConfig;
pub use types::SessionConfigJson;

// Re-export TOML config types
pub use toml_config::ConfigToml;
pub use toml_config::FeaturesToml;
pub use toml_config::LoggingConfig;

// Re-export constants
pub use loader::AGENTS_MD_FILE;
pub use loader::COCODE_HOME_ENV;
pub use loader::COCODE_LOG_DIR_ENV;
pub use loader::CONFIG_TOML_FILE;
pub use loader::DEFAULT_CONFIG_DIR;
pub use loader::LOG_DIR_NAME;

// Re-export helper functions
pub use loader::default_config_dir;
pub use loader::find_cocode_home;
pub use loader::load_instructions;
pub use loader::log_dir;
