use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::McpInvocation;
use rmcp::model::ToolAnnotations;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::guardian;
use crate::mcp_tool_call::McpToolApprovalMetadata;
use crate::mcp_tool_call::build_guardian_mcp_tool_review_request;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GuardianEvalCase {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub thread: Vec<GuardianEvalThreadItem>,
    #[serde(default)]
    pub config: GuardianEvalConfig,
    pub action: GuardianEvalAction,
    #[serde(default)]
    pub retry_reason: Option<String>,
    pub expected: GuardianEvalExpected,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GuardianEvalConfig {
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub guardian_policy_config: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardianEvalThreadItem {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        name: String,
        call_id: String,
        #[serde(default)]
        arguments: Value,
    },
    ToolResult {
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardianEvalAction {
    McpToolCall {
        call_id: String,
        server: String,
        tool: String,
        #[serde(default)]
        arguments: Option<Value>,
        #[serde(default)]
        metadata: Option<GuardianEvalMcpToolMetadata>,
    },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GuardianEvalMcpToolMetadata {
    #[serde(default)]
    pub connector_id: Option<String>,
    #[serde(default)]
    pub connector_name: Option<String>,
    #[serde(default)]
    pub connector_description: Option<String>,
    #[serde(default)]
    pub tool_title: Option<String>,
    #[serde(default)]
    pub tool_description: Option<String>,
    #[serde(default)]
    pub annotations: Option<GuardianEvalMcpAnnotations>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct GuardianEvalMcpAnnotations {
    #[serde(default)]
    pub destructive_hint: Option<bool>,
    #[serde(default)]
    pub open_world_hint: Option<bool>,
    #[serde(default)]
    pub read_only_hint: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GuardianEvalExpected {
    pub outcome: GuardianEvalOutcome,
    #[serde(default)]
    pub risk_level: Option<GuardianEvalRiskLevel>,
    #[serde(default)]
    pub user_authorization: Option<GuardianEvalUserAuthorization>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianEvalOutcome {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianEvalRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianEvalUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

pub(crate) fn load_guardian_eval_cases(path: &Path) -> Result<Vec<GuardianEvalCase>> {
    let mut paths = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        let mut paths = std::fs::read_dir(path)
            .with_context(|| format!("read eval cases directory {}", path.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| format!("read eval cases directory {}", path.display()))?;
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        paths
    };
    paths.sort();
    let mut seen = HashSet::new();
    let mut cases = Vec::new();
    for path in paths {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("read eval case {}", path.display()))?;
        let case: GuardianEvalCase = serde_json::from_str(&contents)
            .with_context(|| format!("parse eval case {}", path.display()))?;
        if !seen.insert(case.id.clone()) {
            anyhow::bail!("duplicate guardian eval case id {}", case.id);
        }
        cases.push(case);
    }
    Ok(cases)
}

pub(crate) fn select_cases(
    cases: Vec<GuardianEvalCase>,
    selected_ids: &[String],
) -> Result<Vec<GuardianEvalCase>> {
    if selected_ids.is_empty() {
        return Ok(cases);
    }
    let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
    let available = cases
        .iter()
        .map(|case| case.id.clone())
        .collect::<HashSet<_>>();
    let mut missing = selected.difference(&available).cloned().collect::<Vec<_>>();
    missing.sort();
    if !missing.is_empty() {
        anyhow::bail!("unknown guardian eval case id(s): {}", missing.join(", "));
    }
    Ok(cases
        .into_iter()
        .filter(|case| selected.contains(&case.id))
        .collect())
}

impl GuardianEvalThreadItem {
    pub(crate) fn to_response_item(&self) -> Result<ResponseItem> {
        match self {
            Self::User { text } => Ok(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: text.clone() }],
                phase: None,
            }),
            Self::Assistant { text } => Ok(ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText { text: text.clone() }],
                phase: None,
            }),
            Self::ToolCall {
                name,
                call_id,
                arguments,
            } => Ok(ResponseItem::FunctionCall {
                id: None,
                name: name.clone(),
                namespace: None,
                arguments: serde_json::to_string(arguments)
                    .context("serialize fixture tool call arguments")?,
                call_id: call_id.clone(),
            }),
            Self::ToolResult { call_id, output } => Ok(ResponseItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: FunctionCallOutputPayload::from_text(output.clone()),
            }),
        }
    }
}

impl GuardianEvalAction {
    pub(crate) fn to_guardian_request(&self) -> guardian::GuardianApprovalRequest {
        match self {
            Self::McpToolCall {
                call_id,
                server,
                tool,
                arguments,
                metadata,
            } => {
                let invocation = McpInvocation {
                    server: server.clone(),
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                };
                let metadata = metadata
                    .clone()
                    .map(GuardianEvalMcpToolMetadata::into_approval_metadata);
                build_guardian_mcp_tool_review_request(call_id, &invocation, metadata.as_ref())
            }
        }
    }
}

impl GuardianEvalMcpToolMetadata {
    fn into_approval_metadata(self) -> McpToolApprovalMetadata {
        let annotations = self.annotations.map(|annotations| {
            ToolAnnotations::from_raw(
                /*title*/ None,
                annotations.read_only_hint,
                annotations.destructive_hint,
                /*idempotent_hint*/ None,
                annotations.open_world_hint,
            )
        });
        McpToolApprovalMetadata::new_for_guardian_review(
            annotations,
            self.connector_id,
            self.connector_name,
            self.connector_description,
            self.tool_title,
            self.tool_description,
        )
    }
}
