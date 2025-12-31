//! Query processing module.
//!
//! Provides query preprocessing, tokenization, and rewriting.
//!
//! ## LLM Providers
//!
//! - `openai`: OpenAI Chat Completions API (remote)
//! - `ollama`: Ollama local LLM (requires `ollama serve`)

pub mod cache;
pub mod llm_provider;
pub mod ollama_provider;
pub mod preprocessor;
pub mod rewriter;
pub mod semantic_cache;
pub mod service;

pub use cache::CacheStats;
pub use cache::RewriteCache;
pub use llm_provider::CompletionRequest;
pub use llm_provider::CompletionResponse;
pub use llm_provider::LlmProvider;
pub use llm_provider::LlmRewriteResponse;
pub use llm_provider::NoopProvider;
pub use llm_provider::OpenAiProvider;
pub use llm_provider::QUERY_REWRITE_SYSTEM_PROMPT;
pub use ollama_provider::OllamaLlmProvider;
pub use preprocessor::ProcessedQuery;
pub use preprocessor::QueryPreprocessor;
pub use preprocessor::QueryType;
pub use rewriter::ExpansionType;
pub use rewriter::IntentBoosts;
pub use rewriter::LlmRewriter;
pub use rewriter::QueryExpansion;
pub use rewriter::QueryIntent;
pub use rewriter::QueryRewriter;
pub use rewriter::RewriteSource;
pub use rewriter::RewrittenQuery;
pub use rewriter::SimpleRewriter;
pub use rewriter::Translator;
pub use semantic_cache::CacheStats as SemanticCacheStats;
pub use semantic_cache::SemanticCacheConfig;
pub use semantic_cache::SemanticQueryCache;
pub use service::QueryRewriteService;
