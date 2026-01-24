//! Local embedding provider using fastembed-rs (ONNX Runtime).
//!
//! Provides local text embeddings without external API dependencies.
//! Models are downloaded on first use and cached locally.
//!
//! ## Supported Models
//!
//! - `nomic-embed-text-v1.5`: Nomic AI embedding (768 dims, default)
//! - `bge-small-en-v1.5`: BAAI BGE small (384 dims)
//! - `all-MiniLM-L6-v2`: Sentence Transformers (384 dims)
//! - `mxbai-embed-large-v1`: MixedBread AI (1024 dims)
//!
//! ## Example
//!
//! ```toml
//! [retrieval.embedding]
//! provider = "fastembed"
//! model = "nomic-embed-text-v1.5"
//! ```

use async_trait::async_trait;
use fastembed::EmbeddingModel;
use fastembed::InitOptions;
use fastembed::TextEmbedding;

use crate::config::LocalEmbeddingConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::traits::EmbeddingProvider;

/// Local embedding provider using fastembed-rs.
pub struct FastembedEmbeddingProvider {
    model: TextEmbedding,
    model_name: String,
    dimension: i32,
}

impl std::fmt::Debug for FastembedEmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastembedEmbeddingProvider")
            .field("model_name", &self.model_name)
            .field("dimension", &self.dimension)
            .finish()
    }
}

impl FastembedEmbeddingProvider {
    /// Create a new fastembed embedding provider from config.
    pub fn new(config: &LocalEmbeddingConfig) -> Result<Self> {
        let (model_enum, dimension) = Self::parse_model_name(&config.model)?;

        let mut options =
            InitOptions::new(model_enum).with_show_download_progress(config.show_download_progress);

        if let Some(ref cache_dir) = config.cache_dir {
            options = options.with_cache_dir(cache_dir.clone());
        }

        let model = TextEmbedding::try_new(options).map_err(|e| RetrievalErr::EmbeddingFailed {
            cause: format!(
                "Failed to initialize fastembed model '{}': {}",
                config.model, e
            ),
        })?;

        Ok(Self {
            model,
            model_name: config.model.clone(),
            dimension,
        })
    }

    /// Parse model name string to fastembed enum and return dimension.
    fn parse_model_name(name: &str) -> Result<(EmbeddingModel, i32)> {
        let normalized = name.to_lowercase().replace(['-', '_'], "");
        let (model, dim) = match normalized.as_str() {
            // Nomic AI models (recommended default)
            "nomicembedtextv1.5" | "nomicembedtextv15" | "nomicembedtext" => {
                (EmbeddingModel::NomicEmbedTextV15, 768)
            }
            "nomicembedtextv1" => (EmbeddingModel::NomicEmbedTextV1, 768),

            // BGE models (BAAI)
            "bgesmallenv1.5" | "bgesmallenv15" | "bgesmall" => (EmbeddingModel::BGESmallENV15, 384),
            "bgebaseenv1.5" | "bgebaseenv15" | "bgebase" => (EmbeddingModel::BGEBaseENV15, 768),
            "bgelargeenv1.5" | "bgelargeenv15" | "bgelarge" => {
                (EmbeddingModel::BGELargeENV15, 1024)
            }

            // Sentence Transformers
            "allminilml6v2" | "minilml6" => (EmbeddingModel::AllMiniLML6V2, 384),
            "allminilml12v2" | "minilml12" => (EmbeddingModel::AllMiniLML12V2, 384),

            // MixedBread AI
            "mxbaiembedlargev1" | "mxbailarge" => (EmbeddingModel::MxbaiEmbedLargeV1, 1024),

            // Multilingual E5
            "multilinguale5small" | "e5small" => (EmbeddingModel::MultilingualE5Small, 384),
            "multilinguale5base" | "e5base" => (EmbeddingModel::MultilingualE5Base, 768),
            "multilinguale5large" | "e5large" => (EmbeddingModel::MultilingualE5Large, 1024),

            // GTE models (Alibaba)
            "gtebaseenv1.5" | "gtebase" => (EmbeddingModel::GTEBaseENV15, 768),
            "gtelargeenv1.5" | "gtelarge" => (EmbeddingModel::GTELargeENV15, 1024),

            // Jina AI
            "jinaembeddingsv2basecode" | "jinacode" => {
                (EmbeddingModel::JinaEmbeddingsV2BaseCode, 768)
            }

            // Default to nomic for unknown models
            _ => {
                return Err(RetrievalErr::ConfigError {
                    field: "embedding.model".to_string(),
                    cause: format!(
                        "Unknown fastembed model '{}'. Supported: nomic-embed-text-v1.5, \
                         bge-small-en-v1.5, bge-base-en-v1.5, all-MiniLM-L6-v2, \
                         mxbai-embed-large-v1, multilingual-e5-small/base/large",
                        name
                    ),
                });
            }
        };

        Ok((model, dim))
    }

    /// Embed texts synchronously (internal).
    fn embed_sync(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.model
            .embed(texts, None)
            .map_err(|e| RetrievalErr::EmbeddingFailed {
                cause: format!("Embedding failed: {}", e),
            })
    }
}

#[async_trait]
impl EmbeddingProvider for FastembedEmbeddingProvider {
    fn name(&self) -> &str {
        "fastembed"
    }

    fn dimension(&self) -> i32 {
        self.dimension
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let text = text.to_string();
        // Clone the model reference for the blocking task
        // Note: TextEmbedding is not Clone, so we do sync in the current thread
        // This is acceptable because embedding is typically fast (~10-50ms)
        let results = self.embed_sync(vec![text])?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| RetrievalErr::EmbeddingFailed {
                cause: "No embedding returned".to_string(),
            })
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        self.embed_sync(texts.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_name() {
        // Test various name formats
        let (model, dim) = FastembedEmbeddingProvider::parse_model_name("nomic-embed-text-v1.5")
            .expect("should parse");
        assert_eq!(dim, 768);
        assert!(matches!(model, EmbeddingModel::NomicEmbedTextV15));

        let (model, dim) = FastembedEmbeddingProvider::parse_model_name("bge-small-en-v1.5")
            .expect("should parse");
        assert_eq!(dim, 384);
        assert!(matches!(model, EmbeddingModel::BGESmallENV15));

        let (model, dim) =
            FastembedEmbeddingProvider::parse_model_name("all-MiniLM-L6-v2").expect("should parse");
        assert_eq!(dim, 384);
        assert!(matches!(model, EmbeddingModel::AllMiniLML6V2));
    }

    #[test]
    fn test_parse_model_name_unknown() {
        let result = FastembedEmbeddingProvider::parse_model_name("unknown-model");
        assert!(result.is_err());
    }
}
