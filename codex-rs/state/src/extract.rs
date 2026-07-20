use crate::model::ThreadMetadata;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::strip_user_message_prefix;
use codex_protocol::protocol::user_message_preview;
use serde::Serialize;
use serde_json::Value;

/// Apply a rollout item to the metadata structure.
pub fn apply_rollout_item(
    metadata: &mut ThreadMetadata,
    item: &RolloutItem,
    default_provider: &str,
) {
    match item {
        RolloutItem::SessionMeta(meta_line) => apply_session_meta_from_item(metadata, meta_line),
        RolloutItem::TurnContext(turn_ctx) => apply_turn_context(metadata, turn_ctx),
        RolloutItem::EventMsg(event) => apply_event_msg(metadata, event),
        RolloutItem::ResponseItem(item) => apply_response_item(metadata, item),
        RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. } => {}
        RolloutItem::Compacted(_) => {}
        RolloutItem::WorldState(_) => {}
    }
    if metadata.model_provider.is_empty() {
        metadata.model_provider = default_provider.to_string();
    }
}

/// Return whether this rollout item can mutate thread metadata stored in SQLite.
pub fn rollout_item_affects_thread_metadata(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::SessionMeta(_) | RolloutItem::TurnContext(_) => true,
        RolloutItem::EventMsg(
            EventMsg::TokenCount(_)
            | EventMsg::UserMessage(_)
            | EventMsg::ThreadGoalUpdated(_)
            | EventMsg::ThreadSettingsApplied(_),
        ) => true,
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event))
            if matches!(event.item, TurnItem::UserMessage(_)) =>
        {
            true
        }
        RolloutItem::EventMsg(_)
        | RolloutItem::ResponseItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. }
        | RolloutItem::Compacted(_)
        | RolloutItem::WorldState(_) => false,
    }
}

fn apply_session_meta_from_item(metadata: &mut ThreadMetadata, meta_line: &SessionMetaLine) {
    if metadata.id != meta_line.meta.id {
        // Ignore session_meta lines that don't match the canonical thread ID,
        // e.g., forked rollouts that embed the source session metadata.
        return;
    }
    metadata.id = meta_line.meta.id;
    metadata.source = enum_to_string(&meta_line.meta.source);
    // Later SessionMeta lines do not redefine the canonical history_mode.
    metadata.thread_source = meta_line.meta.thread_source.clone();
    metadata.agent_nickname = meta_line.meta.agent_nickname.clone();
    metadata.agent_role = meta_line.meta.agent_role.clone();
    metadata.agent_path = meta_line.meta.agent_path.clone();
    if let Some(provider) = meta_line.meta.model_provider.as_deref() {
        metadata.model_provider = provider.to_string();
    }
    if !meta_line.meta.cli_version.is_empty() {
        metadata.cli_version = meta_line.meta.cli_version.clone();
    }
    if !meta_line.meta.cwd.as_os_str().is_empty() {
        metadata.cwd = meta_line.meta.cwd.clone();
    }
    if let Some(git) = meta_line.git.as_ref() {
        metadata.git_sha = git.commit_hash.as_ref().map(|sha| sha.0.clone());
        metadata.git_branch = git.branch.clone();
        metadata.git_origin_url = git.repository_url.clone();
    }
}

fn apply_turn_context(metadata: &mut ThreadMetadata, turn_ctx: &TurnContextItem) {
    if metadata.cwd.as_os_str().is_empty() {
        metadata.cwd = turn_ctx.cwd.clone().into_path_buf();
    }
    metadata.model = Some(turn_ctx.model.clone());
    metadata.reasoning_effort = turn_ctx.effort.clone();
    metadata.sandbox_policy =
        serde_json::to_string(&turn_ctx.permission_profile()).unwrap_or_default();
    metadata.approval_mode = enum_to_string(&turn_ctx.approval_policy);
}

