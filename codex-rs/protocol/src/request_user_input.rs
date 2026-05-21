use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RequestUserInputType {
    Structured,
    OptionPicker,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub enum RequestUserInputPlacement {
    Composer,
    Inline,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(rename = "inputType", default, skip_serializing_if = "Option::is_none")]
    #[schemars(rename = "inputType")]
    #[ts(rename = "inputType")]
    pub input_type: Option<RequestUserInputType>,
    #[serde(
        rename = "allowMultiple",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(rename = "allowMultiple")]
    #[ts(rename = "allowMultiple")]
    pub allow_multiple: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(rename = "isOther", default)]
    #[schemars(rename = "isOther")]
    #[ts(rename = "isOther")]
    pub is_other: bool,
    #[serde(rename = "isSecret", default)]
    #[schemars(rename = "isSecret")]
    #[ts(rename = "isSecret")]
    pub is_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<RequestUserInputQuestionOption>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputArgs {
    #[serde(rename = "inputType", default, skip_serializing_if = "Option::is_none")]
    #[schemars(rename = "inputType")]
    #[ts(rename = "inputType")]
    pub input_type: Option<RequestUserInputType>,
    #[serde(
        rename = "optionPickerAllowMultiple",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(rename = "optionPickerAllowMultiple")]
    #[ts(rename = "optionPickerAllowMultiple")]
    pub option_picker_allow_multiple: Option<bool>,
    pub questions: Vec<RequestUserInputQuestion>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputResponse {
    pub answers: HashMap<String, RequestUserInputAnswer>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    #[serde(rename = "inputType", default, skip_serializing_if = "Option::is_none")]
    #[schemars(rename = "inputType")]
    #[ts(rename = "inputType")]
    pub input_type: Option<RequestUserInputType>,
    #[serde(
        rename = "optionPickerAllowMultiple",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(rename = "optionPickerAllowMultiple")]
    #[ts(rename = "optionPickerAllowMultiple")]
    pub option_picker_allow_multiple: Option<bool>,
    pub questions: Vec<RequestUserInputQuestion>,
}
