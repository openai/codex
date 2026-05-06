#[derive(Debug, Clone)]
pub(crate) struct UniversalOutput {
    pub continue_processing: bool,
    pub stop_reason: Option<String>,
    pub suppress_output: bool,
    pub system_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionStartOutput {
    pub universal: UniversalOutput,
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreToolUseOutput {
    pub universal: UniversalOutput,
    pub block_reason: Option<String>,
    pub additional_context: Option<String>,
    pub updated_input: Option<serde_json::Value>,
    pub permission_decision: Option<crate::events::pre_tool_use::PreToolUsePermissionDecision>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PermissionRequestDecision {
    Allow {
        updated_input: Option<serde_json::Value>,
    },
    Deny {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PermissionRequestOutput {
    pub universal: UniversalOutput,
    pub decision: Option<PermissionRequestDecision>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PostToolUseOutput {
    pub universal: UniversalOutput,
    pub should_block: bool,
    pub reason: Option<String>,
    pub invalid_block_reason: Option<String>,
    pub additional_context: Option<String>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct UserPromptSubmitOutput {
    pub universal: UniversalOutput,
    pub should_block: bool,
    pub reason: Option<String>,
    pub invalid_block_reason: Option<String>,
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StopOutput {
    pub universal: UniversalOutput,
    pub should_block: bool,
    pub reason: Option<String>,
    pub invalid_block_reason: Option<String>,
}

use crate::schema::BlockDecisionWire;
use crate::schema::HookUniversalOutputWire;
use crate::schema::PermissionRequestBehaviorWire;
use crate::schema::PermissionRequestCommandOutputWire;
use crate::schema::PermissionRequestDecisionWire;
use crate::schema::PostToolUseCommandOutputWire;
use crate::schema::PreToolUseCommandOutputWire;
use crate::schema::PreToolUseDecisionWire;
use crate::schema::PreToolUsePermissionDecisionWire;
use crate::schema::SessionStartCommandOutputWire;
use crate::schema::StopCommandOutputWire;
use crate::schema::UserPromptSubmitCommandOutputWire;

pub(crate) fn parse_session_start(stdout: &str) -> Option<SessionStartOutput> {
    let wire: SessionStartCommandOutputWire = parse_json(stdout)?;
    let additional_context = wire
        .hook_specific_output
        .and_then(|output| output.additional_context);
    Some(SessionStartOutput {
        universal: UniversalOutput::from(wire.universal),
        additional_context,
    })
}

pub(crate) fn parse_pre_tool_use(stdout: &str) -> Option<PreToolUseOutput> {
    let PreToolUseCommandOutputWire {
        universal: universal_wire,
        decision,
        reason,
        hook_specific_output,
    } = parse_json(stdout)?;
    let universal = UniversalOutput::from(universal_wire);
    let hook_specific_output = hook_specific_output.as_ref();
    let additional_context =
        hook_specific_output.and_then(|output| output.additional_context.clone());
    let use_hook_specific_decision = hook_specific_output.is_some_and(|output| {
        output.permission_decision.is_some()
            || output.permission_decision_reason.is_some()
            || output.updated_input.is_some()
    });
    let invalid_reason = unsupported_pre_tool_use_universal(&universal).or_else(|| {
        if use_hook_specific_decision {
            hook_specific_output.and_then(unsupported_pre_tool_use_hook_specific_output)
        } else {
            unsupported_pre_tool_use_legacy_decision(decision.as_ref(), reason.as_deref())
        }
    });
    let block_reason = if invalid_reason.is_none() {
        if use_hook_specific_decision {
            hook_specific_output.and_then(|output| match output.permission_decision {
                Some(PreToolUsePermissionDecisionWire::Deny) => output
                    .permission_decision_reason
                    .as_deref()
                    .and_then(trimmed_reason),
                _ => None,
            })
        } else {
            match decision.as_ref() {
                Some(PreToolUseDecisionWire::Block) => reason.as_deref().and_then(trimmed_reason),
                Some(PreToolUseDecisionWire::Approve) | None => None,
            }
        }
    } else {
        None
    };
    let updated_input = if invalid_reason.is_none() {
        hook_specific_output.and_then(|output| {
            matches!(
                output.permission_decision,
                Some(
                    PreToolUsePermissionDecisionWire::Allow | PreToolUsePermissionDecisionWire::Ask
                )
            )
            .then(|| output.updated_input.clone())
            .flatten()
        })
    } else {
        None
    };
    let permission_decision = if invalid_reason.is_none() && updated_input.is_some() {
        hook_specific_output.and_then(|output| match output.permission_decision {
            Some(PreToolUsePermissionDecisionWire::Allow) => {
                Some(crate::events::pre_tool_use::PreToolUsePermissionDecision::Allow)
            }
            Some(PreToolUsePermissionDecisionWire::Ask) => Some(
                crate::events::pre_tool_use::PreToolUsePermissionDecision::Ask {
                    reason: output
                        .permission_decision_reason
                        .as_deref()
                        .and_then(trimmed_reason),
                },
            ),
            Some(PreToolUsePermissionDecisionWire::Deny) | None => None,
        })
    } else {
        None
    };

    Some(PreToolUseOutput {
        universal,
        block_reason,
        additional_context,
        updated_input,
        permission_decision,
        invalid_reason,
    })
}

pub(crate) fn parse_permission_request(stdout: &str) -> Option<PermissionRequestOutput> {
    let wire: PermissionRequestCommandOutputWire = parse_json(stdout)?;
    let universal = UniversalOutput::from(wire.universal);
    let hook_specific_output = wire.hook_specific_output.as_ref();
    let decision = hook_specific_output.and_then(|output| output.decision.as_ref());
    let invalid_reason = unsupported_permission_request_universal(&universal).or_else(|| {
        hook_specific_output.and_then(|output| {
            unsupported_permission_request_hook_specific_output(output.decision.as_ref())
        })
    });
    let decision = if invalid_reason.is_none() {
        decision.map(permission_request_decision)
    } else {
        None
    };

    Some(PermissionRequestOutput {
        universal,
        decision,
        invalid_reason,
    })
}

pub(crate) fn parse_post_tool_use(stdout: &str) -> Option<PostToolUseOutput> {
    let wire: PostToolUseCommandOutputWire = parse_json(stdout)?;
    let universal = UniversalOutput::from(wire.universal);
    let invalid_reason = unsupported_post_tool_use_universal(&universal).or_else(|| {
        wire.hook_specific_output
            .as_ref()
            .and_then(unsupported_post_tool_use_hook_specific_output)
    });
    let should_block = matches!(wire.decision, Some(BlockDecisionWire::Block));
    let invalid_block_reason = if should_block
        && match wire.reason.as_deref() {
            Some(reason) => reason.trim().is_empty(),
            None => true,
        } {
        Some(invalid_block_message("PostToolUse"))
    } else if !should_block && universal.continue_processing && wire.reason.is_some() {
        Some("PostToolUse hook returned reason without decision".to_string())
    } else {
        None
    };
    let additional_context = wire
        .hook_specific_output
        .and_then(|output| output.additional_context);

    Some(PostToolUseOutput {
        universal,
        should_block: should_block && invalid_reason.is_none() && invalid_block_reason.is_none(),
        reason: wire.reason,
        invalid_block_reason,
        additional_context,
        invalid_reason,
    })
}

pub(crate) fn parse_user_prompt_submit(stdout: &str) -> Option<UserPromptSubmitOutput> {
    let wire: UserPromptSubmitCommandOutputWire = parse_json(stdout)?;
    let should_block = matches!(wire.decision, Some(BlockDecisionWire::Block));
    let invalid_block_reason = if should_block
        && match wire.reason.as_deref() {
            Some(reason) => reason.trim().is_empty(),
            None => true,
        } {
        Some(invalid_block_message("UserPromptSubmit"))
    } else {
        None
    };
    let additional_context = wire
        .hook_specific_output
        .and_then(|output| output.additional_context);
    Some(UserPromptSubmitOutput {
        universal: UniversalOutput::from(wire.universal),
        should_block: should_block && invalid_block_reason.is_none(),
        reason: wire.reason,
        invalid_block_reason,
        additional_context,
    })
}

pub(crate) fn parse_stop(stdout: &str) -> Option<StopOutput> {
    let wire: StopCommandOutputWire = parse_json(stdout)?;
    let should_block = matches!(wire.decision, Some(BlockDecisionWire::Block));
    let invalid_block_reason = if should_block
        && match wire.reason.as_deref() {
            Some(reason) => reason.trim().is_empty(),
            None => true,
        } {
        Some(invalid_block_message("Stop"))
    } else {
        None
    };
    Some(StopOutput {
        universal: UniversalOutput::from(wire.universal),
        should_block: should_block && invalid_block_reason.is_none(),
        reason: wire.reason,
        invalid_block_reason,
    })
}

impl From<HookUniversalOutputWire> for UniversalOutput {
    fn from(value: HookUniversalOutputWire) -> Self {
        Self {
            continue_processing: value.r#continue,
            stop_reason: value.stop_reason,
            suppress_output: value.suppress_output,
            system_message: value.system_message,
        }
    }
}

fn parse_json<T>(stdout: &str) -> Option<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    if !value.is_object() {
        return None;
    }
    serde_json::from_value(value).ok()
}

fn invalid_block_message(event_name: &str) -> String {
    format!("{event_name} hook returned decision:block without a non-empty reason")
}

fn unsupported_pre_tool_use_universal(universal: &UniversalOutput) -> Option<String> {
    if !universal.continue_processing {
        Some("PreToolUse hook returned unsupported continue:false".to_string())
    } else if universal.stop_reason.is_some() {
        Some("PreToolUse hook returned unsupported stopReason".to_string())
    } else if universal.suppress_output {
        Some("PreToolUse hook returned unsupported suppressOutput".to_string())
    } else {
        None
    }
}

fn unsupported_permission_request_universal(universal: &UniversalOutput) -> Option<String> {
    if !universal.continue_processing {
        Some("PermissionRequest hook returned unsupported continue:false".to_string())
    } else if universal.stop_reason.is_some() {
        Some("PermissionRequest hook returned unsupported stopReason".to_string())
    } else if universal.suppress_output {
        Some("PermissionRequest hook returned unsupported suppressOutput".to_string())
    } else {
        None
    }
}

fn unsupported_post_tool_use_universal(universal: &UniversalOutput) -> Option<String> {
    if universal.suppress_output {
        Some("PostToolUse hook returned unsupported suppressOutput".to_string())
    } else {
        None
    }
}

fn unsupported_permission_request_hook_specific_output(
    decision: Option<&PermissionRequestDecisionWire>,
) -> Option<String> {
    let decision = decision?;
    if decision.updated_input.is_some()
        && !matches!(decision.behavior, PermissionRequestBehaviorWire::Allow)
    {
        Some("PermissionRequest hook returned unsupported updatedInput".to_string())
    } else if decision.updated_permissions.is_some() {
        Some("PermissionRequest hook returned unsupported updatedPermissions".to_string())
    } else if decision.interrupt {
        Some("PermissionRequest hook returned unsupported interrupt:true".to_string())
    } else {
        None
    }
}

fn permission_request_decision(
    decision: &PermissionRequestDecisionWire,
) -> PermissionRequestDecision {
    match decision.behavior {
        PermissionRequestBehaviorWire::Allow => PermissionRequestDecision::Allow {
            updated_input: decision.updated_input.clone(),
        },
        PermissionRequestBehaviorWire::Deny => PermissionRequestDecision::Deny {
            message: decision
                .message
                .as_deref()
                .and_then(trimmed_reason)
                .unwrap_or_else(|| "PermissionRequest hook denied approval".to_string()),
        },
    }
}

fn unsupported_post_tool_use_hook_specific_output(
    output: &crate::schema::PostToolUseHookSpecificOutputWire,
) -> Option<String> {
    if output.updated_mcp_tool_output.is_some() {
        Some("PostToolUse hook returned unsupported updatedMCPToolOutput".to_string())
    } else {
        None
    }
}

fn unsupported_pre_tool_use_hook_specific_output(
    output: &crate::schema::PreToolUseHookSpecificOutputWire,
) -> Option<String> {
    match output.permission_decision {
        Some(PreToolUsePermissionDecisionWire::Allow)
        | Some(PreToolUsePermissionDecisionWire::Ask)
            if output.updated_input.is_some() =>
        {
            None
        }
        Some(PreToolUsePermissionDecisionWire::Allow) => {
            Some("PreToolUse hook returned unsupported permissionDecision:allow".to_string())
        }
        Some(PreToolUsePermissionDecisionWire::Ask) => {
            Some("PreToolUse hook returned unsupported permissionDecision:ask".to_string())
        }
        Some(PreToolUsePermissionDecisionWire::Deny) => {
            if output
                .permission_decision_reason
                .as_deref()
                .and_then(trimmed_reason)
                .is_none()
            {
                Some(invalid_pre_tool_use_reason_message())
            } else {
                None
            }
        }
        None => {
            if output.permission_decision_reason.is_some() {
                Some(
                    "PreToolUse hook returned permissionDecisionReason without permissionDecision"
                        .to_string(),
                )
            } else {
                None
            }
        }
    }
}

fn unsupported_pre_tool_use_legacy_decision(
    decision: Option<&PreToolUseDecisionWire>,
    reason: Option<&str>,
) -> Option<String> {
    match decision {
        Some(PreToolUseDecisionWire::Approve) => {
            Some("PreToolUse hook returned unsupported decision:approve".to_string())
        }
        Some(PreToolUseDecisionWire::Block) => {
            if reason.and_then(trimmed_reason).is_none() {
                Some(invalid_block_message("PreToolUse"))
            } else {
                None
            }
        }
        None => {
            if reason.is_some() {
                Some("PreToolUse hook returned reason without decision".to_string())
            } else {
                None
            }
        }
    }
}

fn invalid_pre_tool_use_reason_message() -> String {
    "PreToolUse hook returned permissionDecision:deny without a non-empty permissionDecisionReason"
        .to_string()
}

fn trimmed_reason(reason: &str) -> Option<String> {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::parse_permission_request;
    use super::parse_pre_tool_use;

    #[test]
    fn permission_request_accepts_allow_updated_input_field() {
        let parsed = parse_permission_request(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedInput": {}
                    }
                }
            })
            .to_string(),
        )
        .expect("permission request hook output should parse");

        assert_eq!(
            parsed.decision,
            Some(super::PermissionRequestDecision::Allow {
                updated_input: Some(json!({})),
            })
        );
    }

    #[test]
    fn permission_request_rejects_deny_updated_input_field() {
        let parsed = parse_permission_request(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "deny",
                        "updatedInput": {}
                    }
                }
            })
            .to_string(),
        )
        .expect("permission request hook output should parse");

