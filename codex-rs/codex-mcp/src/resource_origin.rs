//! Thread-owned, bounded provenance for app-hosted widget resources.

use std::collections::VecDeque;

use anyhow::Context;
use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_protocol::ThreadId;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::McpResourceOrigin;
use codex_protocol::mcp::McpResourceOriginCheckpoint;
use codex_protocol::protocol::EventMsg;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;

use crate::CODEX_APPS_MCP_SERVER_NAME;
use crate::McpBinding;

const MAX_ORIGINS: usize = 64;
const MAX_ORIGIN_BYTES: usize = 1024;

#[derive(Default)]
pub(crate) struct ResourceOrigins {
    origins: VecDeque<ResourceOrigin>,
    turns: VecDeque<String>,
    current_turn_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResourceOrigin {
    call_id: String,
    turn_id: Option<String>,
    tool: String,
    connector_id: String,
    link_id: Option<String>,
    uri: String,
    ambiguous_account: bool,
}

impl ResourceOrigins {
    pub(crate) fn checkpoint(&self) -> Option<McpResourceOriginCheckpoint> {
        (!self.origins.is_empty()).then(|| McpResourceOriginCheckpoint {
            origins: self
                .origins
                .iter()
                .map(|origin| McpResourceOrigin {
                    call_id: origin.call_id.clone(),
                    turn_id: origin.turn_id.clone(),
                    tool: origin.tool.clone(),
                    connector_id: origin.connector_id.clone(),
                    link_id: origin.link_id.clone(),
                    uri: origin.uri.clone(),
                    ambiguous_account: origin.ambiguous_account,
                })
                .collect(),
            turns: self.turns.iter().cloned().collect(),
            current_turn_id: self.current_turn_id.clone(),
        })
    }

    pub(crate) fn restore_checkpoint(&mut self, checkpoint: &McpResourceOriginCheckpoint) {
        if checkpoint.origins.len() > MAX_ORIGINS
            || checkpoint.turns.len() > MAX_ORIGINS
            || checkpoint
                .turns
                .iter()
                .any(|turn_id| turn_id.len() > MAX_ORIGIN_BYTES)
            || checkpoint
                .current_turn_id
                .as_ref()
                .is_some_and(|turn_id| turn_id.len() > MAX_ORIGIN_BYTES)
        {
            *self = Self::default();
            return;
        }

        let origins = checkpoint
            .origins
            .iter()
            .map(|origin| ResourceOrigin {
                call_id: origin.call_id.clone(),
                turn_id: origin.turn_id.clone(),
                tool: origin.tool.clone(),
                connector_id: origin.connector_id.clone(),
                link_id: origin.link_id.clone(),
                uri: origin.uri.clone(),
                ambiguous_account: origin.ambiguous_account,
            })
            .collect::<VecDeque<_>>();
        if origins.iter().any(|origin| {
            origin.byte_len() > MAX_ORIGIN_BYTES || origin.connector_id.trim().is_empty()
        }) {
            *self = Self::default();
            return;
        }

        *self = Self {
            origins,
            turns: checkpoint.turns.iter().cloned().collect(),
            current_turn_id: checkpoint.current_turn_id.clone(),
        };
    }

    pub(crate) fn observe(&mut self, event: &EventMsg) {
        match event {
            EventMsg::TurnStarted(event) if event.turn_id.len() <= MAX_ORIGIN_BYTES => {
                self.current_turn_id = Some(event.turn_id.clone());
                if self.turns.back() != Some(&event.turn_id) {
                    self.turns.push_back(event.turn_id.clone());
                    if self.turns.len() > MAX_ORIGINS {
                        self.turns.pop_front();
                    }
                }
            }
            EventMsg::ItemCompleted(event) => {
                let TurnItem::McpToolCall(item) = &event.item else {
                    return;
                };
                if item.status == McpToolCallStatus::Completed {
                    self.remember(
                        &item.id,
                        Some(&event.turn_id),
                        &item.server,
                        &item.tool,
                        &item.arguments,
                        item.connector_id.as_deref(),
                        item.link_id.as_deref(),
                        item.mcp_app_resource_uri.as_deref(),
                    );
                }
            }
            EventMsg::McpToolCallEnd(event)
                if event
                    .result
                    .as_ref()
                    .is_ok_and(|result| !result.is_error.unwrap_or(false)) =>
            {
                let turn_id = self.current_turn_id.clone();
                self.remember(
                    &event.call_id,
                    turn_id.as_deref(),
                    &event.invocation.server,
                    &event.invocation.tool,
                    event
                        .invocation
                        .arguments
                        .as_ref()
                        .unwrap_or(&serde_json::Value::Null),
                    event.connector_id.as_deref(),
                    event.link_id.as_deref(),
                    event.mcp_app_resource_uri.as_deref(),
                );
            }
            EventMsg::ThreadRolledBack(event) => {
                for _ in 0..event.num_turns {
                    let Some(turn_id) = self.turns.pop_back() else {
                        *self = Self::default();
                        return;
                    };
                    self.origins.retain(|origin| {
                        origin.turn_id.is_some()
                            && origin.turn_id.as_deref() != Some(turn_id.as_str())
                    });
                }
                self.current_turn_id = self.turns.back().cloned();
            }
            _ => {}
        }
    }

