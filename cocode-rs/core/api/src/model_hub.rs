//! Unified model hub for model acquisition and caching.
//!
//! `ModelHub` is the central service for acquiring model instances and building
//! inference contexts. It is **stateless** regarding role selections - selections
//! are passed as parameters to enable proper session isolation.
//!
//! # Key Features
//!
//! - **Role-agnostic**: ModelHub does NOT know about roles; it only resolves ModelSpec → Model
//! - **Stateless for selections**: RoleSelections are passed as parameters (owned by Session)
//! - **Provider and model caching**: Reuses expensive HTTP clients and model instances
//! - **Full context preparation**: `prepare_inference_with_selections()` returns complete `InferenceContext`
//!
//! # Architecture
//!
//! ```text
//! Session (OWNS selections)
//!     │
//!     ├─► resolve_identity() → (ModelSpec, ThinkingLevel)
//!     │       Uses session.selections
//!     │
//!     └─► ModelHub (ROLE-AGNOSTIC)
//!             │
//!             └─► get_model(spec) → (Arc<dyn Model>, ModelInfo)
//!                 build_context(spec, ...) → (InferenceContext, Model)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use cocode_api::ModelHub;
//! use cocode_protocol::execution::{ExecutionIdentity, AgentKind, resolve_identity};
//! use cocode_protocol::model::ModelRole;
//!
//! let hub = ModelHub::new(config);
//!
//! // Step 1: Resolve identity using session's selections
//! let (spec, thinking_level) = resolve_identity(
//!     &ExecutionIdentity::main(),
//!     &session.selections,
//!     None,  // no parent spec
//! )?;
//!
//! // Step 2: Get model from hub (role-agnostic)
//! let (ctx, model) = hub.build_context(
//!     &spec,
//!     "session-123",
//!     1,
//!     AgentKind::Main,
//!     thinking_level,
//! )?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use cocode_config::ConfigManager;
use cocode_config::ResolvedModelInfo;
use cocode_protocol::ProviderType;
use cocode_protocol::execution::AgentKind;
use cocode_protocol::execution::ExecutionIdentity;
use cocode_protocol::execution::InferenceContext;
use cocode_protocol::model::ModelInfo;
use cocode_protocol::model::ModelRole;
use cocode_protocol::model::ModelSpec;
use cocode_protocol::model::RoleSelection;
use cocode_protocol::model::RoleSelections;
use cocode_protocol::thinking::ThinkingLevel;
use hyper_sdk::Model;
use hyper_sdk::Provider;
use tracing::debug;
use tracing::info;
use uuid::Uuid;

/// Convert ResolvedModelInfo (from config) to ModelInfo (protocol type).
fn to_model_info(resolved: ResolvedModelInfo) -> ModelInfo {
    ModelInfo {
        slug: resolved.id,
        display_name: Some(resolved.display_name),
        description: resolved.description,
        context_window: Some(resolved.context_window),
        max_output_tokens: Some(resolved.max_output_tokens),
        timeout_secs: Some(resolved.timeout_secs),
        capabilities: Some(resolved.capabilities),
        temperature: resolved.temperature,
        top_p: resolved.top_p,
        frequency_penalty: None,
        presence_penalty: None,
        default_thinking_level: resolved.default_thinking_level,
        supported_thinking_levels: resolved.supported_thinking_levels,
        include_thoughts: resolved.include_thoughts,
        reasoning_summary: resolved.reasoning_summary,
        auto_compact_token_limit: resolved.auto_compact_token_limit,
        effective_context_window_percent: resolved.effective_context_window_percent,
        shell_type: None,
        truncation_policy: None,
        experimental_supported_tools: None,
        apply_patch_tool_type: None,
        base_instructions: resolved.base_instructions,
        base_instructions_file: None,
        extra: resolved.extra,
    }
}

use crate::provider_factory;

// ============================================================================
// Identity Resolution (Standalone Function)
// ============================================================================

