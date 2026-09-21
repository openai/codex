//! Model tools over a caller-bound board. Results stay valid, bounded JSON.

mod arguments;
mod spec;

use crate::AgentMessageBoard;
use crate::ChannelQuery;
use crate::CreateChannelRequest;
use crate::PageRequest;
use crate::PostDestination;
use crate::PostQuery;
use crate::PostRequest;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::SortDirection;
use crate::SubscriptionChange;
use crate::SubscriptionRequest;
use crate::SubscriptionTarget;
use crate::ThreadQuery;
use crate::ThreadSort;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_tools::FunctionCallError;
use codex_tools::JsonToolOutput;
use codex_tools::ToolCall;
use codex_tools::ToolCallSource;
use codex_tools::ToolExecutor;
use codex_tools::ToolExecutorFuture;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use futures::future::BoxFuture;
use serde::Serialize;
use serde_json::Value;
use std::num::NonZeroU32;
use std::sync::Arc;

// Includes JSON escaping and metadata; applies equally to direct and Code Mode calls.
const MAX_RESPONSE_BYTES: usize = 8_000;

/// Creates tools without registering or enabling the extension.
/// The caller and its path must come from the host's authoritative tree metadata.
pub fn message_board_tools(
    board: Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    caller_path: AgentPath,
) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
    spec::NAMES
        .into_iter()
        .map(|name| {
            Arc::new(BoardTool {
                board: board.clone(),
                caller,
                caller_path: caller_path.clone(),
                name,
            }) as Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>
        })
        .collect()
}

struct BoardTool {
    board: Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    caller_path: AgentPath,
    name: &'static str,
}

impl<'call> ToolExecutor<ToolCall<'call>> for BoardTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced("collaboration", self.name)
    }
    fn spec(&self) -> ToolSpec {
        spec::tool(self.name)
    }
    fn supports_parallel_tool_calls(&self) -> bool {
        matches!(
            self.name,
            "get_channels" | "list_threads" | "search_posts" | "read_thread" | "read_post"
        )
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let raw = call.function_arguments()?;
            if raw.len() > 128 * 1024 {
                return Err(model_error("message-board arguments exceed 128 KiB"));
            }
            let budget = call.response_byte_budget(MAX_RESPONSE_BYTES);
            let result = self.execute(raw, &call, budget).await?;
            if result.to_string().len() > budget {
                return Err(model_error(
                    "Result exceeds the output budget. Reduce limit or max_chars_per_post; use read_post with a smaller limit_chars for long posts.",
                ));
            }
            Ok(
                Box::new(JsonToolOutput::new(result).with_external_context())
                    as Box<dyn codex_tools::ToolOutput>,
            )
        })
    }
}

