//! HTTP interceptor registry for cocode-config.
//!
//! This module provides a global registry for HTTP interceptors, allowing
//! configuration files to reference interceptors by name.
//!
//! # Architecture
//!
//! ```text
//! providers.json: interceptors = ["byted_model_hub"]
//!   └── resolve_chain(names) → HttpInterceptorChain
//!       └── get_interceptor("byted_model_hub") → Arc<dyn HttpInterceptor>
//! ```
//!
//! # Built-in Interceptors
//!
//! - `byted_model_hub` - Adds session_id to "extra" header for ByteDance ModelHub
//!
//! # Example
//!
//! ```no_run
//! use cocode_config::interceptors::{
//!     get_interceptor, resolve_chain, list_interceptors,
//!     HttpRequest, HttpInterceptorContext,  // Re-exported from hyper-sdk
//! };
//!
//! // Get an interceptor by name
//! if let Some(interceptor) = get_interceptor("byted_model_hub") {
//!     println!("Found: {}", interceptor.name());
//! }
//!
//! // Resolve a chain from config names
//! let mut chain = resolve_chain(&["byted_model_hub".to_string()]);
//! assert_eq!(chain.len(), 1);
//!
//! // Apply the chain to a request
//! let mut request = HttpRequest::post("https://api.example.com/v1/chat");
//! let ctx = HttpInterceptorContext::new().conversation_id("session-123");
//! chain.apply(&mut request, &ctx);
//!
//! // List all registered interceptors
//! let names = list_interceptors();
//! assert!(names.contains(&"byted_model_hub".to_string()));
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

// Re-export hyper-sdk types for convenience.
pub use hyper_sdk::http_interceptors::BytedModelHubInterceptor;
pub use hyper_sdk::http_interceptors::HttpInterceptor;
pub use hyper_sdk::http_interceptors::HttpInterceptorChain;
pub use hyper_sdk::http_interceptors::HttpInterceptorContext;
pub use hyper_sdk::http_interceptors::HttpRequest;

/// Thread-safe registry for HTTP interceptors.
#[derive(Debug, Default)]
struct InterceptorRegistry {
    interceptors: RwLock<HashMap<String, Arc<dyn HttpInterceptor>>>,
}

impl InterceptorRegistry {
    fn new() -> Self {
        Self {
            interceptors: RwLock::new(HashMap::new()),
        }
    }

    fn register(&self, interceptor: Arc<dyn HttpInterceptor>) {
        let name = interceptor.name().to_string();
        let mut interceptors = self.interceptors.write().expect("lock poisoned");
        interceptors.insert(name, interceptor);
    }

    fn get(&self, name: &str) -> Option<Arc<dyn HttpInterceptor>> {
        let interceptors = self.interceptors.read().expect("lock poisoned");
        interceptors.get(name).cloned()
    }

    fn list(&self) -> Vec<String> {
        let interceptors = self.interceptors.read().expect("lock poisoned");
        interceptors.keys().cloned().collect()
    }
}

/// Global interceptor registry with built-in interceptors pre-registered.
static INTERCEPTOR_REGISTRY: LazyLock<InterceptorRegistry> = LazyLock::new(|| {
    let registry = InterceptorRegistry::new();
    // Register built-in interceptors
    registry.register(Arc::new(BytedModelHubInterceptor));
    registry
});

/// Get an interceptor by name from the global registry.
///
/// Returns `None` if the interceptor is not found.
///
/// # Example
///
/// ```no_run
/// use cocode_config::interceptors::get_interceptor;
///
/// if let Some(interceptor) = get_interceptor("byted_model_hub") {
///     println!("Found interceptor: {}", interceptor.name());
/// }
/// ```
pub fn get_interceptor(name: &str) -> Option<Arc<dyn HttpInterceptor>> {
    INTERCEPTOR_REGISTRY.get(name)
}

