use std::time::Duration;
use std::time::Instant;

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::protocol::UserMessageEvent;

use crate::CreateThreadParams;
use crate::GitInfoPatch;
use crate::ResumeThreadParams;
use crate::ThreadMetadataPatch;

const IMAGE_ONLY_USER_MESSAGE_PLACEHOLDER: &str = "[Image]";
#[cfg(not(test))]
const THREAD_UPDATED_AT_TOUCH_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const THREAD_UPDATED_AT_TOUCH_INTERVAL: Duration = Duration::from_millis(50);

/// Live-thread helper that derives metadata updates from canonical rollout items.
///
/// Stores receive raw history plus explicit metadata patches. This helper keeps append-derived
/// metadata observation in the live layer without owning persistence-policy filtering or making
/// `append_items` infer metadata inside a `ThreadStore` implementation.
pub(crate) struct ThreadMetadataSync {
    thread_id: ThreadId,
    cwd_seen: bool,
    preview_seen: bool,
    first_user_message_seen: bool,
    title_seen: bool,
    pending_update: Option<ThreadMetadataPatch>,
    pending_update_generation: u64,
    last_touch_persisted_at: Option<Instant>,
    defer_create_update_until_history_exists: bool,
    defer_resume_update_until_append: bool,
}

pub(crate) struct PendingThreadMetadataPatch {
    pub(crate) patch: ThreadMetadataPatch,
    generation: u64,
}

impl ThreadMetadataSync {
    pub(crate) async fn for_create(params: &CreateThreadParams) -> Self {
        let created_at = Utc::now();
        let cwd = params.metadata.cwd.clone().unwrap_or_default();
        let git_info = if get_git_repo_root(cwd.as_path()).is_some() {
            collect_git_info(cwd.as_path()).await.map(|info| GitInfo {
                commit_hash: info.commit_hash,
                branch: info.branch,
                repository_url: info.repository_url,
            })
        } else {
            None
        };
        let dynamic_tools =
            (!params.dynamic_tools.is_empty()).then(|| params.dynamic_tools.clone());
        let update = ThreadMetadataPatch {
            model_provider: Some(params.metadata.model_provider.clone()),
            created_at: Some(created_at),
            updated_at: Some(created_at),
            source: Some(params.source.clone()),
            thread_source: Some(params.thread_source),
            agent_nickname: Some(params.source.get_nickname()),
            agent_role: Some(params.source.get_agent_role()),
            agent_path: Some(params.source.get_agent_path().map(Into::into)),
            cwd: Some(cwd.clone()),
            cli_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            git_info: git_info.map(git_info_patch_from_observation),
            memory_mode: Some(params.metadata.memory_mode),
            dynamic_tools,
            ..Default::default()
        };
        Self {
            thread_id: params.thread_id,
            cwd_seen: !cwd.as_os_str().is_empty(),
            preview_seen: false,
            first_user_message_seen: false,
            title_seen: false,
            pending_update: Some(update),
            pending_update_generation: 1,
            last_touch_persisted_at: None,
            defer_create_update_until_history_exists: true,
            defer_resume_update_until_append: false,
        }
    }

    pub(crate) fn for_resume(params: &ResumeThreadParams) -> Self {
        let mut sync = Self {
            thread_id: params.thread_id,
            cwd_seen: params
                .metadata
                .cwd
                .as_ref()
                .is_some_and(|cwd| !cwd.as_os_str().is_empty()),
            preview_seen: false,
            first_user_message_seen: false,
            title_seen: false,
            pending_update: None,
            pending_update_generation: 0,
            last_touch_persisted_at: None,
            defer_create_update_until_history_exists: false,
            defer_resume_update_until_append: false,
        };
        if let Some(history) = params.history.as_deref() {
            let update = sync.observe_resume_history(history);
            sync.merge_pending_update(update);
            sync.defer_resume_update_until_append = sync.pending_update.is_some();
        }
        sync
    }

    pub(crate) fn take_pending_update(&self) -> Option<PendingThreadMetadataPatch> {
        self.pending_update
            .clone()
            .map(|patch| PendingThreadMetadataPatch {
                patch,
                generation: self.pending_update_generation,
            })
    }

    pub(crate) fn take_pending_update_for_existing_history(
        &self,
    ) -> Option<PendingThreadMetadataPatch> {
        if self.defer_create_update_until_history_exists {
            return None;
        }
        if self.defer_resume_update_until_append {
            return None;
        }
        self.take_pending_update()
    }

