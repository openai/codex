use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::r#gen::SchemaSettings;
use schemars::schema::InstanceType;
use schemars::schema::RootSchema;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;

const GENERATED_DIR: &str = "generated";
const SESSION_START_INPUT_FIXTURE: &str = "session-start.command.input.schema.json";
const SESSION_START_OUTPUT_FIXTURE: &str = "session-start.command.output.schema.json";
const USER_PROMPT_SUBMIT_INPUT_FIXTURE: &str = "user-prompt-submit.command.input.schema.json";
const USER_PROMPT_SUBMIT_OUTPUT_FIXTURE: &str = "user-prompt-submit.command.output.schema.json";
const STOP_INPUT_FIXTURE: &str = "stop.command.input.schema.json";
const STOP_OUTPUT_FIXTURE: &str = "stop.command.output.schema.json";

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct NullableString(Option<String>);

impl NullableString {
    fn from_path(path: Option<PathBuf>) -> Self {
        Self(path.map(|path| path.display().to_string()))
    }

    fn from_string(value: Option<String>) -> Self {
        Self(value)
    }
}

impl JsonSchema for NullableString {
    fn schema_name() -> String {
        "NullableString".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(vec![InstanceType::String, InstanceType::Null].into()),
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct HookUniversalOutputWire {
    #[serde(default = "default_continue")]
    pub r#continue: bool,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub suppress_output: bool,
    #[serde(default)]
    pub system_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum HookEventNameWire {
    #[serde(rename = "SessionStart")]
    SessionStart,
    #[serde(rename = "UserPromptSubmit")]
    UserPromptSubmit,
    #[serde(rename = "Stop")]
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "session-start.command.output")]
pub(crate) struct SessionStartCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub hook_specific_output: Option<SessionStartHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionStartHookSpecificOutputWire {
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "user-prompt-submit.command.output")]
pub(crate) struct UserPromptSubmitCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub hook_specific_output: Option<UserPromptSubmitHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct UserPromptSubmitHookSpecificOutputWire {
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "stop.command.output")]
pub(crate) struct StopCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<StopDecisionWire>,
    /// Claude requires `reason` when `decision` is `block`; we enforce that
    /// semantic rule during output parsing rather than in the JSON schema.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum StopDecisionWire {
    #[serde(rename = "block")]
    Block,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "session-start.command.input")]
pub(crate) struct SessionStartCommandInput {
    pub session_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "session_start_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    #[schemars(schema_with = "session_start_source_schema")]
    pub source: String,
}

impl SessionStartCommandInput {
    pub(crate) fn new(
        session_id: impl Into<String>,
        transcript_path: Option<PathBuf>,
        cwd: impl Into<String>,
        model: impl Into<String>,
        permission_mode: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            transcript_path: NullableString::from_path(transcript_path),
            cwd: cwd.into(),
            hook_event_name: "SessionStart".to_string(),
            model: model.into(),
            permission_mode: permission_mode.into(),
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "user-prompt-submit.command.input")]
pub(crate) struct UserPromptSubmitCommandInput {
    pub session_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "user_prompt_submit_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub prompt: String,
}

impl UserPromptSubmitCommandInput {
    pub(crate) fn new(
        session_id: impl Into<String>,
        transcript_path: Option<PathBuf>,
        cwd: impl Into<String>,
        model: impl Into<String>,
        permission_mode: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            transcript_path: NullableString::from_path(transcript_path),
            cwd: cwd.into(),
            hook_event_name: "UserPromptSubmit".to_string(),
            model: model.into(),
            permission_mode: permission_mode.into(),
            prompt: prompt.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "stop.command.input")]
pub(crate) struct StopCommandInput {
    pub session_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "stop_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub last_assistant_message: NullableString,
}

impl StopCommandInput {
    pub(crate) fn new(
        session_id: impl Into<String>,
        transcript_path: Option<PathBuf>,
        cwd: impl Into<String>,
        model: impl Into<String>,
        permission_mode: impl Into<String>,
        stop_hook_active: bool,
        last_assistant_message: Option<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            transcript_path: NullableString::from_path(transcript_path),
            cwd: cwd.into(),
            hook_event_name: "Stop".to_string(),
            model: model.into(),
            permission_mode: permission_mode.into(),
            stop_hook_active,
            last_assistant_message: NullableString::from_string(last_assistant_message),
        }
    }
}

