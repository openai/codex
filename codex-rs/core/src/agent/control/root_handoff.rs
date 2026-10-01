//! Best-effort handoff windows plus recent root context, without saved handoff state.
//! History position approximates dispatch order; absent calls leave the existing context unchanged.
//! User inputs bypass relevance filtering; only assistant context is narrowed.

use std::collections::BTreeSet;

use crate::codex_thread::GuardianRootMessage;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextOrder;
use codex_protocol::AgentPath;
use codex_protocol::ToolName;
use codex_protocol::models::ExecutedToolCallArguments;
use codex_protocol::models::ResponseItem;

const ROOT_CONTEXT_WINDOW: usize = 3;

pub(super) fn selected_message_indices(
    history: &[ResponseItemEnvelope],
    messages: &[(Option<RetainedContextOrder>, GuardianRootMessage)],
    worker: &AgentPath,
    tool_namespace: Option<&str>,
) -> Option<BTreeSet<usize>> {
    let configured_handoff_names = ["spawn_agent", "send_message", "followup_task"].map(|name| {
        (
            codex_tools::code_mode_name_for_tool_name(&ToolName::new(
                tool_namespace.map(str::to_owned),
                name,
            )),
            name,
        )
    });
    let mut selected = BTreeSet::new();
    let mut preceding_order = None;
    for envelope in history {
        if let Some(order) = envelope.metadata.as_ref().and_then(|metadata| {
            (!metadata.inherited_user_message)
                .then_some(metadata.user_input_order)
                .flatten()
        }) {
            preceding_order = Some(RetainedContextOrder::Local(order));
        }
        let direct = match &envelope.item {
            ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                ..
            } if namespace.as_deref().is_none_or(|namespace| {
                namespace == "collaboration"
                    || Some(namespace) == tool_namespace
                    || namespace == codex_protocol::DEFAULT_FUNCTION_NAMESPACE
            }) =>
            {
                serde_json::from_str(arguments)
                    .ok()
                    .is_some_and(|args| is_handoff(name, &args, worker))
            }
            _ => false,
        };
        // Recorded nested calls are usable when present. Never parse JavaScript to infer sends.
        let nested = envelope
            .item
            .executed_tool_call_metadata()
            .is_some_and(|metadata| {
                metadata.executed_tool_calls.iter().flatten().any(|call| {
                    let ExecutedToolCallArguments::Raw(args) = call.arguments() else {
                        return false;
                    };
                    let name = configured_handoff_names
                        .iter()
                        .find_map(|(recorded_name, name)| {
                            (recorded_name == &call.name).then_some(*name)
                        })
                        .unwrap_or(&call.name);
                    is_handoff(name, args, worker)
                })
            });
        if (direct || nested)
            && let Some(boundary) = preceding_order
        {
            selected.extend(
                messages
                    .iter()
                    .enumerate()
                    .rev()
                    .filter_map(|(index, (order, message))| {
                        if matches!(message, GuardianRootMessage::UserInput(_)) {
                            return None;
                        }
                        order.filter(|order| *order <= boundary).map(|_| index)
                    })
                    .take(ROOT_CONTEXT_WINDOW),
            );
        }
    }
    if !selected.is_empty() {
        // Include the latest ordinary messages even without a new handoff to this branch.
        selected.extend(
            messages
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(index, (order, message))| {
                    (order.is_some() && !matches!(message, GuardianRootMessage::UserInput(_)))
                        .then_some(index)
                })
                .take(ROOT_CONTEXT_WINDOW),
        );
        // Later user inputs can restrict an earlier handoff even when they are not recent.
        // Leave all user inputs, verified answers, and unordered legacy evidence to the shared cap.
        selected.extend(
            messages
                .iter()
                .enumerate()
                .filter_map(|(index, (order, message))| {
                    (order.is_none()
                        || matches!(
                            message,
                            GuardianRootMessage::User(_) | GuardianRootMessage::UserInput(_)
                        ))
                    .then_some(index)
                }),
        );
    }
    (!selected.is_empty()).then_some(selected)
}

fn is_handoff(name: &str, args: &serde_json::Value, worker: &AgentPath) -> bool {
    let name = name
        .strip_prefix("collaboration.")
        .or_else(|| name.strip_prefix("collaboration__"))
        .unwrap_or(name);
    let field = match name {
        "spawn_agent" => "task_name",
        "send_message" | "followup_task" => "target",
        _ => return false,
    };
    let Some(recipient) = args[field]
        .as_str()
        .and_then(|target| AgentPath::root().resolve(target).ok())
        .filter(|path| !path.is_root())
    else {
        return false;
    };
    worker == &recipient
        || worker
            .as_str()
            .strip_prefix(recipient.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
}