fn apply_event_msg(metadata: &mut ThreadMetadata, event: &EventMsg) {
    match event {
        EventMsg::TokenCount(token_count) => {
            if let Some(info) = token_count.info.as_ref() {
                metadata.tokens_used = info.total_token_usage.total_tokens.max(0);
            }
        }
        EventMsg::UserMessage(user) => {
            apply_user_message(metadata, user);
        }
        EventMsg::ItemCompleted(event) => {
            if let TurnItem::UserMessage(user) = &event.item {
                apply_user_message(metadata, &user.as_legacy_user_message_event());
            }
        }
        EventMsg::ThreadGoalUpdated(event) => {
            let objective = event.goal.objective.trim();
            if !objective.is_empty() {
                set_preview_if_empty(metadata, Some(objective.to_string()));
            }
        }
        EventMsg::ThreadSettingsApplied(event) => {
            let settings = &event.thread_settings;
            metadata.model = Some(settings.model.clone());
            metadata.model_provider = settings.model_provider_id.clone();
            metadata.reasoning_effort = settings.reasoning_effort.clone();
            metadata.cwd = settings.cwd.clone().into_path_buf();
            metadata.sandbox_policy =
                serde_json::to_string(&settings.permission_profile).unwrap_or_default();
            metadata.approval_mode = enum_to_string(&settings.approval_policy);
        }
        _ => {}
    }
}

fn apply_response_item(_metadata: &mut ThreadMetadata, _item: &ResponseItem) {}

fn apply_user_message(metadata: &mut ThreadMetadata, user: &UserMessageEvent) {
    let preview = user_message_preview(user);
    if metadata.first_user_message.is_none() {
        metadata.first_user_message = preview.clone();
    }
    set_preview_if_empty(metadata, preview);
    if metadata.title.is_empty() {
        let title = strip_user_message_prefix(user.message.as_str());
        if !title.is_empty() {
            metadata.title = title.to_string();
        }
    }
}

fn set_preview_if_empty(metadata: &mut ThreadMetadata, preview: Option<String>) {
    if metadata.preview.is_none() {
        metadata.preview = preview;
    }
}

