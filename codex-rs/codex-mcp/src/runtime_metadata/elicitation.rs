use std::collections::HashMap;

use codex_config::types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::McpToolSource;

use super::McpServerRuntimeMetadata;
use super::McpToolRuntimeMetadata;

/// Immutable server metadata used to route one MCP elicitation review.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpElicitationRuntimeMetadata {
    approvals_reviewer: Option<ApprovalsReviewer>,
    tools: HashMap<String, McpToolElicitationMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct McpToolElicitationMetadata {
    approval_source: Option<McpToolSource>,
    search_aliases: Vec<String>,
}

impl McpElicitationRuntimeMetadata {
    pub fn approvals_reviewer(&self) -> Option<ApprovalsReviewer> {
        self.approvals_reviewer
    }

    /// Resolves the approval source for a raw tool name or one unambiguous trusted alias.
    pub fn approval_source_by_name_or_alias(&self, name: &str) -> Option<&McpToolSource> {
        if let Some(metadata) = self.tools.get(name) {
            return metadata.approval_source.as_ref();
        }
        let mut matches = self
            .tools
            .values()
            .filter(|metadata| metadata.search_aliases.iter().any(|alias| alias == name));
        let matched = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        matched.approval_source.as_ref()
    }
}

impl From<&McpServerRuntimeMetadata> for McpElicitationRuntimeMetadata {
    fn from(metadata: &McpServerRuntimeMetadata) -> Self {
        Self {
            approvals_reviewer: metadata.approvals_reviewer,
            tools: metadata
                .tools
                .iter()
                .map(|(name, metadata)| (name.clone(), metadata.into()))
                .collect(),
        }
    }
}

impl From<&McpToolRuntimeMetadata> for McpToolElicitationMetadata {
    fn from(metadata: &McpToolRuntimeMetadata) -> Self {
        Self {
            approval_source: metadata.approval_source.clone(),
            search_aliases: metadata.search_aliases.clone(),
        }
    }
}
