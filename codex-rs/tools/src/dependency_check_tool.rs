use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_dependency_check_tool() -> ToolSpec {
    let dependency_properties = BTreeMap::from([
        (
            "name".to_string(),
            JsonSchema::string(Some(
                "Exact npm package name, for example `zod` or `@types/node`.".to_string(),
            )),
        ),
        (
            "version".to_string(),
            JsonSchema::string(Some(
                "Exact semantic version. Ranges and tags such as `^1.2.3` or `latest` are rejected."
                    .to_string(),
            )),
        ),
    ]);
    let properties = BTreeMap::from([
        (
            "ecosystem".to_string(),
            JsonSchema::string(Some(
                "Package ecosystem. The first implementation accepts only `npm`.".to_string(),
            )),
        ),
        (
            "dependencies".to_string(),
            JsonSchema::array(
                JsonSchema::object(
                    dependency_properties,
                    Some(vec!["name".to_string(), "version".to_string()]),
                    Some(false.into()),
                ),
                Some("One to twenty direct dependencies to add.".to_string()),
            ),
        ),
        (
            "dependency_kind".to_string(),
            JsonSchema::string(Some(
                "Where to record the dependencies: `runtime`, `development`, or `optional`."
                    .to_string(),
            )),
        ),
        (
            "workdir".to_string(),
            JsonSchema::string(Some(
                "Optional project directory, relative to the current working directory unless absolute."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "dependency_check".to_string(),
        description: "Add exact npm dependencies through a checked two-phase install. Codex resolves the complete lock graph with lifecycle scripts disabled, queries OSV for every exact package version, updates the project lock graph through the normal sandbox and approval path, verifies it matches the checked graph, installs with scripts disabled, and only then runs npm rebuild. Use this instead of raw npm, pnpm, yarn, or bun dependency-add commands when the feature is enabled. The first implementation supports non-workspace npm projects only.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec![
                "ecosystem".to_string(),
                "dependencies".to_string(),
                "dependency_kind".to_string(),
            ]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponsesApiTool;
    use crate::ToolSpec;
    use pretty_assertions::assert_eq;

    #[test]
    fn dependency_check_tool_has_expected_contract() {
        let ToolSpec::Function(ResponsesApiTool {
            name, parameters, ..
        }) = create_dependency_check_tool()
        else {
            panic!("expected function tool");
        };
        assert_eq!(name, "dependency_check");
        assert_eq!(
            parameters.required,
            Some(vec![
                "ecosystem".to_string(),
                "dependencies".to_string(),
                "dependency_kind".to_string(),
            ])
        );
    }
}