        assert_eq!(
            parsed.invalid_reason,
            Some("PermissionRequest hook returned unsupported updatedInput".to_string())
        );
    }

    #[test]
    fn permission_request_rejects_reserved_updated_permissions_field() {
        let parsed = parse_permission_request(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedPermissions": {}
                    }
                }
            })
            .to_string(),
        )
        .expect("permission request hook output should parse");

        assert_eq!(
            parsed.invalid_reason,
            Some("PermissionRequest hook returned unsupported updatedPermissions".to_string())
        );
    }

    #[test]
    fn permission_request_rejects_reserved_interrupt_field() {
        let parsed = parse_permission_request(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "interrupt": true
                    }
                }
            })
            .to_string(),
        )
        .expect("permission request hook output should parse");

        assert_eq!(
            parsed.invalid_reason,
            Some("PermissionRequest hook returned unsupported interrupt:true".to_string())
        );
    }

    #[test]
    fn pre_tool_use_accepts_ask_updated_input_field() {
        let parsed = parse_pre_tool_use(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "ask",
                    "permissionDecisionReason": "show rewritten command",
                    "updatedInput": { "command": "echo rewritten" }
                }
            })
            .to_string(),
        )
        .expect("pre tool use hook output should parse");

        assert_eq!(
            parsed.updated_input,
            Some(json!({ "command": "echo rewritten" }))
        );
        assert_eq!(
            parsed.permission_decision,
            Some(
                crate::events::pre_tool_use::PreToolUsePermissionDecision::Ask {
                    reason: Some("show rewritten command".to_string()),
                }
            )
        );
    }

    #[test]
    fn pre_tool_use_ignores_deferred_updated_input_field() {
        let parsed = parse_pre_tool_use(
            &json!({
                "continue": true,
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "updatedInput": { "command": "echo ignored" }
                }
            })
            .to_string(),
        )
        .expect("pre tool use hook output should parse");

        assert_eq!(parsed.invalid_reason, None);
        assert_eq!(parsed.updated_input, None);
        assert_eq!(parsed.permission_decision, None);
    }
}
