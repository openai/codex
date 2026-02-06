> **Note**: This design document predates recent refactoring (deletion of `ResolvedModelInfo`,
> `auto_compact_token_limit`, `ProviderConfig.model_options`, `apply_overrides()`).
> Refer to the source code as the authoritative reference.

# Multi-LLM Provider Configuration Architecture

## 1. Overview

This document describes the design of a multi-LLM provider configuration system for hyper-sdk. The architecture separates model metadata (ModelInfo) from provider access configuration (ProviderConfig), supports runtime switching, and uses JSON format with `~/.cocode` as the default configuration path.

### Design Goals

1. **Multi-provider support** with dynamic switching (e.g., `/model` command)
2. **Separation of concerns**: Model info (provider-independent) vs Provider config (access config)
3. **Provider override capability**: Provider config can override model info
4. **Configurable path**: Config path can be specified via parameter
5. **Sensible defaults**: `~/.cocode` as default location

### Industry Best Practices Referenced

- **LiteLLM**: Unified interface with provider-specific config
- **OpenRouter**: Model routing with capability metadata
- **LangChain**: Provider abstraction with config layers
- **codex-rs**: ConfigLayerStack with precedence levels

## 2. Configuration Schema

### 2.1 File Organization

```
~/.cocode/                    # Default config directory
├── models.json               # Model metadata (provider-independent)
├── providers.json            # Provider access configuration
├── profiles.json             # Named configuration bundles for quick switching
└── active.json               # Runtime state (managed by SDK)

# Alternative: Single file mode
~/.cocode/config.json         # Combined configuration (optional)
```

### 2.2 models.json - Model Metadata

Model information that is independent of any specific provider. Contains capabilities, context windows, and behavioral settings.

```json
{
  "$schema": "https://cocode.dev/schemas/models.json",
  "version": "1.0",
  "models": {
    "gpt-4o": {
      "display_name": "GPT-4o",
      "description": "OpenAI's most capable multimodal model",
      "context_window": 128000,
      "max_output_tokens": 16384,
      "capabilities": [
        "text_generation",
        "streaming",
        "vision",
        "tool_calling",
        "structured_output"
      ],
      "auto_compact_token_limit": 100000,
      "effective_context_window_percent": 95,
      "supports_reasoning_summaries": false,
      "supports_parallel_tool_calls": true,
      "default_reasoning_effort": null
    },
    "claude-sonnet-4-20250514": {
      "display_name": "Claude Sonnet 4",
      "description": "Anthropic's balanced intelligence and speed model",
      "context_window": 200000,
      "max_output_tokens": 64000,
      "capabilities": [
        "text_generation",
        "streaming",
        "vision",
        "tool_calling",
        "extended_thinking"
      ],
      "auto_compact_token_limit": 160000,
      "effective_context_window_percent": 95,
      "supports_reasoning_summaries": true,
      "supports_parallel_tool_calls": true,
      "default_reasoning_effort": "medium",
      "thinking_budget_default": 10000
    },
    "gemini-2.5-pro": {
      "display_name": "Gemini 2.5 Pro",
      "description": "Google's most capable model",
      "context_window": 2097152,
      "max_output_tokens": 65536,
      "capabilities": [
        "text_generation",
        "streaming",
        "vision",
        "tool_calling",
        "structured_output"
      ],
      "auto_compact_token_limit": 1800000,
      "supports_parallel_tool_calls": true
    },
    "deepseek-r1": {
      "display_name": "DeepSeek R1",
      "description": "DeepSeek's reasoning model",
      "context_window": 64000,
      "max_output_tokens": 8192,
      "capabilities": [
        "text_generation",
        "streaming",
        "extended_thinking"
      ],
      "auto_compact_token_limit": 57600,
      "supports_reasoning_summaries": true,
      "default_reasoning_effort": "high"
    }
  }
}
```

#### ModelInfoConfig Fields