    pub(crate) fn mark_pending_update_applied(&mut self, update: &PendingThreadMetadataPatch) {
        if self.pending_update_generation == update.generation {
            self.pending_update = None;
        }
        if update.patch.updated_at.is_some() {
            self.last_touch_persisted_at = Some(Instant::now());
        }
    }

    pub(crate) fn observe_appended_items(
        &mut self,
        items: &[RolloutItem],
    ) -> Option<PendingThreadMetadataPatch> {
        self.defer_create_update_until_history_exists = false;
        self.defer_resume_update_until_append = false;
        let affects_metadata = items
            .iter()
            .any(codex_state::rollout_item_affects_thread_metadata);
        let update = if affects_metadata {
            self.observe_items(items)?
        } else {
            thread_updated_at_touch()
        };
        self.merge_pending_update(Some(update));
        if !affects_metadata
            && !self
                .pending_update
                .as_ref()
                .is_some_and(update_has_metadata_facts)
            && self.last_touch_persisted_at.is_some_and(|last_touch| {
                Instant::now().duration_since(last_touch) < THREAD_UPDATED_AT_TOUCH_INTERVAL
            })
        {
            return None;
        }
        self.take_pending_update()
    }

    fn observe_items(&mut self, items: &[RolloutItem]) -> Option<ThreadMetadataPatch> {
        self.observe_items_with_update(
            items,
            ThreadMetadataPatch {
                updated_at: Some(Utc::now()),
                ..Default::default()
            },
        )
    }

    fn observe_resume_history(&mut self, items: &[RolloutItem]) -> Option<ThreadMetadataPatch> {
        self.observe_items_with_update(items, ThreadMetadataPatch::default())
    }

    fn observe_items_with_update(
        &mut self,
        items: &[RolloutItem],
        mut update: ThreadMetadataPatch,
    ) -> Option<ThreadMetadataPatch> {
        if items.is_empty() {
            return None;
        }
        for item in items {
            match item {
                RolloutItem::SessionMeta(meta_line) if meta_line.meta.id == self.thread_id => {
                    update.created_at = parse_session_timestamp(meta_line.meta.timestamp.as_str());
                    update.source = Some(meta_line.meta.source.clone());
                    update.thread_source = Some(meta_line.meta.thread_source);
                    update.agent_nickname = Some(meta_line.meta.agent_nickname.clone());
                    update.agent_role = Some(meta_line.meta.agent_role.clone());
                    update.agent_path = Some(meta_line.meta.agent_path.clone());
                    if let Some(model_provider) = meta_line.meta.model_provider.clone()
                        && !model_provider.is_empty()
                    {
                        update.model_provider = Some(model_provider);
                    }
                    if !meta_line.meta.cli_version.is_empty() {
                        update.cli_version = Some(meta_line.meta.cli_version.clone());
                    }
                    if !meta_line.meta.cwd.as_os_str().is_empty() {
                        self.cwd_seen = true;
                        update.cwd = Some(meta_line.meta.cwd.clone());
                    }
                    if let Some(git_info) = meta_line.git.clone() {
                        update.git_info = Some(git_info_patch_from_observation(git_info));
                    }
                    if let Some(memory_mode) = meta_line.meta.memory_mode.as_deref()
                        && let Some(memory_mode) = parse_memory_mode(memory_mode)
                    {
                        update.memory_mode = Some(memory_mode);
                    }
                    if let Some(dynamic_tools) = meta_line.meta.dynamic_tools.clone() {
                        update.dynamic_tools = Some(dynamic_tools);
                    }
                }
                RolloutItem::TurnContext(turn_ctx) => {
                    if !self.cwd_seen && !turn_ctx.cwd.as_os_str().is_empty() {
                        self.cwd_seen = true;
                        update.cwd = Some(turn_ctx.cwd.clone());
                    }
                    update.model = Some(turn_ctx.model.clone());
                    update.reasoning_effort = turn_ctx.effort;
                    update.approval_mode = Some(turn_ctx.approval_policy);
                    update.sandbox_policy = Some(turn_ctx.sandbox_policy.clone());
                }
                RolloutItem::EventMsg(EventMsg::UserMessage(user)) => {
                    if let Some(preview) = user_message_preview(user) {
                        if !self.first_user_message_seen {
                            self.first_user_message_seen = true;
                            update.first_user_message = Some(preview.clone());
                        }
                        if !self.preview_seen {
                            self.preview_seen = true;
                            update.preview = Some(preview);
                        }
                    }
                    if !self.title_seen {
                        let title = strip_user_message_prefix(user.message.as_str());
                        if !title.is_empty() {
                            self.title_seen = true;
                            update.title = Some(title.to_string());
                        }
                    }
                }
                RolloutItem::EventMsg(EventMsg::TokenCount(token_count)) => {
                    if let Some(info) = token_count.info.as_ref() {
                        update.token_usage = Some(info.total_token_usage.clone());
                    }
                }
                RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(event)) => {
                    if !self.preview_seen {
                        let objective = event.goal.objective.trim();
                        if !objective.is_empty() {
                            self.preview_seen = true;
                            update.preview = Some(objective.to_string());
                        }
                    }
                }
                RolloutItem::SessionMeta(_)
                | RolloutItem::EventMsg(_)
                | RolloutItem::ResponseItem(_)
                | RolloutItem::Compacted(_) => {}
            }
        }
        Some(update)
    }

    fn merge_pending_update(&mut self, update: Option<ThreadMetadataPatch>) {
        let Some(update) = update else {
            return;
        };
        match self.pending_update.as_mut() {
            Some(pending_update) => pending_update.merge(update),
            None => self.pending_update = Some(update),
        }
        self.pending_update_generation = self.pending_update_generation.wrapping_add(1);
    }
}

