use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use tempfile::NamedTempFile;

const ARTIFACTS_DIR: &str = "artifacts";
const MEMORY_DIR: &str = "memory";
const INDEX_DIR: &str = "index";
const DELIVERABLE_DIR: &str = "deliverable";
const METADATA_FILE: &str = "run.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleMetadata {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    pub run_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub roles: Vec<RoleMetadata>,
}

#[derive(Debug, Clone)]
pub struct RunStore {
    path: PathBuf,
    metadata: RunMetadata,
}

impl RunStore {
    pub fn initialize(
        run_path: impl AsRef<Path>,
        run_id: &str,
        roles: &[RoleMetadata],
    ) -> Result<Self> {
        let run_path = run_path.as_ref().to_path_buf();
        fs::create_dir_all(&run_path)
            .with_context(|| format!("failed to create run directory {}", run_path.display()))?;

        for child in [ARTIFACTS_DIR, MEMORY_DIR, INDEX_DIR, DELIVERABLE_DIR] {
            fs::create_dir_all(run_path.join(child))
                .with_context(|| format!("failed to create subdirectory {child}"))?;
        }

        let metadata_path = run_path.join(METADATA_FILE);
        if metadata_path.exists() {
            return Err(anyhow!(
                "run metadata already exists at {}",
                metadata_path.display()
            ));
        }

        let now = Utc::now();
        let metadata = RunMetadata {
            run_id: run_id.to_string(),
            created_at: now,
            updated_at: now,
            roles: roles.to_vec(),
        };
        write_metadata(&metadata_path, &metadata)?;

        Ok(Self {
            path: run_path,
            metadata,
        })
    }

    pub fn load(run_path: impl AsRef<Path>) -> Result<Self> {
        let run_path = run_path.as_ref().to_path_buf();
        let metadata_path = run_path.join(METADATA_FILE);
        let metadata: RunMetadata = serde_json::from_slice(
            &fs::read(&metadata_path)
                .with_context(|| format!("failed to read {}", metadata_path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", metadata_path.display()))?;

        Ok(Self {
            path: run_path,
            metadata,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> &RunMetadata {
        &self.metadata
    }

    pub fn role_metadata(&self, role: &str) -> Option<&RoleMetadata> {
        self.metadata.roles.iter().find(|meta| meta.role == role)
    }

    pub fn update_rollout_path(&mut self, role: &str, rollout_path: PathBuf) -> Result<()> {
        if let Some(meta) = self
            .metadata
            .roles
            .iter_mut()
            .find(|meta| meta.role == role)
        {
            meta.rollout_path = Some(rollout_path);
            self.commit_metadata()
        } else {
            Err(anyhow!("role {role} not found in run store"))
        }
    }

    pub fn set_role_config_path(&mut self, role: &str, path: PathBuf) -> Result<()> {
        if let Some(meta) = self
            .metadata
            .roles
            .iter_mut()
            .find(|meta| meta.role == role)
        {
            meta.config_path = Some(path);
            self.commit_metadata()
        } else {
            Err(anyhow!("role {role} not found in run store"))
        }
    }

    pub fn touch(&mut self) -> Result<()> {
        self.metadata.updated_at = Utc::now();
        self.commit_metadata()
    }

    fn commit_metadata(&mut self) -> Result<()> {
        self.metadata.updated_at = Utc::now();
        let metadata_path = self.path.join(METADATA_FILE);
        write_metadata(&metadata_path, &self.metadata)
    }
}

fn write_metadata(path: &Path, metadata: &RunMetadata) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("metadata path must have parent"))?;
    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    serde_json::to_writer_pretty(&mut temp, metadata)?;
    temp.flush()?;
    temp.persist(path)
        .with_context(|| format!("failed to persist metadata to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn initialize_creates_directories_and_metadata() {
        let temp = TempDir::new().unwrap();
        let run_path = temp.path().join("run_1");
        let roles = vec![
            RoleMetadata {
                role: "solver".into(),
                rollout_path: None,
                config_path: None,
            },
            RoleMetadata {
                role: "director".into(),
                rollout_path: None,
                config_path: None,
            },
        ];

        let store = RunStore::initialize(&run_path, "run_1", &roles).unwrap();
        assert!(store.path().join(ARTIFACTS_DIR).is_dir());
        assert!(store.path().join(MEMORY_DIR).is_dir());
        assert!(store.path().join(INDEX_DIR).is_dir());
        assert!(store.path().join(DELIVERABLE_DIR).is_dir());
        assert_eq!(store.metadata().roles.len(), 2);
    }

    #[test]
    fn update_rollout_persists_metadata() {
        let temp = TempDir::new().unwrap();
        let run_path = temp.path().join("run_2");
        let roles = vec![RoleMetadata {
            role: "solver".into(),
            rollout_path: None,
            config_path: None,
        }];
        let mut store = RunStore::initialize(&run_path, "run_2", &roles).unwrap();
        let rollout = PathBuf::from("/tmp/rollout.jsonl");
        store
            .update_rollout_path("solver", rollout.clone())
            .unwrap();

        let loaded = RunStore::load(&run_path).unwrap();
        let solver = loaded.role_metadata("solver").unwrap();
        assert_eq!(solver.rollout_path.as_ref().unwrap(), &rollout);
    }
}