| Field | Type | Description |
|-------|------|-------------|
| `display_name` | string | Human-readable name |
| `description` | string? | Brief description |
| `context_window` | i64 | Maximum context window in tokens |
| `max_output_tokens` | i64 | Maximum output tokens |
| `capabilities` | string[] | Supported capabilities (see Capability enum) |
| `auto_compact_token_limit` | i64? | Token count to trigger compaction (default: 90% of context_window) |
| `effective_context_window_percent` | i32 | Usable % after reserves (default: 95) |
| `supports_reasoning_summaries` | bool | Extended thinking/reasoning support |
| `supports_parallel_tool_calls` | bool | Parallel tool execution support |
| `default_reasoning_effort` | string? | Default reasoning level: "low", "medium", "high" |
| `thinking_budget_default` | i32? | Default thinking budget tokens |

#### Capability Values

```
text_generation      - Basic text generation
streaming            - Streaming responses
vision               - Image understanding
audio                - Audio input/output
tool_calling         - Function/tool calling
embedding            - Vector embeddings
extended_thinking    - Extended reasoning/thinking
structured_output    - JSON mode/structured output
```

### 2.3 providers.json - Provider Access Configuration

Provider-specific access configuration with optional model info overrides.

```json
{
  "$schema": "https://cocode.dev/schemas/providers.json",
  "version": "1.0",
  "providers": {
    "openai": {
      "name": "OpenAI",
      "type": "openai",
      "env_key": "OPENAI_API_KEY",
      "base_url": "https://api.openai.com/v1",
      "default_model": "gpt-4o",
      "timeout_secs": 600,
      "models": {
        "gpt-4o": {},
        "gpt-4o-mini": {},
        "o1": {},
        "o3-mini": {}
      }
    },
    "anthropic": {
      "name": "Anthropic",
      "type": "anthropic",
      "env_key": "ANTHROPIC_API_KEY",
      "base_url": "https://api.anthropic.com",
      "default_model": "claude-sonnet-4-20250514",
      "timeout_secs": 600,
      "models": {
        "claude-sonnet-4-20250514": {
          "model_info_override": {
            "thinking_budget_default": 15000
          }
        },
        "claude-haiku-3-5-20241022": {}
      }
    },
    "gemini": {
      "name": "Google Gemini",
      "type": "gemini",
      "env_key": "GOOGLE_API_KEY",
      "default_model": "gemini-2.5-pro",
      "models": {
        "gemini-2.5-pro": {},
        "gemini-2.0-flash": {}
      }
    },
    "volcengine_ark": {
      "name": "Volcengine Ark",
      "type": "volcengine",
      "env_key": "ARK_API_KEY",
      "base_url": "https://ark.cn-beijing.volces.com/api/v3",
      "models": {
        "ep-20250109-xxxxx": {
          "model_id": "deepseek-r1",
          "model_info_override": {
            "context_window": 64000,
            "auto_compact_token_limit": 57600
          }
        }
      }
    },
    "local_ollama": {
      "name": "Local Ollama",
      "type": "openai_compat",
      "base_url": "http://localhost:11434/v1",
      "env_key": null,
      "default_model": "llama3.2",
      "models": {
        "llama3.2": {
          "model_id": "llama3.2",
          "model_info_override": {
            "context_window": 131072,
            "capabilities": ["text_generation", "streaming", "tool_calling"]
          }
        },
        "qwen2.5-coder:32b": {
          "model_info_override": {
            "context_window": 32768,
            "capabilities": ["text_generation", "streaming"]
          }
        }
      }
    }
  }
}
```

#### ProviderJsonConfig Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Human-readable provider name |
| `type` | ProviderType | Provider type (see enum below) |
| `env_key` | string? | Environment variable for API key |
| `api_key` | string? | Direct API key (not recommended, use env_key) |
| `base_url` | string? | API base URL override |
| `organization_id` | string? | Organization ID (for providers that support it) |
| `default_model` | string? | Default model for this provider |
| `timeout_secs` | i64? | Request timeout in seconds |
| `models` | Map | Model-specific configurations |
| `extra` | object? | Provider-specific extra configuration |

#### ProviderType Values

```
openai          - OpenAI API
anthropic       - Anthropic API
gemini          - Google Gemini API
volcengine      - Volcengine Ark API
zai             - Z.AI (Zhipu) API
openai_compat   - OpenAI-compatible endpoints (Ollama, LMStudio, etc.)
```

#### ProviderModelConfig Fields