fn parse_memory_mode(value: &str) -> Option<ThreadMemoryMode> {
    match value {
        "enabled" => Some(ThreadMemoryMode::Enabled),
        "disabled" => Some(ThreadMemoryMode::Disabled),
        _ => None,
    }
}

fn parse_session_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H-%M-%S")
                .map(|timestamp| DateTime::from_naive_utc_and_offset(timestamp, Utc))
        })
        .ok()
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

fn user_message_preview(user: &UserMessageEvent) -> Option<String> {
    let message = strip_user_message_prefix(user.message.as_str());
    if !message.is_empty() {
        return Some(message.to_string());
    }
    if user
        .images
        .as_ref()
        .is_some_and(|images| !images.is_empty())
        || !user.local_images.is_empty()
    {
        return Some(IMAGE_ONLY_USER_MESSAGE_PLACEHOLDER.to_string());
    }
    None
}

fn thread_updated_at_touch() -> ThreadMetadataPatch {
    ThreadMetadataPatch {
        updated_at: Some(Utc::now()),
        ..Default::default()
    }
}

fn update_has_metadata_facts(update: &ThreadMetadataPatch) -> bool {
    update.rollout_path.is_some()
        || update.preview.is_some()
        || update.title.is_some()
        || update.model_provider.is_some()
        || update.model.is_some()
        || update.reasoning_effort.is_some()
        || update.created_at.is_some()
        || update.source.is_some()
        || update.thread_source.is_some()
        || update.agent_nickname.is_some()
        || update.agent_role.is_some()
        || update.agent_path.is_some()
        || update.cwd.is_some()
        || update.cli_version.is_some()
        || update.approval_mode.is_some()
        || update.sandbox_policy.is_some()
        || update.token_usage.is_some()
        || update.first_user_message.is_some()
        || update.git_info.is_some()
        || update.memory_mode.is_some()
        || update.dynamic_tools.is_some()
}

