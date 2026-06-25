use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_config::types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::McpToolSource;
use sha2::Digest as _;
use sha2::Sha256;

mod elicitation;
pub use elicitation::McpElicitationRuntimeMetadata;

/// Non-serializable metadata attached to one effective MCP registration.
///
/// This describes the runtime source of a registration rather than user configuration. Server-
/// wide metadata lives beside tool-specific runtime metadata so source-specific behavior does not
/// become part of serializable MCP configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpServerRuntimeMetadata {
    pub(crate) telemetry_origin: Option<String>,
    pub(crate) plugin_display_names: Vec<String>,
    pub(crate) suppress_physical_tools_list_metric: bool,
    pub(crate) tools: HashMap<String, McpToolRuntimeMetadata>,
    pub(crate) trusts_tool_input: bool,
    pub(crate) trusts_approval_context: bool,
    pub(crate) sandbox_state_source: McpSandboxStateSource,
    pub(crate) approvals_reviewer: Option<ApprovalsReviewer>,
}

/// Selects which environment Codex describes in MCP sandbox-state metadata.
///
/// The default follows the server's configured execution environment. Runtime HTTP servers can
/// instead stay local while receiving file-system context for the primary environment selected
/// for the current turn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum McpSandboxStateSource {
    #[default]
    ServerEnvironment,
    PrimaryTurnEnvironment,
}

impl McpServerRuntimeMetadata {
    /// Attributes telemetry to a stable HTTP origin instead of the physical transport endpoint.
    ///
    /// Runtime-owned HTTP proxies can use this to keep ephemeral loopback ports out of metrics and
    /// traces. The supplied URL is reduced to its origin; invalid URLs leave the transport-derived
    /// origin unchanged.
    pub fn with_telemetry_origin(mut self, url: impl AsRef<str>) -> Self {
        self.telemetry_origin = url::Url::parse(url.as_ref())
            .ok()
            .map(|url| url.origin().ascii_serialization());
        self
    }

    /// Records every plugin package that contributes this runtime server.
    pub fn with_plugin_display_names(
        mut self,
        plugin_display_names: impl IntoIterator<Item = String>,
    ) -> Self {
        self.plugin_display_names = plugin_display_names
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        self.plugin_display_names.sort_unstable();
        self.plugin_display_names.dedup();
        self
    }

    pub fn plugin_display_names(&self) -> &[String] {
        &self.plugin_display_names
    }

    /// Suppresses latency telemetry for this registration's physical `tools/list` hop.
    ///
    /// Transparent proxies can use this when their owner records the logical upstream inventory
    /// operation separately. Ordinary configured servers keep the metric enabled by default.
    pub fn without_physical_tools_list_metric(mut self) -> Self {
        self.suppress_physical_tools_list_metric = true;
        self
    }

    pub fn records_physical_tools_list_metric(&self) -> bool {
        !self.suppress_physical_tools_list_metric
    }

    /// Records non-serializable metadata for raw MCP tool names exposed by this server.
    pub fn with_tools(mut self, tools: HashMap<String, McpToolRuntimeMetadata>) -> Self {
        self.tools = tools;
        self
    }

    pub fn tool(&self, name: &str) -> Option<&McpToolRuntimeMetadata> {
        self.tools.get(name)
    }

    pub fn with_tool(mut self, name: impl Into<String>, metadata: McpToolRuntimeMetadata) -> Self {
        self.tools.insert(name.into(), metadata);
        self
    }

    /// Allows this trusted registration owner to replace the recorded tool input from result
    /// metadata when the server also negotiates the matching protocol capability.
    pub fn with_trusted_tool_input(mut self) -> Self {
        self.trusts_tool_input = true;
        self
    }

    /// Allows this trusted registration owner to provide private approval-review context in
    /// listed tool metadata. This metadata is never trusted for configured MCP servers by default.
    pub fn with_trusted_approval_context(mut self) -> Self {
        self.trusts_approval_context = true;
        self
    }

    /// Uses the primary environment selected for the current turn when Codex sends sandbox-state
    /// metadata to this server.
    pub fn with_primary_turn_sandbox_state(mut self) -> Self {
        self.sandbox_state_source = McpSandboxStateSource::PrimaryTurnEnvironment;
        self
    }

    /// Overrides the approval reviewer for calls and elicitations from this server.
    pub fn with_approvals_reviewer(mut self, approvals_reviewer: ApprovalsReviewer) -> Self {
        self.approvals_reviewer = Some(approvals_reviewer);
        self
    }

    pub fn approvals_reviewer(&self) -> Option<ApprovalsReviewer> {
        self.approvals_reviewer
    }
}