| Field | Type | Description |
|-------|------|-------------|
| `model_id` | string? | Alias to resolve model info (e.g., "ep-xxx" -> "deepseek-r1") |
| `model_info_override` | ModelInfoConfig? | Override model info fields |

### 2.4 profiles.json - Quick Switching

Named configuration bundles for quick switching between different setups.

```json
{
  "$schema": "https://cocode.dev/schemas/profiles.json",
  "version": "1.0",
  "default_profile": "default",
  "profiles": {
    "default": {
      "provider": "openai",
      "model": "gpt-4o",
      "session_config": {
        "temperature": 0.7,
        "max_tokens": 4096
      }
    },
    "coding": {
      "provider": "anthropic",
      "model": "claude-sonnet-4-20250514",
      "session_config": {
        "temperature": 0.3,
        "max_tokens": 8192,
        "thinking_config": {
          "enabled": true,
          "budget_tokens": 10000
        }
      }
    },
    "fast": {
      "provider": "openai",
      "model": "gpt-4o-mini",
      "session_config": {
        "temperature": 0.5
      }
    },
    "local": {
      "provider": "local_ollama",
      "model": "llama3.2",
      "session_config": {
        "temperature": 0.8
      }
    },
    "reasoning": {
      "provider": "openai",
      "model": "o3-mini",
      "session_config": {
        "reasoning_effort": "high"
      }
    }
  }
}
```

#### ProfileConfig Fields

| Field | Type | Description |
|-------|------|-------------|
| `provider` | string | Provider ID to use |
| `model` | string | Model ID to use |
| `session_config` | SessionConfigJson? | Session-level configuration |

#### SessionConfigJson Fields

| Field | Type | Description |
|-------|------|-------------|
| `temperature` | f64? | Sampling temperature (0.0-2.0) |
| `max_tokens` | i32? | Maximum output tokens |
| `top_p` | f64? | Nucleus sampling parameter |
| `reasoning_effort` | string? | "low", "medium", "high" |
| `thinking_config` | ThinkingConfig? | Extended thinking configuration |

### 2.5 active.json - Runtime State

Managed by the SDK to track current selection. Users should not edit this file directly.

```json
{
  "current_profile": "default",
  "current_provider": "openai",
  "current_model": "gpt-4o",
  "last_updated": "2025-01-24T10:30:00Z"
}
```

## 3. Resolution Logic

### 3.1 Configuration Precedence

Resolution order (highest to lowest priority):

```
1. Runtime overrides      (API calls, /model command)
2. Environment variables  (OPENAI_API_KEY, etc.)
3. Profile config         (profiles.json -> session_config)
4. Provider model override (providers.json -> models -> model_info_override)
5. User model config      (models.json)
6. Built-in defaults      (compiled into binary)
```

### 3.2 Model Info Resolution Algorithm

```rust
fn resolve_model_info(
    provider_id: &str,
    model_id: &str,
    config: &ConfigManager,
) -> ResolvedModelInfo {
    // 1. Start with built-in defaults
    let mut info = built_in_model_info(model_id)
        .unwrap_or_else(|| default_model_info(model_id));

    // 2. Apply user-defined model info (models.json)
    if let Some(user_info) = config.models.get(model_id) {
        info.merge_with(user_info);
    }

    // 3. Apply provider-specific model override
    if let Some(provider) = config.providers.get(provider_id) {
        if let Some(model_config) = provider.models.get(model_id) {
            // 3a. Resolve model_id alias (e.g., "ep-xxx" -> "deepseek-r1")
            if let Some(alias_id) = &model_config.model_id {
                if let Some(aliased_info) = config.models.get(alias_id) {
                    info.merge_with(aliased_info);
                }
            }
            // 3b. Apply direct overrides
            if let Some(override_info) = &model_config.model_info_override {
                info.merge_with(override_info);
            }
        }
    }

    info
}
```

### 3.3 Provider Config Resolution

