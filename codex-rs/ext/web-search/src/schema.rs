use codex_api::SearchCommands;
use schemars::r#gen::SchemaSettings;
use serde_json::Map;
use serde_json::Value;

pub(crate) const LITERATURE_SEARCH_QUERY: &str = "literature_search_query";

pub(crate) fn commands_schema(supports_literature_search: bool) -> Value {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<SearchCommands>();
    let schema = match serde_json::to_value(schema) {
        Ok(schema) => schema,
        Err(err) => panic!("search commands schema should serialize: {err}"),
    };
    let Value::Object(mut schema) = schema else {
        unreachable!("search commands schema must be an object");
    };
    if !supports_literature_search
        && let Some(Value::Object(properties)) = schema.get_mut("properties")
    {
        properties.remove(LITERATURE_SEARCH_QUERY);
    }

    let mut tool_schema = Map::new();
    for key in [
        "properties",
        "required",
        "type",
        "additionalProperties",
        "$defs",
        "definitions",
    ] {
        if let Some(value) = schema.remove(key) {
            tool_schema.insert(key.to_string(), value);
        }
    }
    Value::Object(tool_schema)
}
