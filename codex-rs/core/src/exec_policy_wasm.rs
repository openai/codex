use std::path::PathBuf;
use std::sync::Arc;

use codex_execpolicy::Decision;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use thiserror::Error;

use crate::config::Config;
use crate::config_loader::ConfigLayerStack;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::ExecApprovalRequirement;

pub(crate) fn child_uses_parent_exec_policy(parent_config: &Config, child_config: &Config) -> bool {
    parent_config.config_layer_stack.requirements().exec_policy
        == child_config.config_layer_stack.requirements().exec_policy
}

pub(crate) fn prompt_is_rejected_by_policy(
    approval_policy: AskForApproval,
    _prompt_is_rule: bool,
) -> Option<&'static str> {
    match approval_policy {
        AskForApproval::Never => {
            Some("approval required by policy, but AskForApproval is set to Never")
        }
        AskForApproval::OnFailure
        | AskForApproval::OnRequest
        | AskForApproval::UnlessTrusted
        | AskForApproval::Granular(_) => None,
    }
}

#[derive(Debug, Error)]
pub enum ExecPolicyError {
    #[error("exec policy is unavailable in wasm")]
    Unsupported,

    #[error("failed to parse rules file {path}: {source}")]
    ParsePolicy {
        path: String,
        source: codex_execpolicy::Error,
    },
}

#[derive(Debug, Error)]
pub enum ExecPolicyUpdateError {
    #[error("exec policy updates are unavailable in wasm")]
    Unsupported,
}

pub(crate) struct ExecApprovalRequest<'a> {
    pub(crate) command: &'a [String],
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: &'a SandboxPolicy,
    pub(crate) file_system_sandbox_policy: &'a FileSystemSandboxPolicy,
    pub(crate) sandbox_permissions: SandboxPermissions,
    pub(crate) prefix_rule: Option<Vec<String>>,
}

pub(crate) struct ExecPolicyManager {
    policy: Arc<Policy>,
}

impl ExecPolicyManager {
    pub(crate) fn new(policy: Arc<Policy>) -> Self {
        Self { policy }
    }

    pub(crate) async fn load(_config_stack: &ConfigLayerStack) -> Result<Self, ExecPolicyError> {
        Ok(Self::default())
    }

    pub(crate) fn current(&self) -> Arc<Policy> {
        Arc::clone(&self.policy)
    }

    pub(crate) fn compiled_network_domains(&self) -> (Vec<String>, Vec<String>) {
        (Vec::new(), Vec::new())
    }

    pub(crate) async fn create_exec_approval_requirement_for_command(
        &self,
        _req: ExecApprovalRequest<'_>,
    ) -> ExecApprovalRequirement {
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        }
    }

    pub(crate) async fn amend(
        &self,
        _amendment: &ExecPolicyAmendment,
    ) -> Result<(), ExecPolicyUpdateError> {
        Ok(())
    }

    pub(crate) fn requirement_for(
        &self,
        _request: &ExecApprovalRequest<'_>,
    ) -> Option<ExecApprovalRequirement> {
        None
    }

    pub(crate) async fn append_amendment_and_update(
        &self,
        _codex_home: &std::path::Path,
        _amendment: &ExecPolicyAmendment,
    ) -> Result<(), ExecPolicyUpdateError> {
        Ok(())
    }

    pub(crate) async fn append_network_rule_and_update(
        &self,
        _codex_home: &std::path::Path,
        _host: &str,
        _protocol: NetworkRuleProtocol,
        _decision: Decision,
        _justification: Option<String>,
    ) -> Result<(), ExecPolicyUpdateError> {
        Ok(())
    }
}

impl Default for ExecPolicyManager {
    fn default() -> Self {
        Self::new(Arc::new(Policy::empty()))
    }
}

pub async fn check_execpolicy_for_warnings(
    _config_stack: &ConfigLayerStack,
) -> Option<ExecPolicyError> {
    None
}

pub fn format_exec_policy_error_with_source(error: &ExecPolicyError) -> String {
    error.to_string()
}

pub async fn load_exec_policy(config_stack: &ConfigLayerStack) -> Result<Policy, ExecPolicyError> {
    Ok(ExecPolicyManager::load(config_stack)
        .await?
        .current()
        .as_ref()
        .clone())
}

#[allow(clippy::too_many_arguments)]
pub fn render_decision_for_unmatched_command(
    _decision: &Decision,
    _approval_policy: AskForApproval,
    _sandbox_policy: &SandboxPolicy,
    _file_system_sandbox_policy: &FileSystemSandboxPolicy,
    _sandbox_permissions: SandboxPermissions,
    _prefix_rule: Option<&[String]>,
    _command: &[String],
    _cwd: Option<&PathBuf>,
    _windows_sandbox_kind: Option<FileSystemSandboxKind>,
) -> Option<ExecApprovalRequirement> {
    None
}
