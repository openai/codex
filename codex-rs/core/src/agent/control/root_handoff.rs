//! Best-effort handoff windows plus recent root context, without saved handoff state.
//! History position approximates dispatch order; absent calls leave the existing context unchanged.
//! User inputs bypass relevance filtering; only assistant context is narrowed.

use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextOrder;
use codex_protocol::AgentPath;
use codex_protocol::ToolName;
use codex_protocol::models::ExecutedToolCallArguments;
use codex_protocol::models::ResponseItem;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RootHandoff {
    recipient: AgentPath,
    boundary: RetainedContextOrder,
}

pub(super) fn handoffs(
    history: &[ResponseItemEnvelope],
    tool_namespace: Option<&str>,
) -> Vec<RootHandoff> {
    let configured_handoff_names = ["spawn_agent", "send_message", "followup_task"].map(|name| {
        (
            codex_tools::code_mode_name_for_tool_name(&ToolName::new(
                tool_namespace.map(str::to_owned),
                name,
            )),
            name,
        )
    });
    let mut handoffs = Vec::new();
    let mut preceding_order = None;
    for envelope in history {
        if let Some(order) = envelope.metadata.as_ref().and_then(|metadata| {
            (!metadata.inherited_user_message)
                .then_some(metadata.user_input_order)
                .flatten()
        }) {
            preceding_order = Some(RetainedContextOrder::Local(order));
        }
        let Some(boundary) = preceding_order else {
            continue;
        };
        if let ResponseItem::FunctionCall {
            name,
            namespace,
            arguments,
            ..
        } = &envelope.item
            && namespace.as_deref().is_none_or(|namespace| {
                namespace == "collaboration"
                    || Some(namespace) == tool_namespace
                    || namespace == codex_protocol::DEFAULT_FUNCTION_NAMESPACE
            })
            && let Ok(args) = serde_json::from_str(arguments)
            && let Some(recipient) = recipient(name, &args)
        {
            handoffs.push(RootHandoff {
                recipient,
                boundary,
            });
        }
        // Only recorded nested invocations are evidence; never infer sends by parsing JavaScript.
        if let Some(metadata) = envelope.item.executed_tool_call_metadata() {
            for call in metadata.executed_tool_calls.iter().flatten() {
                let ExecutedToolCallArguments::Raw(args) = call.arguments() else {
                    continue;
                };
                let name = configured_handoff_names
                    .iter()
                    .find_map(|(recorded_name, name)| {
                        (recorded_name == &call.name).then_some(*name)
                    })
                    .unwrap_or(&call.name);
                if let Some(recipient) = recipient(name, args) {
                    handoffs.push(RootHandoff {
                        recipient,
                        boundary,
                    });
                }
            }
        }
    }
    handoffs
}

pub(super) fn handoff_boundaries(
    handoffs: &[RootHandoff],
    worker: &AgentPath,
) -> Vec<RetainedContextOrder> {
    handoffs
        .iter()
        .filter(|handoff| {
            worker == &handoff.recipient
                || worker
                    .as_str()
                    .strip_prefix(handoff.recipient.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
        .map(|handoff| handoff.boundary)
        .collect()
}

fn recipient(name: &str, args: &serde_json::Value) -> Option<AgentPath> {
    let name = name
        .strip_prefix("collaboration.")
        .or_else(|| name.strip_prefix("collaboration__"))
        .unwrap_or(name);
    let field = match name {
        "spawn_agent" => "task_name",
        "send_message" | "followup_task" => "target",
        _ => return None,
    };
    args[field]
        .as_str()
        .and_then(|target| AgentPath::root().resolve(target).ok())
        .filter(|path| !path.is_root())
}
