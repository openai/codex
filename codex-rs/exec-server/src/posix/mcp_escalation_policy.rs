use std::path::Path;

use codex_execpolicy::Policy;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::CreateElicitationResult;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::service::RequestContext;

use crate::posix::escalate_protocol::EscalateAction;
use crate::posix::escalation_policy::EscalationPolicy;
use crate::posix::stopwatch::Stopwatch;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ExecPolicyOutcome {
    Allow {
        run_with_escalated_permissions: bool,
    },
    Prompt {
        run_with_escalated_permissions: bool,
    },
    Forbidden,
}

/// ExecPolicy with access to the MCP RequestContext so that it can leverage
/// elicitations.
pub(crate) struct McpEscalationPolicy {
    /// In-memory execpolicy rules that drive how to handle an exec() call.
    policy: Arc<RwLock<Policy>>,
    context: RequestContext<RoleServer>,
    stopwatch: Stopwatch,
    preserve_program_paths: bool,
}

impl McpEscalationPolicy {
    pub(crate) fn new(
        policy: Arc<RwLock<Policy>>,
        context: RequestContext<RoleServer>,
        stopwatch: Stopwatch,
        preserve_program_paths: bool,
    ) -> Self {
        Self {
            policy,
            context,
            stopwatch,
            preserve_program_paths,
        }
    }

    async fn prompt(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
        context: RequestContext<RoleServer>,
    ) -> Result<CreateElicitationResult, McpError> {
        let args = shlex::try_join(argv.iter().skip(1).map(String::as_str)).unwrap_or_default();
        let command = if args.is_empty() {
            file.display().to_string()
        } else {
            format!("{} {}", file.display(), args)
        };
        self.stopwatch
            .pause_for(async {
                context
                    .peer
                    .create_elicitation(CreateElicitationRequestParam {
                        message: format!(
                            "Allow agent to run `{command}` in `{}`?",
                            workdir.display()
                        ),
                        requested_schema: ElicitationSchema::builder()
                            .title("Execution Permission Request")
                            .optional_string_with("reason", |schema| {
                                schema.description(
                                    "Optional reason for allowing or denying execution",
                                )
                            })
                            .build()
                            .map_err(|e| {
                                McpError::internal_error(
                                    format!("failed to build elicitation schema: {e}"),
                                    None,
                                )
                            })?,
                    })
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))
            })
            .await
    }
}

#[async_trait::async_trait]
impl EscalationPolicy for McpEscalationPolicy {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
    ) -> Result<EscalateAction, rmcp::ErrorData> {
        let policy = self.policy.read().await;
        let outcome =
            crate::posix::evaluate_exec_policy(&policy, file, argv, self.preserve_program_paths)?;
        let action = match outcome {
            ExecPolicyOutcome::Allow {
                run_with_escalated_permissions,
            } => {
                if run_with_escalated_permissions {
                    EscalateAction::Escalate
                } else {
                    EscalateAction::Run
                }
            }
            ExecPolicyOutcome::Prompt {
                run_with_escalated_permissions,
            } => {
                let result = self
                    .prompt(file, argv, workdir, self.context.clone())
                    .await?;
                // TODO: Extract reason from `result.content`.
                match result.action {
                    ElicitationAction::Accept => {
                        if run_with_escalated_permissions {
                            EscalateAction::Escalate
                        } else {
                            EscalateAction::Run
                        }
                    }
                    ElicitationAction::Decline => EscalateAction::Deny {
                        reason: Some("User declined execution".to_string()),
                    },
                    ElicitationAction::Cancel => EscalateAction::Deny {
                        reason: Some("User cancelled execution".to_string()),
                    },
                }
            }
            ExecPolicyOutcome::Forbidden => EscalateAction::Deny {
                reason: Some("Execution forbidden by policy".to_string()),
            },
        };
        Ok(action)
    }
}
