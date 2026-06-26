use pretty_assertions::assert_eq;

use super::*;

fn template(
    server_name: &str,
    connector_id: &str,
    tool_title: &str,
    question: &str,
    parameters: &[(&str, &str)],
) -> ApprovalTemplate {
    ApprovalTemplate {
        connector_id: connector_id.to_string(),
        server_name: server_name.to_string(),
        tool_title: tool_title.to_string(),
        template: question.to_string(),
        template_params: parameters
            .iter()
            .map(|(name, label)| ApprovalTemplateParameter {
                name: (*name).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
    }
}

#[test]
fn renders_exact_source_match_with_ordered_parameter_labels() {
    let templates = [template(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar",
        "create_event",
        "Allow {connector_name} to create an event?",
        &[("calendar_id", "Calendar"), ("title", "Title")],
    )];

    assert_eq!(
        render_from_templates(
            &templates,
            "calendar",
            Some("Calendar"),
            Some("create_event")
        ),
        Some(AppsApprovalPresentation {
            question: "Allow Calendar to create an event?".to_string(),
            parameter_labels: vec![
                AppsApprovalParameterLabel {
                    name: "calendar_id".to_string(),
                    label: "Calendar".to_string(),
                },
                AppsApprovalParameterLabel {
                    name: "title".to_string(),
                    label: "Title".to_string(),
                },
            ],
        })
    );
}

#[test]
fn ignores_virtual_or_other_source_servers() {
    let templates = [template(
        "codex_apps__calendar",
        "calendar",
        "create_event",
        "wrong source",
        &[],
    )];

    assert_eq!(
        render_from_templates(
            &templates,
            "calendar",
            Some("Calendar"),
            Some("create_event")
        ),
        None
    );
}

#[test]
fn requires_an_exact_connector_and_upstream_title_match() {
    let templates = [template(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar",
        "create_event",
        "Allow an event?",
        &[],
    )];

    assert_eq!(
        render_from_templates(&templates, "drive", Some("Drive"), Some("create_event")),
        None
    );
    assert_eq!(
        render_from_templates(
            &templates,
            "calendar",
            Some("Calendar"),
            Some("delete_event")
        ),
        None
    );
}

#[test]
fn connector_placeholder_requires_a_nonempty_name() {
    let templates = [template(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar",
        "create_event",
        "Allow {connector_name} to create an event?",
        &[],
    )];

    assert_eq!(
        render_from_templates(
            &templates,
            "calendar",
            /*connector_name*/ None,
            Some("create_event"),
        ),
        None
    );
    assert_eq!(
        render_from_templates(&templates, "calendar", Some("  "), Some("create_event")),
        None
    );
}

#[test]
fn literal_question_does_not_require_a_connector_name() {
    let templates = [template(
        CODEX_APPS_MCP_SERVER_NAME,
        "github",
        "add_comment",
        "Allow GitHub to add a comment to a pull request?",
        &[],
    )];

    assert_eq!(
        render_from_templates(
            &templates,
            "github",
            /*connector_name*/ None,
            Some("add_comment"),
        ),
        Some(AppsApprovalPresentation {
            question: "Allow GitHub to add a comment to a pull request?".to_string(),
            parameter_labels: Vec::new(),
        })
    );
}

#[test]
fn bundled_schema_v4_templates_render() {
    let presentation = render_approval_presentation(
        "connector_2128aebfecb84f64a069897515042a44",
        Some("Gmail"),
        Some("send_email"),
    )
    .expect("bundled Gmail send template");

    assert_eq!(presentation.question, "Allow Gmail to send an email?");
    assert_eq!(
        presentation.parameter_labels,
        vec![
            AppsApprovalParameterLabel {
                name: "to".to_string(),
                label: "To".to_string(),
            },
            AppsApprovalParameterLabel {
                name: "subject".to_string(),
                label: "Subject".to_string(),
            },
            AppsApprovalParameterLabel {
                name: "body".to_string(),
                label: "Body".to_string(),
            },
        ]
    );
}
