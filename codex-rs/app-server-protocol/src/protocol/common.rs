use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::JSONRPCNotification;
use crate::JSONRPCRequest;
use crate::RequestId;
use crate::export::GeneratedSchema;
use crate::export::write_json_schema;
use crate::protocol::v1;
use crate::protocol::v2;
use codex_protocol::ConversationId;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxCommandAssessment;
use paste::paste;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
#[ts(type = "string")]
pub struct GitSha(pub String);

impl GitSha {
    pub fn new(sha: &str) -> Self {
        Self(sha.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Display, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    ApiKey,
    ChatGPT,
}

/// Generates an `enum ClientRequest` where each variant is a request that the
/// client can send to the server. Each variant has associated `params` and
/// `response` types. Also generates a `export_client_responses()` function to
/// export all response types to TypeScript.
macro_rules! client_request_definitions {
    (
        $(
            $(#[$variant_meta:meta])*
            $variant:ident {
                params: $(#[$params_meta:meta])* $params:ty,
                response: $response:ty,
            }
        ),* $(,)?
    ) => {
        /// Request from the client to the server.
        #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
        #[serde(tag = "method", rename_all = "camelCase")]
        pub enum ClientRequest {
            $(
                $(#[$variant_meta])*
                $variant {
                    #[serde(rename = "id")]
                    request_id: RequestId,
                    $(#[$params_meta])*
                    params: $params,
                },
            )*
        }

        pub fn export_client_responses(
            out_dir: &::std::path::Path,
        ) -> ::std::result::Result<(), ::ts_rs::ExportError> {
            $(
                <$response as ::ts_rs::TS>::export_all_to(out_dir)?;
            )*
            Ok(())
        }

        #[allow(clippy::vec_init_then_push)]
        pub fn export_client_response_schemas(
            out_dir: &::std::path::Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let mut schemas = Vec::new();
            $(
                schemas.push(write_json_schema::<$response>(out_dir, stringify!($response))?);
            )*
            Ok(schemas)
        }

        #[allow(clippy::vec_init_then_push)]
        pub fn export_client_param_schemas(
            out_dir: &::std::path::Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let mut schemas = Vec::new();
            $(
                schemas.push(write_json_schema::<$params>(out_dir, stringify!($params))?);
            )*
            Ok(schemas)
        }
    };
}

client_request_definitions! {
    /// NEW APIs
    // Thread lifecycle
    #[serde(rename = "thread/start")]
    #[ts(rename = "thread/start")]
    ThreadStart {
        params: v2::ThreadStartParams,
        response: v2::ThreadStartResponse,
    },
    #[serde(rename = "thread/resume")]
    #[ts(rename = "thread/resume")]
    ThreadResume {
        params: v2::ThreadResumeParams,
        response: v2::ThreadResumeResponse,
    },
    #[serde(rename = "thread/archive")]
    #[ts(rename = "thread/archive")]
    ThreadArchive {
        params: v2::ThreadArchiveParams,
        response: v2::ThreadArchiveResponse,
    },
    #[serde(rename = "thread/list")]
    #[ts(rename = "thread/list")]
    ThreadList {
        params: v2::ThreadListParams,
        response: v2::ThreadListResponse,
    },
    #[serde(rename = "thread/compact")]
    #[ts(rename = "thread/compact")]
    ThreadCompact {
        params: v2::ThreadCompactParams,
        response: v2::ThreadCompactResponse,
    },
    #[serde(rename = "turn/start")]
    #[ts(rename = "turn/start")]
    TurnStart {
        params: v2::TurnStartParams,
        response: v2::TurnStartResponse,
    },
    #[serde(rename = "turn/interrupt")]
    #[ts(rename = "turn/interrupt")]
    TurnInterrupt {
        params: v2::TurnInterruptParams,
        response: v2::TurnInterruptResponse,
    },

    #[serde(rename = "model/list")]
    #[ts(rename = "model/list")]
    ModelList {
        params: v2::ModelListParams,
        response: v2::ModelListResponse,
    },

    #[serde(rename = "account/login/start")]
    #[ts(rename = "account/login/start")]
    LoginAccount {
        params: v2::LoginAccountParams,
        response: v2::LoginAccountResponse,
    },

    #[serde(rename = "account/login/cancel")]
    #[ts(rename = "account/login/cancel")]
    CancelLoginAccount {
        params: v2::CancelLoginAccountParams,
        response: v2::CancelLoginAccountResponse,
    },

    #[serde(rename = "account/logout")]
    #[ts(rename = "account/logout")]
    LogoutAccount {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v2::LogoutAccountResponse,
    },

    #[serde(rename = "account/rateLimits/read")]
    #[ts(rename = "account/rateLimits/read")]
    GetAccountRateLimits {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v2::GetAccountRateLimitsResponse,
    },

    #[serde(rename = "feedback/upload")]
    #[ts(rename = "feedback/upload")]
    FeedbackUpload {
        params: v2::FeedbackUploadParams,
        response: v2::FeedbackUploadResponse,
    },

    #[serde(rename = "account/read")]
    #[ts(rename = "account/read")]
    GetAccount {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v2::GetAccountResponse,
    },

    /// DEPRECATED APIs below
    Initialize {
        params: v1::InitializeParams,
        response: v1::InitializeResponse,
    },
    NewConversation {
        params: v1::NewConversationParams,
        response: v1::NewConversationResponse,
    },
    GetConversationSummary {
        params: v1::GetConversationSummaryParams,
        response: v1::GetConversationSummaryResponse,
    },
    /// List recorded Codex conversations (rollouts) with optional pagination and search.
    ListConversations {
        params: v1::ListConversationsParams,
        response: v1::ListConversationsResponse,
    },
    /// Resume a recorded Codex conversation from a rollout file.
    ResumeConversation {
        params: v1::ResumeConversationParams,
        response: v1::ResumeConversationResponse,
    },
    ArchiveConversation {
        params: v1::ArchiveConversationParams,
        response: v1::ArchiveConversationResponse,
    },
    SendUserMessage {
        params: v1::SendUserMessageParams,
        response: v1::SendUserMessageResponse,
    },
    SendUserTurn {
        params: v1::SendUserTurnParams,
        response: v1::SendUserTurnResponse,
    },
    InterruptConversation {
        params: v1::InterruptConversationParams,
        response: v1::InterruptConversationResponse,
    },
    AddConversationListener {
        params: v1::AddConversationListenerParams,
        response: v1::AddConversationSubscriptionResponse,
    },
    RemoveConversationListener {
        params: v1::RemoveConversationListenerParams,
        response: v1::RemoveConversationSubscriptionResponse,
    },
    GitDiffToRemote {
        params: v1::GitDiffToRemoteParams,
        response: v1::GitDiffToRemoteResponse,
    },
    LoginApiKey {
        params: v1::LoginApiKeyParams,
        response: v1::LoginApiKeyResponse,
    },
    LoginChatGpt {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v1::LoginChatGptResponse,
    },
    // DEPRECATED in favor of CancelLoginAccount
    CancelLoginChatGpt {
        params: v1::CancelLoginChatGptParams,
        response: v1::CancelLoginChatGptResponse,
    },
    LogoutChatGpt {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v1::LogoutChatGptResponse,
    },
    GetAuthStatus {
        params: v1::GetAuthStatusParams,
        response: v1::GetAuthStatusResponse,
    },
    GetUserSavedConfig {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v1::GetUserSavedConfigResponse,
    },
    SetDefaultModel {
        params: v1::SetDefaultModelParams,
        response: v1::SetDefaultModelResponse,
    },
    GetUserAgent {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v1::GetUserAgentResponse,
    },
    UserInfo {
        params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>,
        response: v1::UserInfoResponse,
    },
    FuzzyFileSearch {
        params: FuzzyFileSearchParams,
        response: FuzzyFileSearchResponse,
    },
    /// Execute a command (argv vector) under the server's sandbox.
    ExecOneOffCommand {
        params: v1::ExecOneOffCommandParams,
        response: v1::ExecOneOffCommandResponse,
    },
}

/// Generates an `enum ServerRequest` where each variant is a request that the
/// server can send to the client along with the corresponding params and
/// response types. It also generates helper types used by the app/server
/// infrastructure (payload enum, request constructor, and export helpers).
macro_rules! server_request_definitions {
    (
        $(
            $(#[$variant_meta:meta])*
            $variant:ident
        ),* $(,)?
    ) => {
        paste! {
            /// Request initiated from the server and sent to the client.
            #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
            #[serde(tag = "method", rename_all = "camelCase")]
            pub enum ServerRequest {
                $(
                    $(#[$variant_meta])*
                    $variant {
                        #[serde(rename = "id")]
                        request_id: RequestId,
                        params: [<$variant Params>],
                    },
                )*
            }

            #[derive(Debug, Clone, PartialEq, JsonSchema)]
            pub enum ServerRequestPayload {
                $( $variant([<$variant Params>]), )*
            }

            impl ServerRequestPayload {
                pub fn request_with_id(self, request_id: RequestId) -> ServerRequest {
                    match self {
                        $(Self::$variant(params) => ServerRequest::$variant { request_id, params },)*
                    }
                }
            }
        }

        pub fn export_server_responses(
            out_dir: &::std::path::Path,
        ) -> ::std::result::Result<(), ::ts_rs::ExportError> {
            paste! {
                $(<[<$variant Response>] as ::ts_rs::TS>::export_all_to(out_dir)?;)*
            }
            Ok(())
        }

        #[allow(clippy::vec_init_then_push)]
        pub fn export_server_response_schemas(
            out_dir: &Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let mut schemas = Vec::new();
            paste! {
                $(schemas.push(crate::export::write_json_schema::<[<$variant Response>]>(out_dir, stringify!([<$variant Response>]))?);)*
            }
            Ok(schemas)
        }

        #[allow(clippy::vec_init_then_push)]
        pub fn export_server_param_schemas(
            out_dir: &Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let mut schemas = Vec::new();
            paste! {
                $(schemas.push(crate::export::write_json_schema::<[<$variant Params>]>(out_dir, stringify!([<$variant Params>]))?);)*
            }
            Ok(schemas)
        }
    };
}

/// Generates `ServerNotification` enum and helpers, including a JSON Schema
/// exporter for each notification.
macro_rules! server_notification_definitions {
    (
        $(
            $(#[$variant_meta:meta])*
            $variant:ident $(=> $wire:literal)? ( $payload:ty )
        ),* $(,)?
    ) => {
        /// Notification sent from the server to the client.
        #[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS, Display)]
        #[serde(tag = "method", content = "params", rename_all = "camelCase")]
        #[strum(serialize_all = "camelCase")]
        pub enum ServerNotification {
            $(
                $(#[$variant_meta])*
                $(#[serde(rename = $wire)] #[ts(rename = $wire)] #[strum(serialize = $wire)])?
                $variant($payload),
            )*
        }

        impl ServerNotification {
            pub fn to_params(self) -> Result<serde_json::Value, serde_json::Error> {
                match self {
                    $(Self::$variant(params) => serde_json::to_value(params),)*
                }
            }
        }

        impl TryFrom<JSONRPCNotification> for ServerNotification {
            type Error = serde_json::Error;

            fn try_from(value: JSONRPCNotification) -> Result<Self, Self::Error> {
                serde_json::from_value(serde_json::to_value(value)?)
            }
        }

        #[allow(clippy::vec_init_then_push)]
        pub fn export_server_notification_schemas(
            out_dir: &::std::path::Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let mut schemas = Vec::new();
            $(schemas.push(crate::export::write_json_schema::<$payload>(out_dir, stringify!($payload))?);)*
            Ok(schemas)
        }
    };
}
/// Notifications sent from the client to the server.
macro_rules! client_notification_definitions {
    (
        $(
            $(#[$variant_meta:meta])*
            $variant:ident $( ( $payload:ty ) )?
        ),* $(,)?
    ) => {
        #[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS, Display)]
        #[serde(tag = "method", content = "params", rename_all = "camelCase")]
        #[strum(serialize_all = "camelCase")]
        pub enum ClientNotification {
            $(
                $(#[$variant_meta])*
                $variant $( ( $payload ) )?,
            )*
        }

        pub fn export_client_notification_schemas(
            _out_dir: &::std::path::Path,
        ) -> ::anyhow::Result<Vec<GeneratedSchema>> {
            let schemas = Vec::new();
            $( $(schemas.push(crate::export::write_json_schema::<$payload>(_out_dir, stringify!($payload))?);)? )*
            Ok(schemas)
        }
    };
}

impl TryFrom<JSONRPCRequest> for ServerRequest {
    type Error = serde_json::Error;

    fn try_from(value: JSONRPCRequest) -> Result<Self, Self::Error> {
        serde_json::from_value(serde_json::to_value(value)?)
    }
}

server_request_definitions! {
    /// Request to approve a patch.
    ApplyPatchApproval,
    /// Request to exec a command.
    ExecCommandApproval,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPatchApprovalParams {
    pub conversation_id: ConversationId,
    /// Use to correlate this with [codex_core::protocol::PatchApplyBeginEvent]
    /// and [codex_core::protocol::PatchApplyEndEvent].
    pub call_id: String,
    pub file_changes: HashMap<PathBuf, FileChange>,
    /// Optional explanatory reason (e.g. request for extra write access).
    pub reason: Option<String>,
    /// When set, the agent is asking the user to allow writes under this root
    /// for the remainder of the session (unclear if this is honored today).
    pub grant_root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecCommandApprovalParams {
    pub conversation_id: ConversationId,
    /// Use to correlate this with [codex_core::protocol::ExecCommandBeginEvent]
    /// and [codex_core::protocol::ExecCommandEndEvent].
    pub call_id: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub reason: Option<String>,
    pub risk: Option<SandboxCommandAssessment>,
    pub parsed_cmd: Vec<ParsedCommand>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct ExecCommandApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct ApplyPatchApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct FuzzyFileSearchParams {
    pub query: String,
    pub roots: Vec<String>,
    // if provided, will cancel any previous request that used the same value
    pub cancellation_token: Option<String>,
}

/// Superset of [`codex_file_search::FileMatch`]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct FuzzyFileSearchResult {
    pub root: String,
    pub path: String,
    pub file_name: String,
    pub score: u32,
    pub indices: Option<Vec<u32>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct FuzzyFileSearchResponse {
    pub files: Vec<FuzzyFileSearchResult>,
}

server_notification_definitions! {
    /// NEW NOTIFICATIONS
    ThreadStarted => "thread/started" (v2::ThreadStartedNotification),
    TurnStarted => "turn/started" (v2::TurnStartedNotification),
    TurnCompleted => "turn/completed" (v2::TurnCompletedNotification),
    ItemStarted => "item/started" (v2::ItemStartedNotification),
    ItemCompleted => "item/completed" (v2::ItemCompletedNotification),
    AgentMessageDelta => "item/agentMessage/delta" (v2::AgentMessageDeltaNotification),
    CommandExecutionOutputDelta => "item/commandExecution/outputDelta" (v2::CommandExecutionOutputDeltaNotification),
    McpToolCallProgress => "item/mcpToolCall/progress" (v2::McpToolCallProgressNotification),
    AccountUpdated => "account/updated" (v2::AccountUpdatedNotification),
    AccountRateLimitsUpdated => "account/rateLimits/updated" (v2::AccountRateLimitsUpdatedNotification),

    #[serde(rename = "account/login/completed")]
    #[ts(rename = "account/login/completed")]
    #[strum(serialize = "account/login/completed")]
    AccountLoginCompleted(v2::AccountLoginCompletedNotification),

    /// DEPRECATED NOTIFICATIONS below
    AuthStatusChange(v1::AuthStatusChangeNotification),

    /// Deprecated: use `account/login/completed` instead.
    LoginChatGptComplete(v1::LoginChatGptCompleteNotification),
    SessionConfigured(v1::SessionConfiguredNotification),
}

client_notification_definitions! {
    Initialized,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_protocol::account::PlanType;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn serialize_new_conversation() -> Result<()> {
        let request = ClientRequest::NewConversation {
            request_id: RequestId::Integer(42),
            params: v1::NewConversationParams {
                model: Some("gpt-5-codex".to_string()),
                model_provider: None,
                profile: None,
                cwd: None,
                approval_policy: Some(AskForApproval::OnRequest),
                sandbox: None,
                config: None,
                base_instructions: None,
                developer_instructions: None,
                compact_prompt: None,
                include_apply_patch_tool: None,
            },
        };
        assert_eq!(
            json!({
                "method": "newConversation",
                "id": 42,
                "params": {
                    "model": "gpt-5-codex",
                    "modelProvider": null,
                    "profile": null,
                    "cwd": null,
                    "approvalPolicy": "on-request",
                    "sandbox": null,
                    "config": null,
                    "baseInstructions": null,
                    "includeApplyPatchTool": null
                }
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn conversation_id_serializes_as_plain_string() -> Result<()> {
        let id = ConversationId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8")?;

        assert_eq!(
            json!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            serde_json::to_value(id)?
        );
        Ok(())
    }

    #[test]
    fn conversation_id_deserializes_from_plain_string() -> Result<()> {
        let id: ConversationId =
            serde_json::from_value(json!("67e55044-10b1-426f-9247-bb680e5fe0c8"))?;

        assert_eq!(
            ConversationId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8")?,
            id,
        );
        Ok(())
    }

    #[test]
    fn serialize_client_notification() -> Result<()> {
        let notification = ClientNotification::Initialized;
        // Note there is no "params" field for this notification.
        assert_eq!(
            json!({
                "method": "initialized",
            }),
            serde_json::to_value(&notification)?,
        );
        Ok(())
    }

    #[test]
    fn serialize_server_request() -> Result<()> {
        let conversation_id = ConversationId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8")?;
        let params = ExecCommandApprovalParams {
            conversation_id,
            call_id: "call-42".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            cwd: PathBuf::from("/tmp"),
            reason: Some("because tests".to_string()),
            risk: None,
            parsed_cmd: vec![ParsedCommand::Unknown {
                cmd: "echo hello".to_string(),
            }],
        };
        let request = ServerRequest::ExecCommandApproval {
            request_id: RequestId::Integer(7),
            params: params.clone(),
        };

        assert_eq!(
            json!({
                "method": "execCommandApproval",
                "id": 7,
                "params": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "callId": "call-42",
                    "command": ["echo", "hello"],
                    "cwd": "/tmp",
                    "reason": "because tests",
                    "risk": null,
                    "parsedCmd": [
                        {
                            "type": "unknown",
                            "cmd": "echo hello"
                        }
                    ]
                }
            }),
            serde_json::to_value(&request)?,
        );

        let payload = ServerRequestPayload::ExecCommandApproval(params);
        assert_eq!(payload.request_with_id(RequestId::Integer(7)), request);
        Ok(())
    }

    #[test]
    fn serialize_get_account_rate_limits() -> Result<()> {
        let request = ClientRequest::GetAccountRateLimits {
            request_id: RequestId::Integer(1),
            params: None,
        };
        assert_eq!(
            json!({
                "method": "account/rateLimits/read",
                "id": 1,
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn serialize_account_login_api_key() -> Result<()> {
        let request = ClientRequest::LoginAccount {
            request_id: RequestId::Integer(2),
            params: v2::LoginAccountParams::ApiKey {
                api_key: "secret".to_string(),
            },
        };
        assert_eq!(
            json!({
                "method": "account/login/start",
                "id": 2,
                "params": {
                    "type": "apiKey",
                    "apiKey": "secret"
                }
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn serialize_account_login_chatgpt() -> Result<()> {
        let request = ClientRequest::LoginAccount {
            request_id: RequestId::Integer(3),
            params: v2::LoginAccountParams::Chatgpt,
        };
        assert_eq!(
            json!({
                "method": "account/login/start",
                "id": 3,
                "params": {
                    "type": "chatgpt"
                }
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn serialize_account_logout() -> Result<()> {
        let request = ClientRequest::LogoutAccount {
            request_id: RequestId::Integer(4),
            params: None,
        };
        assert_eq!(
            json!({
                "method": "account/logout",
                "id": 4,
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn serialize_get_account() -> Result<()> {
        let request = ClientRequest::GetAccount {
            request_id: RequestId::Integer(5),
            params: None,
        };
        assert_eq!(
            json!({
                "method": "account/read",
                "id": 5,
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }

    #[test]
    fn account_serializes_fields_in_camel_case() -> Result<()> {
        let api_key = v2::Account::ApiKey {
            api_key: "secret".to_string(),
        };
        assert_eq!(
            json!({
                "type": "apiKey",
                "apiKey": "secret",
            }),
            serde_json::to_value(&api_key)?,
        );

        let chatgpt = v2::Account::Chatgpt {
            email: Some("user@example.com".to_string()),
            plan_type: PlanType::Plus,
        };
        assert_eq!(
            json!({
                "type": "chatgpt",
                "email": "user@example.com",
                "planType": "plus",
            }),
            serde_json::to_value(&chatgpt)?,
        );

        Ok(())
    }

    #[test]
    fn serialize_list_models() -> Result<()> {
        let request = ClientRequest::ModelList {
            request_id: RequestId::Integer(6),
            params: v2::ModelListParams::default(),
        };
        assert_eq!(
            json!({
                "method": "model/list",
                "id": 6,
                "params": {
                    "limit": null,
                    "cursor": null
                }
            }),
            serde_json::to_value(&request)?,
        );
        Ok(())
    }
}
