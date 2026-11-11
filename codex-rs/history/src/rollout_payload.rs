use std::borrow::Cow;

use super::CodexHarnessMetadata;
use super::CompactedItem;
use super::EventMsg;
use super::InterAgentCommunication;
use super::McpResourceOriginCheckpoint;
use super::RealtimeItem;
use super::ResponseItem;
use super::ResponseItemEnvelope;
use super::RolloutItem;
use super::SecurityRiskScore;
use super::SessionMetaLine;
use super::TokenUsageRecord;
use super::TurnContextItem;
use super::WorldStateItem;
use crate::RetainedContextEvent;
use crate::RetainedUserMessage;
use codex_protocol::ContextWindowId;
use codex_protocol::ResponseItemId;
use codex_protocol::ResponseItemIdAllocation;
use codex_protocol::models::ContentItem;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::RawFileSystemSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

const CODE_MODE_DELIVERY_COMPLETE: &str = "codex:code-mode-delivery:v1:complete";
const CODE_MODE_DELIVERY_INCOMPLETE: &str = "codex:code-mode-delivery:v1:incomplete:";
const CODE_MODE_DELIVERY_UNAVAILABLE: &str = "The content of a confirmed assistant message is unavailable. Do not infer what was asked or authorized.";

/// Persisted rollout item used by core history and rollout storage.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum RolloutItemWire<'a> {
    SessionMeta {
        payload: Cow<'a, SessionMetaLine>,
    },
    DynamicToolsUpdated {
        payload: DynamicToolsUpdatedPayload<'a>,
    },
    ResponseItem {
        payload: ItemPayload<'a, ResponseItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<Cow<'a, CodexHarnessMetadata>>,
    },
    InterAgentCommunication {
        payload: ItemPayload<'a, InterAgentCommunication>,
    },
    InterAgentCommunicationMetadata {
        payload: InterAgentCommunicationMetadataPayload,
    },
    Compacted {
        payload: Cow<'a, Box<CompactedItem>>,
    },
    TurnContext {
        #[serde(serialize_with = "serialize_turn_context")]
        payload: Cow<'a, TurnContextItem>,
    },
    TokenUsageRecord {
        payload: Cow<'a, TokenUsageRecord>,
    },
    WorldState {
        payload: Cow<'a, WorldStateItem>,
    },
    RetainedContext {
        payload: Cow<'a, crate::RetainedContextEvent>,
    },
    SecurityRiskScore {
        payload: Cow<'a, SecurityRiskScore>,
    },
    EventMsg {
        payload: Cow<'a, EventMsg>,
    },
    RealtimeItem {
        payload: Cow<'a, RealtimeItem>,
    },
}

fn serialize_turn_context<S>(context: &TurnContextItem, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // Keep older rollout readers usable during deployment and rollback. Runtime
    // contexts stay profile-only; compatibility fields exist only on the wire.
    #[derive(Serialize)]
    struct Wire<'a> {
        #[serde(flatten)]
        context: &'a TurnContextItem,
        sandbox_policy: SandboxPolicy,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_system_sandbox_policy: Option<RawFileSystemSandboxPolicy>,
    }

    let cwd = context.cwd.as_path();
    let (file_system, network) = context.permission_profile.to_runtime_permissions();
    let sandbox_policy = context
        .permission_profile
        .to_legacy_sandbox_policy(cwd)
        .unwrap_or_else(|_| {
            let tmpdir_writable = std::env::var_os("TMPDIR")
                .filter(|path| !path.is_empty())
                .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
                .is_some_and(|path| file_system.can_write_local_path_with_cwd(path.as_path(), cwd));
            let slash_tmp = std::path::Path::new("/tmp");
            let slash_tmp_writable = slash_tmp.is_absolute()
                && slash_tmp.is_dir()
                && file_system.can_write_local_path_with_cwd(slash_tmp, cwd);
            SandboxPolicy::WorkspaceWrite {
                writable_roots: file_system
                    .get_writable_roots_with_cwd(cwd)
                    .into_iter()
                    .map(|root| root.root)
                    .filter(|root| root != &context.cwd)
                    .collect(),
                network_access: network.is_enabled(),
                exclude_tmpdir_env_var: !tmpdir_writable,
                exclude_slash_tmp: !slash_tmp_writable,
            }
        });
    let legacy_file_system =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&sandbox_policy, cwd);
    let file_system_sandbox_policy = (file_system != legacy_file_system)
        .then(|| file_system.try_into().ok())
        .flatten();
    Wire {
        context,
        sandbox_policy,
        file_system_sandbox_policy,
    }
    .serialize(serializer)
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub(super) struct DynamicToolsUpdatedPayload<'a> {
    thread_id: codex_protocol::ThreadId,
    tools: Cow<'a, [codex_protocol::dynamic_tools::DynamicToolSpec]>,
}