/// Resolve an ExecutionIdentity to a ModelSpec and optional RoleSelection.
///
/// This is the **public** function for identity resolution. It takes selections
/// as a parameter (not from internal state), enabling proper session isolation.
///
/// # Arguments
///
/// * `identity` - How to resolve the model (Role, Spec, or Inherit)
/// * `selections` - Role selections (owned by Session, passed as parameter)
/// * `parent_spec` - Parent model spec for Inherit identity
///
/// # Returns
///
/// A tuple of (ModelSpec, Option<RoleSelection>) on success:
/// - For Role: returns (spec from selection, full selection with thinking level)
/// - For Spec: returns (the spec directly, None)
/// - For Inherit: returns (parent spec, None)
///
/// # Example
///
/// ```ignore
/// use cocode_api::resolve_identity;
/// use cocode_protocol::execution::ExecutionIdentity;
///
/// // Resolve main role to model spec
/// let (spec, selection) = resolve_identity(
///     &ExecutionIdentity::main(),
///     &session.selections,
///     None,
/// )?;
///
/// // Get thinking level from selection
/// let thinking_level = selection.and_then(|s| s.thinking_level);
/// ```
pub fn resolve_identity(
    identity: &ExecutionIdentity,
    selections: &RoleSelections,
    parent_spec: Option<&ModelSpec>,
) -> Result<(ModelSpec, Option<RoleSelection>), HubError> {
    match identity {
        ExecutionIdentity::Role(role) => {
            let selection = selections
                .get_or_main(*role)
                .ok_or_else(|| HubError::NoModelConfigured {
                    identity: format!("role:{role}"),
                })?
                .clone();

            Ok((selection.model.clone(), Some(selection)))
        }
        ExecutionIdentity::Spec(spec) => {
            // Direct spec: no selection override
            Ok((spec.clone(), None))
        }
        ExecutionIdentity::Inherit => {
            let spec = parent_spec.ok_or(HubError::InheritWithoutParent)?.clone();
            Ok((spec, None))
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when working with ModelHub.
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    /// No model is configured for the requested identity.
    #[error("No model configured for {identity}")]
    NoModelConfigured { identity: String },

    /// Inherit identity used without parent context.
    #[error("Inherit identity requires parent_spec but none was provided")]
    InheritWithoutParent,

    /// Failed to resolve provider configuration.
    #[error("Failed to resolve provider '{provider}': {source}")]
    ProviderResolution {
        provider: String,
        #[source]
        source: anyhow::Error,
    },

    /// Failed to create provider instance.
    #[error("Failed to create provider '{provider}': {source}")]
    ProviderCreation {
        provider: String,
        #[source]
        source: crate::error::ApiError,
    },

    /// Failed to create model instance.
    #[error("Failed to create model '{model}' from provider '{provider}': {source}")]
    ModelCreation {
        provider: String,
        model: String,
        #[source]
        source: crate::error::ApiError,
    },

    /// Failed to resolve model info.
    #[error("Failed to resolve model info for '{model}': {source}")]
    ModelInfoResolution {
        model: String,
        #[source]
        source: anyhow::Error,
    },

    /// Internal lock was poisoned.
    #[error("Internal lock poisoned")]
    LockPoisoned,
}

impl HubError {
    /// Check if this error indicates no model is configured.
    pub fn is_no_model_configured(&self) -> bool {
        matches!(self, Self::NoModelConfigured { .. })
    }
}

// ============================================================================
// Cached Types
// ============================================================================

/// A cached provider instance.
struct CachedProvider {
    provider: Arc<dyn Provider>,
    provider_type: ProviderType,
}

/// A cached model instance with resolved info.
struct CachedModel {
    model: Arc<dyn Model>,
    model_info: ModelInfo,
    provider_type: ProviderType,
}

// ============================================================================
// ModelHub
// ============================================================================

/// Unified model hub for model acquisition and caching.
///
/// `ModelHub` is a role-agnostic service that:
/// - Acquires model instances from providers
/// - Caches providers and models for reuse
/// - Builds `InferenceContext` for request building
///
/// # Stateless Design
///
/// ModelHub does NOT own or manage `RoleSelections`. Instead:
/// - Session owns its `RoleSelections`
/// - Callers resolve `ExecutionIdentity → ModelSpec` using `resolve_identity()`
/// - Then call ModelHub with the resolved `ModelSpec`
///
/// This design enables proper session isolation - subagents receive cloned
/// selections at spawn time and are unaffected by parent model changes.
///
/// # Thread Safety
///
/// Uses `RwLock` for caches to allow concurrent reads with exclusive writes.
/// Model/provider creation happens outside locks to avoid blocking.
pub struct ModelHub {
    config: Arc<ConfigManager>,
    /// Cached providers keyed by provider name.
    providers: RwLock<HashMap<String, CachedProvider>>,
    /// Cached models keyed by ModelSpec.
    models: RwLock<HashMap<ModelSpec, CachedModel>>,
}

impl ModelHub {
    /// Create a new model hub.
    pub fn new(config: Arc<ConfigManager>) -> Self {
        Self {
            config,
            providers: RwLock::new(HashMap::new()),
            models: RwLock::new(HashMap::new()),
        }
    }

    // ========================================================================
    // Core API: Prepare Inference Context
    // ========================================================================

    /// Build a complete inference context from a resolved model spec.
    ///
    /// This is the **recommended** entry point. The caller is responsible for:
    /// 1. Resolving `ExecutionIdentity → ModelSpec` using `resolve_identity()`
    /// 2. Calling this method with the resolved spec
    ///
    /// # Arguments
    ///
    /// * `spec` - Already-resolved model specification
    /// * `session_id` - Session ID for correlation
    /// * `turn_number` - Turn number within the session
    /// * `agent_kind` - Type of agent making the request
    /// * `thinking_level` - Optional thinking level override
    /// * `original_identity` - The original identity (for telemetry)
    ///
    /// # Returns
    ///
    /// A tuple of (InferenceContext, Model) on success.
    pub fn build_context(
        &self,
        spec: &ModelSpec,
        session_id: &str,
        turn_number: i32,
        agent_kind: AgentKind,
        thinking_level: Option<ThinkingLevel>,
        original_identity: ExecutionIdentity,
    ) -> Result<(InferenceContext, Arc<dyn Model>), HubError> {
        // Get or create model
        let (model, model_info, _provider_type) = self.get_or_create_model(spec)?;

        // Build inference context
        let call_id = Uuid::new_v4().to_string();
        let mut ctx = InferenceContext::new(
            call_id,
            session_id,
            turn_number,
            spec.clone(),
            model_info,
            agent_kind,
            original_identity,
        );

        // Apply thinking level if provided
        if let Some(level) = thinking_level {
            ctx = ctx.with_thinking_level(level);
        }

        debug!(
            call_id = %ctx.call_id,
            model = %ctx.model_spec,
            thinking = ?ctx.thinking_level,
            "Built inference context"
        );

        Ok((ctx, model))
    }

    /// Prepare inference context with selections passed as parameter.
    ///
    /// This is the main entry point that:
    /// 1. Resolves the `ExecutionIdentity` to a `ModelSpec` using provided selections
    /// 2. Gets or creates the model instance
    /// 3. Builds `InferenceContext` ready for `RequestBuilder`
    ///
    /// # Arguments
    ///
    /// * `identity` - How to resolve the model (Role, Spec, or Inherit)
    /// * `selections` - Role selections (owned by Session, passed as parameter)
    /// * `session_id` - Session ID for correlation
    /// * `turn_number` - Turn number within the session
    /// * `agent_kind` - Type of agent making the request
    /// * `parent_spec` - Parent model spec for Inherit identity
    pub fn prepare_inference_with_selections(
        &self,
        identity: &ExecutionIdentity,
        selections: &RoleSelections,
        session_id: &str,
        turn_number: i32,
        agent_kind: AgentKind,
        parent_spec: Option<&ModelSpec>,
    ) -> Result<(InferenceContext, Arc<dyn Model>), HubError> {
        // Step 1: Resolve identity to spec and selection
        let (spec, selection) = resolve_identity(identity, selections, parent_spec)?;

        // Step 2: Get thinking level from selection
        let thinking_level = selection.and_then(|s| s.thinking_level);

        // Step 3: Build context with resolved spec
        self.build_context(
            &spec,
            session_id,
            turn_number,
            agent_kind,
            thinking_level,
            identity.clone(),
        )
    }

    /// Convenience: prepare context for main conversation with selections.
    pub fn prepare_main_with_selections(
        &self,
        selections: &RoleSelections,
        session_id: &str,
        turn_number: i32,
    ) -> Result<(InferenceContext, Arc<dyn Model>), HubError> {
        self.prepare_inference_with_selections(
            &ExecutionIdentity::main(),
            selections,
            session_id,
            turn_number,
            AgentKind::Main,
            None,
        )
    }

    /// Convenience: prepare context for compaction with selections.
    pub fn prepare_compact_with_selections(
        &self,
        selections: &RoleSelections,
        session_id: &str,
        turn_number: i32,
    ) -> Result<(InferenceContext, Arc<dyn Model>), HubError> {
        self.prepare_inference_with_selections(
            &ExecutionIdentity::compact(),
            selections,
            session_id,
            turn_number,
            AgentKind::Compaction,
            None,
        )
    }

    // ========================================================================
    // Model Access (Direct - Role-Agnostic)
    // ========================================================================

    /// Get model by explicit ModelSpec.
    ///
    /// This is the core model acquisition method - completely role-agnostic.
    pub fn get_model(&self, spec: &ModelSpec) -> Result<(Arc<dyn Model>, ProviderType), HubError> {
        self.get_or_create_model(spec).map(|(m, _, pt)| (m, pt))
    }

    /// Get model and info by explicit ModelSpec.
    ///
    /// Returns the model instance, model info, and provider type.
    pub fn get_model_with_info(
        &self,
        spec: &ModelSpec,
    ) -> Result<(Arc<dyn Model>, ModelInfo, ProviderType), HubError> {
        self.get_or_create_model(spec)
    }

    /// Get model for a role using provided selections.
    ///
    /// Use `prepare_inference_with_selections()` when you need the full `InferenceContext`.
    /// This method is for cases where you only need the model instance.
    pub fn get_model_for_role_with_selections(
        &self,
        role: ModelRole,
        selections: &RoleSelections,
    ) -> Result<(Arc<dyn Model>, ProviderType), HubError> {
        let selection =
            selections
                .get_or_main(role)
                .ok_or_else(|| HubError::NoModelConfigured {
                    identity: format!("role:{role}"),
                })?;

        let spec = &selection.model;
        self.get_or_create_model(spec).map(|(m, _, pt)| (m, pt))
    }

    // ========================================================================
    // Cache Management
    // ========================================================================

    /// Invalidate cached model for a specific spec.
    pub fn invalidate_model(&self, spec: &ModelSpec) {
        if let Ok(mut cache) = self.models.write()
            && cache.remove(spec).is_some()
        {
            debug!(
                provider = %spec.provider,
                model = %spec.model,
                "Invalidated cached model"
            );
        }
    }

    /// Invalidate cached provider (and all its models).
    pub fn invalidate_provider(&self, provider_name: &str) {
        // Remove provider
        if let Ok(mut cache) = self.providers.write()
            && cache.remove(provider_name).is_some()
        {
            debug!(provider = %provider_name, "Invalidated cached provider");
        }

        // Remove all models for this provider
        if let Ok(mut cache) = self.models.write() {
            let to_remove: Vec<ModelSpec> = cache
                .keys()
                .filter(|spec| spec.provider == provider_name)
                .cloned()
                .collect();

            for spec in to_remove {
                cache.remove(&spec);
                debug!(
                    provider = %spec.provider,
                    model = %spec.model,
                    "Invalidated cached model (provider invalidation)"
                );
            }
        }
    }

    /// Invalidate all caches.
    pub fn invalidate_all(&self) {
        if let Ok(mut cache) = self.providers.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.models.write() {
            cache.clear();
        }
        debug!("Invalidated all cached providers and models");
    }

    /// Get the number of cached providers.
    pub fn provider_cache_size(&self) -> usize {
        self.providers.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Get the number of cached models.
    pub fn model_cache_size(&self) -> usize {
        self.models.read().map(|c| c.len()).unwrap_or(0)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Get or create a model instance, returning model, info, and provider type.
    fn get_or_create_model(
        &self,
        spec: &ModelSpec,
    ) -> Result<(Arc<dyn Model>, ModelInfo, ProviderType), HubError> {
        // Phase 1: Check model cache (read lock)
        {
            let cache = self.models.read().map_err(|_| HubError::LockPoisoned)?;
            if let Some(cached) = cache.get(spec) {
                debug!(
                    provider = %spec.provider,
                    model = %spec.model,
                    "Model cache hit"
                );
                return Ok((
                    cached.model.clone(),
                    cached.model_info.clone(),
                    cached.provider_type,
                ));
            }
        }

        // Phase 2: Get or create provider
        let (provider, provider_type) = self.get_or_create_provider(&spec.provider)?;

        // Phase 3: Resolve provider info and model info
        let provider_info = self.config.resolve_provider(&spec.provider).map_err(|e| {
            HubError::ProviderResolution {
                provider: spec.provider.clone(),
                source: e.into(),
            }
        })?;

        // Get model info from config and convert to protocol type
        let resolved_info = self
            .config
            .resolve_model_info(&spec.provider, &spec.model)
            .map_err(|e| HubError::ModelInfoResolution {
                model: spec.to_string(),
                source: e.into(),
            })?;
        let model_info = to_model_info(resolved_info);

        // Get the actual API model name (handles aliases)
        let api_model_name = provider_info
            .api_model_name(&spec.model)
            .unwrap_or(&spec.model);

        info!(
            provider = %spec.provider,
            model = %spec.model,
            api_model = %api_model_name,
            "Creating model"
        );

        let model = provider
            .model(api_model_name)
            .map_err(|e| HubError::ModelCreation {
                provider: spec.provider.clone(),
                model: spec.model.clone(),
                source: e.into(),
            })?;

        // Phase 4: Double-check and store in model cache (write lock)
        {
            let mut cache = self.models.write().map_err(|_| HubError::LockPoisoned)?;

            // Another thread might have created it
            if let Some(cached) = cache.get(spec) {
                debug!(
                    provider = %spec.provider,
                    model = %spec.model,
                    "Model created by another thread, using existing"
                );
                return Ok((
                    cached.model.clone(),
                    cached.model_info.clone(),
                    cached.provider_type,
                ));
            }

            cache.insert(
                spec.clone(),
                CachedModel {
                    model: model.clone(),
                    model_info: model_info.clone(),
                    provider_type,
                },
            );
        }

        Ok((model, model_info, provider_type))
    }

    /// Get or create a provider instance.
    fn get_or_create_provider(
        &self,
        provider_name: &str,
    ) -> Result<(Arc<dyn Provider>, ProviderType), HubError> {
        // Phase 1: Check provider cache (read lock)
        {
            let cache = self.providers.read().map_err(|_| HubError::LockPoisoned)?;
            if let Some(cached) = cache.get(provider_name) {
                debug!(provider = %provider_name, "Provider cache hit");
                return Ok((cached.provider.clone(), cached.provider_type));
            }
        }

        // Phase 2: Resolve provider info and create provider
        let provider_info = self.config.resolve_provider(provider_name).map_err(|e| {
            HubError::ProviderResolution {
                provider: provider_name.to_string(),
                source: e.into(),
            }
        })?;

        info!(provider = %provider_name, "Creating provider");
        let provider = provider_factory::create_provider(&provider_info).map_err(|e| {
            HubError::ProviderCreation {
                provider: provider_name.to_string(),
                source: e,
            }
        })?;
        let provider_type = provider_info.provider_type;

        // Phase 3: Double-check and store in cache (write lock)
        {
            let mut cache = self.providers.write().map_err(|_| HubError::LockPoisoned)?;

            // Another thread might have created it
            if let Some(cached) = cache.get(provider_name) {
                debug!(
                    provider = %provider_name,
                    "Provider created by another thread, using existing"
                );
                return Ok((cached.provider.clone(), cached.provider_type));
            }

            cache.insert(
                provider_name.to_string(),
                CachedProvider {
                    provider: provider.clone(),
                    provider_type,
                },
            );
        }

        Ok((provider, provider_type))
    }
}

impl std::fmt::Debug for ModelHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelHub")
            .field("provider_cache_size", &self.provider_cache_size())
            .field("model_cache_size", &self.model_cache_size())
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_error_no_model_configured() {
        let err = HubError::NoModelConfigured {
            identity: "role:main".to_string(),
        };
        assert!(err.is_no_model_configured());
        assert!(err.to_string().contains("No model configured"));
    }

    #[test]
    fn test_hub_error_inherit_without_parent() {
        let err = HubError::InheritWithoutParent;
        assert!(!err.is_no_model_configured());
        assert!(err.to_string().contains("parent_spec"));
    }

    #[test]
    fn test_hub_new() {
        let config = ConfigManager::empty();
        let hub = ModelHub::new(Arc::new(config));
        assert_eq!(hub.provider_cache_size(), 0);
        assert_eq!(hub.model_cache_size(), 0);
    }

    #[test]
    fn test_hub_debug() {
        let config = ConfigManager::empty();
        let hub = ModelHub::new(Arc::new(config));
        let debug_str = format!("{:?}", hub);
        assert!(debug_str.contains("ModelHub"));
        assert!(debug_str.contains("provider_cache_size"));
        assert!(debug_str.contains("model_cache_size"));
    }

    // ========================================================================
    // resolve_identity() function tests (now standalone)
    // ========================================================================

    #[test]
    fn test_resolve_identity_role() {
        let mut selections = RoleSelections::default();
        selections.set(
            ModelRole::Main,
            RoleSelection::with_thinking(
                ModelSpec::new("anthropic", "claude-opus-4"),
                ThinkingLevel::high(),
            ),
        );

        let result = resolve_identity(&ExecutionIdentity::main(), &selections, None);
        assert!(result.is_ok());

        let (spec, selection) = result.unwrap();
        assert_eq!(spec.provider, "anthropic");
        assert_eq!(spec.model, "claude-opus-4");
        assert!(selection.is_some());
        assert!(selection.unwrap().thinking_level.is_some());
    }

    #[test]
    fn test_resolve_identity_spec() {
        let selections = RoleSelections::default();
        let direct_spec = ModelSpec::new("openai", "gpt-5");

        let result = resolve_identity(
            &ExecutionIdentity::Spec(direct_spec.clone()),
            &selections,
            None,
        );

        assert!(result.is_ok());
        let (spec, selection) = result.unwrap();
        assert_eq!(spec, direct_spec);
        assert!(selection.is_none()); // No selection for direct spec
    }

    #[test]
    fn test_resolve_identity_inherit() {
        let selections = RoleSelections::default();
        let parent = ModelSpec::new("anthropic", "claude-opus-4");

        let result = resolve_identity(&ExecutionIdentity::Inherit, &selections, Some(&parent));

        assert!(result.is_ok());
        let (spec, _) = result.unwrap();
        assert_eq!(spec, parent);
    }

    #[test]
    fn test_resolve_identity_inherit_without_parent_returns_error() {
        let selections = RoleSelections::default();

        let result = resolve_identity(&ExecutionIdentity::Inherit, &selections, None);

        assert!(result.is_err());
        matches!(result.unwrap_err(), HubError::InheritWithoutParent);
    }

    #[test]
    fn test_resolve_identity_empty_selections_returns_error() {
        let selections = RoleSelections::default();

        let result = resolve_identity(&ExecutionIdentity::main(), &selections, None);

        assert!(result.is_err());
        assert!(result.unwrap_err().is_no_model_configured());
    }

    #[test]
    fn test_resolve_identity_role_fallback_to_main() {
        let mut selections = RoleSelections::default();
        // Only set Main, not Fast
        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );

        // Fast should fall back to Main
        let result = resolve_identity(&ExecutionIdentity::fast(), &selections, None);
        assert!(result.is_ok());

        let (spec, _) = result.unwrap();
        assert_eq!(spec.model, "claude-opus-4"); // Got Main's model
    }

    // ========================================================================
    // Hub method tests (using selections as parameter)
    // ========================================================================

    #[test]
    fn test_hub_prepare_main_with_selections_empty_selections_returns_error() {
        let config = ConfigManager::empty();
        let hub = ModelHub::new(Arc::new(config));
        let selections = RoleSelections::default();

        let result = hub.prepare_main_with_selections(&selections, "session-123", 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_no_model_configured());
    }

    #[test]
    fn test_hub_get_model_for_role_with_selections() {
        let config = ConfigManager::empty();
        let hub = ModelHub::new(Arc::new(config));

        let mut selections = RoleSelections::default();
        selections.set(
            ModelRole::Main,
            RoleSelection::new(ModelSpec::new("anthropic", "claude-opus-4")),
        );

        // This will fail because we don't have the actual provider configured,
        // but the error should be about provider resolution, not selection lookup
        let result = hub.get_model_for_role_with_selections(ModelRole::Main, &selections);
        assert!(result.is_err());
        // Should be a provider error, not "no model configured"
        assert!(!result.unwrap_err().is_no_model_configured());
    }

    #[test]
    fn test_hub_get_model_for_role_with_selections_empty_returns_error() {
        let config = ConfigManager::empty();
        let hub = ModelHub::new(Arc::new(config));
        let selections = RoleSelections::default();

        let result = hub.get_model_for_role_with_selections(ModelRole::Main, &selections);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_no_model_configured());
    }
}
