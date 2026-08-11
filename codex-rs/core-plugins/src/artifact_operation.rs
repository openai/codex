use crate::PluginCommandAttribution;
use crate::command_script_arguments;

const PRIMARY_RUNTIME_MARKETPLACE_NAME: &str = "openai-primary-runtime";
const MAX_EXPECTED_OUTPUT_COUNT: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactOperation {
    pub plugin_name: &'static str,
    pub script_path: &'static str,
    pub artifact_type: &'static str,
    pub operation_kind: &'static str,
    pub expected_output_count: u32,
    pub output_format: &'static str,
}

struct ArtifactSkill {
    plugin_name: &'static str,
    script_path: &'static str,
    artifact_type: &'static str,
    output_formats: &'static [&'static str],
}

const ARTIFACT_SKILLS: &[ArtifactSkill] = &[
    ArtifactSkill {
        plugin_name: "presentations",
        script_path: "skills/presentations/container_tools/mark_artifact_operation_started.mjs",
        artifact_type: "presentation",
        output_formats: &["ppt", "pptx"],
    },
    ArtifactSkill {
        plugin_name: "documents",
        script_path: "skills/documents/container_tools/mark_artifact_operation_started.mjs",
        artifact_type: "document",
        output_formats: &["doc", "docx"],
    },
    ArtifactSkill {
        plugin_name: "spreadsheets",
        script_path: "skills/spreadsheets/container_tools/mark_artifact_operation_started.mjs",
        artifact_type: "spreadsheet",
        output_formats: &["csv", "tsv", "xls", "xlsm", "xlsx"],
    },
    ArtifactSkill {
        plugin_name: "pdf",
        script_path: "skills/pdf/container_tools/mark_artifact_operation_started.mjs",
        artifact_type: "pdf",
        output_formats: &["pdf"],
    },
];

pub fn recognize_artifact_operation(
    attribution: Option<&PluginCommandAttribution>,
    command: &[String],
) -> Option<ArtifactOperation> {
    let attribution = attribution?;
    if attribution.plugin_id.marketplace_name != PRIMARY_RUNTIME_MARKETPLACE_NAME {
        return None;
    }
    let skill = ARTIFACT_SKILLS.iter().find(|skill| {
        attribution.plugin_id.plugin_name == skill.plugin_name
            && attribution.normalized_relative_path == skill.script_path
    })?;
    let script_arguments = command_script_arguments(command)?;
    let [
        operation_kind_flag,
        operation_kind,
        expected_output_count_flag,
        expected_output_count,
        output_format_flag,
        output_format,
    ] = script_arguments.as_slice()
    else {
        return None;
    };
    if operation_kind_flag != "--operation-kind"
        || expected_output_count_flag != "--expected-output-count"
        || output_format_flag != "--output-format"
    {
        return None;
    }
    let operation_kind = match operation_kind.as_str() {
        "create" => "create",
        "edit" => "edit",
        _ => return None,
    };
    let expected_output_count = expected_output_count.parse::<u32>().ok()?;
    if !(1..=MAX_EXPECTED_OUTPUT_COUNT).contains(&expected_output_count) {
        return None;
    }
    let output_format = skill
        .output_formats
        .iter()
        .copied()
        .find(|known_format| known_format.eq_ignore_ascii_case(output_format))?;

    Some(ArtifactOperation {
        plugin_name: skill.plugin_name,
        script_path: skill.script_path,
        artifact_type: skill.artifact_type,
        operation_kind,
        expected_output_count,
        output_format,
    })
}

#[cfg(test)]
#[path = "artifact_operation_tests.rs"]
mod tests;