fn git_info_patch_from_observation(git_info: GitInfo) -> GitInfoPatch {
    GitInfoPatch {
        sha: git_info.commit_hash.map(|sha| Some(sha.0)),
        branch: git_info.branch.map(Some),
        origin_url: git_info.repository_url.map(Some),
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;
    use codex_protocol::protocol::ThreadGoalUpdatedEvent;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadPersistenceMetadata;

    #[test]
    fn resume_history_keeps_derived_metadata_pending_until_applied() {
        let thread_id = ThreadId::new();
        let mut sync = ThreadMetadataSync::for_resume(&resume_params(
            thread_id,
            vec![
                RolloutItem::SessionMeta(session_meta(thread_id)),
                RolloutItem::EventMsg(EventMsg::UserMessage(user_message("hello metadata"))),
            ],
        ));

        let update = sync.take_pending_update().expect("pending metadata update");
        assert_eq!(
            update
                .patch
                .created_at
                .expect("created_at should come from session metadata")
                .to_rfc3339(),
            "2025-01-03T12:00:00+00:00"
        );
        assert_eq!(update.patch.preview.as_deref(), Some("hello metadata"));
        assert_eq!(update.patch.title.as_deref(), Some("hello metadata"));
        assert_eq!(
            update.patch.first_user_message.as_deref(),
            Some("hello metadata")
        );
        assert_eq!(update.patch.updated_at, None);
        assert!(
            sync.take_pending_update().is_some(),
            "taking the pending update should not drop retry state"
        );

        sync.mark_pending_update_applied(&update);
        assert!(sync.take_pending_update().is_none());
    }

    #[test]
    fn goal_update_sets_preview_without_overriding_existing_preview() {
        let thread_id = ThreadId::new();
        let sync = ThreadMetadataSync::for_resume(&resume_params(
            thread_id,
            vec![
                RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(goal_update(
                    thread_id,
                    "ship the refactor",
                ))),
                RolloutItem::EventMsg(EventMsg::UserMessage(user_message("first user text"))),
            ],
        ));

        let update = sync.take_pending_update().expect("pending metadata update");
        assert_eq!(update.patch.preview.as_deref(), Some("ship the refactor"));
        assert_eq!(
            update.patch.first_user_message.as_deref(),
            Some("first user text")
        );
        assert_eq!(update.patch.title.as_deref(), Some("first user text"));
    }

    #[test]
    fn later_user_messages_do_not_emit_existing_preview_fields() {
        let thread_id = ThreadId::new();
        let mut sync = ThreadMetadataSync::for_resume(&resume_params(
            thread_id,
            vec![RolloutItem::EventMsg(EventMsg::UserMessage(user_message(
                "first user text",
            )))],
        ));
        let pending = sync.take_pending_update().expect("pending resume metadata");
        sync.mark_pending_update_applied(&pending);

        let update = sync
            .observe_appended_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(user_message(
                "later user text",
            )))])
            .expect("updated_at touch");

        assert_eq!(update.patch.preview, None);
        assert_eq!(update.patch.title, None);
        assert_eq!(update.patch.first_user_message, None);
        assert!(update.patch.updated_at.is_some());
    }

    #[test]
    fn metadata_irrelevant_items_coalesce_updated_at_touches() {
        let thread_id = ThreadId::new();
        let mut sync = ThreadMetadataSync::for_resume(&resume_params(thread_id, Vec::new()));
        let item = RolloutItem::Compacted(CompactedItem {
            message: "compacted".to_string(),
            replacement_history: None,
        });

        let first = sync
            .observe_appended_items(std::slice::from_ref(&item))
            .expect("first touch should apply immediately");
        assert!(first.patch.updated_at.is_some());
        sync.mark_pending_update_applied(&first);

        assert!(
            sync.observe_appended_items(std::slice::from_ref(&item))
                .is_none(),
            "second touch inside the coalescing window should wait for a barrier"
        );
        assert!(
            sync.take_pending_update().is_some(),
            "coalesced touches still flush at the next barrier"
        );
    }

    #[test]
    fn resume_history_waits_for_append_before_flushing_metadata() {
        let thread_id = ThreadId::new();
        let mut sync = ThreadMetadataSync::for_resume(&resume_params(
            thread_id,
            vec![
                RolloutItem::SessionMeta(session_meta(thread_id)),
                RolloutItem::EventMsg(EventMsg::UserMessage(user_message("hello metadata"))),
            ],
        ));

        assert!(
            sync.take_pending_update_for_existing_history().is_none(),
            "resume-only metadata should not flush without a new append"
        );
        assert!(
            sync.observe_appended_items(&[RolloutItem::EventMsg(EventMsg::UserMessage(
                user_message("new append"),
            ))])
            .is_some(),
            "the first append should flush resume metadata together with append metadata"
        );
    }

    fn resume_params(thread_id: ThreadId, history: Vec<RolloutItem>) -> ResumeThreadParams {
        ResumeThreadParams {
            thread_id,
            rollout_path: None,
            history: Some(history),
            include_archived: false,
            metadata: ThreadPersistenceMetadata {
                cwd: None,
                model_provider: "test-provider".to_string(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
            event_persistence_mode: ThreadEventPersistenceMode::Limited,
        }
    }

    fn user_message(message: &str) -> UserMessageEvent {
        UserMessageEvent {
            message: message.to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
            ..Default::default()
        }
    }

    fn session_meta(thread_id: ThreadId) -> SessionMetaLine {
        SessionMetaLine {
            meta: SessionMeta {
                id: thread_id,
                timestamp: "2025-01-03T12:00:00Z".to_string(),
                source: SessionSource::Exec,
                ..Default::default()
            },
            git: None,
        }
    }

    fn goal_update(thread_id: ThreadId, objective: &str) -> ThreadGoalUpdatedEvent {
        ThreadGoalUpdatedEvent {
            thread_id,
            turn_id: None,
            goal: ThreadGoal {
                thread_id,
                objective: objective.to_string(),
                status: ThreadGoalStatus::Active,
                token_budget: None,
                tokens_used: 0,
                time_used_seconds: 0,
                created_at: 0,
                updated_at: 0,
            },
        }
    }
}
