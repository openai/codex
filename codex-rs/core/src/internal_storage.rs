use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const INTERNAL_STORAGE_FILE: &str = "internal_storage.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InternalStorage {
    #[serde(skip)]
    storage_path: PathBuf,
    #[serde(default)]
    pub gpt_5_codex_model_prompt_seen: bool,
}

// TODO(jif) generalise all the file writers and build proper async channel inserters.
impl InternalStorage {
    pub fn load(codex_home: &Path) -> Self {
        let storage_path = codex_home.join(INTERNAL_STORAGE_FILE);

        match std::fs::read_to_string(&storage_path) {
            Ok(serialized) => match serde_json::from_str::<Self>(&serialized) {
                Ok(mut storage) => {
                    storage.storage_path = storage_path;
                    storage
                }
                Err(error) => {
                    tracing::warn!("failed to parse internal storage: {error:?}");
                    Self::empty(storage_path)
                }
            },
            Err(error) => {
                if error.kind() == ErrorKind::NotFound {
                    tracing::debug!(
                        "internal storage not found at {}; initializing defaults",
                        storage_path.display()
                    );
                } else {
                    tracing::warn!("failed to read internal storage: {error:?}");
                }
                Self::empty(storage_path)
            }
        }
    }

    fn empty(storage_path: PathBuf) -> Self {
        Self {
            storage_path,
            ..Default::default()
        }
    }

    pub async fn persist(&self) -> anyhow::Result<()> {
        let serialized = serde_json::to_string_pretty(self)?;

        if let Some(parent) = self.storage_path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create internal storage directory at {}",
                    parent.display()
                )
            })?;
        }

        tokio::fs::write(&self.storage_path, serialized)
            .await
            .with_context(|| {
                format!(
                    "failed to persist internal storage at {}",
                    self.storage_path.display()
                )
            })
    }
}
