use crate::tools::sandboxing::ToolCtx;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_login::CodexAuth;
use codex_login::default_client::originator;
use codex_network_proxy::NetworkProxyAuditMetadata;
use codex_rollout::state_db::StateDbHandle;
use codex_sandboxing::SandboxViolationEvent;
use codex_state::SecurityEventCreateParams;
use codex_state::SecurityEventKind;
use codex_state::SecurityEventResource;
use codex_terminal_detection::user_agent;

const AUDIT_TARGET: &str = "codex_otel.sandbox_violation";
const SANDBOX_VIOLATION_EVENT_NAME: &str = "codex.sandbox.violation";

#[derive(Clone)]
pub(crate) struct SandboxViolationAuditContext {
    pub(crate) state_db: Option<StateDbHandle>,
    pub(crate) metadata: NetworkProxyAuditMetadata,
    pub(crate) turn_id: Option<String>,
    pub(crate) call_id: Option<String>,
    pub(crate) tool_name: Option<String>,
}

impl std::fmt::Debug for SandboxViolationAuditContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxViolationAuditContext")
            .field("has_state_db", &self.state_db.is_some())
            .field("metadata", &self.metadata)
            .field("turn_id", &self.turn_id)
            .field("call_id", &self.call_id)
            .field("tool_name", &self.tool_name)
            .finish()
    }
}

impl SandboxViolationAuditContext {
    pub(crate) fn from_tool_ctx(ctx: &ToolCtx) -> Self {
        let auth = ctx.session.services.auth_manager.auth_cached();
        let auth_mode = ctx
            .session
            .services
            .auth_manager
            .auth_mode()
            .map(|mode| mode.to_string());
        let metadata = NetworkProxyAuditMetadata {
            conversation_id: Some(ctx.session.conversation_id.to_string()),
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            user_account_id: auth.as_ref().and_then(CodexAuth::get_account_id),
            auth_mode,
            originator: Some(originator().value),
            user_email: auth.as_ref().and_then(CodexAuth::get_account_email),
            terminal_type: Some(user_agent()),
            model: Some(ctx.turn.model_info.slug.clone()),
            slug: Some(ctx.turn.model_info.slug.clone()),
        };
        Self {
            state_db: ctx.session.services.state_db.clone(),
            metadata,
            turn_id: Some(ctx.turn.sub_id.clone()),
            call_id: Some(ctx.call_id.clone()),
            tool_name: Some(ctx.tool_name.clone()),
        }
    }

    pub(crate) fn from_network_proxy(
        state_db: Option<StateDbHandle>,
        metadata: NetworkProxyAuditMetadata,
    ) -> Self {
        Self {
            state_db,
            metadata,
            turn_id: None,
            call_id: None,
            tool_name: None,
        }
    }
}

pub(crate) async fn record_sandbox_violation_audit(
    context: Option<&SandboxViolationAuditContext>,
    event: &SandboxViolationEvent,
) {
    emit_sandbox_violation_audit_event(context, event);

    let Some(context) = context else {
        return;
    };
    let Some(state_db) = context.state_db.as_deref() else {
        return;
    };
    let event = security_event_create_params(context, event);
    if let Err(err) = state_db.insert_security_event(&event).await {
        tracing::warn!("failed to persist sandbox violation security event: {err}");
    }
}

fn security_event_create_params(
    context: &SandboxViolationAuditContext,
    event: &SandboxViolationEvent,
) -> SecurityEventCreateParams {
    match event {
        SandboxViolationEvent::FileSystem(violation) => SecurityEventCreateParams {
            created_at: Utc::now().timestamp(),
            kind: SecurityEventKind::SandboxViolation,
            thread_id: context.metadata.conversation_id.clone(),
            turn_id: context.turn_id.clone(),
            call_id: context.call_id.clone(),
            tool_name: context.tool_name.clone(),
            resource: Some(SecurityEventResource::FileSystem),
            sandbox_type: Some(violation.sandbox_type.as_metric_tag().to_string()),
            reason: Some(violation.reason.as_str().to_string()),
            path: violation.path.clone(),
            host: None,
            port: None,
            protocol: None,
            method: None,
            decision: None,
            source: None,
            review_id: None,
            reviewer: None,
            review_decision: None,
            details_json: None,
        },
        SandboxViolationEvent::Network(violation) => SecurityEventCreateParams {
            created_at: violation.timestamp,
            kind: SecurityEventKind::SandboxViolation,
            thread_id: context.metadata.conversation_id.clone(),
            turn_id: context.turn_id.clone(),
            call_id: context.call_id.clone(),
            tool_name: context.tool_name.clone(),
            resource: Some(SecurityEventResource::Network),
            sandbox_type: None,
            reason: Some(violation.reason.clone()),
            path: None,
            host: Some(violation.host.clone()),
            port: violation.port,
            protocol: Some(violation.protocol.clone()),
            method: violation.method.clone(),
            decision: violation.decision.clone(),
            source: violation.source.clone(),
            review_id: None,
            reviewer: None,
            review_decision: None,
            details_json: None,
        },
    }
}

