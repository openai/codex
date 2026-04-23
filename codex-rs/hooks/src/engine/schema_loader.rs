use std::sync::OnceLock;

use serde_json::Value;

#[allow(dead_code)]
pub(crate) struct GeneratedHookSchemas {
    pub post_tool_use_command_input: Value,
    pub post_tool_use_command_output: Value,
    pub permission_request_command_input: Value,
    pub permission_request_command_output: Value,
    pub post_compact_command_input: Value,
    pub post_compact_command_output: Value,
    pub post_model_response_command_input: Value,
    pub post_model_response_command_output: Value,
    pub pre_tool_use_command_input: Value,
    pub pre_tool_use_command_output: Value,
    pub pre_compact_command_input: Value,
    pub pre_compact_command_output: Value,
    pub pre_model_request_command_input: Value,
    pub pre_model_request_command_output: Value,
    pub session_start_command_input: Value,
    pub session_start_command_output: Value,
    pub user_prompt_submit_command_input: Value,
    pub user_prompt_submit_command_output: Value,
    pub stop_command_input: Value,
    pub stop_command_output: Value,
}

pub(crate) fn generated_hook_schemas() -> &'static GeneratedHookSchemas {
    static SCHEMAS: OnceLock<GeneratedHookSchemas> = OnceLock::new();
    SCHEMAS.get_or_init(|| GeneratedHookSchemas {
        post_tool_use_command_input: parse_json_schema(
            "post-tool-use.command.input",
            include_str!("../../schema/generated/post-tool-use.command.input.schema.json"),
        ),
        post_tool_use_command_output: parse_json_schema(
            "post-tool-use.command.output",
            include_str!("../../schema/generated/post-tool-use.command.output.schema.json"),
        ),
        permission_request_command_input: parse_json_schema(
            "permission-request.command.input",
            include_str!("../../schema/generated/permission-request.command.input.schema.json"),
        ),
        permission_request_command_output: parse_json_schema(
            "permission-request.command.output",
            include_str!("../../schema/generated/permission-request.command.output.schema.json"),
        ),
        post_compact_command_input: parse_json_schema(
            "post-compact.command.input",
            include_str!("../../schema/generated/post-compact.command.input.schema.json"),
        ),
        post_compact_command_output: parse_json_schema(
            "post-compact.command.output",
            include_str!("../../schema/generated/post-compact.command.output.schema.json"),
        ),
        post_model_response_command_input: parse_json_schema(
            "post-model-response.command.input",
            include_str!("../../schema/generated/post-model-response.command.input.schema.json"),
        ),
        post_model_response_command_output: parse_json_schema(
            "post-model-response.command.output",
            include_str!("../../schema/generated/post-model-response.command.output.schema.json"),
        ),
        pre_tool_use_command_input: parse_json_schema(
            "pre-tool-use.command.input",
            include_str!("../../schema/generated/pre-tool-use.command.input.schema.json"),
        ),
        pre_tool_use_command_output: parse_json_schema(
            "pre-tool-use.command.output",
            include_str!("../../schema/generated/pre-tool-use.command.output.schema.json"),
        ),
        pre_compact_command_input: parse_json_schema(
            "pre-compact.command.input",
            include_str!("../../schema/generated/pre-compact.command.input.schema.json"),
        ),
        pre_compact_command_output: parse_json_schema(
            "pre-compact.command.output",
            include_str!("../../schema/generated/pre-compact.command.output.schema.json"),
        ),
        pre_model_request_command_input: parse_json_schema(
            "pre-model-request.command.input",
            include_str!("../../schema/generated/pre-model-request.command.input.schema.json"),
        ),
        pre_model_request_command_output: parse_json_schema(
            "pre-model-request.command.output",
            include_str!("../../schema/generated/pre-model-request.command.output.schema.json"),
        ),
        session_start_command_input: parse_json_schema(
            "session-start.command.input",
            include_str!("../../schema/generated/session-start.command.input.schema.json"),
        ),
        session_start_command_output: parse_json_schema(
            "session-start.command.output",
            include_str!("../../schema/generated/session-start.command.output.schema.json"),
        ),
        user_prompt_submit_command_input: parse_json_schema(
            "user-prompt-submit.command.input",
            include_str!("../../schema/generated/user-prompt-submit.command.input.schema.json"),
        ),
        user_prompt_submit_command_output: parse_json_schema(
            "user-prompt-submit.command.output",
            include_str!("../../schema/generated/user-prompt-submit.command.output.schema.json"),
        ),
        stop_command_input: parse_json_schema(
            "stop.command.input",
            include_str!("../../schema/generated/stop.command.input.schema.json"),
        ),
        stop_command_output: parse_json_schema(
            "stop.command.output",
            include_str!("../../schema/generated/stop.command.output.schema.json"),
        ),
    })
}

fn parse_json_schema(name: &str, schema: &str) -> Value {
    serde_json::from_str(schema)
        .unwrap_or_else(|err| panic!("invalid generated hooks schema {name}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::generated_hook_schemas;
    use pretty_assertions::assert_eq;

    #[test]
    fn loads_generated_hook_schemas() {
        let schemas = generated_hook_schemas();

        assert_eq!(schemas.post_tool_use_command_input["type"], "object");
        assert_eq!(schemas.post_tool_use_command_output["type"], "object");
        assert_eq!(schemas.permission_request_command_input["type"], "object");
        assert_eq!(schemas.permission_request_command_output["type"], "object");
        assert_eq!(schemas.post_compact_command_input["type"], "object");
        assert_eq!(schemas.post_compact_command_output["type"], "object");
        assert_eq!(schemas.post_model_response_command_input["type"], "object");
        assert_eq!(schemas.post_model_response_command_output["type"], "object");
        assert_eq!(schemas.pre_tool_use_command_input["type"], "object");
        assert_eq!(schemas.pre_tool_use_command_output["type"], "object");
        assert_eq!(schemas.pre_compact_command_input["type"], "object");
        assert_eq!(schemas.pre_compact_command_output["type"], "object");
        assert_eq!(schemas.pre_model_request_command_input["type"], "object");
        assert_eq!(schemas.pre_model_request_command_output["type"], "object");
        assert_eq!(schemas.session_start_command_input["type"], "object");
        assert_eq!(schemas.session_start_command_output["type"], "object");
        assert_eq!(schemas.user_prompt_submit_command_input["type"], "object");
        assert_eq!(schemas.user_prompt_submit_command_output["type"], "object");
        assert_eq!(schemas.stop_command_input["type"], "object");
        assert_eq!(schemas.stop_command_output["type"], "object");
    }
}
