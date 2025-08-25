use crate::codex::Codex;
use crate::error::Result as CodexResult;

use super::definition::SubagentDefinition;
use super::definition::SubagentSource;
use crate::openai_tools::JsonSchema;
use serde_json::Value as JsonValue;

/// Arguments expected for the `subagent.run` tool.
#[derive(serde::Deserialize)]
pub struct RunSubagentArgs {
    pub name: String,
    pub input: String,
    #[serde(default)]
    pub context: Option<String>,
}

/// Build the effective base instructions for a subagent run.
///
/// For user- and project-scoped subagents, we append their instructions to the
/// parent session's base instructions. For embedded defaults, we use only the
/// subagent's instructions. We always augment the subagent instructions with
/// strict JSON output requirements based on its schema.
fn compose_base_instructions_for_subagent(
    def: &SubagentDefinition,
    parent_base_instructions: Option<&str>,
) -> String {
    // Start with the subagent's own instructions, optionally augmented with
    // structured output requirements.
    let schema_json =
        serde_json::to_string_pretty(def.output_schema()).unwrap_or_else(|_| "{}".to_string());
    let child_instructions = format!(
        "{}\n\nOutput format requirements:\n- Reply with a single JSON value that strictly matches the following JSON Schema.\n- Do not include any commentary, markdown, or extra text.\n- Do not include trailing explanations.\n\nSchema:\n{}\n",
        def.instructions, schema_json
    );

    match def.source {
        SubagentSource::User | SubagentSource::Project => match parent_base_instructions {
            Some(parent) if !parent.trim().is_empty() => {
                format!("{parent}\n\n{child}", child = child_instructions)
            }
            _ => child_instructions,
        },
        SubagentSource::EmbeddedDefault => child_instructions,
    }
}

/// Run a subagent in a nested Codex session and return the final message.
pub(crate) async fn run(
    sess: &crate::codex::Session,
    turn_context: &crate::codex::TurnContext,
    args: RunSubagentArgs,
    _parent_sub_id: &str,
) -> CodexResult<String> {
    let def: &SubagentDefinition =
        turn_context
            .subagents_registry
            .get(&args.name)
            .ok_or_else(|| {
                crate::error::CodexErr::Stream(format!("unknown subagent: {}", args.name), None)
            })?;

    let mut nested_cfg = (*turn_context.base_config).clone();
    let base_instructions =
        compose_base_instructions_for_subagent(def, turn_context.base_instructions.as_deref());
    nested_cfg.base_instructions = Some(base_instructions);
    nested_cfg.user_instructions = None;
    // Apply subagent-specific overrides for model and reasoning effort.
    if let Some(model) = &def.model {
        nested_cfg.model = model.clone();
    }
    if let Some(re) = def.reasoning_effort {
        nested_cfg.model_reasoning_effort = re;
    }
    nested_cfg.approval_policy = turn_context.approval_policy;
    nested_cfg.sandbox_policy = turn_context.sandbox_policy.clone();
    nested_cfg.cwd = turn_context.cwd.clone();
    nested_cfg.include_subagent_tool = false;

    let nested = Codex::spawn(nested_cfg, turn_context.auth_manager.clone(), None).await?;
    let nested_codex = nested.codex;

    let subagent_id = uuid::Uuid::new_v4().to_string();
    forward_begin(sess, _parent_sub_id, &subagent_id, &def.name).await;

    let text = match args.context {
        Some(ctx) if !ctx.trim().is_empty() => format!("{ctx}\n\n{input}", input = args.input),
        _ => args.input,
    };

    nested_codex
        .submit(crate::protocol::Op::UserInput {
            items: vec![crate::protocol::InputItem::Text { text }],
        })
        .await
        .map_err(|e| {
            crate::error::CodexErr::Stream(format!("failed to submit to subagent: {e}"), None)
        })?;

    let mut last_message: Option<String> = None;
    loop {
        let ev = nested_codex.next_event().await?;
        match ev.msg.clone() {
            crate::protocol::EventMsg::AgentMessage(m) => {
                last_message = Some(m.message);
            }
            crate::protocol::EventMsg::TaskComplete(t) => {
                let _ = nested_codex.submit(crate::protocol::Op::Shutdown).await;
                forward_forwarded(sess, _parent_sub_id, &subagent_id, &def.name, ev.msg).await;
                forward_end(
                    sess,
                    _parent_sub_id,
                    &subagent_id,
                    &def.name,
                    true,
                    t.last_agent_message.clone(),
                )
                .await;
                return Ok(t
                    .last_agent_message
                    .unwrap_or_else(|| last_message.unwrap_or_default()));
            }
            _ => {}
        }
        forward_forwarded(sess, _parent_sub_id, &subagent_id, &def.name, ev.msg).await;
    }
}

