use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyJson {
    pub mode: String,
    #[serde(default)]
    pub workspace_roots: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
}

#[derive(Clone, Debug)]
pub struct SandboxPolicy(pub SandboxMode);

impl SandboxPolicy {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "read-only" => Ok(SandboxPolicy(SandboxMode::ReadOnly)),
            "workspace-write" => Ok(SandboxPolicy(SandboxMode::WorkspaceWrite)),
            other => {
                let pj: PolicyJson = serde_json::from_str(other)?;
                Ok(match pj.mode.as_str() {
                    "read-only" => SandboxPolicy(SandboxMode::ReadOnly),
                    "workspace-write" => SandboxPolicy(SandboxMode::WorkspaceWrite),
                    _ => SandboxPolicy(SandboxMode::ReadOnly),
                })
            }
        }
    }
}