```rust
fn resolve_provider_config(
    provider_id: &str,
    config: &ConfigManager,
) -> Result<ResolvedProviderConfig, ConfigError> {
    // 1. Get provider from config or built-in
    let provider = config.providers.get(provider_id)
        .or_else(|| built_in_providers().get(provider_id))
        .ok_or(ConfigError::ProviderNotFound(provider_id.to_string()))?;

    // 2. Resolve API key from environment (takes precedence)
    let api_key = if let Some(env_key) = &provider.env_key {
        std::env::var(env_key).ok()
    } else {
        provider.api_key.clone()
    };

    // 3. Build resolved config
    Ok(ResolvedProviderConfig {
        id: provider_id.to_string(),
        name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        api_key,
        base_url: provider.base_url.clone()
            .unwrap_or_else(|| default_base_url(&provider.provider_type)),
        timeout_secs: provider.timeout_secs.unwrap_or(600),
        default_model: provider.default_model.clone(),
        extra: provider.extra.clone(),
    })
}
```

### 3.4 Merge Strategy

For `ModelInfoConfig` fields, use Option-based override:

```rust
impl ModelInfoConfig {
    fn merge_with(&mut self, other: &ModelInfoConfig) {
        // Only override if other has a value
        if other.display_name.is_some() {
            self.display_name = other.display_name.clone();
        }
        if other.context_window.is_some() {
            self.context_window = other.context_window;
        }
        if other.max_output_tokens.is_some() {
            self.max_output_tokens = other.max_output_tokens;
        }
        if other.capabilities.is_some() {
            self.capabilities = other.capabilities.clone();
        }
        // ... other fields
    }
}
```

## 4. Code Architecture

### 4.1 Module Structure

```
hyper-sdk/src/
├── config/
│   ├── mod.rs              # Module exports
│   ├── types.rs            # Core config types
│   ├── loader.rs           # JSON file loading
│   ├── resolver.rs         # Config resolution and merging
│   ├── manager.rs          # ConfigManager with caching
│   ├── builtin.rs          # Built-in model/provider defaults
│   └── error.rs            # Config-specific errors
├── capability.rs           # Capability enum, ModelInfo (existing, extend)
├── provider.rs             # Provider trait, ProviderConfig (existing)
├── client.rs               # HyperClient (existing, extend)
└── lib.rs                  # Re-exports
```

### 4.2 Core Types

```rust
// config/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User-defined model metadata from models.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelInfoConfig {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub capabilities: Option<Vec<Capability>>,
    pub auto_compact_token_limit: Option<i64>,
    pub effective_context_window_percent: Option<i32>,
    pub default_reasoning_effort: Option<ReasoningEffort>,
    pub supports_reasoning_summaries: Option<bool>,
    pub supports_parallel_tool_calls: Option<bool>,
    pub thinking_budget_default: Option<i32>,
}

/// Provider configuration from providers.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderJsonConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    #[serde(default)]
    pub env_key: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<i64>,
    #[serde(default)]
    pub models: HashMap<String, ProviderModelConfig>,
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

/// Model-specific config within a provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderModelConfig {
    /// Alias to resolve model info (e.g., "ep-xxx" -> "deepseek-r1")
    #[serde(default)]
    pub model_id: Option<String>,
    /// Provider-specific overrides to model info
    #[serde(default)]
    pub model_info_override: Option<ModelInfoConfig>,
}

/// Provider type enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Gemini,
    Volcengine,
    Zai,
    OpenAICompat,
}

/// Reasoning effort level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

/// Profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub session_config: Option<SessionConfigJson>,
}

/// Session config in JSON format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionConfigJson {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub thinking_config: Option<ThinkingConfigJson>,
}

/// Thinking configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThinkingConfigJson {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub budget_tokens: Option<i32>,
}

/// Resolved model info after all merging
#[derive(Debug, Clone)]
pub struct ResolvedModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provider: String,
    pub context_window: i64,
    pub max_output_tokens: i64,
    pub capabilities: Vec<Capability>,
    pub auto_compact_token_limit: Option<i64>,
    pub effective_context_window_percent: i32,
    pub supports_reasoning_summaries: bool,
    pub supports_parallel_tool_calls: bool,
    pub default_reasoning_effort: Option<ReasoningEffort>,
    pub thinking_budget_default: Option<i32>,
}

/// Resolved provider config
#[derive(Debug, Clone)]
pub struct ResolvedProviderConfig {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub base_url: String,
    pub timeout_secs: i64,
    pub default_model: Option<String>,
    pub extra: Option<serde_json::Value>,
}
```

