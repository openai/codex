mod analytics_server;
mod auth_fixtures;
mod config;
mod mock_model_server;
mod models_cache;
mod responses;
mod rollout;
mod test_app_server;

pub use analytics_server::start_analytics_events_server;
pub use auth_fixtures::ChatGptAuthFixture;
pub use auth_fixtures::ChatGptIdTokenClaims;
pub use auth_fixtures::encode_id_token;
pub use auth_fixtures::write_chatgpt_auth;
use codex_app_server_protocol::ClientResponse;
pub use config::write_mock_responses_config_toml;
pub use config::write_mock_responses_config_toml_with_chatgpt_base_url;
pub use core_test_support::PathBufExt;
pub use core_test_support::format_with_current_shell;
pub use core_test_support::format_with_current_shell_display;
pub use core_test_support::format_with_current_shell_display_non_login;
pub use core_test_support::format_with_current_shell_non_login;
pub use core_test_support::test_absolute_path;
pub use core_test_support::test_path_buf_with_windows;
pub use core_test_support::test_tmp_path;
pub use core_test_support::test_tmp_path_buf;
pub use mock_model_server::create_mock_responses_server_repeating_assistant;
pub use mock_model_server::create_mock_responses_server_sequence;
pub use mock_model_server::create_mock_responses_server_sequence_unchecked;
pub use models_cache::write_models_cache;
pub use models_cache::write_models_cache_with_models;
pub use responses::create_apply_patch_sse_response;
pub use responses::create_exec_command_sse_response;
pub use responses::create_final_assistant_message_sse_response;
pub use responses::create_request_permissions_sse_response;
pub use responses::create_request_user_input_sse_response;
pub use responses::create_shell_command_sse_response;
pub use rollout::create_fake_parented_rollout_with_source;
pub use rollout::create_fake_rollout;
pub use rollout::create_fake_rollout_with_source;
pub use rollout::create_fake_rollout_with_text_elements;
pub use rollout::create_fake_rollout_with_token_usage;
pub use rollout::rollout_path;
pub use test_app_server::DEFAULT_CLIENT_NAME;
pub use test_app_server::DISABLE_PLUGIN_STARTUP_TASKS_ARG;
pub use test_app_server::IntoServerResponse;
pub use test_app_server::TestAppServer;

pub trait FromClientResponse: Sized {
    fn from_client_response(response: ClientResponse) -> anyhow::Result<Self>;
}

macro_rules! client_response_conversions {
    ($($response:ty => $($variant:ident)|+),* $(,)?) => {
        $(
            impl FromClientResponse for $response {
                fn from_client_response(response: ClientResponse) -> anyhow::Result<Self> {
                    match response {
                        $(
                            ClientResponse::$variant { response, .. } => Ok(response),
                        )+
                        other => anyhow::bail!(
                            "expected {} response, got {}",
                            stringify!($($variant)|+),
                            other.method(),
                        ),
                    }
                }
            }
        )*
    };
}

