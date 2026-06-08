use super::*;

fn decode_session_source(
    value: String,
    field: &'static str,
) -> Result<codex_protocol::protocol::SessionSource, Status> {
    use codex_protocol::protocol::InternalSessionSource;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;

    if value.is_empty() {
        return Err(invalid(field, "session source cannot be empty"));
    }

    let source = match value.as_str() {
        "cli" => SessionSource::Cli,
        "vscode" => SessionSource::VSCode,
        "exec" => SessionSource::Exec,
        "mcp" | "appserver" | "app-server" | "app_server" => SessionSource::Mcp,
        "unknown" => SessionSource::Unknown,
        "internal_memory_consolidation" => {
            SessionSource::Internal(InternalSessionSource::MemoryConsolidation)
        }
        "subagent_review" => SessionSource::SubAgent(SubAgentSource::Review),
        "subagent_compact" => SessionSource::SubAgent(SubAgentSource::Compact),
        "subagent_memory_consolidation" => {
            SessionSource::SubAgent(SubAgentSource::MemoryConsolidation)
        }
        value => {
            if let Some(thread_spawn) = value.strip_prefix("subagent_thread_spawn_")
                && let Some((parent_thread_id, depth)) = thread_spawn.rsplit_once("_d")
            {
                let parent_thread_id = codex_protocol::ThreadId::from_string(parent_thread_id)
                    .map_err(|error| invalid(field, error))?;
                let depth = depth.parse().map_err(|error| invalid(field, error))?;
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })
            } else if let Some(other) = value.strip_prefix("subagent_") {
                SessionSource::SubAgent(SubAgentSource::Other(other.to_owned()))
            } else {
                SessionSource::Custom(value.to_owned())
            }
        }
    };
    Ok(source)
}

fn decode_network_policy_rule_action(
    value: String,
    field: &'static str,
) -> Result<codex_protocol::approvals::NetworkPolicyRuleAction, Status> {
    match value.as_str() {
        "allow" => Ok(codex_protocol::approvals::NetworkPolicyRuleAction::Allow),
        "deny" => Ok(codex_protocol::approvals::NetworkPolicyRuleAction::Deny),
        value => Err(invalid(field, format!("unknown value `{value}`"))),
    }
}

fn encode_network_policy_rule_action(
    value: codex_protocol::approvals::NetworkPolicyRuleAction,
) -> String {
    match value {
        codex_protocol::approvals::NetworkPolicyRuleAction::Allow => "allow".to_owned(),
        codex_protocol::approvals::NetworkPolicyRuleAction::Deny => "deny".to_owned(),
    }
}

impl DirectProtoString for codex_app_server_protocol::AuthMode {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        match value.as_str() {
            "apikey" | "apiKey" => Ok(Self::ApiKey),
            "chatgpt" => Ok(Self::Chatgpt),
            "chatgptAuthTokens" => Ok(Self::ChatgptAuthTokens),
            "agentIdentity" => Ok(Self::AgentIdentity),
            value => Err(invalid(field, format!("unknown value `{value}`"))),
        }
    }

    fn encode_string(self) -> String {
        match self {
            Self::ApiKey => "apikey",
            Self::Chatgpt => "chatgpt",
            Self::ChatgptAuthTokens => "chatgptAuthTokens",
            Self::AgentIdentity => "agentIdentity",
        }
        .to_owned()
    }
}

impl DirectSchemaProto<proto::V2AuthMode> for codex_app_server_protocol::AuthMode {
    fn decode_schema(payload: proto::V2AuthMode) -> Result<Self, Status> {
        Self::decode_string(payload.value, "AuthMode")
    }

    fn encode_schema(self) -> Result<proto::V2AuthMode, Status> {
        Ok(proto::V2AuthMode {
            value: self.encode_string(),
        })
    }
}

impl DirectProtoString for codex_protocol::ThreadId {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        Self::from_string(&value).map_err(|error| invalid(field, error))
    }

    fn encode_string(self) -> String {
        self.to_string()
    }
}