impl BoardTool {
    async fn execute(
        &self,
        raw: &str,
        call: &ToolCall<'_>,
        budget: usize,
    ) -> Result<Value, FunctionCallError> {
        let board = &self.board;
        let caller = self.caller;
        let page = |limit: Option<NonZeroU32>, cursor, scale| PageRequest {
            limit: nonzero(limit.map_or(20, NonZeroU32::get).min(50) / scale),
            cursor,
        };
        let preview = |requested: Option<NonZeroU32>, scale| {
            nonzero(requested.map_or(1000, NonZeroU32::get).min(20_000) / scale)
        };
        let direction = |recent_first: Option<bool>| {
            if recent_first.unwrap_or(true) {
                SortDirection::NewestFirst
            } else {
                SortDirection::OldestFirst
            }
        };
        match self.name {
            "create_channel" => {
                let arguments::CreateChannel {
                    channel_name,
                    subscribe,
                } = serde_json::from_str(raw).map_err(model_error)?;
                self.check_mutation_budget(budget, /*target_path_bytes*/ 0)?;
                encode(
                    board
                        .create_channel(
                            caller,
                            CreateChannelRequest {
                                channel_name,
                                subscription: if subscribe.unwrap_or(true) {
                                    SubscriptionChange::Subscribe
                                } else {
                                    SubscriptionChange::Unsubscribe
                                },
                            },
                        )
                        .await,
                )
            }
            "get_channels" => {
                let arguments::GetChannels {
                    query,
                    recent_first,
                    limit,
                    cursor,
                } = serde_json::from_str(raw).map_err(model_error)?;
                bounded_read(budget, |scale| {
                    board.list_channels(
                        caller,
                        ChannelQuery {
                            query: query.clone(),
                            direction: direction(recent_first),
                            page: page(limit, cursor.clone(), scale),
                        },
                    )
                })
                .await
            }
            "list_threads" => {
                let arguments::ListThreads {
                    channel_name,
                    sort,
                    recent_first,
                    limit,
                    cursor,
                    max_chars_per_post,
                } = serde_json::from_str(raw).map_err(model_error)?;
                bounded_read(budget, |scale| {
                    board.list_threads(
                        caller,
                        ThreadQuery {
                            channel_name: channel_name.clone(),
                            sort: sort.unwrap_or(ThreadSort::Created),
                            direction: direction(recent_first),
                            max_chars_per_post: preview(max_chars_per_post, scale),
                            page: page(limit, cursor.clone(), scale),
                        },
                    )
                })
                .await
            }
            "search_posts" => {
                let arguments::SearchPosts {
                    channel_name,
                    query,
                    after_message_id,
                    author,
                    limit,
                    cursor,
                    max_chars_per_post,
                } = serde_json::from_str(raw).map_err(model_error)?;
                let author = author
                    .map(|path| self.caller_path.resolve(&path))
                    .transpose()
                    .map_err(model_error)?;
                bounded_read(budget, |scale| {
                    board.search_posts(
                        caller,
                        PostQuery {
                            channel_name: channel_name.clone(),
                            query: query.clone(),
                            after_message_id,
                            author: author.clone(),
                            max_chars_per_post: preview(max_chars_per_post, scale),
                            page: page(limit, cursor.clone(), scale),
                        },
                    )
                })
                .await
            }
            "read_thread" => {
                let arguments::ReadThread {
                    thread_id,
                    limit,
                    cursor,
                    max_chars_per_post,
                } = serde_json::from_str(raw).map_err(model_error)?;
                bounded_read(budget, |scale| {
                    board.read_thread(
                        caller,
                        ReadThreadRequest {
                            thread_id,
                            max_chars_per_post: preview(max_chars_per_post, scale),
                            page: page(limit, cursor.clone(), scale),
                        },
                    )
                })
                .await
            }
            "read_post" => {
                let arguments::ReadPost {
                    message_id,
                    offset_chars,
                    limit_chars,
                } = serde_json::from_str(raw).map_err(model_error)?;
                bounded_read(budget, |scale| {
                    board.read_post(
                        caller,
                        ReadPostRequest {
                            message_id,
                            offset_chars: offset_chars.unwrap_or_default(),
                            limit_chars: nonzero(
                                limit_chars.map_or(20_000, NonZeroU32::get).min(20_000) / scale,
                            ),
                        },
                    )
                })
                .await
            }
            "subscribe" | "unsubscribe" => {
                let arguments::Subscription {
                    channel_name,
                    thread_id,
                    target_agent,
                } = serde_json::from_str(raw).map_err(model_error)?;
                self.check_mutation_budget(budget, target_agent.as_ref().map_or(0, String::len))?;
                let target = match (channel_name, thread_id) {
                    (Some(channel), None) => SubscriptionTarget::Channel(channel),
                    (None, Some(thread)) => SubscriptionTarget::Thread(thread),
                    _ => {
                        return Err(model_error(
                            "Supply exactly one of channel_name or thread_id",
                        ));
                    }
                };
                encode(
                    board
                        .set_subscription(
                            caller,
                            SubscriptionRequest {
                                target,
                                target_agent: target_agent
                                    .map(|path| self.caller_path.resolve(&path))
                                    .transpose()
                                    .map_err(model_error)?,
                                change: if self.name == "subscribe" {
                                    SubscriptionChange::Subscribe
                                } else {
                                    SubscriptionChange::Unsubscribe
                                },
                            },
                        )
                        .await,
                )
            }
            "post" => {
                let arguments::Post {
                    text,
                    channel_name,
                    new_channel_name,
                    thread_id,
                    agents_to_notify,
                } = serde_json::from_str(raw).map_err(model_error)?;
                self.check_mutation_budget(budget, /*target_path_bytes*/ 0)?;
                let destination = match (channel_name, new_channel_name, thread_id) {
                    (Some(channel), None, None) => PostDestination::Channel(channel),
                    (None, Some(channel), None) => PostDestination::NewChannel(channel),
                    (None, None, Some(thread)) => PostDestination::Thread(thread),
                    _ => {
                        return Err(model_error(
                            "Supply exactly one of channel_name, new_channel_name or thread_id",
                        ));
                    }
                };
                let invocation = match &call.source {
                    ToolCallSource::Direct => call.call_id.clone(),
                    ToolCallSource::CodeMode {
                        cell_id,
                        runtime_tool_call_id,
                    } => format!("{cell_id}:{runtime_tool_call_id}"),
                };
                encode(
                    board
                        .post(
                            caller,
                            PostRequest {
                                request_id: format!("{}:{invocation}", call.turn_id),
                                destination,
                                text,
                                agents_to_notify: agents_to_notify
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|path| self.caller_path.resolve(&path))
                                    .collect::<Result<_, _>>()
                                    .map_err(model_error)?,
                            },
                        )
                        .await,
                )
            }
            _ => unreachable!("only registered message-board tools can be called"),
        }
    }

    fn check_mutation_budget(
        &self,
        budget: usize,
        target_path_bytes: usize,
    ) -> Result<(), FunctionCallError> {
        // Reserve metadata, escaped channel names and agent paths before any write.
        if budget < 2048 + self.caller_path.len() + target_path_bytes {
            return Err(model_error(
                "Output budget is too small to acknowledge a message-board mutation; no change was made.",
            ));
        }
        Ok(())
    }
}

/// Try the requested read first. Halve limits only when its serialized output is too large.
/// Reissuing the read lets each backend generate a cursor for exactly the returned page.
async fn bounded_read<'a, T: Serialize>(
    budget: usize,
    fetch: impl Fn(u32) -> BoxFuture<'a, codex_protocol::error::Result<T>>,
) -> Result<Value, FunctionCallError> {
    // Read limits are capped at 20,000, so the final attempt asks for one character/item.
    for shift in 0..=15 {
        let result = encode(fetch(1 << shift).await)?;
        if result.to_string().len() <= budget {
            return Ok(result);
        }
    }
    Err(model_error(
        "The output budget is too small for this result's metadata.",
    ))
}

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).unwrap_or(NonZeroU32::MIN)
}
fn model_error(error: impl std::fmt::Display) -> FunctionCallError {
    FunctionCallError::RespondToModel(error.to_string().chars().take(512).collect())
}
fn encode<T: Serialize>(
    value: codex_protocol::error::Result<T>,
) -> Result<Value, FunctionCallError> {
    serde_json::to_value(value.map_err(model_error)?).map_err(model_error)
}
