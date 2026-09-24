//! Trait adapter and queries over indexed in-memory posts. Only page results clone content.

use super::InMemoryAgentMessageBoard;
use super::invalid;
use crate::AgentMessageBoard;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::CreateChannelRequest;
use crate::Page;
use crate::PageRequest;
use crate::PostContent;
use crate::PostMetadata;
use crate::PostPreview;
use crate::PostQuery;
use crate::PostRequest;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::SortDirection;
use crate::SubscriptionRequest;
use crate::SubscriptionState;
use crate::ThreadPage;
use crate::ThreadQuery;
use crate::ThreadSort;
use crate::ThreadSummary;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use caseless::default_case_fold_str;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use std::cmp::Reverse;

impl AgentMessageBoard for InMemoryAgentMessageBoard {
    fn identity(&self) -> SessionId {
        self.identity
    }

    fn create_channel(
        &self,
        caller: ThreadId,
        request: CreateChannelRequest,
    ) -> BoxFuture<'_, Result<ChannelSummary>> {
        Box::pin(self.create_channel(caller, request))
    }

    fn post(&self, caller: ThreadId, request: PostRequest) -> BoxFuture<'_, Result<PostMetadata>> {
        Box::pin(self.post(caller, request))
    }

    fn set_subscription(
        &self,
        caller: ThreadId,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<SubscriptionState>> {
        Box::pin(self.set_subscription(caller, request))
    }

    fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> BoxFuture<'_, Result<PostContent>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let state = self.state.lock().await;
            let post = &state.posts[state.post_index(request.message_id)?];
            let offset = (request.offset_chars as usize).min(post.n_chars);
            let text: String = post
                .request
                .text
                .chars()
                .skip(offset)
                .take((request.limit_chars.get() as usize).min(20_000))
                .collect();
            Ok(PostContent {
                metadata: post.metadata.clone(),
                n_chars: post.n_chars,
                next_offset_chars: offset + text.chars().count(),
                text,
            })
        })
    }

    fn list_channels(
        &self,
        caller: ThreadId,
        query: ChannelQuery,
    ) -> BoxFuture<'_, Result<Page<ChannelSummary>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let search = default_case_fold_str(&query.query.unwrap_or_default());
            let state = self.state.lock().await;
            let mut channels: Vec<_> = state
                .channels
                .values()
                .filter(|channel| channel.search.contains(&search))
                .collect();
            channels.sort_unstable_by_key(|channel| {
                let activity = channel
                    .summary
                    .last_message_id
                    .map_or(channel.summary.created_at.timestamp_micros(), |id| {
                        state.key(state.by_id[&id]).0
                    });
                (activity, &channel.summary.channel_name)
            });
            if query.direction == SortDirection::NewestFirst {
                channels.reverse();
            }
            let page = page(&query.page, channels)?;
            Ok(Page {
                results: page
                    .results
                    .into_iter()
                    .map(|channel| channel.summary.clone())
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
    }

    fn list_threads(
        &self,
        caller: ThreadId,
        query: ThreadQuery,
    ) -> BoxFuture<'_, Result<Page<ThreadSummary>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let state = self.state.lock().await;
            let channel = state
                .channels
                .get(&query.channel_name)
                .ok_or_else(|| invalid("channel not found in this board"))?;
            let mut roots = channel.roots.clone();
            roots.sort_unstable_by_key(|index| {
                let (created, sequence) = state.key(*index);
                let timestamp = match query.sort {
                    ThreadSort::Created => created,
                    ThreadSort::Activity => state.posts[*index]
                        .latest_reply
                        .map_or(created, |reply| created.max(state.key(reply).0)),
                };
                (timestamp, sequence)
            });
            if query.direction == SortDirection::NewestFirst {
                roots.reverse();
            }
            let page = page(&query.page, roots)?;
            let chars = (query.max_chars_per_post.get() as usize)
                .min(20_000 / (2 * page.results.len().max(1)));
            Ok(Page {
                results: page
                    .results
                    .into_iter()
                    .map(|index| {
                        let root = &state.posts[index];
                        let last = root.latest_reply.map(|reply| &state.posts[reply]);
                        ThreadSummary {
                            thread_id: root.metadata.message_id,
                            root_post: root.preview(chars),
                            reply_count: root.replies.len(),
                            last_activity_at: last.map_or(root.metadata.created_at, |last| {
                                last.metadata.created_at.max(root.metadata.created_at)
                            }),
                            latest_reply: last.map(|post| post.preview(chars)),
                        }
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
    }

    fn search_posts(
        &self,
        caller: ThreadId,
        query: PostQuery,
    ) -> BoxFuture<'_, Result<Page<PostPreview>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let search = query.query.as_deref().map(default_case_fold_str);
            let state = self.state.lock().await;
            let after = query
                .after_message_id
                .map(|id| state.post_index(id).map(|index| state.key(index)))
                .transpose()?;
            let mut posts: Vec<_> = match &query.channel_name {
                Some(channel) => state
                    .channels
                    .get(channel)
                    .map(|channel| channel.posts.clone())
                    .unwrap_or_default(),
                None => (0..state.posts.len()).collect(),
            };
            posts.retain(|index| {
                let post = &state.posts[*index];
                query
                    .author
                    .as_ref()
                    .is_none_or(|author| *author == post.metadata.author)
                    && search
                        .as_ref()
                        .is_none_or(|text| post.search.contains(text))
                    && after.is_none_or(|after| state.key(*index) > after)
            });
            posts.sort_unstable_by_key(|index| Reverse(state.key(*index)));
            let page = page(&query.page, posts)?;
            let chars =
                (query.max_chars_per_post.get() as usize).min(20_000 / page.results.len().max(1));
            Ok(Page {
                results: page
                    .results
                    .into_iter()
                    .map(|index| state.posts[index].preview(chars))
                    .collect(),
                next_cursor: page.next_cursor,
            })
        })
    }

    fn read_thread(
        &self,
        caller: ThreadId,
        request: ReadThreadRequest,
    ) -> BoxFuture<'_, Result<ThreadPage>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let state = self.state.lock().await;
            let root = &state.posts[state.thread_index(request.thread_id)?];
            let mut replies = root.replies.clone();
            replies.sort_unstable_by_key(|index| Reverse(state.key(*index)));
            let page = page(&request.page, replies)?;
            let chars =
                (request.max_chars_per_post.get() as usize).min(20_000 / (page.results.len() + 1));
            Ok(ThreadPage {
                root_post: root.preview(chars),
                replies: Page {
                    results: page
                        .results
                        .into_iter()
                        .map(|index| state.posts[index].preview(chars))
                        .collect(),
                    next_cursor: page.next_cursor,
                },
            })
        })
    }
}

fn page<T>(request: &PageRequest, results: Vec<T>) -> Result<Page<T>> {
    let offset = match &request.cursor {
        Some(cursor) => {
            let mut bytes = [0; 4];
            let len = BASE64_URL_SAFE_NO_PAD
                .decode_slice(cursor, &mut bytes)
                .map_err(|_| invalid("invalid cursor"))?;
            if len != bytes.len() {
                return Err(invalid("invalid cursor"));
            }
            u32::from_be_bytes(bytes)
        }
        None => 0,
    };
    let limit = (request.limit.get() as usize).min(50);
    let mut results: Vec<_> = results
        .into_iter()
        .skip(offset as usize)
        .take(limit + 1)
        .collect();
    let has_more = results.len() > limit;
    results.truncate(limit);
    let next_cursor = if has_more {
        let offset = offset
            .checked_add(results.len() as u32)
            .ok_or_else(|| invalid("cursor offset exceeds the board limit"))?;
        Some(BASE64_URL_SAFE_NO_PAD.encode(offset.to_be_bytes()))
    } else {
        None
    };
    Ok(Page {
        results,
        next_cursor,
    })
}