### 4.3 ConfigManager

```rust
// config/manager.rs

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// Central configuration manager with caching and reload support
#[derive(Debug)]
pub struct ConfigManager {
    /// Path to config directory
    config_path: PathBuf,
    /// Cached models config
    models: RwLock<HashMap<String, ModelInfoConfig>>,
    /// Cached providers config
    providers: RwLock<HashMap<String, ProviderJsonConfig>>,
    /// Cached profiles config
    profiles: RwLock<ProfilesConfig>,
    /// Active state
    active: RwLock<ActiveState>,
    /// Last reload timestamp
    last_reload: RwLock<SystemTime>,
}

impl ConfigManager {
    /// Create from default path (~/.cocode)
    pub fn from_default() -> Result<Self, ConfigError> {
        let path = dirs::home_dir()
            .ok_or(ConfigError::HomeDirNotFound)?
            .join(".cocode");
        Self::from_path(path)
    }

    /// Create from custom path
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let config_path = path.as_ref().to_path_buf();
        let manager = Self {
            config_path,
            models: RwLock::new(HashMap::new()),
            providers: RwLock::new(HashMap::new()),
            profiles: RwLock::new(ProfilesConfig::default()),
            active: RwLock::new(ActiveState::default()),
            last_reload: RwLock::new(SystemTime::now()),
        };
        manager.reload()?;
        Ok(manager)
    }

    /// Resolve model info with all layers merged
    pub fn resolve_model_info(&self, provider: &str, model: &str) -> ResolvedModelInfo {
        resolve_model_info(provider, model, self)
    }

    /// Resolve provider config
    pub fn resolve_provider(&self, provider: &str) -> Result<ResolvedProviderConfig, ConfigError> {
        resolve_provider_config(provider, self)
    }

    /// Get current active provider/model
    pub fn current(&self) -> (String, String) {
        let active = self.active.read();
        (active.current_provider.clone(), active.current_model.clone())
    }

    /// Switch active provider/model
    pub fn switch(&self, provider: &str, model: &str) -> Result<(), ConfigError> {
        // Validate provider exists
        self.resolve_provider(provider)?;

        // Update active state
        let mut active = self.active.write();
        active.current_provider = provider.to_string();
        active.current_model = model.to_string();
        active.last_updated = SystemTime::now();

        // Persist to active.json
        self.save_active_state(&active)?;
        Ok(())
    }

    /// Switch to profile
    pub fn switch_profile(&self, profile: &str) -> Result<(), ConfigError> {
        let profiles = self.profiles.read();
        let profile_config = profiles.profiles.get(profile)
            .ok_or(ConfigError::ProfileNotFound(profile.to_string()))?;

        let mut active = self.active.write();
        active.current_profile = Some(profile.to_string());
        active.current_provider = profile_config.provider.clone();
        active.current_model = profile_config.model.clone();
        active.last_updated = SystemTime::now();

        self.save_active_state(&active)?;
        Ok(())
    }

    /// Reload configuration from disk
    pub fn reload(&self) -> Result<(), ConfigError> {
        // Load models.json
        let models_path = self.config_path.join("models.json");
        if models_path.exists() {
            let content = std::fs::read_to_string(&models_path)?;
            let models_config: ModelsConfig = serde_json::from_str(&content)?;
            *self.models.write() = models_config.models;
        }

        // Load providers.json
        let providers_path = self.config_path.join("providers.json");
        if providers_path.exists() {
            let content = std::fs::read_to_string(&providers_path)?;
            let providers_config: ProvidersConfig = serde_json::from_str(&content)?;
            *self.providers.write() = providers_config.providers;
        }

        // Load profiles.json
        let profiles_path = self.config_path.join("profiles.json");
        if profiles_path.exists() {
            let content = std::fs::read_to_string(&profiles_path)?;
            let profiles_config: ProfilesConfig = serde_json::from_str(&content)?;
            *self.profiles.write() = profiles_config;
        }

        // Load active.json
        let active_path = self.config_path.join("active.json");
        if active_path.exists() {
            let content = std::fs::read_to_string(&active_path)?;
            let active_state: ActiveState = serde_json::from_str(&content)?;
            *self.active.write() = active_state;
        }

        *self.last_reload.write() = SystemTime::now();
        Ok(())
    }

    /// List available providers
    pub fn list_providers(&self) -> Vec<ProviderSummary> {
        let providers = self.providers.read();
        providers.iter()
            .map(|(id, config)| ProviderSummary {
                id: id.clone(),
                name: config.name.clone(),
                provider_type: config.provider_type.clone(),
                model_count: config.models.len(),
            })
            .collect()
    }

    /// List models for a provider
    pub fn list_models(&self, provider: &str) -> Vec<ModelSummary> {
        let providers = self.providers.read();
        providers.get(provider)
            .map(|p| {
                p.models.keys()
                    .map(|id| {
                        let info = self.resolve_model_info(provider, id);
                        ModelSummary {
                            id: id.clone(),
                            display_name: info.display_name,
                            context_window: info.context_window,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
```