/// Runtime-only behavior supplied by the trusted owner of one MCP tool.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpToolRuntimeMetadata {
    approval_identity: Option<McpToolApprovalIdentity>,
    approval_presentation: Option<McpToolApprovalPresentation>,
    approval_header: Option<String>,
    approval_form_metadata: serde_json::Map<String, serde_json::Value>,
    approval_persistence: Option<McpToolApprovalPersistence>,
    approval_source: Option<McpToolSource>,
    metric_labels: Vec<(String, String)>,
    search_aliases: Vec<String>,
    telemetry_identity: Option<McpToolTelemetryIdentity>,
}

const MAX_MCP_TOOL_METRIC_LABELS: usize = 8;
const MAX_MCP_TOOL_METRIC_LABEL_KEY_CHARS: usize = 64;
const MAX_MCP_TOOL_METRIC_LABEL_VALUE_CHARS: usize = 256;
const MAX_MCP_TOOL_RUNTIME_IDENTITY_BYTES: usize = 256;

impl McpToolRuntimeMetadata {
    /// Overrides the routed server and tool names used for session approval identity.
    pub fn with_approval_identity(mut self, identity: McpToolApprovalIdentity) -> Self {
        self.approval_identity = Some(identity);
        self
    }

    pub fn with_approval_presentation(
        mut self,
        approval_presentation: McpToolApprovalPresentation,
    ) -> Self {
        self.approval_presentation = Some(approval_presentation);
        self
    }

    pub fn with_approval_persistence(
        mut self,
        approval_persistence: McpToolApprovalPersistence,
    ) -> Self {
        self.approval_persistence = Some(approval_persistence);
        self
    }

    /// Adds a trusted source identity for Guardian review and telemetry.
    pub fn with_approval_source(mut self, approval_source: McpToolSource) -> Self {
        self.approval_source = Some(approval_source);
        self
    }

    /// Adds bounded metric labels supplied by the trusted registration owner.
    ///
    /// Keys are restricted to the metric backend's portable identifier characters. Values remain
    /// opaque here and are sanitized by the telemetry sink before emission.
    pub fn with_metric_labels<K, V>(mut self, labels: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.metric_labels.clear();
        for (key, value) in labels {
            if self.metric_labels.len() == MAX_MCP_TOOL_METRIC_LABELS {
                break;
            }
            let key = key.into();
            let key = key.trim();
            if key.is_empty()
                || key.chars().count() > MAX_MCP_TOOL_METRIC_LABEL_KEY_CHARS
                || !key
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
                || self
                    .metric_labels
                    .iter()
                    .any(|(existing, _)| existing == key)
            {
                continue;
            }
            let value = value.into();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            self.metric_labels.push((
                key.to_string(),
                value
                    .chars()
                    .take(MAX_MCP_TOOL_METRIC_LABEL_VALUE_CHARS)
                    .collect(),
            ));
        }
        self.metric_labels
            .sort_unstable_by(|left, right| left.0.cmp(&right.0));
        self
    }

    /// Overrides the server and tool names used only for this tool's telemetry.
    pub fn with_telemetry_identity(mut self, identity: McpToolTelemetryIdentity) -> Self {
        self.telemetry_identity = Some(identity);
        self
    }

    /// Adds trusted names that should match this tool during deferred search.
    pub fn with_search_aliases(
        mut self,
        aliases: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.search_aliases = aliases
            .into_iter()
            .map(Into::into)
            .map(|alias: String| alias.trim().to_string())
            .filter(|alias| !alias.is_empty())
            .collect();
        self.search_aliases.sort_unstable();
        self.search_aliases.dedup();
        self
    }

    /// Overrides the generic header shown for this tool's approval prompt.
    pub fn with_approval_header(mut self, header: impl Into<String>) -> Self {
        let header = header.into();
        let header = header.trim();
        self.approval_header = (!header.is_empty()).then(|| header.to_string());
        self
    }

    /// Adds opaque fields to form-elicitation metadata for this tool's approval prompt.
    ///
    /// Codex-owned approval fields take precedence when the form is built.
    pub fn with_approval_form_metadata(
        mut self,
        metadata: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.approval_form_metadata = metadata;
        self
    }

    pub fn approval_presentation(&self) -> Option<&McpToolApprovalPresentation> {
        self.approval_presentation.as_ref()
    }

    pub fn approval_identity(&self) -> Option<&McpToolApprovalIdentity> {
        self.approval_identity.as_ref()
    }

    pub fn approval_header(&self) -> Option<&str> {
        self.approval_header.as_deref()
    }

    /// Returns opaque form-elicitation metadata supplied by the registration owner.
    pub fn approval_form_metadata(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.approval_form_metadata
    }

    pub fn approval_persistence(&self) -> Option<&McpToolApprovalPersistence> {
        self.approval_persistence.as_ref()
    }

    pub fn approval_source(&self) -> Option<&McpToolSource> {
        self.approval_source.as_ref()
    }

    pub fn metric_labels(&self) -> &[(String, String)] {
        &self.metric_labels
    }

