use std::path::Path;

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

/// This is the policy which decides how to handle an exec() call.
///
/// `file` is the absolute, canonical path to the executable to run, i.e. the first arg to exec.
/// `argv` is the argv, including the program name (`argv[0]`).
/// `workdir` is the absolute, canonical path to the working directory in which to execute the
/// command.
pub(crate) type ExecPolicy = fn(file: &Path, argv: &[String], workdir: &Path) -> ExecPolicyOutcome;

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
    policy: ExecPolicy,
    context: RequestContext<RoleServer>,
    stopwatch: Stopwatch,
}

impl McpEscalationPolicy {
    pub(crate) fn new(
        policy: ExecPolicy,
        context: RequestContext<RoleServer>,
        stopwatch: Stopwatch,
    ) -> Self {
        Self {
            policy,
            context,
            stopwatch,
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
        let outcome = (self.policy)(file, argv, workdir);
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
