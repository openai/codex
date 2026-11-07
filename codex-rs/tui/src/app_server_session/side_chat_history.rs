//! Discovers local side chats from their own rollouts and reads their visible labels.

use super::*;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_context_fragments::ContextualUserFragment;
use codex_context_fragments::SIDE_BOUNDARY_TITLE;
use codex_context_fragments::SideConversationPolicy;
use codex_protocol::models::ContentItem;
use codex_rollout::ResponseItemEnvelope;
use codex_rollout::RolloutItem;
use codex_rollout::read_session_meta_line;
use std::io;
use std::path::Path;

pub(super) async fn side_context_checkpoint(path: Option<&Path>) -> io::Result<Option<String>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let mut reader = codex_rollout::open_rollout_line_reader(path).await?;
    // The two bootstrap items are written before the first child turn. Inspect
    // only the child's own records, even when its parent has side context.
    let mut has_policy = false;
    while let Some(line) = reader.next_line().await? {
        let record = codex_rollout::parse_rollout_line(&line)?;
        match record.item {
            RolloutItem::SessionMeta(meta) if !meta.meta.attached_thread => return Ok(None),
            RolloutItem::ResponseItem(ResponseItemEnvelope {
                item: ResponseItem::Message { role, content, .. },
                ..
            }) => {
                let [ContentItem::InputText { text }] = content.as_slice() else {
                    continue;
                };
                match role.as_str() {
                    "developer" => has_policy |= SideConversationPolicy::matches_text(text),
                    "user" if has_policy && text.lines().next() == Some(SIDE_BOUNDARY_TITLE) => {
                        return Ok(Some(record.timestamp));
                    }
                    _ => {}
                }
            }
            RolloutItem::EventMsg(codex_protocol::protocol::EventMsg::TurnStarted(_)) => break,
            _ => {}
        }
    }
    Ok(None)
}

#[derive(Debug)]
pub(crate) struct SideChat {
    pub(crate) id: ThreadId,
    pub(crate) name: Option<String>,
    pub(crate) preview: String,
}

impl From<ThreadId> for SideChat {
    fn from(id: ThreadId) -> Self {
        Self {
            id,
            name: None,
            preview: String::new(),
        }
    }
}

pub(crate) fn side_chat_prompt_preview(content: &[UserInput]) -> Option<&str> {
    if crate::chatwidget::realtime_delegation_input(content).is_some() {
        return None;
    }
    content
        .iter()
        .find_map(|input| match input {
            UserInput::Text { text, .. } => {
                codex_protocol::protocol::strip_user_message_prefix(text)
                    .lines()
                    .next()
                    .map(str::trim)
            }
            _ => None,
        })
        .or_else(|| {
            content.iter().find_map(|input| match input {
                UserInput::Image { .. } | UserInput::LocalImage { .. } => Some("[Image]"),
                UserInput::Audio { .. } | UserInput::LocalAudio { .. } => Some("[Audio]"),
                _ => None,
            })
        })
}

pub(super) fn is_attached_thread_store_unsupported(err: &color_eyre::Report) -> bool {
    matches!(
        err.downcast_ref::<TypedRequestError>(),
        Some(TypedRequestError::Server { source, .. })
            if source.code == JSONRPC_METHOD_NOT_FOUND
                && source.message == "attached threads is not supported yet"
    )
}

impl AppServerSession {
    pub(crate) fn can_list_saved_side_chats(&self, parent: ThreadId) -> bool {
        !self.uses_remote_workspace()
            && self.history_support == ThreadHistorySupport::Paginated
            && self
                .history_pagination
                .get(&parent)
                .is_none_or(|state| state.history_mode == ThreadHistoryMode::Paginated)
    }

    pub(super) async fn attached_side_chat_query(
        &mut self,
        parent: ThreadId,
    ) -> Result<Option<ThreadListParams>> {
        if self.uses_remote_workspace() {
            return Ok(None);
        }
        let query = ThreadListParams {
            attached_to_thread_id: Some(parent.to_string()),
            current_account_only: true,
            archived: Some(false),
            model_providers: Some(Vec::new()),
            sort_direction: Some(SortDirection::Asc),
            use_state_db_only: true,
            cursor: None,
            limit: None,
            sort_key: None,
            source_kinds: None,
            section_id: None,
            project_id: None,
            cwd: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
            search_term: None,
            originators: None,
            excluded_thread_ids: None,
        };
        if matches!(self.request_handle, AppServerRequestHandle::InProcess(_)) {
            return Ok(Some(query));
        }
        // Older local daemons can silently ignore unknown fields. Require the
        // validation error for these incompatible filters before using them.
        let probe = ThreadListParams {
            parent_thread_id: query.attached_to_thread_id.clone(),
            limit: Some(1),
            ..query.clone()
        };
        let Err(err) = self.thread_list(probe).await else {
            return Ok(None);
        };
        let Some(TypedRequestError::Server { source, .. }) = err.downcast_ref() else {
            return Err(err);
        };
        Ok((source.code == JSONRPC_INVALID_REQUEST
            && source.message
                == "attachedToThreadId cannot be combined with parentThreadId or ancestorThreadId")
            .then_some(query))
    }