    pub(crate) fn find(&self, call_id: &str) -> anyhow::Result<ResourceOrigin> {
        self.origins
            .iter()
            .rev()
            .find(|origin| origin.call_id == call_id)
            .cloned()
            .context("originating MCP tool call was not found or did not complete successfully")
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "only bounded provenance fields are retained"
    )]
    fn remember(
        &mut self,
        call_id: &str,
        turn_id: Option<&str>,
        server: &str,
        tool: &str,
        arguments: &serde_json::Value,
        connector_id: Option<&str>,
        link_id: Option<&str>,
        uri: Option<&str>,
    ) {
        if server != CODEX_APPS_MCP_SERVER_NAME {
            return;
        }
        let Some(connector_id) = connector_id.filter(|value| !value.trim().is_empty()) else {
            return;
        };
        let Some(uri) = uri else {
            return;
        };
        let link_id = link_id.filter(|value| !value.trim().is_empty());
        let ambiguous_account = arguments
            .get("link_id")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .is_some_and(|argument_link_id| Some(argument_link_id) != link_id);
        let origin = ResourceOrigin {
            call_id: call_id.to_owned(),
            turn_id: turn_id.map(str::to_owned),
            tool: tool.to_owned(),
            connector_id: connector_id.to_owned(),
            link_id: link_id.map(str::to_owned),
            uri: uri.to_owned(),
            ambiguous_account,
        };
        if origin.byte_len() > MAX_ORIGIN_BYTES {
            return;
        }

        if let Some(index) = self
            .origins
            .iter()
            .position(|existing| existing.call_id == origin.call_id)
        {
            self.origins.remove(index);
        }
        if self.origins.len() >= MAX_ORIGINS {
            self.origins.pop_front();
        }
        self.origins.push_back(origin);
    }
}

impl ResourceOrigin {
    fn byte_len(&self) -> usize {
        self.call_id.len()
            + self.turn_id.as_ref().map_or(0, String::len)
            + self.tool.len()
            + self.connector_id.len()
            + self.link_id.as_ref().map_or(0, String::len)
            + self.uri.len()
    }

    pub(crate) async fn read(
        &self,
        binding: &McpBinding,
        thread_id: ThreadId,
        uri: &str,
    ) -> anyhow::Result<ReadResourceResult> {
        if self.uri != uri {
            anyhow::bail!("originating MCP tool call does not match the requested resource");
        }
        if self.ambiguous_account {
            anyhow::bail!("originating MCP tool call has ambiguous account selection");
        }

        let tool_info = binding
            .tool_info(CODEX_APPS_MCP_SERVER_NAME, &self.tool)
            .context("originating MCP tool is unavailable")?;
        if tool_info.connector_id.as_deref() != Some(self.connector_id.as_str()) {
            anyhow::bail!("originating MCP tool connector does not match its app context");
        }
        let tool_meta = tool_info.tool.meta.as_ref().map(|meta| &meta.0);
        let current_link_id = tool_meta
            .and_then(|meta| meta.get("link_id"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty());
        if current_link_id != self.link_id.as_deref() {
            anyhow::bail!("originating MCP tool link does not match its app context");
        }
        if self.link_id.is_none()
            && tool_meta
                .and_then(|meta| meta.get("_codex_apps"))
                .and_then(|meta| meta.get("requires_explicit_link_id"))
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            anyhow::bail!("originating MCP tool requires an explicit account link");
        }

        let annotations = tool_info.tool.annotations.as_ref();
        if !AppToolPolicyEvaluator::new(&binding.config().config_layer_stack)
            .policy(AppToolPolicyInput {
                connector_id: Some(&self.connector_id),
                tool_name: tool_info.tool.name.as_ref(),
                tool_title: tool_info.tool.title.as_deref(),
                destructive_hint: annotations.and_then(|value| value.destructive_hint),
                open_world_hint: annotations.and_then(|value| value.open_world_hint),
            })
            .enabled
        {
            anyhow::bail!("originating MCP tool is disabled by app configuration");
        }

        let meta = serde_json::from_value(serde_json::json!({
            "threadId": thread_id,
            "x-codex-turn-metadata": {
                "mcp_request_meta": {
                    "selected_connector_ids": [&self.connector_id],
                    "link_id": &self.link_id,
                }
            },
        }))?;
        binding
            .read_resource(
                CODEX_APPS_MCP_SERVER_NAME,
                ReadResourceRequestParams::new(uri).with_meta(meta),
            )
            .await
    }
}