/// The protocol item keeps its wire ID. The optional allocation lets replay resume the sequence.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(super) struct ItemPayload<'a, T: Clone> {
    #[serde(flatten)]
    item: Cow<'a, T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id_allocation: Option<Cow<'a, ResponseItemIdAllocation>>,
}

impl<'a, T: Clone> ItemPayload<'a, T> {
    fn borrowed(item: &'a T, id: Option<&'a ResponseItemId>) -> Self {
        Self {
            item: Cow::Borrowed(item),
            id_allocation: id.and_then(ResponseItemId::allocation).map(Cow::Borrowed),
        }
    }
}

impl<'a> From<&'a ResponseItem> for ItemPayload<'a, ResponseItem> {
    fn from(item: &'a ResponseItem) -> Self {
        Self::borrowed(item, item.id())
    }
}

impl From<ItemPayload<'_, ResponseItem>> for ResponseItem {
    fn from(payload: ItemPayload<'_, ResponseItem>) -> Self {
        let mut item = payload.item.into_owned();
        if let Some(allocation) = payload.id_allocation
            && let Some(id) = item.id().cloned()
        {
            item.set_id(Some(id.with_allocation(allocation.into_owned())));
        }
        item
    }
}

impl<'a> From<&'a RolloutItem> for RolloutItemWire<'a> {
    fn from(item: &'a RolloutItem) -> Self {
        match item {
            RolloutItem::SessionMeta(payload) => Self::SessionMeta {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::DynamicToolsUpdated { thread_id, tools } => Self::DynamicToolsUpdated {
                payload: DynamicToolsUpdatedPayload {
                    thread_id: *thread_id,
                    tools: Cow::Borrowed(tools),
                },
            },
            RolloutItem::ResponseItem(envelope) => Self::ResponseItem {
                payload: (&envelope.item).into(),
                metadata: envelope.metadata.as_ref().map(Cow::Borrowed),
            },
            RolloutItem::InterAgentCommunication(payload) => Self::InterAgentCommunication {
                payload: ItemPayload::borrowed(payload, payload.id.as_ref()),
            },
            RolloutItem::InterAgentCommunicationMetadata { trigger_turn } => {
                Self::InterAgentCommunicationMetadata {
                    payload: InterAgentCommunicationMetadataPayload {
                        trigger_turn: *trigger_turn,
                    },
                }
            }
            RolloutItem::Compacted(payload) => Self::Compacted {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::TurnContext(payload) => Self::TurnContext {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::TokenUsageRecord(payload) => Self::TokenUsageRecord {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::WorldState(payload) => Self::WorldState {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::RetainedContext(RetainedContextEvent::DeliveredAssistantMessage {
                message,
                acceptance_order,
                retained_source,
            }) => {
                // Earlier clients preserve this as genuine assistant context, including across compaction.
                let text = if message.complete {
                    message.text.clone()
                } else {
                    CODE_MODE_DELIVERY_UNAVAILABLE.to_owned()
                };
                let marker = if message.complete {
                    CODE_MODE_DELIVERY_COMPLETE.to_owned()
                } else {
                    format!("{CODE_MODE_DELIVERY_INCOMPLETE}{}", message.text)
                };
                Self::ResponseItem {
                    payload: ItemPayload {
                        item: Cow::Owned(ResponseItem::Message {
                            status: None,
                            encrypted_content: None,
                            id: message.message_id.clone().map(ResponseItemId::from_server),
                            role: "assistant".to_owned(),
                            content: vec![ContentItem::OutputText {
                                annotations: None,
                                logprobs: None,
                                text,
                            }],
                            // Older Guardian clients exclude commentary from root-to-worker
                            // review. Keep the phase unspecified so they can review this
                            // confirmed delivery without claiming it ended the turn.
                            phase: None,
                            internal_chat_message_metadata_passthrough: Some(
                                InternalChatMessageMetadataPassthrough {
                                    turn_id: Some(message.turn_id.clone()),
                                    ..Default::default()
                                },
                            ),
                        }),
                        id_allocation: None,
                    },
                    metadata: Some(Cow::Owned(CodexHarnessMetadata {
                        retained_source: retained_source.clone(),
                        delivered_assistant_message: Some(marker),
                        user_input_order: Some(*acceptance_order),
                        ..Default::default()
                    })),
                }
            }
            RolloutItem::RetainedContext(payload) => Self::RetainedContext {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::SecurityRiskScore(payload) => Self::SecurityRiskScore {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::EventMsg(payload) => Self::EventMsg {
                payload: Cow::Borrowed(payload),
            },
            RolloutItem::RealtimeItem(payload) => Self::RealtimeItem {
                payload: Cow::Borrowed(payload),
            },
        }
    }
}

impl From<RolloutItemWire<'_>> for RolloutItem {
    fn from(item: RolloutItemWire<'_>) -> Self {
        match item {
            RolloutItemWire::SessionMeta { payload } => {
                Self::SessionMeta(Box::new(payload.into_owned()))
            }
            RolloutItemWire::DynamicToolsUpdated { payload } => Self::DynamicToolsUpdated {
                thread_id: payload.thread_id,
                tools: payload.tools.into_owned(),
            },
            RolloutItemWire::ResponseItem { payload, metadata } => {
                if let (
                    ResponseItem::Message {
                        id,
                        role,
                        content,
                        internal_chat_message_metadata_passthrough: Some(passthrough),
                        ..
                    },
                    Some(metadata),
                ) = (payload.item.as_ref(), metadata.as_deref())
                    && role == "assistant"
                    && let [ContentItem::OutputText { text, .. }] = content.as_slice()
                    && let Some(turn_id) = passthrough.turn_id.as_ref()
                    && let Some(acceptance_order) = metadata.user_input_order
                    && let Some(marker) = metadata.delivered_assistant_message.as_deref()
                    && let Some((text, complete)) = if marker == CODE_MODE_DELIVERY_COMPLETE {
                        Some((text.as_str(), true))
                    } else {
                        marker
                            .strip_prefix(CODE_MODE_DELIVERY_INCOMPLETE)
                            .map(|text| (text, false))
                    }
                {
                    return Self::RetainedContext(
                        RetainedContextEvent::DeliveredAssistantMessage {
                            message: RetainedUserMessage {
                                origin: crate::UserInputOrigin::User,
                                turn_id: turn_id.clone(),
                                message_id: id.as_ref().map(ToString::to_string),
                                text: text.to_owned(),
                                complete,
                                phase: None,
                            },
                            acceptance_order,
                            retained_source: metadata.retained_source.clone(),
                        },
                    );
                }
                Self::ResponseItem(ResponseItemEnvelope {
                    item: payload.into(),
                    metadata: metadata.map(Cow::into_owned),
                })
            }
            RolloutItemWire::InterAgentCommunication { payload } => {
                let mut message = payload.item.into_owned();
                if let Some(allocation) = payload.id_allocation
                    && let Some(id) = message.id.take()
                {
                    message.id = Some(id.with_allocation(allocation.into_owned()));
                }
                Self::InterAgentCommunication(message)
            }
            RolloutItemWire::InterAgentCommunicationMetadata { payload } => {
                Self::InterAgentCommunicationMetadata {
                    trigger_turn: payload.trigger_turn,
                }
            }
            RolloutItemWire::Compacted { payload } => Self::Compacted(payload.into_owned()),
            RolloutItemWire::TurnContext { payload } => Self::TurnContext(payload.into_owned()),
            RolloutItemWire::TokenUsageRecord { payload } => {
                Self::TokenUsageRecord(payload.into_owned())
            }
            RolloutItemWire::WorldState { payload } => Self::WorldState(payload.into_owned()),
            RolloutItemWire::RetainedContext { payload } => {
                Self::RetainedContext(payload.into_owned())
            }
            RolloutItemWire::SecurityRiskScore { payload } => {
                Self::SecurityRiskScore(payload.into_owned())
            }
            RolloutItemWire::EventMsg { payload } => Self::EventMsg(payload.into_owned()),
            RolloutItemWire::RealtimeItem { payload } => Self::RealtimeItem(payload.into_owned()),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub(super) struct InterAgentCommunicationMetadataPayload {
    trigger_turn: bool,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub(super) struct CompactedItemWire<'a> {
    message: Cow<'a, str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replacement_history: Option<Vec<ItemPayload<'a, ResponseItem>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replacement_history_metadata: Option<Vec<Cow<'a, CodexHarnessMetadata>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guardian_history: Option<Cow<'a, crate::GuardianHistoryCheckpoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retained_context: Option<Cow<'a, crate::RetainedContext>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mcp_resource_origins: Option<Cow<'a, McpResourceOriginCheckpoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    window_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_window_id: Option<Cow<'a, ContextWindowId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_window_id: Option<Cow<'a, ContextWindowId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ContextWindowId>")]
    window_id: Option<WindowIdWire<'a>>,
    #[serde(default)]
    compaction_response_id: Option<Cow<'a, str>>,
    #[serde(default)]
    latest_token_usage_record: Option<Cow<'a, TokenUsageRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resume_metadata: Option<Cow<'a, crate::CompactionResumeMetadata>>,
}

impl<'a> From<&'a CompactedItem> for CompactedItemWire<'a> {
    fn from(item: &'a CompactedItem) -> Self {
        let replacement_history_metadata = item
            .replacement_history
            .as_ref()
            .filter(|items| items.iter().any(|envelope| envelope.metadata.is_some()))
            .map(|items| {
                items
                    .iter()
                    .map(|envelope| match envelope.metadata.as_ref() {
                        Some(metadata) => Cow::Borrowed(metadata),
                        None => Cow::Owned(CodexHarnessMetadata::default()),
                    })
                    .collect()
            });

        Self {
            message: Cow::Borrowed(&item.message),
            replacement_history: item.replacement_history.as_ref().map(|items| {
                items
                    .iter()
                    .map(|envelope| (&envelope.item).into())
                    .collect()
            }),
            replacement_history_metadata,
            guardian_history: item.guardian_history.as_ref().map(Cow::Borrowed),
            retained_context: item.retained_context.as_ref().map(Cow::Borrowed),
            mcp_resource_origins: item.mcp_resource_origins.as_ref().map(Cow::Borrowed),
            window_number: item.window_number,
            first_window_id: item.first_window_id.as_ref().map(Cow::Borrowed),
            previous_window_id: item.previous_window_id.as_ref().map(Cow::Borrowed),
            window_id: item
                .window_id
                .as_ref()
                .map(|window_id| WindowIdWire::Id(Cow::Borrowed(window_id))),
            compaction_response_id: item.compaction_response_id.as_deref().map(Cow::Borrowed),
            latest_token_usage_record: item.latest_token_usage_record.as_ref().map(Cow::Borrowed),
            resume_metadata: item.resume_metadata.as_ref().map(Cow::Borrowed),
        }
    }
}

impl TryFrom<CompactedItemWire<'_>> for CompactedItem {
    type Error = String;

    fn try_from(item: CompactedItemWire<'_>) -> Result<Self, Self::Error> {
        let replacement_history = match (
            item.replacement_history,
            item.replacement_history_metadata,
        ) {
            (Some(items), Some(metadata)) => {
                let history_len = items.len();
                let metadata_len = metadata.len();
                if history_len != metadata_len {
                    return Err(format!(
                        "replacement_history_metadata has {metadata_len} entries for {history_len} history items"
                    ));
                }
                Some(
                    items
                        .into_iter()
                        .zip(metadata)
                        .map(|(item, metadata)| ResponseItemEnvelope {
                            item: item.into(),
                            metadata: Some(metadata.into_owned()),
                        })
                        .collect(),
                )
            }
            (Some(items), None) => Some(
                items
                    .into_iter()
                    .map(|item| ResponseItemEnvelope::new(item.into()))
                    .collect(),
            ),
            (None, Some(_)) => {
                return Err("replacement_history_metadata requires replacement_history".to_string());
            }
            (None, None) => None,
        };

        let mut window_number = item.window_number;
        let window_id = match item.window_id {
            Some(WindowIdWire::Id(window_id)) => Some(window_id.into_owned()),
            Some(WindowIdWire::LegacyWindowNumber(legacy_window_number)) => {
                window_number.get_or_insert(legacy_window_number);
                None
            }
            None => None,
        };

        Ok(Self {
            message: item.message.into_owned(),
            replacement_history,
            guardian_history: item.guardian_history.map(Cow::into_owned),
            retained_context: item.retained_context.map(Cow::into_owned),
            mcp_resource_origins: item.mcp_resource_origins.map(Cow::into_owned),
            window_number,
            first_window_id: item.first_window_id.map(Cow::into_owned),
            previous_window_id: item.previous_window_id.map(Cow::into_owned),
            window_id,
            compaction_response_id: item.compaction_response_id.map(Cow::into_owned),
            latest_token_usage_record: item.latest_token_usage_record.map(Cow::into_owned),
            resume_metadata: item.resume_metadata.map(Cow::into_owned),
        })
    }
}

// Older rollouts stored the numeric window number in `window_id`.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WindowIdWire<'a> {
    Id(Cow<'a, ContextWindowId>),
    LegacyWindowNumber(u64),
}
