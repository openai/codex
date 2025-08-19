use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display as DeriveDisplay;

use crate::models::ContentItem;
use crate::models::ResponseItem;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use codex_protocol::config_types::SandboxMode;
use std::fmt::Display;
use std::path::PathBuf;

/// wraps environment context message in a tag for the model to parse more easily.
pub(crate) const ENVIRONMENT_CONTEXT_START: &str = "<environment_context>\n";
pub(crate) const ENVIRONMENT_CONTEXT_END: &str = "</environment_context>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, DeriveDisplay)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_mode: SandboxMode,
    pub network_access: NetworkAccess,
}

impl EnvironmentContext {
    pub fn new(
        cwd: PathBuf,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) -> Self {
        Self {
            cwd,
            approval_policy,
            sandbox_mode: match sandbox_policy {
                SandboxPolicy::DangerFullAccess => SandboxMode::DangerFullAccess,
                SandboxPolicy::ReadOnly => SandboxMode::ReadOnly,
                SandboxPolicy::WorkspaceWrite { .. } => SandboxMode::WorkspaceWrite,
            },
            network_access: match sandbox_policy {
                SandboxPolicy::DangerFullAccess => NetworkAccess::Enabled,
                SandboxPolicy::ReadOnly => NetworkAccess::Restricted,
                SandboxPolicy::WorkspaceWrite { network_access, .. } => {
                    if network_access {
                        NetworkAccess::Enabled
                    } else {
                        NetworkAccess::Restricted
                    }
                }
            },
        }
    }
}

impl Display for EnvironmentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Current working directory: {}",
            self.cwd.to_string_lossy()
        )?;
        writeln!(f, "Approval policy: {}", self.approval_policy)?;
        writeln!(f, "Sandbox mode: {}", self.sandbox_mode)?;
        writeln!(f, "Network access: {}", self.network_access)?;
        Ok(())
    }
}

impl From<EnvironmentContext> for ResponseItem {
    fn from(ec: EnvironmentContext) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{ENVIRONMENT_CONTEXT_START}{ec}{ENVIRONMENT_CONTEXT_END}"),
            }],
        }
    }
}