/// Register a custom interceptor in the global registry.
///
/// If an interceptor with the same name already exists, it will be replaced.
///
/// # Example
///
/// ```no_run
/// use cocode_config::interceptors::register_interceptor;
/// use hyper_sdk::http_interceptors::{HttpInterceptor, HttpInterceptorContext, HttpRequest};
/// use std::sync::Arc;
///
/// #[derive(Debug)]
/// struct MyInterceptor;
///
/// impl HttpInterceptor for MyInterceptor {
///     fn name(&self) -> &str { "my_interceptor" }
///     fn intercept(&self, _: &mut HttpRequest, _: &HttpInterceptorContext) {}
/// }
///
/// register_interceptor(Arc::new(MyInterceptor));
/// ```
pub fn register_interceptor(interceptor: Arc<dyn HttpInterceptor>) {
    INTERCEPTOR_REGISTRY.register(interceptor);
}

/// List all registered interceptor names.
///
/// # Example
///
/// ```no_run
/// use cocode_config::interceptors::list_interceptors;
///
/// let names = list_interceptors();
/// println!("Available interceptors: {:?}", names);
/// ```
pub fn list_interceptors() -> Vec<String> {
    INTERCEPTOR_REGISTRY.list()
}

/// Resolve a chain of interceptors from configuration names.
///
/// Unknown interceptor names are silently ignored (with a warning logged).
/// Returns an `HttpInterceptorChain` containing all found interceptors.
///
/// # Example
///
/// ```no_run
/// use cocode_config::interceptors::resolve_chain;
///
/// let chain = resolve_chain(&["byted_model_hub".to_string()]);
/// assert_eq!(chain.len(), 1);
/// ```
pub fn resolve_chain(names: &[String]) -> HttpInterceptorChain {
    let mut chain = HttpInterceptorChain::new();
    for name in names {
        if let Some(interceptor) = get_interceptor(name) {
            chain.add(interceptor);
        } else {
            tracing::warn!("Unknown HTTP interceptor: {name}");
        }
    }
    chain
}

/// Apply interceptors to a request.
///
/// This is a convenience function that resolves a chain and applies it
/// in a single call.
///
/// # Example
///
/// ```no_run
/// use cocode_config::interceptors::apply_interceptors;
/// use hyper_sdk::http_interceptors::{HttpRequest, HttpInterceptorContext};
///
/// let mut request = HttpRequest::post("https://api.example.com/v1/chat");
/// let ctx = HttpInterceptorContext::new().conversation_id("session-123");
///
/// apply_interceptors(&mut request, &ctx, &["byted_model_hub".to_string()]);
/// ```
pub fn apply_interceptors(
    request: &mut hyper_sdk::http_interceptors::HttpRequest,
    ctx: &hyper_sdk::http_interceptors::HttpInterceptorContext,
    interceptor_names: &[String],
) {
    let mut chain = resolve_chain(interceptor_names);
    chain.apply(request, ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_interceptors_registered() {
        let interceptor = get_interceptor("byted_model_hub");
        assert!(interceptor.is_some());
        assert_eq!(
            interceptor.as_ref().map(|i| i.name()),
            Some("byted_model_hub")
        );
    }

    #[test]
    fn test_list_interceptors() {
        let interceptors = list_interceptors();
        assert!(interceptors.contains(&"byted_model_hub".to_string()));
    }

    #[test]
    fn test_get_unknown_interceptor() {
        let interceptor = get_interceptor("unknown");
        assert!(interceptor.is_none());
    }

    #[test]
    fn test_resolve_chain() {
        let chain = resolve_chain(&["byted_model_hub".to_string()]);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.names(), vec!["byted_model_hub"]);
    }

    #[test]
    fn test_resolve_chain_with_unknown() {
        let chain = resolve_chain(&["byted_model_hub".to_string(), "unknown".to_string()]);
        // Only the known interceptor should be in the chain
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_resolve_chain_empty() {
        let chain = resolve_chain(&[]);
        assert!(chain.is_empty());
    }
}