/// Minimal validator for our limited JsonSchema subset used across the codebase.
pub(crate) fn validate_json_against_schema(
    value: &JsonValue,
    schema: &JsonSchema,
) -> Result<(), String> {
    match schema {
        JsonSchema::Boolean { .. } => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err("expected boolean".to_string())
            }
        }
        JsonSchema::String { .. } => {
            if value.is_string() {
                Ok(())
            } else {
                Err("expected string".to_string())
            }
        }
        JsonSchema::Number { .. } => {
            if value.is_number() {
                Ok(())
            } else {
                Err("expected number".to_string())
            }
        }
        JsonSchema::Array { items, .. } => {
            if let JsonValue::Array(arr) = value {
                for (i, v) in arr.iter().enumerate() {
                    validate_json_against_schema(v, items)
                        .map_err(|e| format!("array[{i}]: {e}"))?;
                }
                Ok(())
            } else {
                Err("expected array".to_string())
            }
        }
        JsonSchema::Object {
            properties,
            required,
            additional_properties,
        } => {
            let obj = match value.as_object() {
                Some(o) => o,
                None => return Err("expected object".to_string()),
            };
            // Check required
            if let Some(req) = required {
                for key in req {
                    if !obj.contains_key(key) {
                        return Err(format!("missing required property: {key}"));
                    }
                }
            }
            // Validate each present property
            for (k, v) in obj.iter() {
                if let Some(child_schema) = properties.get(k) {
                    validate_json_against_schema(v, child_schema)
                        .map_err(|e| format!("property '{k}': {e}"))?;
                } else if matches!(additional_properties, Some(false)) {
                    return Err(format!("unexpected property: {k}"));
                }
            }
            Ok(())
        }
    }
}

async fn forward_begin(
    sess: &crate::codex::Session,
    parent_sub_id: &str,
    subagent_id: &str,
    name: &str,
) {
    sess.send_event(crate::protocol::Event {
        id: parent_sub_id.to_string(),
        msg: crate::protocol::EventMsg::SubagentBegin(crate::protocol::SubagentBeginEvent {
            subagent_id: subagent_id.to_string(),
            name: name.to_string(),
        }),
    })
    .await;
}

async fn forward_forwarded(
    sess: &crate::codex::Session,
    parent_sub_id: &str,
    subagent_id: &str,
    name: &str,
    msg: crate::protocol::EventMsg,
) {
    sess.send_event(crate::protocol::Event {
        id: parent_sub_id.to_string(),
        msg: crate::protocol::EventMsg::SubagentForwarded(
            crate::protocol::SubagentForwardedEvent {
                subagent_id: subagent_id.to_string(),
                name: name.to_string(),
                event: Box::new(msg),
            },
        ),
    })
    .await;
}

async fn forward_end(
    sess: &crate::codex::Session,
    parent_sub_id: &str,
    subagent_id: &str,
    name: &str,
    success: bool,
    last_agent_message: Option<String>,
) {
    sess.send_event(crate::protocol::Event {
        id: parent_sub_id.to_string(),
        msg: crate::protocol::EventMsg::SubagentEnd(crate::protocol::SubagentEndEvent {
            subagent_id: subagent_id.to_string(),
            name: name.to_string(),
            success,
            last_agent_message,
        }),
    })
    .await;
}
