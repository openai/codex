use crate::codex::Session;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::sandbox_override_for_first_attempt;
use crate::tools::sandboxing::with_cached_approval;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct ArtifactApprovalKey {
    pub(crate) command_prefix: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) staged_script: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ArtifactExecRequest {
    pub(crate) command: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) env: HashMap<String, String>,
    pub(crate) approval_key: ArtifactApprovalKey,
    pub(crate) initial_approval_requirement: ExecApprovalRequirement,
    pub(crate) escalation_approval_requirement: ExecApprovalRequirement,
}

#[derive(Default)]
pub(crate) struct ArtifactRuntime;

impl ArtifactRuntime {
    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ArtifactRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
}

impl Approvable<ArtifactExecRequest> for ArtifactRuntime {
    type ApprovalKey = ArtifactApprovalKey;

    fn approval_keys(&self, req: &ArtifactExecRequest) -> Vec<Self::ApprovalKey> {
        vec![req.approval_key.clone()]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ArtifactExecRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let session: &'a Session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let retry_reason = ctx.retry_reason.clone();
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let approval_keys = self.approval_keys(req);
        let approval_requirement = if retry_reason.is_some() {
            req.escalation_approval_requirement.clone()
        } else {
            req.initial_approval_requirement.clone()
        };
        Box::pin(async move {
            if matches!(
                approval_requirement,
                ExecApprovalRequirement::Forbidden { .. }
            ) {
                return ReviewDecision::Denied;
            }

            with_cached_approval(
                &session.services,
                "artifacts",
                approval_keys,
                || async move {
                    session
                        .request_command_approval(
                            turn,
                            call_id,
                            None,
                            command,
                            cwd,
                            retry_reason,
                            ctx.network_approval_context.clone(),
                            approval_requirement
                                .proposed_execpolicy_amendment()
                                .cloned(),
                            None,
                            None,
                        )
                        .await
                },
            )
            .await
        })
    }

    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::Never => false,
            AskForApproval::Reject(reject_config) => !reject_config.rejects_sandbox_approval(),
            AskForApproval::OnFailure => true,
            AskForApproval::OnRequest => true,
            AskForApproval::UnlessTrusted => true,
        }
    }

    fn exec_approval_requirement(
        &self,
        req: &ArtifactExecRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.initial_approval_requirement.clone())
    }

    fn sandbox_mode_for_first_attempt(&self, req: &ArtifactExecRequest) -> SandboxOverride {
        sandbox_override_for_first_attempt(
            SandboxPermissions::UseDefault,
            &req.initial_approval_requirement,
        )
    }
}

impl ToolRuntime<ArtifactExecRequest, ExecToolCallOutput> for ArtifactRuntime {
    async fn run(
        &mut self,
        req: &ArtifactExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = build_command_spec(
            &req.command,
            &req.cwd,
            &req.env,
            req.timeout_ms.into(),
            SandboxPermissions::UseDefault,
            None,
            None,
        )?;
        let env = attempt
            .env_for(spec, None)
            .map_err(|err| ToolError::Codex(err.into()))?;
        execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context_with_rx;
    use crate::protocol::EventMsg;
    use crate::tools::sandboxing::SandboxOverride;
    use codex_protocol::approvals::NetworkApprovalContext;
    use codex_protocol::approvals::NetworkApprovalProtocol;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;

    #[tokio::test]
    async fn retry_with_skip_requirement_requests_approval() {
        let (session, turn, rx_event) = make_session_and_context_with_rx().await;
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());
        let mut runtime = ArtifactRuntime;
        let network_approval_context = NetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: NetworkApprovalProtocol::Http,
        };
        let network_approval_context_for_watcher = network_approval_context.clone();
        let req = ArtifactExecRequest {
            command: vec![
                "/path/to/node".to_string(),
                "/path/to/launcher.mjs".to_string(),
                "/tmp/source.mjs".to_string(),
            ],
            cwd: PathBuf::from("/tmp"),
            timeout_ms: Some(5_000),
            env: HashMap::new(),
            approval_key: ArtifactApprovalKey {
                command_prefix: vec![
                    "/path/to/node".to_string(),
                    "/path/to/launcher.mjs".to_string(),
                ],
                cwd: PathBuf::from("/tmp"),
                staged_script: PathBuf::from("/tmp/source.mjs"),
            },
            initial_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            escalation_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        };