pub fn write_schema_fixtures(schema_root: &Path) -> anyhow::Result<()> {
    let generated_dir = schema_root.join(GENERATED_DIR);
    ensure_empty_dir(&generated_dir)?;

    write_schema(
        &generated_dir.join(SESSION_START_INPUT_FIXTURE),
        schema_json::<SessionStartCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(SESSION_START_OUTPUT_FIXTURE),
        schema_json::<SessionStartCommandOutputWire>()?,
    )?;
    write_schema(
        &generated_dir.join(USER_PROMPT_SUBMIT_INPUT_FIXTURE),
        schema_json::<UserPromptSubmitCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(USER_PROMPT_SUBMIT_OUTPUT_FIXTURE),
        schema_json::<UserPromptSubmitCommandOutputWire>()?,
    )?;
    write_schema(
        &generated_dir.join(STOP_INPUT_FIXTURE),
        schema_json::<StopCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(STOP_OUTPUT_FIXTURE),
        schema_json::<StopCommandOutputWire>()?,
    )?;

    Ok(())
}

fn write_schema(path: &Path, json: Vec<u8>) -> anyhow::Result<()> {
    std::fs::write(path, json)?;
    Ok(())
}

fn ensure_empty_dir(dir: &Path) -> anyhow::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    Ok(())
}

fn schema_json<T>() -> anyhow::Result<Vec<u8>>
where
    T: JsonSchema,
{
    let schema = schema_for_type::<T>();
    let value = serde_json::to_value(schema)?;
    let value = canonicalize_json(&value);
    Ok(serde_json::to_vec_pretty(&value)?)
}

fn schema_for_type<T>() -> RootSchema
where
    T: JsonSchema,
{
    SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<T>()
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = Map::with_capacity(map.len());
            for (key, child) in entries {
                sorted.insert(key.clone(), canonicalize_json(child));
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

fn session_start_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("SessionStart")
}

fn user_prompt_submit_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("UserPromptSubmit")
}

fn stop_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("Stop")
}

fn permission_mode_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_enum_schema(&[
        "default",
        "acceptEdits",
        "plan",
        "dontAsk",
        "bypassPermissions",
    ])
}

fn session_start_source_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_enum_schema(&["startup", "resume", "clear"])
}

fn string_const_schema(value: &str) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    };
    schema.const_value = Some(Value::String(value.to_string()));
    Schema::Object(schema)
}

fn string_enum_schema(values: &[&str]) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    };
    schema.enum_values = Some(
        values
            .iter()
            .map(|value| Value::String((*value).to_string()))
            .collect(),
    );
    Schema::Object(schema)
}

fn default_continue() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::SESSION_START_INPUT_FIXTURE;
    use super::SESSION_START_OUTPUT_FIXTURE;
    use super::STOP_INPUT_FIXTURE;
    use super::STOP_OUTPUT_FIXTURE;
    use super::USER_PROMPT_SUBMIT_INPUT_FIXTURE;
    use super::USER_PROMPT_SUBMIT_OUTPUT_FIXTURE;
    use super::write_schema_fixtures;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn expected_fixture(name: &str) -> &'static str {
        match name {
            SESSION_START_INPUT_FIXTURE => {
                include_str!("../schema/generated/session-start.command.input.schema.json")
            }
            SESSION_START_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/session-start.command.output.schema.json")
            }
            USER_PROMPT_SUBMIT_INPUT_FIXTURE => {
                include_str!("../schema/generated/user-prompt-submit.command.input.schema.json")
            }
            USER_PROMPT_SUBMIT_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/user-prompt-submit.command.output.schema.json")
            }
            STOP_INPUT_FIXTURE => {
                include_str!("../schema/generated/stop.command.input.schema.json")
            }
            STOP_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/stop.command.output.schema.json")
            }
            _ => panic!("unexpected fixture name: {name}"),
        }
    }

    fn normalize_newlines(value: &str) -> String {
        value.replace("\r\n", "\n")
    }

    #[test]
    fn generated_hook_schemas_match_fixtures() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let schema_root = temp_dir.path().join("schema");
        write_schema_fixtures(&schema_root).expect("write generated hook schemas");

        for fixture in [
            SESSION_START_INPUT_FIXTURE,
            SESSION_START_OUTPUT_FIXTURE,
            USER_PROMPT_SUBMIT_INPUT_FIXTURE,
            USER_PROMPT_SUBMIT_OUTPUT_FIXTURE,
            STOP_INPUT_FIXTURE,
            STOP_OUTPUT_FIXTURE,
        ] {
            let expected = normalize_newlines(expected_fixture(fixture));
            let actual = std::fs::read_to_string(schema_root.join("generated").join(fixture))
                .unwrap_or_else(|err| panic!("read generated schema {fixture}: {err}"));
            let actual = normalize_newlines(&actual);
            assert_eq!(expected, actual, "fixture should match generated schema");
        }
    }
}
