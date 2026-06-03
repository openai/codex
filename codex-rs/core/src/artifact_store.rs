use std::future::Future;
use std::pin::Pin;

use codex_protocol::error::Result;
use codex_utils_absolute_path::AbsolutePathBuf;

const GENERATED_IMAGE_ARTIFACTS_DIR: &str = "generated_images";

/// Stores generated artifacts and materializes executor-readable local paths.
pub trait ArtifactStore: Send + Sync + 'static {
    /// Returns the executor-readable path exposed to generated image instructions.
    fn generated_image_path(&self, session_id: &str, call_id: &str) -> AbsolutePathBuf;

    /// Persists a generated image payload and returns its executor-readable path.
    fn write_generated_image(
        &self,
        session_id: &str,
        call_id: &str,
        bytes: Vec<u8>,
    ) -> ArtifactWriteFuture<'_>;
}

/// Future returned by artifact writes.
pub type ArtifactWriteFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AbsolutePathBuf>> + Send + 'a>>;

#[derive(Clone)]
pub struct LocalArtifactStore {
    codex_home: AbsolutePathBuf,
}

impl LocalArtifactStore {
    pub fn from_codex_home(codex_home: &AbsolutePathBuf) -> Self {
        Self {
            codex_home: codex_home.clone(),
        }
    }
}

impl ArtifactStore for LocalArtifactStore {
    fn generated_image_path(&self, session_id: &str, call_id: &str) -> AbsolutePathBuf {
        self.codex_home
            .join(GENERATED_IMAGE_ARTIFACTS_DIR)
            .join(sanitize_artifact_path_component(session_id))
            .join(format!("{}.png", sanitize_artifact_path_component(call_id)))
    }

    fn write_generated_image(
        &self,
        session_id: &str,
        call_id: &str,
        bytes: Vec<u8>,
    ) -> ArtifactWriteFuture<'_> {
        let path = self.generated_image_path(session_id, call_id);
        Box::pin(async move {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, bytes).await?;
            Ok(path)
        })
    }
}

fn sanitize_artifact_path_component(value: &str) -> String {
    let mut sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        sanitized = "generated_image".to_string();
    }
    sanitized
}