impl DirectSchemaProto<proto::V2ThreadId> for codex_protocol::ThreadId {
    fn decode_schema(payload: proto::V2ThreadId) -> Result<Self, Status> {
        Self::decode_string(payload.value, "ThreadId")
    }

    fn encode_schema(self) -> Result<proto::V2ThreadId, Status> {
        Ok(proto::V2ThreadId {
            value: self.encode_string(),
        })
    }
}

impl DirectSchemaProto<u64> for std::num::NonZeroUsize {
    fn decode_schema(payload: u64) -> Result<Self, Status> {
        let value = usize::try_from(payload).map_err(|error| invalid("NonZeroUsize", error))?;
        Self::new(value).ok_or_else(|| invalid("NonZeroUsize", "value must be non-zero"))
    }

    fn encode_schema(self) -> Result<u64, Status> {
        u64::try_from(self.get()).map_err(|error| encode_error("NonZeroUsize", error))
    }
}

impl DirectSchemaProto<proto::LegacyGetConversationSummaryParams>
    for codex_app_server_protocol::GetConversationSummaryParams
{
    fn decode_schema(payload: proto::LegacyGetConversationSummaryParams) -> Result<Self, Status> {
        match (payload.rollout_path, payload.conversation_id) {
            (Some(rollout_path), None) => Ok(Self::RolloutPath {
                rollout_path: rollout_path.into(),
            }),
            (None, Some(conversation_id)) => Ok(Self::ThreadId {
                conversation_id: codex_protocol::ThreadId::from_string(&conversation_id).map_err(
                    |error| invalid("GetConversationSummaryParams.conversationId", error),
                )?,
            }),
            (None, None) => Err(missing(
                "GetConversationSummaryParams.rolloutPath or conversationId",
            )),
            (Some(_), Some(_)) => Err(invalid(
                "GetConversationSummaryParams",
                "rolloutPath and conversationId are mutually exclusive",
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::LegacyGetConversationSummaryParams, Status> {
        match self {
            Self::RolloutPath { rollout_path } => {
                let rollout_path = rollout_path.into_os_string().into_string().map_err(|_| {
                    encode_error(
                        "GetConversationSummaryParams.rolloutPath",
                        "path is not valid UTF-8",
                    )
                })?;
                Ok(proto::LegacyGetConversationSummaryParams {
                    rollout_path: Some(rollout_path),
                    conversation_id: None,
                })
            }
            Self::ThreadId { conversation_id } => Ok(proto::LegacyGetConversationSummaryParams {
                rollout_path: None,
                conversation_id: Some(conversation_id.to_string()),
            }),
        }
    }
}

impl DirectSchemaProto<proto::LegacyGetConversationSummaryResponseSummaryGitInfo>
    for codex_app_server_protocol::ConversationGitInfo
{
    fn decode_schema(
        payload: proto::LegacyGetConversationSummaryResponseSummaryGitInfo,
    ) -> Result<Self, Status> {
        Ok(Self {
            sha: payload.sha,
            branch: payload.branch,
            origin_url: payload.origin_url,
        })
    }

    fn encode_schema(
        self,
    ) -> Result<proto::LegacyGetConversationSummaryResponseSummaryGitInfo, Status> {
        Ok(proto::LegacyGetConversationSummaryResponseSummaryGitInfo {
            sha: self.sha,
            branch: self.branch,
            origin_url: self.origin_url,
        })
    }
}

impl DirectSchemaProto<proto::LegacyGetConversationSummaryResponseSummary>
    for codex_app_server_protocol::ConversationSummary
{
    fn decode_schema(
        payload: proto::LegacyGetConversationSummaryResponseSummary,
    ) -> Result<Self, Status> {
        Ok(Self {
            conversation_id: codex_protocol::ThreadId::from_string(&payload.conversation_id)
                .map_err(|error| invalid("ConversationSummary.conversationId", error))?,
            path: payload.path.into(),
            preview: payload.preview,
            timestamp: payload.timestamp,
            updated_at: payload.updated_at,
            model_provider: payload.model_provider,
            cwd: payload.cwd.into(),
            cli_version: payload.cli_version,
            source: decode_session_source(payload.source, "ConversationSummary.source")?,
            git_info: payload
                .git_info
                .map(
                    <codex_app_server_protocol::ConversationGitInfo as DirectSchemaProto<
                        proto::LegacyGetConversationSummaryResponseSummaryGitInfo,
                    >>::decode_schema,
                )
                .transpose()?,
        })
    }

    fn encode_schema(self) -> Result<proto::LegacyGetConversationSummaryResponseSummary, Status> {
        let path = self
            .path
            .into_os_string()
            .into_string()
            .map_err(|_| encode_error("ConversationSummary.path", "path is not valid UTF-8"))?;
        let cwd = self
            .cwd
            .into_os_string()
            .into_string()
            .map_err(|_| encode_error("ConversationSummary.cwd", "path is not valid UTF-8"))?;
        Ok(proto::LegacyGetConversationSummaryResponseSummary {
            conversation_id: self.conversation_id.to_string(),
            path,
            preview: self.preview,
            timestamp: self.timestamp,
            updated_at: self.updated_at,
            model_provider: self.model_provider,
            cwd,
            cli_version: self.cli_version,
            source: self.source.to_string(),
            git_info: self
                .git_info
                .map(
                    <codex_app_server_protocol::ConversationGitInfo as DirectSchemaProto<
                        proto::LegacyGetConversationSummaryResponseSummaryGitInfo,
                    >>::encode_schema,
                )
                .transpose()?,
        })
    }
}

impl DirectSchemaProto<proto::LegacyGetConversationSummaryResponse>
    for codex_app_server_protocol::GetConversationSummaryResponse
{
    fn decode_schema(payload: proto::LegacyGetConversationSummaryResponse) -> Result<Self, Status> {
        Ok(Self {
            summary: DirectSchemaProto::decode_schema(
                payload
                    .summary
                    .ok_or_else(|| missing("GetConversationSummaryResponse.summary"))?,
            )?,
        })
    }

    fn encode_schema(self) -> Result<proto::LegacyGetConversationSummaryResponse, Status> {
        Ok(proto::LegacyGetConversationSummaryResponse {
            summary: Some(DirectSchemaProto::encode_schema(self.summary)?),
        })
    }
}

impl DirectSchemaProto<Vec<String>> for codex_protocol::approvals::ExecPolicyAmendment {
    fn decode_schema(payload: Vec<String>) -> Result<Self, Status> {
        Ok(Self::new(payload))
    }

    fn encode_schema(self) -> Result<Vec<String>, Status> {
        Ok(self.command)
    }
}

impl DirectSchemaProto<Vec<String>> for codex_app_server_protocol::ExecPolicyAmendment {
    fn decode_schema(payload: Vec<String>) -> Result<Self, Status> {
        Ok(Self { command: payload })
    }

    fn encode_schema(self) -> Result<Vec<String>, Status> {
        Ok(self.command)
    }
}

impl DirectSchemaProto<proto::LegacyNetworkPolicyRuleAction>
    for codex_protocol::approvals::NetworkPolicyRuleAction
{
    fn decode_schema(payload: proto::LegacyNetworkPolicyRuleAction) -> Result<Self, Status> {
        decode_network_policy_rule_action(payload.value, "NetworkPolicyRuleAction")
    }

    fn encode_schema(self) -> Result<proto::LegacyNetworkPolicyRuleAction, Status> {
        Ok(proto::LegacyNetworkPolicyRuleAction {
            value: encode_network_policy_rule_action(self),
        })
    }
}

impl DirectSchemaProto<proto::LegacyNetworkPolicyAmendment>
    for codex_protocol::approvals::NetworkPolicyAmendment
{
    fn decode_schema(payload: proto::LegacyNetworkPolicyAmendment) -> Result<Self, Status> {
        Ok(Self {
            host: payload.host,
            action: DirectSchemaProto::decode_schema(
                payload
                    .action
                    .ok_or_else(|| missing("NetworkPolicyAmendment.action"))?,
            )?,
        })
    }

    fn encode_schema(self) -> Result<proto::LegacyNetworkPolicyAmendment, Status> {
        Ok(proto::LegacyNetworkPolicyAmendment {
            action: Some(DirectSchemaProto::encode_schema(self.action)?),
            host: self.host,
        })
    }
}

impl DirectSchemaProto<proto::V2NetworkPolicyRuleAction>
    for codex_app_server_protocol::NetworkPolicyRuleAction
{
    fn decode_schema(payload: proto::V2NetworkPolicyRuleAction) -> Result<Self, Status> {
        match decode_network_policy_rule_action(payload.value, "NetworkPolicyRuleAction")? {
            codex_protocol::approvals::NetworkPolicyRuleAction::Allow => Ok(Self::Allow),
            codex_protocol::approvals::NetworkPolicyRuleAction::Deny => Ok(Self::Deny),
        }
    }

    fn encode_schema(self) -> Result<proto::V2NetworkPolicyRuleAction, Status> {
        let action = match self {
            Self::Allow => codex_protocol::approvals::NetworkPolicyRuleAction::Allow,
            Self::Deny => codex_protocol::approvals::NetworkPolicyRuleAction::Deny,
        };
        Ok(proto::V2NetworkPolicyRuleAction {
            value: encode_network_policy_rule_action(action),
        })
    }
}

impl DirectSchemaProto<proto::V2NetworkPolicyAmendment>
    for codex_app_server_protocol::NetworkPolicyAmendment
{
    fn decode_schema(payload: proto::V2NetworkPolicyAmendment) -> Result<Self, Status> {
        Ok(Self {
            host: payload.host,
            action: DirectSchemaProto::decode_schema(
                payload
                    .action
                    .ok_or_else(|| missing("NetworkPolicyAmendment.action"))?,
            )?,
        })
    }

    fn encode_schema(self) -> Result<proto::V2NetworkPolicyAmendment, Status> {
        Ok(proto::V2NetworkPolicyAmendment {
            action: Some(DirectSchemaProto::encode_schema(self.action)?),
            host: self.host,
        })
    }
}

impl DirectSchemaProto<proto::V2PlanType> for codex_protocol::account::PlanType {
    fn decode_schema(payload: proto::V2PlanType) -> Result<Self, Status> {
        use codex_protocol::account::PlanType;

        Ok(match payload.value.as_str() {
            "free" => PlanType::Free,
            "go" => PlanType::Go,
            "plus" => PlanType::Plus,
            "pro" => PlanType::Pro,
            "prolite" | "pro_lite" => PlanType::ProLite,
            "team" => PlanType::Team,
            "self_serve_business_usage_based" => PlanType::SelfServeBusinessUsageBased,
            "business" => PlanType::Business,
            "enterprise_cbp_usage_based" => PlanType::EnterpriseCbpUsageBased,
            "enterprise" | "hc" => PlanType::Enterprise,
            "edu" | "education" => PlanType::Edu,
            _ => PlanType::Unknown,
        })
    }

    fn encode_schema(self) -> Result<proto::V2PlanType, Status> {
        use codex_protocol::account::PlanType;

        let value = match self {
            PlanType::Free => "free",
            PlanType::Go => "go",
            PlanType::Plus => "plus",
            PlanType::Pro => "pro",
            PlanType::ProLite => "prolite",
            PlanType::Team => "team",
            PlanType::SelfServeBusinessUsageBased => "self_serve_business_usage_based",
            PlanType::Business => "business",
            PlanType::EnterpriseCbpUsageBased => "enterprise_cbp_usage_based",
            PlanType::Enterprise => "enterprise",
            PlanType::Edu => "edu",
            PlanType::Unknown => "unknown",
        };
        Ok(proto::V2PlanType {
            value: value.to_owned(),
        })
    }
}

impl DirectSchemaProto<proto::V2Account> for codex_app_server_protocol::Account {
    fn decode_schema(payload: proto::V2Account) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "apiKey" => Ok(Self::ApiKey {}),
            "chatgpt" => Ok(Self::Chatgpt {
                email: payload.email.ok_or_else(|| missing("Account.email"))?,
                plan_type: DirectSchemaProto::decode_schema(
                    payload
                        .plan_type
                        .ok_or_else(|| missing("Account.planType"))?,
                )?,
            }),
            "amazonBedrock" => Ok(Self::AmazonBedrock {}),
            value => Err(invalid("Account.type", format!("unknown value `{value}`"))),
        }
    }

    fn encode_schema(self) -> Result<proto::V2Account, Status> {
        match self {
            Self::ApiKey {} => Ok(proto::V2Account {
                r#type: "apiKey".to_owned(),
                email: None,
                plan_type: None,
            }),
            Self::Chatgpt { email, plan_type } => Ok(proto::V2Account {
                r#type: "chatgpt".to_owned(),
                email: Some(email),
                plan_type: Some(DirectSchemaProto::encode_schema(plan_type)?),
            }),
            Self::AmazonBedrock {} => Ok(proto::V2Account {
                r#type: "amazonBedrock".to_owned(),
                email: None,
                plan_type: None,
            }),
        }
    }
}

impl DirectSchemaProto<proto::V2ApprovalsReviewer>
    for codex_app_server_protocol::ApprovalsReviewer
{
    fn decode_schema(payload: proto::V2ApprovalsReviewer) -> Result<Self, Status> {
        match payload.value.as_str() {
            "user" => Ok(Self::User),
            "auto_review" | "guardian_subagent" => Ok(Self::AutoReview),
            value => Err(invalid(
                "ApprovalsReviewer",
                format!("unknown value `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ApprovalsReviewer, Status> {
        let value = match self {
            Self::User => "user",
            Self::AutoReview => "guardian_subagent",
        };
        Ok(proto::V2ApprovalsReviewer {
            value: value.to_owned(),
        })
    }
}

impl DirectSchemaProto<proto::V2CommandExecutionSource>
    for codex_app_server_protocol::CommandExecutionSource
{
    fn decode_schema(payload: proto::V2CommandExecutionSource) -> Result<Self, Status> {
        match payload.value.as_str() {
            "agent" => Ok(Self::Agent),
            "userShell" => Ok(Self::UserShell),
            "unifiedExecStartup" => Ok(Self::UnifiedExecStartup),
            "unifiedExecInteraction" => Ok(Self::UnifiedExecInteraction),
            value => Err(invalid(
                "CommandExecutionSource",
                format!("unknown value `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2CommandExecutionSource, Status> {
        let value = match self {
            Self::Agent => "agent",
            Self::UserShell => "userShell",
            Self::UnifiedExecStartup => "unifiedExecStartup",
            Self::UnifiedExecInteraction => "unifiedExecInteraction",
        };
        Ok(proto::V2CommandExecutionSource {
            value: value.to_owned(),
        })
    }
}

impl DirectProtoString for codex_app_server_protocol::HookSource {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        match value.as_str() {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            "mdm" => Ok(Self::Mdm),
            "sessionFlags" => Ok(Self::SessionFlags),
            "plugin" => Ok(Self::Plugin),
            "cloudRequirements" => Ok(Self::CloudRequirements),
            "cloudManagedConfig" => Ok(Self::CloudManagedConfig),
            "legacyManagedConfigFile" => Ok(Self::LegacyManagedConfigFile),
            "legacyManagedConfigMdm" => Ok(Self::LegacyManagedConfigMdm),
            "unknown" => Ok(Self::Unknown),
            value => Err(invalid(field, format!("unknown value `{value}`"))),
        }
    }

    fn encode_string(self) -> String {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Project => "project",
            Self::Mdm => "mdm",
            Self::SessionFlags => "sessionFlags",
            Self::Plugin => "plugin",
            Self::CloudRequirements => "cloudRequirements",
            Self::CloudManagedConfig => "cloudManagedConfig",
            Self::LegacyManagedConfigFile => "legacyManagedConfigFile",
            Self::LegacyManagedConfigMdm => "legacyManagedConfigMdm",
            Self::Unknown => "unknown",
        }
        .to_owned()
    }
}

impl DirectSchemaProto<proto::V2HookSource> for codex_app_server_protocol::HookSource {
    fn decode_schema(payload: proto::V2HookSource) -> Result<Self, Status> {
        Self::decode_string(payload.value, "HookSource")
    }

    fn encode_schema(self) -> Result<proto::V2HookSource, Status> {
        Ok(proto::V2HookSource {
            value: self.encode_string(),
        })
    }
}

impl DirectProtoString for codex_app_server_protocol::PermissionGrantScope {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        match value.as_str() {
            "turn" => Ok(Self::Turn),
            "session" => Ok(Self::Session),
            value => Err(invalid(field, format!("unknown value `{value}`"))),
        }
    }

    fn encode_string(self) -> String {
        match self {
            Self::Turn => "turn",
            Self::Session => "session",
        }
        .to_owned()
    }
}

impl DirectSchemaProto<proto::V2PermissionGrantScope>
    for codex_app_server_protocol::PermissionGrantScope
{
    fn decode_schema(payload: proto::V2PermissionGrantScope) -> Result<Self, Status> {
        Self::decode_string(payload.value, "PermissionGrantScope")
    }

    fn encode_schema(self) -> Result<proto::V2PermissionGrantScope, Status> {
        Ok(proto::V2PermissionGrantScope {
            value: self.encode_string(),
        })
    }
}

impl DirectProtoString for codex_app_server_protocol::PluginAvailability {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        match value.as_str() {
            "AVAILABLE" | "ENABLED" => Ok(Self::Available),
            "DISABLED_BY_ADMIN" => Ok(Self::DisabledByAdmin),
            value => Err(invalid(field, format!("unknown value `{value}`"))),
        }
    }

    fn encode_string(self) -> String {
        match self {
            Self::Available => "AVAILABLE",
            Self::DisabledByAdmin => "DISABLED_BY_ADMIN",
        }
        .to_owned()
    }
}

impl DirectSchemaProto<proto::V2PluginAvailability>
    for codex_app_server_protocol::PluginAvailability
{
    fn decode_schema(payload: proto::V2PluginAvailability) -> Result<Self, Status> {
        Self::decode_string(payload.value, "PluginAvailability")
    }

    fn encode_schema(self) -> Result<proto::V2PluginAvailability, Status> {
        Ok(proto::V2PluginAvailability {
            value: self.encode_string(),
        })
    }
}

impl DirectProtoString for codex_app_server_protocol::TurnItemsView {
    fn decode_string(value: String, field: &'static str) -> Result<Self, Status> {
        match value.as_str() {
            "notLoaded" => Ok(Self::NotLoaded),
            "summary" => Ok(Self::Summary),
            "full" => Ok(Self::Full),
            value => Err(invalid(field, format!("unknown value `{value}`"))),
        }
    }

    fn encode_string(self) -> String {
        match self {
            Self::NotLoaded => "notLoaded",
            Self::Summary => "summary",
            Self::Full => "full",
        }
        .to_owned()
    }
}

impl DirectSchemaProto<proto::V2TurnItemsView> for codex_app_server_protocol::TurnItemsView {
    fn decode_schema(payload: proto::V2TurnItemsView) -> Result<Self, Status> {
        Self::decode_string(payload.value, "TurnItemsView")
    }

    fn encode_schema(self) -> Result<proto::V2TurnItemsView, Status> {
        Ok(proto::V2TurnItemsView {
            value: self.encode_string(),
        })
    }
}
