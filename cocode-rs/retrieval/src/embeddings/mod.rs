//! Embedding providers for vector search.
//!
//! Provides implementations of the `EmbeddingProvider` trait for various
//! embedding services. Use `EmbeddingRegistry` for runtime provider lookup.
//!
//! ## Available Providers
//!
//! - `openai`: OpenAI Embeddings API (remote)
//! - `fastembed`: Local ONNX models via fastembed-rs (requires `local-embeddings` feature)

use std::collections::HashMap;
use std::sync::Arc;

use crate::traits::EmbeddingProvider;

pub mod cache;
pub mod openai;
pub mod queue;

#[cfg(feature = "local-embeddings")]
pub mod fastembed;

pub use cache::CacheLookupResult;
pub use cache::EmbeddingCache;
pub use openai::OpenAIEmbeddings;
pub use queue::EmbeddingQueue;

#[cfg(feature = "local-embeddings")]
pub use fastembed::FastembedEmbeddingProvider;

/// Registry for embedding providers.
///
/// Allows runtime lookup of providers by name for configuration-driven setup.
///
/// # Example
///
/// ```ignore
/// let mut registry = EmbeddingRegistry::new();
/// registry.register(Arc::new(OpenAIEmbeddings::new("api-key")));
///
/// if let Some(provider) = registry.get("openai") {
///     let embedding = provider.embed("hello world").await?;
/// }
/// ```
#[derive(Default)]
pub struct EmbeddingRegistry {
    providers: HashMap<String, Arc<dyn EmbeddingProvider>>,
}

impl EmbeddingRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider.
    ///
    /// Overwrites any existing provider with the same name.
    pub fn register(&mut self, provider: Arc<dyn EmbeddingProvider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn EmbeddingProvider>> {
        self.providers.get(name).cloned()
    }

    /// List registered provider names.
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider is registered.
    pub fn has(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }
}

impl std::fmt::Debug for EmbeddingRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingRegistry")
            .field("providers", &self.list())
            .finish()
    }
}
