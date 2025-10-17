use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyJson {
    pub mode: String,
    #[serde(default)]
    pub workspace_roots: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite { workspace_roots: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct SandboxPolicy(pub SandboxMode);

impl SandboxPolicy {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "read-only" => Ok(SandboxPolicy(SandboxMode::ReadOnly)),
            "workspace-write" => Ok(SandboxPolicy(SandboxMode::WorkspaceWrite {
                workspace_roots: vec![],
            })),
            other => {
                let pj: PolicyJson = serde_json::from_str(other)?;
                Ok(match pj.mode.as_str() {
                    "read-only" => SandboxPolicy(SandboxMode::ReadOnly),
                    "workspace-write" => SandboxPolicy(SandboxMode::WorkspaceWrite {
                        workspace_roots: pj.workspace_roots,
                    }),
                    _ => SandboxPolicy(SandboxMode::ReadOnly),
                })
            }
        }
    }

    pub fn mode_str(&self) -> &'static str {
        match &self.0 {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite { .. } => "workspace-write",
        }
    }

    pub fn writable_roots_with_cwd(&self, policy_cwd: &Path) -> Vec<PathBuf> {
        match &self.0 {
            SandboxMode::ReadOnly => vec![],
            SandboxMode::WorkspaceWrite { workspace_roots } => workspace_roots
                .iter()
                .map(|p| {
                    let pb = Path::new(p);
                    if pb.is_absolute() {
                        pb.to_path_buf()
                    } else {
                        policy_cwd.join(pb)
                    }
                })
                .collect(),
        }
    }
}