### 4.4 HyperClient Extension

```rust
// client.rs (extend existing)

impl HyperClient {
    /// Create from configuration manager
    pub fn from_config(config: ConfigManager) -> Result<Self, HyperError> {
        let mut client = Self::new();

        // Register providers from config
        for (id, provider_config) in config.providers.read().iter() {
            let resolved = config.resolve_provider(id)?;
            let provider = create_provider_from_config(&resolved)?;
            client.register_arc(provider);
        }

        // Store config manager for runtime operations
        client.config = Some(Arc::new(config));

        Ok(client)
    }

    /// Create from config path
    pub fn from_config_path(path: impl AsRef<Path>) -> Result<Self, HyperError> {
        let config = ConfigManager::from_path(path)?;
        Self::from_config(config)
    }

    /// Create from default config (~/.cocode)
    pub fn from_default_config() -> Result<Self, HyperError> {
        let config = ConfigManager::from_default()?;
        Self::from_config(config)
    }

    /// Get current active provider and model
    pub fn current(&self) -> Option<(&str, &str)> {
        self.config.as_ref().map(|c| {
            let (p, m) = c.current();
            // Note: This requires storing the strings or using a different approach
            // to avoid lifetime issues
            (p.as_str(), m.as_str())
        })
    }

    /// Switch to different provider/model (for /model command)
    pub fn switch(&self, provider: &str, model: &str) -> Result<(), HyperError> {
        // Validate provider exists
        self.require_provider(provider)?;

        // Validate model is supported
        let provider_obj = self.provider(provider).unwrap();
        if !provider_obj.supports_model(model) {
            return Err(HyperError::ModelNotFound(model.to_string()));
        }

        // Update active state
        if let Some(config) = &self.config {
            config.switch(provider, model)?;
        }

        Ok(())
    }

    /// Switch to named profile
    pub fn switch_profile(&self, profile: &str) -> Result<(), HyperError> {
        if let Some(config) = &self.config {
            config.switch_profile(profile)?;
        }
        Ok(())
    }

    /// Get resolved model info
    pub fn model_info(&self, provider: &str, model: &str) -> Option<ResolvedModelInfo> {
        self.config.as_ref().map(|c| c.resolve_model_info(provider, model))
    }

    /// Reload config from disk
    pub fn reload_config(&self) -> Result<(), HyperError> {
        if let Some(config) = &self.config {
            config.reload()?;
        }
        Ok(())
    }

    /// List available providers with summaries
    pub fn list_provider_summaries(&self) -> Vec<ProviderSummary> {
        self.config.as_ref()
            .map(|c| c.list_providers())
            .unwrap_or_default()
    }

    /// List models for a provider with summaries
    pub fn list_model_summaries(&self, provider: &str) -> Vec<ModelSummary> {
        self.config.as_ref()
            .map(|c| c.list_models(provider))
            .unwrap_or_default()
    }
}
```

### 4.5 Error Types

```rust
// config/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Home directory not found")]
    HomeDirNotFound,

    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid JSON in {file}: {error}")]
    InvalidJson { file: String, error: String },

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Environment variable not set: {0}")]
    EnvVarNotSet(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<ConfigError> for HyperError {
    fn from(err: ConfigError) -> Self {
        HyperError::ConfigError(err.to_string())
    }
}
```

## 5. Usage Examples

