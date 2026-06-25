//! Trusted approval presentation for hosted Apps tools.
//!
//! Templates are matched against the hosted source identity. The connector-scoped HTTP MCP
//! server name and exposed tool name are deliberately not part of the lookup.

use std::sync::LazyLock;

use codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME;
use serde::Deserialize;

const TEMPLATES_SCHEMA_VERSION: u8 = 4;
const CONNECTOR_NAME_TEMPLATE_VAR: &str = "{connector_name}";

static TEMPLATES: LazyLock<Option<Vec<ApprovalTemplate>>> = LazyLock::new(load_templates);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppsApprovalPresentation {
    pub(crate) question: String,
    pub(crate) parameter_labels: Vec<AppsApprovalParameterLabel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppsApprovalParameterLabel {
    pub(crate) name: String,
    pub(crate) label: String,
}

#[derive(Debug, Deserialize)]
struct ApprovalTemplatesFile {
    schema_version: u8,
    templates: Vec<ApprovalTemplate>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct ApprovalTemplate {
    connector_id: String,
    server_name: String,
    tool_title: String,
    template: String,
    template_params: Vec<ApprovalTemplateParameter>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct ApprovalTemplateParameter {
    name: String,
    label: String,
}

/// Returns the Apps-owned approval presentation for one hosted source tool.
///
/// `upstream_tool_title` is the title after removal of the connector-name prefix, matching the
/// title stored in the legacy schema-v4 template bundle.
pub(crate) fn render_approval_presentation(
    connector_id: &str,
    connector_name: Option<&str>,
    upstream_tool_title: Option<&str>,
) -> Option<AppsApprovalPresentation> {
    render_from_templates(
        TEMPLATES.as_ref()?,
        connector_id,
        connector_name,
        upstream_tool_title,
    )
}

fn load_templates() -> Option<Vec<ApprovalTemplate>> {
    let file = match serde_json::from_str::<ApprovalTemplatesFile>(include_str!(
        "consequential_tool_message_templates.json"
    )) {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(%error, "failed to parse Apps approval presentation templates");
            return None;
        }
    };
    if file.schema_version != TEMPLATES_SCHEMA_VERSION {
        tracing::warn!(
            found_schema_version = file.schema_version,
            expected_schema_version = TEMPLATES_SCHEMA_VERSION,
            "unexpected Apps approval presentation template schema version"
        );
        return None;
    }
    Some(file.templates)
}

fn render_from_templates(
    templates: &[ApprovalTemplate],
    connector_id: &str,
    connector_name: Option<&str>,
    upstream_tool_title: Option<&str>,
) -> Option<AppsApprovalPresentation> {
    let upstream_tool_title = upstream_tool_title
        .map(str::trim)
        .filter(|title| !title.is_empty())?;
    let template = templates.iter().find(|template| {
        template.server_name == CODEX_APPS_MCP_SERVER_NAME
            && template.connector_id == connector_id
            && template.tool_title == upstream_tool_title
    })?;
    let question = render_question(&template.template, connector_name)?;
    let parameter_labels = template
        .template_params
        .iter()
        .map(|parameter| {
            let label = parameter.label.trim();
            (!label.is_empty()).then(|| AppsApprovalParameterLabel {
                name: parameter.name.clone(),
                label: label.to_string(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(AppsApprovalPresentation {
        question,
        parameter_labels,
    })
}

fn render_question(template: &str, connector_name: Option<&str>) -> Option<String> {
    let template = template.trim();
    if template.is_empty() {
        return None;
    }
    if template.contains(CONNECTOR_NAME_TEMPLATE_VAR) {
        let connector_name = connector_name
            .map(str::trim)
            .filter(|name| !name.is_empty())?;
        return Some(template.replace(CONNECTOR_NAME_TEMPLATE_VAR, connector_name));
    }
    Some(template.to_string())
}

#[cfg(test)]
#[path = "approval_presentation_tests.rs"]
mod tests;