    pub fn telemetry_identity(&self) -> Option<&McpToolTelemetryIdentity> {
        self.telemetry_identity.as_ref()
    }

    pub fn search_aliases(&self) -> &[String] {
        &self.search_aliases
    }
}

/// Stable approval identity supplied by the trusted owner of one MCP tool.
///
/// This identity affects session approval caching, including fallback after runtime-owned
/// persistence fails. It does not affect MCP registration, lookup, invocation routing, or generic
/// MCP configuration persistence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct McpToolApprovalIdentity {
    server_name: McpToolApprovalIdentityComponent,
    source_id: McpToolApprovalIdentityComponent,
    tool_name: McpToolApprovalIdentityComponent,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
enum McpToolApprovalIdentityComponent {
    Raw(String),
    Sha256(String),
}

impl McpToolApprovalIdentity {
    pub fn new(
        server_name: impl Into<String>,
        source_id: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Option<Self> {
        Some(Self {
            server_name: approval_identity_component(server_name)?,
            source_id: approval_identity_component(source_id)?,
            tool_name: approval_identity_component(tool_name)?,
        })
    }

    pub fn server_name(&self) -> &str {
        self.server_name.as_str()
    }

    pub fn source_id(&self) -> &str {
        self.source_id.as_str()
    }

    pub fn tool_name(&self) -> &str {
        self.tool_name.as_str()
    }
}

impl McpToolApprovalIdentityComponent {
    fn as_str(&self) -> &str {
        match self {
            Self::Raw(value) | Self::Sha256(value) => value,
        }
    }
}

/// Stable telemetry names supplied by the trusted owner of one MCP tool.
///
/// These names do not affect MCP registration, lookup, or invocation routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolTelemetryIdentity {
    server_name: String,
    tool_name: String,
}

impl McpToolTelemetryIdentity {
    pub fn new(server_name: impl Into<String>, tool_name: impl Into<String>) -> Option<Self> {
        Some(Self {
            server_name: bounded_runtime_identity_component(server_name)?,
            tool_name: bounded_runtime_identity_component(tool_name)?,
        })
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }
}

fn bounded_runtime_identity_component(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let value = value.trim();
    (!value.is_empty() && value.len() <= MAX_MCP_TOOL_RUNTIME_IDENTITY_BYTES)
        .then(|| value.to_string())
}

fn approval_identity_component(
    value: impl Into<String>,
) -> Option<McpToolApprovalIdentityComponent> {
    let value = value.into();
    if value.trim().is_empty() {
        return None;
    }
    if value.len() <= MAX_MCP_TOOL_RUNTIME_IDENTITY_BYTES {
        return Some(McpToolApprovalIdentityComponent::Raw(value));
    }
    Some(McpToolApprovalIdentityComponent::Sha256(format!(
        "sha256:{:x}",
        Sha256::digest(value.as_bytes())
    )))
}

/// Human-readable approval UI supplied by a trusted MCP registration owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolApprovalPresentation {
    question: String,
    parameter_labels: Vec<McpToolApprovalParameterLabel>,
}

impl McpToolApprovalPresentation {
    pub fn new(
        question: String,
        parameter_labels: Vec<McpToolApprovalParameterLabel>,
    ) -> Option<Self> {
        let question = question.trim();
        if question.is_empty() {
            return None;
        }
        Some(Self {
            question: question.to_string(),
            parameter_labels,
        })
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn parameter_labels(&self) -> &[McpToolApprovalParameterLabel] {
        &self.parameter_labels
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolApprovalParameterLabel {
    name: String,
    label: String,
}

impl McpToolApprovalParameterLabel {
    pub fn new(name: String, label: String) -> Option<Self> {
        let name = name.trim();
        let label = label.trim();
        if name.is_empty() || label.is_empty() {
            return None;
        }
        Some(Self {
            name: name.to_string(),
            label: label.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

type ApprovalPersistenceFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;
type ApprovalPersistenceFn = dyn Fn() -> ApprovalPersistenceFuture + Send + Sync;

/// Runtime-owned persistence for one MCP tool's durable approval decision.
///
/// The registration owner keeps schema-specific config mutation outside generic MCP and core
/// approval code. Equality is identity-based because the callback is process-local runtime state.
#[derive(Clone)]
pub struct McpToolApprovalPersistence(Arc<ApprovalPersistenceFn>);

impl McpToolApprovalPersistence {
    pub fn new<F, Fut>(persist: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        Self(Arc::new(move || Box::pin(persist())))
    }

    pub async fn persist(&self) -> anyhow::Result<()> {
        (self.0)().await
    }
}

impl fmt::Debug for McpToolApprovalPersistence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpToolApprovalPersistence([runtime callback])")
    }
}

impl PartialEq for McpToolApprovalPersistence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for McpToolApprovalPersistence {}
