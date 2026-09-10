use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;

pub(crate) fn without_notification_media(notification: ServerNotification) -> ServerNotification {
    match notification {
        ServerNotification::ItemStarted(mut notification) => {
            notification.item = without_thread_item_media(notification.item);
            ServerNotification::ItemStarted(notification)
        }
        ServerNotification::ItemCompleted(mut notification) => {
            notification.item = without_thread_item_media(notification.item);
            ServerNotification::ItemCompleted(notification)
        }
        ServerNotification::RawResponseItemCompleted(mut notification) => {
            match &mut notification.item {
                ResponseItem::Message { content, .. } => {
                    content.retain(|item| match item {
                        ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => false,
                        ContentItem::InputText { .. } | ContentItem::OutputText { .. } => true,
                    });
                }
                ResponseItem::FunctionCallOutput { output, .. }
                | ResponseItem::CustomToolCallOutput { output, .. } => {
                    if let Some(items) = output.content_items_mut() {
                        items.retain(|item| match item {
                            FunctionCallOutputContentItem::InputImage { .. }
                            | FunctionCallOutputContentItem::InputAudio { .. } => false,
                            FunctionCallOutputContentItem::InputText { .. }
                            | FunctionCallOutputContentItem::EncryptedContent { .. } => true,
                        });
                    }
                }
                ResponseItem::ImageGenerationCall { result, .. } => result.clear(),
                ResponseItem::AdditionalTools { .. }
                | ResponseItem::AgentMessage { .. }
                | ResponseItem::Reasoning { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::FunctionCall { .. }
                | ResponseItem::ToolSearchCall { .. }
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::ToolSearchOutput { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::Compaction { .. }
                | ResponseItem::ConfigurationUpdate { .. }
                | ResponseItem::CompactionTrigger { .. }
                | ResponseItem::ContextCompaction { .. }
                | ResponseItem::Other => {}
            }
            ServerNotification::RawResponseItemCompleted(notification)
        }
        ServerNotification::Error(_)
        | ServerNotification::ThreadStarted(_)
        | ServerNotification::ThreadStatusChanged(_)
        | ServerNotification::ThreadArchived(_)
        | ServerNotification::ThreadDeleted(_)
        | ServerNotification::ThreadUnarchived(_)
        | ServerNotification::ThreadClosed(_)
        | ServerNotification::ThreadReverted(_)
        | ServerNotification::SkillsChanged(_)
        | ServerNotification::ThreadNameUpdated(_)
        | ServerNotification::ThreadAttachmentUpdated(_)
        | ServerNotification::ThreadGoalUpdated(_)
        | ServerNotification::ThreadGoalCleared(_)
        | ServerNotification::ThreadQueueChanged(_)
        | ServerNotification::ProjectChanged(_)
        | ServerNotification::ThreadProjectUpdated(_)
        | ServerNotification::EnvironmentConnected(_)
        | ServerNotification::EnvironmentDisconnected(_)
        | ServerNotification::ThreadSettingsUpdated(_)
        | ServerNotification::ThreadTokenUsageUpdated(_)
        | ServerNotification::TurnStarted(_)
        | ServerNotification::HookStarted(_)
        | ServerNotification::TurnCompleted(_)
        | ServerNotification::HookCompleted(_)
        | ServerNotification::TurnDiffUpdated(_)
        | ServerNotification::TurnPlanUpdated(_)
        | ServerNotification::ItemGuardianApprovalReviewStarted(_)
        | ServerNotification::ItemGuardianApprovalReviewCompleted(_)
        | ServerNotification::StrictReviewRequired(_)
        | ServerNotification::RawResponseCompleted(_)
        | ServerNotification::AgentMessageDelta(_)
        | ServerNotification::PlanDelta(_)
        | ServerNotification::CommandExecOutputDelta(_)
        | ServerNotification::ProcessOutputDelta(_)
        | ServerNotification::ProcessExited(_)
        | ServerNotification::CommandExecutionOutputDelta(_)
        | ServerNotification::TerminalInteraction(_)
        | ServerNotification::FileChangeOutputDelta(_)
        | ServerNotification::FileChangePatchUpdated(_)
        | ServerNotification::ServerRequestResolved(_)
        | ServerNotification::McpToolCallProgress(_)
        | ServerNotification::McpServerOauthLoginCompleted(_)
        | ServerNotification::McpServerStatusUpdated(_)
        | ServerNotification::McpServerEventStream(_)
        | ServerNotification::AccountUpdated(_)
        | ServerNotification::AccountRateLimitsUpdated(_)
        | ServerNotification::AppListUpdated(_)
        | ServerNotification::RemoteControlStatusChanged(_)
        | ServerNotification::ExternalAgentConfigImportProgress(_)
        | ServerNotification::ExternalAgentConfigImportCompleted(_)
        | ServerNotification::FsChanged(_)
        | ServerNotification::ReasoningSummaryTextDelta(_)
        | ServerNotification::ReasoningSummaryPartAdded(_)
        | ServerNotification::ReasoningTextDelta(_)
        | ServerNotification::ContextCompacted(_)
        | ServerNotification::ModelRerouted(_)
        | ServerNotification::ModelVerification(_)
        | ServerNotification::AuthRecoveryStarted(_)
        | ServerNotification::AuthRecoveryCompleted(_)
        | ServerNotification::TurnModerationMetadata(_)
        | ServerNotification::ModelSafetyBufferingUpdated(_)
        | ServerNotification::Warning(_)
        | ServerNotification::GuardianWarning(_)
        | ServerNotification::DeprecationNotice(_)
        | ServerNotification::ConfigWarning(_)
        | ServerNotification::FuzzyFileSearchSessionUpdated(_)
        | ServerNotification::FuzzyFileSearchSessionCompleted(_)
        | ServerNotification::ThreadRealtimeStarted(_)
        | ServerNotification::ThreadRealtimeItemAdded(_)
        | ServerNotification::ThreadRealtimeItemStarted(_)
        | ServerNotification::ThreadRealtimeItemTranscriptDelta(_)
        | ServerNotification::ThreadRealtimeItemCompleted(_)
        | ServerNotification::ThreadRealtimeTranscriptDelta(_)
        | ServerNotification::ThreadRealtimeTranscriptDone(_)
        | ServerNotification::ThreadRealtimeOutputAudioDelta(_)
        | ServerNotification::ThreadRealtimeSdp(_)
        | ServerNotification::ThreadRealtimeError(_)
        | ServerNotification::ThreadRealtimeClosed(_)
        | ServerNotification::WindowsWorldWritableWarning(_)
        | ServerNotification::WindowsSandboxSetupCompleted(_)
        | ServerNotification::AccountLoginCompleted(_) => notification,
    }
}

fn without_thread_item_media(mut item: ThreadItem) -> ThreadItem {
    match &mut item {
        ThreadItem::UserMessage { content, .. } => {
            content.retain(|item| match item {
                UserInput::Image { .. } | UserInput::Audio { .. } => false,
                UserInput::Text { .. }
                | UserInput::LocalImage { .. }
                | UserInput::LocalAudio { .. }
                | UserInput::Skill { .. }
                | UserInput::Mention { .. } => true,
            });
        }
        ThreadItem::DynamicToolCall {
            content_items: Some(content_items),
            ..
        } => {
            content_items.retain(|item| match item {
                DynamicToolCallOutputContentItem::InputImage { .. }
                | DynamicToolCallOutputContentItem::InputAudio { .. } => false,
                DynamicToolCallOutputContentItem::InputText { .. } => true,
            });
        }
        ThreadItem::FunctionCallOutput {
            output: FunctionCallOutputBody::ContentItems(items),
            ..
        } => {
            items.retain(|item| match item {
                FunctionCallOutputContentItem::InputImage { .. }
                | FunctionCallOutputContentItem::InputAudio { .. } => false,
                FunctionCallOutputContentItem::InputText { .. }
                | FunctionCallOutputContentItem::EncryptedContent { .. } => true,
            });
        }
        ThreadItem::McpToolCall {
            result: Some(result),
            ..
        } => {
            // TODO(ruslan): Handle oversized results that core has already collapsed into a
            // truncated text preview, which can contain inline media that this filter retains.
            result.content.retain(|item| {
                !matches!(
                    item.get("type").and_then(serde_json::Value::as_str),
                    Some("image" | "audio")
                ) && item
                    .get("resource")
                    .and_then(|resource| resource.get("blob"))
                    .is_none()
            });
        }
        ThreadItem::ImageGeneration(item) => item.result.clear(),
        ThreadItem::HookPrompt { .. }
        | ThreadItem::AgentMessage { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::FunctionCallOutput {
            output: FunctionCallOutputBody::Text(_),
            ..
        }
        | ThreadItem::CommandExecution { .. }
        | ThreadItem::FileChange { .. }
        | ThreadItem::McpToolCall { result: None, .. }
        | ThreadItem::DynamicToolCall {
            content_items: None,
            ..
        }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::SubAgentActivity { .. }
        | ThreadItem::WebSearch(_)
        | ThreadItem::ImageView { .. }
        | ThreadItem::Sleep(_)
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => {}
    }
    item
}
