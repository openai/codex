use std::collections::HashSet;
use std::sync::LazyLock;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use tracing::warn;

const CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES_SCHEMA_VERSION: u8 = 4;
const CONNECTOR_NAME_TEMPLATE_VAR: &str = "{connector_name}";

static CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES: LazyLock<
    Option<Vec<ConsequentialToolMessageTemplate>>,
> = LazyLock::new(load_consequential_tool_message_templates);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RenderedMcpToolApprovalTemplate {
    pub(crate) question: String,
    pub(crate) elicitation_message: String,
    pub(crate) tool_params: Option<Value>,
    pub(crate) tool_params_display: Vec<RenderedMcpToolApprovalParam>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct RenderedMcpToolApprovalParam {
    pub(crate) name: String,
    pub(crate) value: Value,
}

#[derive(Debug, Deserialize)]
struct ConsequentialToolMessageTemplatesFile {
    schema_version: u8,
    templates: Vec<ConsequentialToolMessageTemplate>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ConsequentialToolMessageTemplate {
    connector_id: String,
    server_name: String,
    tool_title: String,
    template: String,
    template_params: Vec<ConsequentialToolTemplateParam>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ConsequentialToolTemplateParam {
    name: String,
    label: String,
}

pub(crate) fn render_mcp_tool_approval_template(
    server_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    tool_title: Option<&str>,
    tool_params: Option<&Value>,
) -> Option<RenderedMcpToolApprovalTemplate> {
    let templates = CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES.as_ref()?;
    render_mcp_tool_approval_template_from_templates(
        templates,
        server_name,
        connector_id,
        connector_name,
        tool_title,
        tool_params,
    )
}

fn load_consequential_tool_message_templates() -> Option<Vec<ConsequentialToolMessageTemplate>> {
    let templates = match serde_json::from_str::<ConsequentialToolMessageTemplatesFile>(
        include_str!("consequential_tool_message_templates.json"),
    ) {
        Ok(templates) => templates,
        Err(err) => {
            warn!(error = %err, "failed to parse consequential tool approval templates");
            return None;
        }
    };

    if templates.schema_version != CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES_SCHEMA_VERSION {
        warn!(
            found_schema_version = templates.schema_version,
            expected_schema_version = CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES_SCHEMA_VERSION,
            "unexpected consequential tool approval templates schema version"
        );
        return None;
    }

    Some(templates.templates)
}

fn render_mcp_tool_approval_template_from_templates(
    templates: &[ConsequentialToolMessageTemplate],
    server_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    tool_title: Option<&str>,
    tool_params: Option<&Value>,
) -> Option<RenderedMcpToolApprovalTemplate> {
    let connector_id = connector_id?;
    let tool_title = tool_title.map(str::trim).filter(|name| !name.is_empty())?;
    let template = templates.iter().find(|template| {
        template.server_name == server_name
            && template.connector_id == connector_id
            && template.tool_title == tool_title
    })?;
    let elicitation_message = render_question_template(&template.template, connector_name)?;
    let (tool_params, tool_params_display) = match tool_params {
        Some(Value::Object(tool_params)) => {
            render_tool_params(tool_params, &template.template_params)?
        }
        Some(_) => return None,
        None => (None, Vec::new()),
    };

    Some(RenderedMcpToolApprovalTemplate {
        question: elicitation_message.clone(),
        elicitation_message,
        tool_params,
        tool_params_display,
    })
}

fn render_question_template(template: &str, connector_name: Option<&str>) -> Option<String> {
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

fn render_tool_params(
    tool_params: &Map<String, Value>,
    template_params: &[ConsequentialToolTemplateParam],
) -> Option<(Option<Value>, Vec<RenderedMcpToolApprovalParam>)> {
    let mut display_params = Vec::new();
    let mut display_names = HashSet::new();
    let mut handled_names = HashSet::new();

    for template_param in template_params {
        let label = template_param.label.trim();
        if label.is_empty() {
            return None;
        }
        let Some(value) = tool_params.get(&template_param.name) else {
            continue;
        };
        if !display_names.insert(label.to_string()) {
            return None;
        }
        display_params.push(RenderedMcpToolApprovalParam {
            name: label.to_string(),
            value: value.clone(),
        });
        handled_names.insert(template_param.name.as_str());
    }

    let mut remaining_params = tool_params
        .iter()
        .filter(|(name, _)| !handled_names.contains(name.as_str()))
        .collect::<Vec<_>>();
    remaining_params.sort_by(|(left_name, _), (right_name, _)| left_name.cmp(right_name));

    for (name, value) in remaining_params {
        if handled_names.contains(name.as_str()) {
            continue;
        }
        if !display_names.insert(name.clone()) {
            return None;
        }
        display_params.push(RenderedMcpToolApprovalParam {
            name: name.clone(),
            value: value.clone(),
        });
    }

    Some((Some(Value::Object(tool_params.clone())), display_params))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn renders_exact_match_with_readable_param_labels() {
        let templates = vec![ConsequentialToolMessageTemplate {
            connector_id: "calendar".to_string(),
            server_name: "codex_apps".to_string(),
            tool_title: "create_event".to_string(),
            template: "Allow {connector_name} to create an event?".to_string(),
            template_params: vec![
                ConsequentialToolTemplateParam {
                    name: "calendar_id".to_string(),
                    label: "Calendar".to_string(),
                },
                ConsequentialToolTemplateParam {
                    name: "title".to_string(),
                    label: "Title".to_string(),
                },
            ],
        }];

        let rendered = render_mcp_tool_approval_template_from_templates(
            &templates,
            "codex_apps",
            Some("calendar"),
            Some("Calendar"),
            Some("create_event"),
            Some(&json!({
                "title": "Roadmap review",
                "calendar_id": "primary",
                "timezone": "UTC",
            })),
        );

        assert_eq!(
            rendered,
            Some(RenderedMcpToolApprovalTemplate {
                question: "Allow Calendar to create an event?".to_string(),
                elicitation_message: "Allow Calendar to create an event?".to_string(),
                tool_params: Some(json!({
                    "title": "Roadmap review",
                    "calendar_id": "primary",
                    "timezone": "UTC",
                })),
                tool_params_display: vec![
                    RenderedMcpToolApprovalParam {
                        name: "Calendar".to_string(),
                        value: json!("primary"),
                    },
                    RenderedMcpToolApprovalParam {
                        name: "Title".to_string(),
                        value: json!("Roadmap review"),
                    },
                    RenderedMcpToolApprovalParam {
                        name: "timezone".to_string(),
                        value: json!("UTC"),
                    },
                ],
            })
        );
    }

    #[test]
    fn returns_none_when_no_exact_match_exists() {
        let templates = vec![ConsequentialToolMessageTemplate {
            connector_id: "calendar".to_string(),
            server_name: "codex_apps".to_string(),
            tool_title: "create_event".to_string(),
            template: "Allow {connector_name} to create an event?".to_string(),
            template_params: Vec::new(),
        }];

        assert_eq!(
            render_mcp_tool_approval_template_from_templates(
                &templates,
                "codex_apps",
                Some("calendar"),
                Some("Calendar"),
                Some("delete_event"),
                Some(&json!({})),
            ),
            None
        );
    }

    #[test]
    fn returns_none_when_relabeling_would_collide() {
        let templates = vec![ConsequentialToolMessageTemplate {
            connector_id: "calendar".to_string(),
            server_name: "codex_apps".to_string(),
            tool_title: "create_event".to_string(),
            template: "Allow {connector_name} to create an event?".to_string(),
            template_params: vec![ConsequentialToolTemplateParam {
                name: "calendar_id".to_string(),
                label: "timezone".to_string(),
            }],
        }];

        assert_eq!(
            render_mcp_tool_approval_template_from_templates(
                &templates,
                "codex_apps",
                Some("calendar"),
                Some("Calendar"),
                Some("create_event"),
                Some(&json!({
                    "calendar_id": "primary",
                    "timezone": "UTC",
                })),
            ),
            None
        );
    }

    #[test]
    fn bundled_templates_load() {
        assert_eq!(CONSEQUENTIAL_TOOL_MESSAGE_TEMPLATES.is_some(), true);
    }

    #[test]
    fn renders_literal_template_without_connector_substitution() {
        let templates = vec![ConsequentialToolMessageTemplate {
            connector_id: "github".to_string(),
            server_name: "codex_apps".to_string(),
            tool_title: "add_comment".to_string(),
            template: "Allow GitHub to add a comment to a pull request?".to_string(),
            template_params: Vec::new(),
        }];

        let rendered = render_mcp_tool_approval_template_from_templates(
            &templates,
            "codex_apps",
            Some("github"),
            None,
            Some("add_comment"),
            Some(&json!({})),
        );

        assert_eq!(
            rendered,
            Some(RenderedMcpToolApprovalTemplate {
                question: "Allow GitHub to add a comment to a pull request?".to_string(),
                elicitation_message: "Allow GitHub to add a comment to a pull request?".to_string(),
                tool_params: Some(json!({})),
                tool_params_display: Vec::new(),
            })
        );
    }

    #[test]
    fn returns_none_when_connector_placeholder_has_no_value() {
        let templates = vec![ConsequentialToolMessageTemplate {
            connector_id: "calendar".to_string(),
            server_name: "codex_apps".to_string(),
            tool_title: "create_event".to_string(),
            template: "Allow {connector_name} to create an event?".to_string(),
            template_params: Vec::new(),
        }];

        assert_eq!(
            render_mcp_tool_approval_template_from_templates(
                &templates,
                "codex_apps",
                Some("calendar"),
                None,
                Some("create_event"),
                Some(&json!({})),
            ),
            None
        );
    }
}