### 5.1 Basic Usage

```rust
use hyper_sdk::{HyperClient, GenerateRequest};

// Load from default config (~/.cocode)
let client = HyperClient::from_default_config()?;

// Use current active provider/model
let (provider, model) = client.current().unwrap();
let response = client.model(provider, model)?
    .generate(GenerateRequest::from_text("Hello!"))
    .await?;
```

### 5.2 Custom Config Path

```rust
// Load from custom path
let client = HyperClient::from_config_path("/path/to/config")?;
```

### 5.3 Runtime Switching

```rust
// Switch provider/model
client.switch("anthropic", "claude-sonnet-4-20250514")?;

// Switch to profile
client.switch_profile("coding")?;

// List available options
for provider in client.list_provider_summaries() {
    println!("{}: {}", provider.id, provider.name);
    for model in client.list_model_summaries(&provider.id) {
        println!("  - {}: {} ({}k tokens)",
            model.id, model.display_name, model.context_window / 1000);
    }
}
```

### 5.4 Model Info Access

```rust
// Get resolved model info
let info = client.model_info("openai", "gpt-4o").unwrap();
println!("Context window: {}", info.context_window);
println!("Capabilities: {:?}", info.capabilities);
println!("Auto compact at: {:?} tokens", info.auto_compact_token_limit);
```

## 6. Backward Compatibility

### 6.1 Existing ProviderConfig Support

The existing `ProviderConfig` struct remains supported:

```rust
// Existing code continues to work
let client = HyperClient::new()
    .with_provider(OpenAIProvider::from_env()?);

// New config-based approach is additive
let client = HyperClient::from_default_config()?;
```

### 6.2 Environment Variable Precedence

Environment variables always take precedence for secrets:
- `OPENAI_API_KEY` overrides providers.json api_key
- `ANTHROPIC_API_KEY` overrides providers.json api_key
- `COCODE_CONFIG_PATH` overrides default config path

## 7. Implementation Plan

### Phase 1: Core Config Types
- [ ] Create `config/types.rs` with all config structs
- [ ] Create `config/error.rs` with error types
- [ ] Add serde derives and validation

### Phase 2: Config Loading
- [ ] Create `config/loader.rs` for JSON parsing
- [ ] Create `config/builtin.rs` with built-in defaults
- [ ] Handle missing files gracefully

### Phase 3: Resolution Logic
- [ ] Create `config/resolver.rs` with merge algorithms
- [ ] Implement model info resolution with aliases
- [ ] Implement provider config resolution with env vars

### Phase 4: ConfigManager
- [ ] Create `config/manager.rs` with caching
- [ ] Add switch/reload/list methods
- [ ] Implement active state persistence

### Phase 5: Client Integration
- [ ] Extend `HyperClient` with config-aware methods
- [ ] Add `from_config_path` and `from_default_config`
- [ ] Add runtime switching API

### Phase 6: Testing
- [ ] Unit tests for config loading/parsing
- [ ] Unit tests for resolution/merging
- [ ] Integration tests for runtime switching
- [ ] Test env var precedence

## 8. Files to Modify/Create

| File | Action | Purpose |
|------|--------|---------|
| `src/config/mod.rs` | Create | Module exports |
| `src/config/types.rs` | Create | Config type definitions |
| `src/config/error.rs` | Create | Error types |
| `src/config/loader.rs` | Create | JSON file loading |
| `src/config/resolver.rs` | Create | Merge/resolution logic |
| `src/config/manager.rs` | Create | ConfigManager implementation |
| `src/config/builtin.rs` | Create | Built-in model defaults |
| `src/capability.rs` | Extend | Add new ModelInfo fields |
| `src/client.rs` | Extend | Add config-aware methods |
| `src/lib.rs` | Extend | Export config module |

## 9. Verification

1. **Config Loading**: Create test JSON files, verify parsing works
2. **Merging**: Test provider override of model info fields
3. **Runtime Switching**: Test `/model` equivalent API calls
4. **Env Vars**: Verify environment variables take precedence
5. **Default Path**: Verify `~/.cocode` works correctly
6. **Profile Switching**: Test profile-based configuration bundles
7. **Backward Compatibility**: Verify existing `from_env()` still works
