//! Multi-provider configuration management.
//!
//! This module provides a layered configuration system for managing multiple
//! LLM providers, models, and settings. Configuration is stored in JSON files
//! in the `~/.cocode` directory by default.
//!
//! # Configuration Files
//!
//! - `config.json`: Application configuration (model, provider, features, profiles)
//! - `*model.json`: Model definitions (e.g., `gpt_model.json`, `model.json`)
//! - `*provider.json`: Provider configurations (e.g., `openai_provider.json`, `provider.json`)
//!
//! # Configuration Resolution
//!
//! Values are resolved with the following precedence (highest to lowest):
//! 1. Runtime overrides (API calls, `/model` command) - in-memory only
//! 2. JSON config (`config.json`) with profile resolution
//! 3. Built-in defaults (compiled into binary)
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
//! // Get resolved model info
//! let info = manager.resolve_model_info("anthropic", "claude-sonnet-4-20250514")?;
//! println!("Context window: {}", info.context_window);
//! # Ok(())
//! # }
//! ```

pub mod builtin;
pub mod builtin_agents;
pub mod config;
pub mod env_loader;
pub mod error;
pub mod interceptors;
pub mod json_config;
pub mod loader;
pub mod manager;
pub mod resolver;
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
pub use types::ModelSummary;
pub use types::ModelsFile;
pub use types::ProviderConfig;
pub use types::ProviderModelEntry;
pub use types::ProviderSummary;
pub use types::ProvidersFile;
// Re-export provider types from protocol (via types)
pub use cocode_protocol::ProviderModel;
pub use types::ProviderInfo;
pub use types::ProviderType;
pub use types::WireApi;

// Re-export JSON config types
pub use json_config::AppConfig;
pub use json_config::ConfigProfile;
pub use json_config::FeaturesConfig;
pub use json_config::LoggingConfig;
pub use json_config::ResolvedAppConfig;

// Re-export constants
pub use loader::AGENTS_MD_FILE;
pub use loader::COCODE_HOME_ENV;
pub use loader::COCODE_LOG_DIR_ENV;
pub use loader::CONFIG_FILE;
pub use loader::DEFAULT_CONFIG_DIR;
pub use loader::LOG_DIR_NAME;

// Re-export helper functions
pub use loader::default_config_dir;
pub use loader::find_cocode_home;
pub use loader::load_instructions;
pub use loader::log_dir;

// Re-export Config types
pub use config::Config;
pub use config::ConfigBuilder;
pub use config::ConfigOverrides;

// Re-export sandbox types from protocol
pub use cocode_protocol::SandboxMode;

// Re-export extended config types from protocol
pub use cocode_protocol::AttachmentConfig;
pub use cocode_protocol::CompactConfig;
pub use cocode_protocol::PathConfig;
pub use cocode_protocol::PlanModeConfig;
pub use cocode_protocol::ToolConfig;

// Re-export env loader
pub use env_loader::EnvLoader;

// Re-export builtin agents config
pub use builtin_agents::BUILTIN_AGENT_TYPES;
pub use builtin_agents::BuiltinAgentOverride;
pub use builtin_agents::BuiltinAgentsConfig;
pub use builtin_agents::apply_env_overrides;
pub use builtin_agents::is_builtin_agent;
pub use builtin_agents::load_builtin_agents_config;