client_response_conversions! {
    codex_app_server_protocol::InitializeResponse => Initialize,
    codex_app_server_protocol::ThreadStartResponse => ThreadStart,
    codex_app_server_protocol::ThreadResumeResponse => ThreadResume,
    codex_app_server_protocol::ThreadForkResponse => ThreadFork,
    codex_app_server_protocol::ThreadArchiveResponse => ThreadArchive,
    codex_app_server_protocol::ThreadUnsubscribeResponse => ThreadUnsubscribe,
    codex_app_server_protocol::ThreadIncrementElicitationResponse => ThreadIncrementElicitation,
    codex_app_server_protocol::ThreadDecrementElicitationResponse => ThreadDecrementElicitation,
    codex_app_server_protocol::ThreadSetNameResponse => ThreadSetName,
    codex_app_server_protocol::ThreadGoalSetResponse => ThreadGoalSet,
    codex_app_server_protocol::ThreadGoalGetResponse => ThreadGoalGet,
    codex_app_server_protocol::ThreadGoalClearResponse => ThreadGoalClear,
    codex_app_server_protocol::ThreadMetadataUpdateResponse => ThreadMetadataUpdate,
    codex_app_server_protocol::ThreadSettingsUpdateResponse => ThreadSettingsUpdate,
    codex_app_server_protocol::ThreadMemoryModeSetResponse => ThreadMemoryModeSet,
    codex_app_server_protocol::MemoryResetResponse => MemoryReset,
    codex_app_server_protocol::ThreadUnarchiveResponse => ThreadUnarchive,
    codex_app_server_protocol::ThreadCompactStartResponse => ThreadCompactStart,
    codex_app_server_protocol::ThreadShellCommandResponse => ThreadShellCommand,
    codex_app_server_protocol::ThreadApproveGuardianDeniedActionResponse
        => ThreadApproveGuardianDeniedAction,
    codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse
        => ThreadBackgroundTerminalsClean,
    codex_app_server_protocol::ThreadRollbackResponse => ThreadRollback,
    codex_app_server_protocol::ThreadListResponse => ThreadList,
    codex_app_server_protocol::ThreadSearchResponse => ThreadSearch,
    codex_app_server_protocol::ThreadLoadedListResponse => ThreadLoadedList,
    codex_app_server_protocol::ThreadReadResponse => ThreadRead,
    codex_app_server_protocol::ThreadTurnsListResponse => ThreadTurnsList,
    codex_app_server_protocol::ThreadTurnsItemsListResponse => ThreadTurnsItemsList,
    codex_app_server_protocol::ThreadInjectItemsResponse => ThreadInjectItems,
    codex_app_server_protocol::SkillsListResponse => SkillsList,
    codex_app_server_protocol::SkillsExtraRootsSetResponse => SkillsExtraRootsSet,
    codex_app_server_protocol::HooksListResponse => HooksList,
    codex_app_server_protocol::MarketplaceAddResponse => MarketplaceAdd,
    codex_app_server_protocol::MarketplaceRemoveResponse => MarketplaceRemove,
    codex_app_server_protocol::MarketplaceUpgradeResponse => MarketplaceUpgrade,
    codex_app_server_protocol::PluginListResponse => PluginList,
    codex_app_server_protocol::PluginInstalledResponse => PluginInstalled,
    codex_app_server_protocol::PluginReadResponse => PluginRead,
    codex_app_server_protocol::PluginSkillReadResponse => PluginSkillRead,
    codex_app_server_protocol::PluginShareSaveResponse => PluginShareSave,
    codex_app_server_protocol::PluginShareUpdateTargetsResponse => PluginShareUpdateTargets,
    codex_app_server_protocol::PluginShareListResponse => PluginShareList,
    codex_app_server_protocol::PluginShareCheckoutResponse => PluginShareCheckout,
    codex_app_server_protocol::PluginShareDeleteResponse => PluginShareDelete,
    codex_app_server_protocol::AppsListResponse => AppsList,
    codex_app_server_protocol::FsReadFileResponse => FsReadFile,
    codex_app_server_protocol::FsWriteFileResponse => FsWriteFile,
    codex_app_server_protocol::FsCreateDirectoryResponse => FsCreateDirectory,
    codex_app_server_protocol::FsGetMetadataResponse => FsGetMetadata,
    codex_app_server_protocol::FsReadDirectoryResponse => FsReadDirectory,
    codex_app_server_protocol::FsRemoveResponse => FsRemove,
    codex_app_server_protocol::FsCopyResponse => FsCopy,
    codex_app_server_protocol::FsWatchResponse => FsWatch,
    codex_app_server_protocol::FsUnwatchResponse => FsUnwatch,
    codex_app_server_protocol::SkillsConfigWriteResponse => SkillsConfigWrite,
    codex_app_server_protocol::PluginInstallResponse => PluginInstall,
    codex_app_server_protocol::PluginUninstallResponse => PluginUninstall,
    codex_app_server_protocol::TurnStartResponse => TurnStart,
    codex_app_server_protocol::TurnSteerResponse => TurnSteer,
    codex_app_server_protocol::TurnInterruptResponse => TurnInterrupt,
    codex_app_server_protocol::ThreadRealtimeStartResponse => ThreadRealtimeStart,
    codex_app_server_protocol::ThreadRealtimeAppendAudioResponse => ThreadRealtimeAppendAudio,
    codex_app_server_protocol::ThreadRealtimeAppendTextResponse => ThreadRealtimeAppendText,
    codex_app_server_protocol::ThreadRealtimeStopResponse => ThreadRealtimeStop,
    codex_app_server_protocol::ThreadRealtimeListVoicesResponse => ThreadRealtimeListVoices,
    codex_app_server_protocol::ReviewStartResponse => ReviewStart,
    codex_app_server_protocol::ModelListResponse => ModelList,
    codex_app_server_protocol::ModelProviderCapabilitiesReadResponse
        => ModelProviderCapabilitiesRead,
    codex_app_server_protocol::ExperimentalFeatureListResponse => ExperimentalFeatureList,
    codex_app_server_protocol::PermissionProfileListResponse => PermissionProfileList,
    codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse
        => ExperimentalFeatureEnablementSet,
    codex_app_server_protocol::RemoteControlEnableResponse => RemoteControlEnable,
    codex_app_server_protocol::RemoteControlDisableResponse => RemoteControlDisable,
    codex_app_server_protocol::RemoteControlStatusReadResponse => RemoteControlStatusRead,
    codex_app_server_protocol::RemoteControlPairingStartResponse => RemoteControlPairingStart,
    codex_app_server_protocol::RemoteControlClientsListResponse => RemoteControlClientsList,
    codex_app_server_protocol::RemoteControlClientsRevokeResponse => RemoteControlClientsRevoke,
    codex_app_server_protocol::CollaborationModeListResponse => CollaborationModeList,
    codex_app_server_protocol::MockExperimentalMethodResponse => MockExperimentalMethod,
    codex_app_server_protocol::EnvironmentAddResponse => EnvironmentAdd,
    codex_app_server_protocol::McpServerOauthLoginResponse => McpServerOauthLogin,
    codex_app_server_protocol::McpServerRefreshResponse => McpServerRefresh,
    codex_app_server_protocol::ListMcpServerStatusResponse => McpServerStatusList,
    codex_app_server_protocol::McpResourceReadResponse => McpResourceRead,
    codex_app_server_protocol::McpServerToolCallResponse => McpServerToolCall,
    codex_app_server_protocol::WindowsSandboxSetupStartResponse => WindowsSandboxSetupStart,
    codex_app_server_protocol::WindowsSandboxReadinessResponse => WindowsSandboxReadiness,
    codex_app_server_protocol::LoginAccountResponse => LoginAccount,
    codex_app_server_protocol::CancelLoginAccountResponse => CancelLoginAccount,
    codex_app_server_protocol::LogoutAccountResponse => LogoutAccount,
    codex_app_server_protocol::GetAccountRateLimitsResponse => GetAccountRateLimits,
    codex_app_server_protocol::SendAddCreditsNudgeEmailResponse => SendAddCreditsNudgeEmail,
    codex_app_server_protocol::FeedbackUploadResponse => FeedbackUpload,
    codex_app_server_protocol::CommandExecResponse => OneOffCommandExec,
    codex_app_server_protocol::CommandExecWriteResponse => CommandExecWrite,
    codex_app_server_protocol::CommandExecTerminateResponse => CommandExecTerminate,
    codex_app_server_protocol::CommandExecResizeResponse => CommandExecResize,
    codex_app_server_protocol::ProcessSpawnResponse => ProcessSpawn,
    codex_app_server_protocol::ProcessWriteStdinResponse => ProcessWriteStdin,
    codex_app_server_protocol::ProcessKillResponse => ProcessKill,
    codex_app_server_protocol::ProcessResizePtyResponse => ProcessResizePty,
    codex_app_server_protocol::ConfigReadResponse => ConfigRead,
    codex_app_server_protocol::ExternalAgentConfigDetectResponse => ExternalAgentConfigDetect,
    codex_app_server_protocol::ExternalAgentConfigImportResponse => ExternalAgentConfigImport,
    codex_app_server_protocol::ConfigWriteResponse => ConfigValueWrite | ConfigBatchWrite,
    codex_app_server_protocol::ConfigRequirementsReadResponse => ConfigRequirementsRead,
    codex_app_server_protocol::GetAccountResponse => GetAccount,
    codex_app_server_protocol::GetConversationSummaryResponse => GetConversationSummary,
    codex_app_server_protocol::GitDiffToRemoteResponse => GitDiffToRemote,
    codex_app_server_protocol::GetAuthStatusResponse => GetAuthStatus,
    codex_app_server_protocol::FuzzyFileSearchResponse => FuzzyFileSearch,
    codex_app_server_protocol::FuzzyFileSearchSessionStartResponse => FuzzyFileSearchSessionStart,
    codex_app_server_protocol::FuzzyFileSearchSessionUpdateResponse => FuzzyFileSearchSessionUpdate,
    codex_app_server_protocol::FuzzyFileSearchSessionStopResponse => FuzzyFileSearchSessionStop,
}

pub fn to_response<T: FromClientResponse>(response: ClientResponse) -> anyhow::Result<T> {
    T::from_client_response(response)
}