pub(crate) fn enum_to_string<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::apply_rollout_item;
    use super::rollout_item_affects_thread_metadata;
    use crate::model::ThreadMetadata;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::ApprovalsReviewer;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::ReasoningSummary;
    use codex_protocol::config_types::Settings;
    use codex_protocol::items::TurnItem;
    use codex_protocol::items::UserMessageItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::ItemCompletedEvent;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadGoal;
    use codex_protocol::protocol::ThreadGoalStatus;
    use codex_protocol::protocol::ThreadGoalUpdatedEvent;
    use codex_protocol::protocol::ThreadHistoryMode;
    use codex_protocol::protocol::ThreadSettingsAppliedEvent;
    use codex_protocol::protocol::ThreadSettingsSnapshot;
    use codex_protocol::protocol::TurnContextItem;
    use codex_protocol::protocol::USER_MESSAGE_BEGIN;
    use codex_protocol::protocol::UserMessageEvent;
    use codex_protocol::user_input::UserInput;

    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn response_item_user_messages_do_not_set_title_or_first_user_message() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello from response item".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        });

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message, None);
        assert_eq!(metadata.preview, None);
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_user_messages_set_title_and_first_user_message() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: format!("{USER_MESSAGE_BEGIN} actual user request"),
            images: Some(vec![]),
            local_images: vec![],
            text_elements: vec![],
            ..Default::default()
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("actual user request")
        );
        assert_eq!(metadata.preview.as_deref(), Some("actual user request"));
        assert_eq!(metadata.title, "actual user request");
    }

    #[test]
    fn completed_user_message_items_set_title_and_first_user_message() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::default(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::UserMessage(UserMessageItem::new(&[UserInput::Text {
                text: format!("{USER_MESSAGE_BEGIN} actual user request"),
                text_elements: Vec::new(),
            }])),
            completed_at_ms: 0,
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("actual user request")
        );
        assert_eq!(metadata.preview.as_deref(), Some("actual user request"));
        assert_eq!(metadata.title, "actual user request");
    }

    #[test]
    fn event_msg_image_only_user_message_sets_image_placeholder_preview() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: String::new(),
            images: Some(vec!["https://example.com/image.png".to_string()]),
            local_images: vec![],
            text_elements: vec![],
            ..Default::default()
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message.as_deref(), Some("[Image]"));
        assert_eq!(metadata.preview.as_deref(), Some("[Image]"));
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_audio_only_user_message_sets_audio_placeholder_preview() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: String::new(),
            images: None,
            local_images: vec![],
            audio: Some(vec!["https://example.com/audio.mp3".to_string()]),
            local_audio: vec![],
            text_elements: vec![],
            ..Default::default()
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message.as_deref(), Some("[Audio]"));
        assert_eq!(metadata.preview.as_deref(), Some("[Audio]"));
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_blank_user_message_without_images_keeps_first_user_message_empty() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: "   ".to_string(),
            images: Some(vec![]),
            local_images: vec![],
            text_elements: vec![],
            ..Default::default()
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message, None);
        assert_eq!(metadata.preview, None);
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_thread_goal_sets_preview_only_and_later_user_sets_message_title() {
        let mut metadata = metadata_for_test();
        let goal_item =
            RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(ThreadGoalUpdatedEvent {
                thread_id: metadata.id,
                turn_id: None,
                goal: ThreadGoal {
                    thread_id: metadata.id,
                    objective: "optimize the benchmark".to_string(),
                    status: ThreadGoalStatus::Active,
                    token_budget: None,
                    tokens_used: 0,
                    time_used_seconds: 0,
                    created_at: 1,
                    updated_at: 1,
                },
            }));

        apply_rollout_item(&mut metadata, &goal_item, "test-provider");

        assert_eq!(metadata.preview.as_deref(), Some("optimize the benchmark"));
        assert_eq!(metadata.first_user_message, None);
        assert_eq!(metadata.title, "");

        let user_item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: format!("{USER_MESSAGE_BEGIN} next normal prompt"),
            images: Some(vec![]),
            local_images: vec![],
            text_elements: vec![],
            ..Default::default()
        }));

        apply_rollout_item(&mut metadata, &user_item, "test-provider");

        assert_eq!(metadata.preview.as_deref(), Some("optimize the benchmark"));
        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("next normal prompt")
        );
        assert_eq!(metadata.title, "next normal prompt");
    }

    #[test]
    fn turn_context_does_not_override_session_cwd() {
        let mut metadata = metadata_for_test();
        metadata.cwd = PathBuf::new();
        let thread_id = metadata.id;

        apply_rollout_item(
            &mut metadata,
            &RolloutItem::SessionMeta(SessionMetaLine {
                meta: SessionMeta {
                    session_id: thread_id.into(),
                    id: thread_id,
                    forked_from_id: Some(
                        ThreadId::from_string(&Uuid::now_v7().to_string()).expect("thread id"),
                    ),
                    parent_thread_id: None,
                    timestamp: "2026-02-26T00:00:00.000Z".to_string(),
                    cwd: PathBuf::from("/child/worktree"),
                    originator: "codex_cli_rs".to_string(),
                    cli_version: "0.0.0".to_string(),
                    source: SessionSource::Cli,
                    thread_source: None,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                    model_provider: Some("openai".to_string()),
                    base_instructions: None,
                    dynamic_tools: None,
                    selected_capability_roots: Vec::new(),
                    memory_mode: None,
                    history_mode: Default::default(),
                    history_base: None,
                    subagent_history_start_ordinal: None,
                    multi_agent_version: None,
                    context_window: None,
                },
                git: None,
            }),
            "test-provider",
        );
        apply_rollout_item(
            &mut metadata,
            &RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("turn-1".to_string()),
                cwd: serde_json::from_value(serde_json::json!(
                    std::env::current_dir()
                        .expect("current directory")
                        .join("parent/workspace")
                ))
                .expect("absolute parent cwd"),
                workspace_roots: None,
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: None,
                sandbox_policy: SandboxPolicy::DangerFullAccess,
                permission_profile: None,
                network: None,
                file_system_sandbox_policy: None,
                model: "gpt-5".to_string(),
                comp_hash: None,
                personality: None,
                collaboration_mode: None,
                multi_agent_version: None,
                multi_agent_mode: None,
                realtime_active: None,
                effort: None,
                summary: codex_protocol::config_types::ReasoningSummary::Auto,
            }),
            "test-provider",
        );

        assert_eq!(metadata.cwd, PathBuf::from("/child/worktree"));
        let permission_profile: PermissionProfile = PermissionProfile::Disabled;
        assert_eq!(
            metadata.sandbox_policy,
            serde_json::to_string(&permission_profile).expect("serialize permission profile")
        );
        assert_eq!(metadata.approval_mode, "never");
    }

    #[test]
    fn turn_context_sets_permission_profile_metadata() {
        let mut metadata = metadata_for_test();
        let permission_profile = PermissionProfile::workspace_write();

        apply_rollout_item(
            &mut metadata,
            &RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("turn-1".to_string()),
                cwd: serde_json::from_value(serde_json::json!(
                    std::env::current_dir()
                        .expect("current directory")
                        .join("workspace")
                ))
                .expect("absolute workspace cwd"),
                workspace_roots: None,
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: None,
                sandbox_policy: SandboxPolicy::DangerFullAccess,
                permission_profile: Some(permission_profile.clone()),
                network: None,
                file_system_sandbox_policy: None,
                model: "gpt-5".to_string(),
                comp_hash: None,
                personality: None,
                collaboration_mode: None,
                multi_agent_version: None,
                multi_agent_mode: None,
                realtime_active: None,
                effort: None,
                summary: codex_protocol::config_types::ReasoningSummary::Auto,
            }),
            "test-provider",
        );

        assert_eq!(
            metadata.sandbox_policy,
            serde_json::to_string(&permission_profile).expect("serialize permission profile")
        );
    }

    #[test]
    fn turn_context_sets_cwd_when_session_cwd_missing() {
        let mut metadata = metadata_for_test();
        metadata.cwd = PathBuf::new();
        let fallback_cwd = std::env::current_dir()
            .expect("current directory")
            .join("fallback/workspace");

        apply_rollout_item(
            &mut metadata,
            &RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("turn-1".to_string()),
                cwd: serde_json::from_value(serde_json::json!(&fallback_cwd))
                    .expect("absolute fallback cwd"),
                workspace_roots: None,
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: None,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                permission_profile: None,
                network: None,
                file_system_sandbox_policy: None,
                model: "gpt-5".to_string(),
                comp_hash: None,
                personality: None,
                collaboration_mode: None,
                multi_agent_version: None,
                multi_agent_mode: None,
                realtime_active: None,
                effort: Some(ReasoningEffort::High),
                summary: codex_protocol::config_types::ReasoningSummary::Auto,
            }),
            "test-provider",
        );

        assert_eq!(metadata.cwd, fallback_cwd);
    }

    #[test]
    fn turn_context_sets_model_and_reasoning_effort() {
        let mut metadata = metadata_for_test();

        apply_rollout_item(
            &mut metadata,
            &RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("turn-1".to_string()),
                cwd: serde_json::from_value(serde_json::json!(
                    std::env::current_dir()
                        .expect("current directory")
                        .join("fallback/workspace")
                ))
                .expect("absolute fallback cwd"),
                workspace_roots: None,
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: None,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                permission_profile: None,
                network: None,
                file_system_sandbox_policy: None,
                model: "gpt-5".to_string(),
                comp_hash: None,
                personality: None,
                collaboration_mode: None,
                multi_agent_version: None,
                multi_agent_mode: None,
                realtime_active: None,
                effort: Some(ReasoningEffort::High),
                summary: codex_protocol::config_types::ReasoningSummary::Auto,
            }),
            "test-provider",
        );

        assert_eq!(metadata.model.as_deref(), Some("gpt-5"));
        assert_eq!(metadata.reasoning_effort, Some(ReasoningEffort::High));
    }

    #[test]
    fn thread_settings_applied_updates_resume_metadata() {
        let mut metadata = metadata_for_test();
        let permission_profile = PermissionProfile::workspace_write();
        let cwd = std::env::current_dir()
            .expect("current directory")
            .join("updated/workspace");
        let item = RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(
            ThreadSettingsAppliedEvent {
                thread_settings: ThreadSettingsSnapshot {
                    model: "gpt-5.2-codex".to_string(),
                    model_provider_id: "updated-provider".to_string(),
                    service_tier: None,
                    approval_policy: AskForApproval::Never,
                    approvals_reviewer: ApprovalsReviewer::User,
                    permission_profile: permission_profile.clone(),
                    active_permission_profile: None,
                    cwd: cwd.clone().try_into().expect("absolute settings cwd"),
                    reasoning_effort: Some(ReasoningEffort::Ultra),
                    reasoning_summary: Some(ReasoningSummary::Auto),
                    personality: None,
                    collaboration_mode: CollaborationMode {
                        mode: ModeKind::Default,
                        settings: Settings {
                            model: "gpt-5.2-codex".to_string(),
                            reasoning_effort: Some(ReasoningEffort::Ultra),
                            developer_instructions: None,
                        },
                    },
                },
            },
        ));

        assert!(rollout_item_affects_thread_metadata(&item));
        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(metadata.model_provider, "updated-provider");
        assert_eq!(metadata.reasoning_effort, Some(ReasoningEffort::Ultra));
        assert_eq!(metadata.cwd, cwd);
        assert_eq!(metadata.approval_mode, "never");
        assert_eq!(
            metadata.sandbox_policy,
            serde_json::to_string(&permission_profile)
                .expect("permission profile should serialize")
        );
    }

    #[test]
    fn session_meta_does_not_set_model_or_reasoning_effort() {
        let mut metadata = metadata_for_test();
        metadata.history_mode = ThreadHistoryMode::Paginated;
        let thread_id = metadata.id;

        apply_rollout_item(
            &mut metadata,
            &RolloutItem::SessionMeta(SessionMetaLine {
                meta: SessionMeta {
                    session_id: thread_id.into(),
                    id: thread_id,
                    forked_from_id: None,
                    parent_thread_id: None,
                    timestamp: "2026-02-26T00:00:00.000Z".to_string(),
                    cwd: PathBuf::from("/workspace"),
                    originator: "codex_cli_rs".to_string(),
                    cli_version: "0.0.0".to_string(),
                    source: SessionSource::Cli,
                    thread_source: None,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                    model_provider: Some("openai".to_string()),
                    base_instructions: None,
                    dynamic_tools: None,
                    selected_capability_roots: Vec::new(),
                    memory_mode: None,
                    history_mode: ThreadHistoryMode::Legacy,
                    history_base: None,
                    subagent_history_start_ordinal: None,
                    multi_agent_version: None,
                    context_window: None,
                },
                git: None,
            }),
            "test-provider",
        );

        assert_eq!(metadata.model, None);
        assert_eq!(metadata.reasoning_effort, None);
        assert_eq!(metadata.history_mode, ThreadHistoryMode::Paginated);
    }

    fn metadata_for_test() -> ThreadMetadata {
        let id = ThreadId::from_string(&Uuid::from_u128(42).to_string()).expect("thread id");
        let created_at = DateTime::<Utc>::from_timestamp(1_735_689_600, 0).expect("timestamp");
        ThreadMetadata {
            id,
            rollout_path: PathBuf::from("/tmp/a.jsonl"),
            created_at,
            updated_at: created_at,
            recency_at: created_at,
            source: "cli".to_string(),
            history_mode: Default::default(),
            thread_source: None,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
            model_provider: "openai".to_string(),
            model: None,
            reasoning_effort: None,
            cwd: PathBuf::from("/tmp"),
            cli_version: "0.0.0".to_string(),
            title: String::new(),
            name: None,
            preview: None,
            sandbox_policy: "read-only".to_string(),
            approval_mode: "on-request".to_string(),
            tokens_used: 1,
            first_user_message: None,
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        }
    }

    #[test]
    fn diff_fields_detects_changes() {
        let mut base = metadata_for_test();
        base.id = ThreadId::from_string(&Uuid::now_v7().to_string()).expect("thread id");
        base.title = "hello".to_string();
        let mut other = base.clone();
        other.tokens_used = 2;
        other.title = "world".to_string();
        let diffs = base.diff_fields(&other);
        assert_eq!(diffs, vec!["title", "tokens_used"]);
    }
}
