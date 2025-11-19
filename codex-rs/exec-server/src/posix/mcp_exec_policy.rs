use std::path::Path;

use codex_execpolicy2::Decision;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::CreateElicitationResult;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::PrimitiveSchema;
use rmcp::model::StringSchema;
use rmcp::service::RequestContext;

use crate::posix::escalate_protocol::EscalateAction;
use crate::posix::escalation_policy::EscalationPolicy;

/// This is the policy which decides how to handle an exec() call.
///
/// `file` is the absolute, canonical path to the executable to run, i.e. the first arg to exec.
/// `argv` is the argv, including the program name (`argv[0]`).
/// `workdir` is the absolute, canonical path to the working directory in which to execute the
/// command.
pub(crate) type ExecPolicy = fn(file: &Path, argv: &[String], workdir: &Path) -> ExecPolicyOutcome;

pub(crate) struct ExecPolicyOutcome {
    pub(crate) decision: Decision,
    pub(crate) run_with_escalated_permissions: bool,
}

/// ExecPolicy with access to the MCP RequestContext so that it can leverage
/// elicitations.
pub(crate) struct McpExecPolicy {
    policy: ExecPolicy,
    context: RequestContext<RoleServer>,
}

impl McpExecPolicy {
    pub(crate) fn new(policy: ExecPolicy, context: RequestContext<RoleServer>) -> Self {
        Self { policy, context }
    }

    async fn prompt(
        &self,
        _file: &Path,
        argv: &[String],
        workdir: &Path,
        context: RequestContext<RoleServer>,
    ) -> Result<CreateElicitationResult, McpError> {
        let command = shlex::try_join(argv.iter().map(String::as_str)).unwrap_or_default();
        context
            .peer
            .create_elicitation(CreateElicitationRequestParam {
                message: format!("Allow Codex to run `{command:?}` in `{workdir:?}`?"),
                #[allow(clippy::expect_used)]
                requested_schema: ElicitationSchema::builder()
                    .property("dummy", PrimitiveSchema::String(StringSchema::new()))
                    .build()
                    .expect("failed to build elicitation schema"),
            })
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }
}

#[async_trait::async_trait]
impl EscalationPolicy for McpExecPolicy {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
    ) -> Result<EscalateAction, rmcp::ErrorData> {
        let outcome = (self.policy)(file, argv, workdir);
        let ExecPolicyOutcome {
            decision,
            run_with_escalated_permissions,
        } = outcome;
        let action = match decision {
            Decision::Allow => {
                if run_with_escalated_permissions {
                    EscalateAction::Escalate
                } else {
                    EscalateAction::Run
                }
            }
            Decision::Prompt => {
                let result = self
                    .prompt(file, argv, workdir, self.context.clone())
                    .await?;
                // TODO: Extract reason from `result.content`.
                match result.action {
                    ElicitationAction::Accept => EscalateAction::Escalate,
                    ElicitationAction::Decline => EscalateAction::Deny {
                        reason: Some("User declined execution".to_string()),
                    },
                    ElicitationAction::Cancel => EscalateAction::Deny {
                        reason: Some("User cancelled execution".to_string()),
                    },
                }
            }
            Decision::Forbidden => EscalateAction::Deny {
                reason: Some("Execution forbidden by policy".to_string()),
            },
        };
        Ok(action)
    }
}