    pub(crate) async fn list_saved_side_chats(
        &mut self,
        parent: ThreadId,
        codex_home: &Path,
    ) -> Result<Vec<SideChat>> {
        let Some(query) = self.attached_side_chat_query(parent).await? else {
            return Ok(Vec::new());
        };
        let mut chats = Vec::new();
        let threads = self.list_attached_threads(query).await?;
        let named_ids = threads
            .iter()
            .filter(|thread| thread.name.is_some())
            .filter_map(|thread| ThreadId::from_string(&thread.id).ok())
            .collect();
        let indexed_names = codex_rollout::find_thread_name_entries_by_ids(codex_home, &named_ids)
            .await
            .unwrap_or_else(|err| {
                tracing::debug!(%err, "could not read side-chat name timestamps");
                Default::default()
            });
        for thread in threads {
            let Ok(id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
            let checkpoint = match side_context_checkpoint(thread.path.as_deref()).await {
                Ok(Some(timestamp)) => timestamp,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%id, %error, "failed to read side chat context");
                    continue;
                }
            };
            // Older servers write the inherited name during fork, before the TUI installs the
            // side-chat boundary. A later name write is an explicit rename, even to the same text.
            let name = thread.name.filter(|name| {
                if let Some(entry) = indexed_names.get(&id)
                    && entry.thread_name == *name
                    && let Ok(written) = chrono::DateTime::parse_from_rfc3339(&entry.updated_at)
                    && let Ok(opened) = chrono::DateTime::parse_from_rfc3339(&checkpoint)
                {
                    return written > opened;
                }
                true
            });
            let preview = if name.is_none() {
                self.first_side_chat_prompt(id)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::debug!(%id, %error, "failed to load side-chat prompt");
                        String::new()
                    })
            } else {
                String::new()
            };
            chats.push(SideChat { id, name, preview });
        }
        Ok(chats)
    }

    /// Delete the parent's TUI side chats before deleting the parent itself.
    pub(crate) async fn delete_thread_with_saved_side_chats(
        &mut self,
        parent: ThreadId,
    ) -> Result<()> {
        if self.can_list_saved_side_chats(parent)
            && let Some(mut query) = self.attached_side_chat_query(parent).await?
        {
            // Resolve the complete list before deleting anything. Archived and partially
            // initialized side chats also block the app-server's parent delete.
            let mut attached = self.list_attached_threads(query.clone()).await?;
            query.archived = Some(true);
            attached.extend(self.list_attached_threads(query).await?);
            let mut children = HashSet::new();
            for thread in attached {
                let child = ThreadId::from_string(&thread.id)?;
                let path = thread.path.as_deref().ok_or_else(|| {
                    color_eyre::eyre::eyre!("No rollout found for attached thread {child}")
                })?;
                // The thread list omits fork origins. Verify the child's own metadata
                // before passing its ID to the delete API.
                let meta = read_session_meta_line(path)
                    .await
                    .wrap_err_with(|| format!("failed to verify attached thread {child}"))?
                    .meta;
                color_eyre::eyre::ensure!(
                    meta.id == child
                        && meta.attached_thread
                        && meta.forked_from_id == Some(parent)
                        && matches!(
                            meta.thread_source.as_ref(),
                            Some(codex_protocol::protocol::ThreadSource::Feature(source))
                                if source == ATTACHED_SIDE_CHAT_SOURCE
                        ),
                    "The conversation has an attachment outside the TUI sidebar. Remove it before deleting the conversation."
                );
                children.insert(child);
            }
            for child in children {
                self.thread_delete(child)
                    .await
                    .wrap_err_with(|| format!("failed to delete saved side chat {child}"))?;
            }
        }
        self.thread_delete(parent).await
    }

    async fn list_attached_threads(&mut self, mut query: ThreadListParams) -> Result<Vec<Thread>> {
        let mut threads = Vec::new();
        let mut seen = HashSet::new();
        let mut cursors = HashSet::new();
        loop {
            let page = match self.thread_list(query.clone()).await {
                Err(err) if is_attached_thread_store_unsupported(&err) => return Ok(Vec::new()),
                result => result?,
            };
            threads.extend(
                page.data
                    .into_iter()
                    .filter(|thread| seen.insert(thread.id.clone())),
            );
            let Some(cursor) = page.next_cursor else {
                return Ok(threads);
            };
            color_eyre::eyre::ensure!(
                cursors.insert(cursor.clone()),
                "Repeated side chat list cursor"
            );
            query.cursor = Some(cursor);
        }
    }

    async fn first_side_chat_prompt(&mut self, id: ThreadId) -> Result<String> {
        // The summary preview can come from inherited model context. Item pages only contain
        // the child's visible history, including for chats saved by older clients.
        let mut cursor = None;
        let mut cursors = HashSet::new();
        loop {
            let mut params =
                thread_items_page_params(id, /*turn_id*/ None, cursor, /*limit*/ 16);
            params.sort_direction = Some(SortDirection::Asc);
            let request_id = self.next_request_id();
            let page: ThreadItemsListResponse = self
                .request_handle
                .request_typed(ClientRequest::ThreadItemsList { request_id, params })
                .await?;
            if let Some(preview) = page
                .data
                .iter()
                .find_map(|entry| match &entry.item.payload {
                    ThreadItemPayload::UserMessage { content, .. } => {
                        side_chat_prompt_preview(content)
                    }
                    _ => None,
                })
            {
                return Ok(preview.to_owned());
            }
            let Some(next) = page.next_cursor else {
                return Ok(String::new());
            };
            color_eyre::eyre::ensure!(
                cursors.insert(next.clone()),
                "Repeated side chat item cursor"
            );
            cursor = Some(next);
        }
    }
}
