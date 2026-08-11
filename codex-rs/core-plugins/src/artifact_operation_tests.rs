use super::*;
use codex_plugin::PluginId;
use pretty_assertions::assert_eq;

#[test]
fn recognizes_supported_artifact_markers() {
    for skill in ARTIFACT_SKILLS {
        for operation_kind in ["create", "edit"] {
            let attribution = attribution(skill.plugin_name, skill.script_path);
            assert_eq!(
                recognize_artifact_operation(
                    Some(&attribution),
                    &marker_command(
                        skill.script_path,
                        operation_kind,
                        "2",
                        skill.output_formats[0],
                    ),
                ),
                Some(ArtifactOperation {
                    plugin_name: skill.plugin_name,
                    script_path: skill.script_path,
                    artifact_type: skill.artifact_type,
                    operation_kind,
                    expected_output_count: 2,
                    output_format: skill.output_formats[0],
                })
            );
        }
    }
}

#[test]
fn rejects_untrusted_mismatched_or_invalid_markers() {
    let presentation = &ARTIFACT_SKILLS[0];
    let cases = [
        (
            None,
            marker_command(presentation.script_path, "create", "1", "pptx"),
        ),
        (
            Some(PluginCommandAttribution {
                plugin_id: PluginId::parse("documents@openai-primary-runtime").expect("plugin id"),
                normalized_relative_path: presentation.script_path.to_string(),
            }),
            marker_command(presentation.script_path, "create", "1", "pptx"),
        ),
        (
            Some(attribution(
                "presentations",
                "skills/presentations/container_tools/render_slides.mjs",
            )),
            marker_command(presentation.script_path, "create", "1", "pptx"),
        ),
        (
            Some(attribution(
                presentation.plugin_name,
                presentation.script_path,
            )),
            command_with_arguments(presentation.script_path, Vec::new()),
        ),
        (
            Some(attribution(
                presentation.plugin_name,
                presentation.script_path,
            )),
            command_with_arguments(
                presentation.script_path,
                marker_arguments("read", "1", "pptx"),
            ),
        ),
        (
            Some(attribution(
                presentation.plugin_name,
                presentation.script_path,
            )),
            command_with_arguments(
                presentation.script_path,
                marker_arguments("create", "0", "pptx"),
            ),
        ),
        (
            Some(attribution(
                presentation.plugin_name,
                presentation.script_path,
            )),
            command_with_arguments(
                presentation.script_path,
                marker_arguments("create", "101", "pptx"),
            ),
        ),
        (
            Some(attribution(
                presentation.plugin_name,
                presentation.script_path,
            )),
            command_with_arguments(
                presentation.script_path,
                marker_arguments("create", "1", "html"),
            ),
        ),
    ];

    for (attribution, command) in cases {
        assert_eq!(
            recognize_artifact_operation(attribution.as_ref(), &command),
            None
        );
    }
}

fn attribution(plugin_name: &str, script_path: &str) -> PluginCommandAttribution {
    PluginCommandAttribution {
        plugin_id: PluginId::parse(&format!("{plugin_name}@openai-primary-runtime"))
            .expect("plugin id"),
        normalized_relative_path: script_path.to_string(),
    }
}

fn marker_command(
    script_path: &str,
    operation_kind: &str,
    expected_output_count: &str,
    output_format: &str,
) -> Vec<String> {
    command_with_arguments(
        script_path,
        marker_arguments(operation_kind, expected_output_count, output_format),
    )
}

fn command_with_arguments(script_path: &str, arguments: Vec<String>) -> Vec<String> {
    [vec!["node".to_string(), script_path.to_string()], arguments].concat()
}

fn marker_arguments(
    operation_kind: &str,
    expected_output_count: &str,
    output_format: &str,
) -> Vec<String> {
    [
        "--operation-kind",
        operation_kind,
        "--expected-output-count",
        expected_output_count,
        "--output-format",
        output_format,
    ]
    .map(str::to_string)
    .to_vec()
}