        let session_for_response = session.clone();
        let approval_watcher = async move {
            loop {
                let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv())
                    .await
                    .expect("wait for approval event")
                    .expect("receive approval event");
                if let EventMsg::ExecApprovalRequest(request) = event.msg {
                    assert_eq!(request.call_id, "call_artifact");
                    assert_eq!(
                        request.network_approval_context,
                        Some(network_approval_context_for_watcher.clone())
                    );
                    session_for_response
                        .notify_approval(&request.call_id, ReviewDecision::Approved)
                        .await;
                    return;
                }
            }
        };

        let decision = tokio::join!(
            runtime.start_approval_async(
                &req,
                ApprovalCtx {
                    session: &session,
                    turn: &turn,
                    call_id: "call_artifact",
                    retry_reason: Some("command failed; retry without sandbox?".to_string()),
                    network_approval_context: Some(network_approval_context),
                },
            ),
            approval_watcher,
        )
        .0;

        assert_eq!(decision, ReviewDecision::Approved);
    }

    #[test]
    fn approval_keys_differ_for_different_staged_scripts() {
        let runtime = ArtifactRuntime;
        let req_one = ArtifactExecRequest {
            command: vec![
                "/path/to/node".to_string(),
                "/path/to/launcher.mjs".to_string(),
                "/tmp/source-one.mjs".to_string(),
            ],
            cwd: PathBuf::from("/tmp"),
            timeout_ms: Some(5_000),
            env: HashMap::new(),
            approval_key: ArtifactApprovalKey {
                command_prefix: vec![
                    "/path/to/node".to_string(),
                    "/path/to/launcher.mjs".to_string(),
                ],
                cwd: PathBuf::from("/tmp"),
                staged_script: PathBuf::from("/tmp/source-one.mjs"),
            },
            initial_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            escalation_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        };
        let req_two = ArtifactExecRequest {
            command: vec![
                "/path/to/node".to_string(),
                "/path/to/launcher.mjs".to_string(),
                "/tmp/source-two.mjs".to_string(),
            ],
            cwd: PathBuf::from("/tmp"),
            timeout_ms: Some(5_000),
            env: HashMap::new(),
            approval_key: ArtifactApprovalKey {
                command_prefix: vec![
                    "/path/to/node".to_string(),
                    "/path/to/launcher.mjs".to_string(),
                ],
                cwd: PathBuf::from("/tmp"),
                staged_script: PathBuf::from("/tmp/source-two.mjs"),
            },
            initial_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            escalation_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        };

        assert_ne!(
            runtime.approval_keys(&req_one),
            runtime.approval_keys(&req_two)
        );
    }

    #[test]
    fn exec_approval_requirement_uses_initial_requirement() {
        let runtime = ArtifactRuntime;
        let req = ArtifactExecRequest {
            command: vec![
                "/path/to/node".to_string(),
                "/path/to/launcher.mjs".to_string(),
                "/tmp/source.mjs".to_string(),
            ],
            cwd: PathBuf::from("/tmp"),
            timeout_ms: Some(5_000),
            env: HashMap::new(),
            approval_key: ArtifactApprovalKey {
                command_prefix: vec![
                    "/path/to/node".to_string(),
                    "/path/to/launcher.mjs".to_string(),
                ],
                cwd: PathBuf::from("/tmp"),
                staged_script: PathBuf::from("/tmp/source.mjs"),
            },
            initial_approval_requirement: ExecApprovalRequirement::Forbidden {
                reason: "blocked before first attempt".to_string(),
            },
            escalation_approval_requirement: ExecApprovalRequirement::Forbidden {
                reason: "blocked on retry".to_string(),
            },
        };

        assert_eq!(
            runtime.exec_approval_requirement(&req),
            Some(ExecApprovalRequirement::Forbidden {
                reason: "blocked before first attempt".to_string(),
            })
        );
    }

    #[test]
    fn sandbox_mode_for_first_attempt_uses_initial_requirement() {
        let runtime = ArtifactRuntime;
        let req = ArtifactExecRequest {
            command: vec![
                "/path/to/node".to_string(),
                "/path/to/launcher.mjs".to_string(),
                "/tmp/source.mjs".to_string(),
            ],
            cwd: PathBuf::from("/tmp"),
            timeout_ms: Some(5_000),
            env: HashMap::new(),
            approval_key: ArtifactApprovalKey {
                command_prefix: vec![
                    "/path/to/node".to_string(),
                    "/path/to/launcher.mjs".to_string(),
                ],
                cwd: PathBuf::from("/tmp"),
                staged_script: PathBuf::from("/tmp/source.mjs"),
            },
            initial_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            escalation_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            },
        };

        assert_eq!(
            runtime.sandbox_mode_for_first_attempt(&req),
            SandboxOverride::NoOverride
        );
    }
}
