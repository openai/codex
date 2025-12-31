//! Request interceptor support for codex-api.
//!
//! Interceptors modify HTTP requests before they are sent.
//! Can modify: headers, URL, query params, body, timeout.
//!
//! # Architecture
//!
//! The interceptor system uses the same registry pattern as adapters:
//!
//! ```text
//! config (ext.interceptors = ["gpt_openapi"])
//!   └── Provider.interceptors: Vec<String>
//!       └── streaming.rs: apply_interceptors()
//!           └── get_interceptor(name) → interceptor.intercept(&mut Request, ctx)
//! ```
//!
//! # Built-in Interceptors
//!
//! - `gpt_openapi` - Adds session_id to "extra" header as JSON

pub mod gpt_openapi;

use codex_client::Request;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

/// Context passed to interceptors.
///
/// Contains request metadata that interceptors can use to modify requests.
#[derive(Debug, Clone, Default)]
pub struct InterceptorContext {
    /// Conversation/session ID for tracking.
    pub conversation_id: Option<String>,
    /// Model being used.
    pub model: Option<String>,
    /// Provider name.
    pub provider_name: Option<String>,
}

/// Trait for request interceptors.
///
/// Interceptors can modify the entire HTTP request before it's sent.
/// This includes headers, URL, body, and timeout.
///
/// # Example
///
/// ```ignore
/// #[derive(Debug)]
/// pub struct MyInterceptor;
///
/// impl Interceptor for MyInterceptor {
///     fn name(&self) -> &str {
///         "my_interceptor"
///     }
///
///     fn intercept(&self, request: &mut Request, ctx: &InterceptorContext) {
///         request.headers.insert("X-Custom", "value".parse().unwrap());
///     }
/// }
/// ```
pub trait Interceptor: Send + Sync + Debug {
    /// Unique name identifying this interceptor.
    fn name(&self) -> &str;

    /// Modify the request.
    ///
    /// Can modify any field: url, headers, body, timeout.
    fn intercept(&self, request: &mut Request, ctx: &InterceptorContext);
}

// ============================================================================
// Registry (same pattern as AdapterRegistry)
// ============================================================================

/// Thread-safe registry for request interceptors.
#[derive(Debug, Default)]
struct InterceptorRegistry {
    interceptors: RwLock<HashMap<String, Arc<dyn Interceptor>>>,
}

impl InterceptorRegistry {
    fn new() -> Self {
        Self {
            interceptors: RwLock::new(HashMap::new()),
        }
    }

    fn register(&self, interceptor: Arc<dyn Interceptor>) {
        let name = interceptor.name().to_string();
        let mut interceptors = self.interceptors.write().unwrap();
        interceptors.insert(name, interceptor);
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Interceptor>> {
        let interceptors = self.interceptors.read().unwrap();
        interceptors.get(name).cloned()
    }

    fn list(&self) -> Vec<String> {
        let interceptors = self.interceptors.read().unwrap();
        interceptors.keys().cloned().collect()
    }
}

/// Global interceptor registry with built-in interceptors pre-registered.
static INTERCEPTOR_REGISTRY: LazyLock<InterceptorRegistry> = LazyLock::new(|| {
    let registry = InterceptorRegistry::new();
    // Register built-in interceptors
    registry.register(Arc::new(gpt_openapi::GptOpenapiInterceptor));
    registry
});

/// Get an interceptor by name from the global registry.
///
/// Returns `None` if the interceptor is not found.
///
/// # Example
///
/// ```ignore
/// if let Some(interceptor) = get_interceptor("gpt_openapi") {
///     interceptor.intercept(&mut request, &ctx);
/// }
/// ```
pub fn get_interceptor(name: &str) -> Option<Arc<dyn Interceptor>> {
    INTERCEPTOR_REGISTRY.get(name)
}

/// Register a custom interceptor in the global registry.
///
/// If an interceptor with the same name already exists, it will be replaced.
pub fn register_interceptor(interceptor: Arc<dyn Interceptor>) {
    INTERCEPTOR_REGISTRY.register(interceptor);
}

/// List all registered interceptor names.
pub fn list_interceptors() -> Vec<String> {
    INTERCEPTOR_REGISTRY.list()
}

// ============================================================================
// Helper function for applying interceptors
// ============================================================================

/// Apply all configured interceptors to a request.
///
/// This is called after the request is built but before it's sent.
/// Interceptors are applied in the order they appear in the list.
pub fn apply_interceptors(
    request: &mut Request,
    ctx: &InterceptorContext,
    interceptor_names: &[String],
) {
    for name in interceptor_names {
        if let Some(interceptor) = get_interceptor(name) {
            interceptor.intercept(request, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_interceptors_registered() {
        // GptOpenapiInterceptor should be pre-registered
        let interceptor = get_interceptor("gpt_openapi");
        assert!(interceptor.is_some());
        assert_eq!(interceptor.unwrap().name(), "gpt_openapi");
    }

    #[test]
    fn test_list_interceptors() {
        let interceptors = list_interceptors();
        assert!(interceptors.contains(&"gpt_openapi".to_string()));
    }

    #[test]
    fn test_get_unknown_interceptor() {
        let interceptor = get_interceptor("unknown");
        assert!(interceptor.is_none());
    }
}