fn emit_sandbox_violation_audit_event(
    context: Option<&SandboxViolationAuditContext>,
    event: &SandboxViolationEvent,
) {
    let metadata = context.map(|context| &context.metadata);
    let turn_id = context.and_then(|context| context.turn_id.as_deref());
    let call_id = context.and_then(|context| context.call_id.as_deref());
    let tool_name = context.and_then(|context| context.tool_name.as_deref());
    let fields = SandboxViolationAuditFields::new(event);

    tracing::event!(
        target: AUDIT_TARGET,
        tracing::Level::INFO,
        event.name = SANDBOX_VIOLATION_EVENT_NAME,
        event.timestamp = %audit_timestamp(),
        conversation.id = metadata.and_then(|metadata| metadata.conversation_id.as_deref()),
        app.version = metadata.and_then(|metadata| metadata.app_version.as_deref()),
        auth_mode = metadata.and_then(|metadata| metadata.auth_mode.as_deref()),
        originator = metadata.and_then(|metadata| metadata.originator.as_deref()),
        user.account_id = metadata.and_then(|metadata| metadata.user_account_id.as_deref()),
        user.email = metadata.and_then(|metadata| metadata.user_email.as_deref()),
        terminal.type = metadata.and_then(|metadata| metadata.terminal_type.as_deref()),
        model = metadata.and_then(|metadata| metadata.model.as_deref()),
        slug = metadata.and_then(|metadata| metadata.slug.as_deref()),
        security.event.kind = SecurityEventKind::SandboxViolation.as_str(),
        sandbox.resource = fields.resource.as_str(),
        sandbox.type = fields.sandbox_type,
        sandbox.reason = fields.reason,
        tool.call_id = call_id,
        tool.name = tool_name,
        turn.id = turn_id,
        file.path = fields.path,
        server.address = fields.host,
        server.port = fields.port,
        network.transport.protocol = fields.protocol,
        http.request.method = fields.method,
        network.policy.decision = fields.decision,
        network.policy.source = fields.source,
    );
}

struct SandboxViolationAuditFields<'a> {
    resource: SecurityEventResource,
    sandbox_type: Option<&'a str>,
    reason: &'a str,
    path: Option<&'a str>,
    host: Option<&'a str>,
    port: Option<i64>,
    protocol: Option<&'a str>,
    method: Option<&'a str>,
    decision: Option<&'a str>,
    source: Option<&'a str>,
}

impl<'a> SandboxViolationAuditFields<'a> {
    fn new(event: &'a SandboxViolationEvent) -> Self {
        match event {
            SandboxViolationEvent::FileSystem(violation) => Self {
                resource: SecurityEventResource::FileSystem,
                sandbox_type: Some(violation.sandbox_type.as_metric_tag()),
                reason: violation.reason.as_str(),
                path: violation.path.as_deref(),
                host: None,
                port: None,
                protocol: None,
                method: None,
                decision: None,
                source: None,
            },
            SandboxViolationEvent::Network(violation) => Self {
                resource: SecurityEventResource::Network,
                sandbox_type: None,
                reason: violation.reason.as_str(),
                path: None,
                host: Some(violation.host.as_str()),
                port: violation.port.map(i64::from),
                protocol: Some(violation.protocol.as_str()),
                method: violation.method.as_deref(),
                decision: violation.decision.as_deref(),
                source: violation.source.as_deref(),
            },
        }
    }
}

fn audit_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
