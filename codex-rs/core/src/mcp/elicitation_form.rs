use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use regex_lite::Regex;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

const REQUEST_USER_INPUT_NOTE_PREFIX: &str = "user_note: ";
const MCP_ELICITATION_FIELD_ID_PREFIX: &str = "mcp_elicitation_field";

#[derive(Clone, Debug)]
struct EnumChoice {
    value: Value,
    label: String,
}

#[derive(Clone, Debug)]
enum FieldKind {
    String {
        min_length: Option<usize>,
        max_length: Option<usize>,
        pattern: Option<String>,
        format: Option<String>,
    },
    Number {
        integer: bool,
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    Boolean,
    SingleSelectEnum {
        choices: Vec<EnumChoice>,
    },
    MultiSelectEnum {
        choices: Vec<EnumChoice>,
        min_items: Option<usize>,
        max_items: Option<usize>,
    },
}

#[derive(Clone, Debug)]
struct FieldSpec {
    property_name: String,
    question_id: String,
    header: String,
    description: Option<String>,
    required: bool,
    default: Option<Value>,
    kind: FieldKind,
}

#[derive(Clone, Debug)]
struct ParsedSchema {
    fields: Vec<FieldSpec>,
}

impl ParsedSchema {
    fn from_requested_schema(requested_schema: &Value) -> Option<Self> {
        let properties = requested_schema.get("properties")?.as_object()?;
        let required: HashSet<&str> = requested_schema
            .get("required")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let mut used_ids = HashSet::new();
        let fields = properties
            .iter()
            .enumerate()
            .filter_map(|(idx, (property_name, schema))| {
                FieldSpec::from_property(
                    property_name,
                    schema,
                    required.contains(property_name.as_str()),
                    idx,
                    &mut used_ids,
                )
            })
            .collect();

        Some(Self { fields })
    }
}

impl FieldSpec {
    fn from_property(
        property_name: &str,
        schema: &Value,
        required: bool,
        index: usize,
        used_ids: &mut HashSet<String>,
    ) -> Option<Self> {
        let schema_obj = schema.as_object()?;
        let title = schema_obj
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned);
        let description = schema_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|desc| !desc.is_empty())
            .map(ToOwned::to_owned);
        let default = schema_obj.get("default").cloned();
        let kind = parse_field_kind(schema_obj)?;
        let question_id = build_question_id(property_name, index, used_ids);

        Some(Self {
            property_name: property_name.to_string(),
            question_id,
            header: title.unwrap_or_else(|| property_name.to_string()),
            description,
            required,
            default,
            kind,
        })
    }

    fn to_question(&self) -> RequestUserInputQuestion {
        let question = self.render_prompt();
        let options = match &self.kind {
            FieldKind::Boolean => Some(vec![
                RequestUserInputQuestionOption {
                    label: "Yes".to_string(),
                    description: "Choose Yes.".to_string(),
                },
                RequestUserInputQuestionOption {
                    label: "No".to_string(),
                    description: "Choose No.".to_string(),
                },
            ]),
            FieldKind::SingleSelectEnum { choices } => Some(
                choices
                    .iter()
                    .map(|choice| RequestUserInputQuestionOption {
                        label: choice.label.clone(),
                        description: format!(
                            "Use value `{}`.",
                            render_json_primitive(&choice.value)
                        ),
                    })
                    .collect(),
            ),
            FieldKind::String { .. }
            | FieldKind::Number { .. }
            | FieldKind::MultiSelectEnum { .. } => None,
        };

        // This keeps the minimal implementation isolated: multi-select enum values are entered
        // as comma-separated text instead of true multi-select controls. A complete solution
        // should add first-class multi-select support in RequestUserInput by:
        // 1) Extending protocol question/answer types to represent multiple selected options.
        // 2) Updating `tui/src/bottom_pane/request_user_input/mod.rs` to render selectable
        //    checkboxes (or equivalent) and commit multiple selections.
        // 3) Updating answer rendering/history (`tui/src/history_cell.rs`) to display
        //    multi-select responses without relying on comma-splitting.
        // 4) Removing comma-splitting/parsing in this adapter once structured answers exist.
        RequestUserInputQuestion {
            id: self.question_id.clone(),
            header: self.header.clone(),
            question,
            is_other: false,
            is_secret: false,
            options,
        }
    }

    fn render_prompt(&self) -> String {
        let mut lines = Vec::new();
        if let Some(description) = &self.description {
            lines.push(description.clone());
        }

        let mut constraints = Vec::new();
        match &self.kind {
            FieldKind::String {
                min_length,
                max_length,
                pattern,
                format,
            } => {
                constraints.push("Enter text.".to_string());
                if format.is_some() {
                    constraints.push(
                        "Use text in the requested format (for example: \"example\").".to_string(),
                    );
                }
                if let Some(min_length) = min_length {
                    constraints.push(format!("Use at least {min_length} characters."));
                }
                if let Some(max_length) = max_length {
                    constraints.push(format!("Use no more than {max_length} characters."));
                }
                if let Some(pattern) = pattern {
                    constraints.push(format!("Must follow this pattern: {pattern}."));
                }
            }
            FieldKind::Number {
                integer,
                minimum,
                maximum,
            } => {
                constraints.push(if *integer {
                    "Enter a whole number.".to_string()
                } else {
                    "Enter a number.".to_string()
                });
                if let Some(minimum) = minimum {
                    constraints.push(format!("Must be at least {minimum}."));
                }
                if let Some(maximum) = maximum {
                    constraints.push(format!("Must be at most {maximum}."));
                }
            }
            FieldKind::Boolean => {
                constraints.push("Choose Yes or No below.".to_string());
            }
            FieldKind::SingleSelectEnum { .. } => {
                constraints.push("Choose from one option below.".to_string());
            }
            FieldKind::MultiSelectEnum {
                choices,
                min_items,
                max_items,
            } => {
                constraints.push("Choose one or more options.".to_string());
                constraints.push(
                    "Enter multiple values separated by commas (for example: Red, Blue)."
                        .to_string(),
                );
                let values = choices
                    .iter()
                    .map(|choice| choice.label.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                constraints.push(format!("Available options: {values}."));
                if let Some(min_items) = min_items {
                    constraints.push(format!("Choose at least {min_items}."));
                }
                if let Some(max_items) = max_items {
                    constraints.push(format!("Choose no more than {max_items}."));
                }
            }
        }

        constraints.push(if self.required {
            "This answer is required.".to_string()
        } else {
            "You can leave this blank.".to_string()
        });
        if let Some(default) = &self.default {
            constraints.push(format!(
                "If left blank, default is {}.",
                render_json_primitive(default)
            ));
        } else {
            constraints.push("No default value is set.".to_string());
        }

        lines.extend(constraints);
        lines.push(format!("Please enter a value for \"{}\".", self.header));
        lines.join("\n")
    }

    fn parse_response_value(
        &self,
        answers: &HashMap<String, RequestUserInputAnswer>,
    ) -> Result<Option<Value>, ()> {
        let answer = answers.get(&self.question_id);
        let (selection, note) = extract_selection_and_note(answer);
        let raw = note
            .or(selection)
            .map(str::trim)
            .filter(|input| !input.is_empty());

        match &self.kind {
            FieldKind::String {
                min_length,
                max_length,
                pattern,
                ..
            } => {
                let Some(raw) = raw else {
                    return self.default_or_required_value();
                };
                if let Some(min_length) = min_length
                    && raw.chars().count() < *min_length
                {
                    return Err(());
                }
                if let Some(max_length) = max_length
                    && raw.chars().count() > *max_length
                {
                    return Err(());
                }
                if let Some(pattern) = pattern
                    && let Ok(regex) = Regex::new(pattern)
                    && !regex.is_match(raw)
                {
                    return Err(());
                }
                Ok(Some(Value::String(raw.to_string())))
            }
            FieldKind::Number {
                integer,
                minimum,
                maximum,
            } => {
                let Some(raw) = raw else {
                    return self.default_or_required_value();
                };
                if *integer {
                    let parsed = raw.parse::<i64>().map_err(|_| ())?;
                    let parsed_f64 = parsed as f64;
                    if let Some(minimum) = minimum
                        && parsed_f64 < *minimum
                    {
                        return Err(());
                    }
                    if let Some(maximum) = maximum
                        && parsed_f64 > *maximum
                    {
                        return Err(());
                    }
                    Ok(Some(Value::Number(Number::from(parsed))))
                } else {
                    let parsed = raw.parse::<f64>().map_err(|_| ())?;
                    if let Some(minimum) = minimum
                        && parsed < *minimum
                    {
                        return Err(());
                    }
                    if let Some(maximum) = maximum
                        && parsed > *maximum
                    {
                        return Err(());
                    }
                    let number = Number::from_f64(parsed).ok_or(())?;
                    Ok(Some(Value::Number(number)))
                }
            }
            FieldKind::Boolean => {
                let Some(raw) = raw else {
                    return self.default_or_required_value();
                };
                let bool_value = match raw.to_ascii_lowercase().as_str() {
                    "true" | "yes" | "y" | "1" => true,
                    "false" | "no" | "n" | "0" => false,
                    _ => return Err(()),
                };
                Ok(Some(Value::Bool(bool_value)))
            }
            FieldKind::SingleSelectEnum { choices } => {
                let Some(raw) = raw else {
                    return self.default_or_required_value();
                };
                let selected = choices.iter().find_map(|choice| {
                    if choice.label == raw || render_json_primitive(&choice.value) == raw {
                        Some(choice.value.clone())
                    } else {
                        None
                    }
                });
                selected.map(Some).ok_or(())
            }
            FieldKind::MultiSelectEnum {
                choices,
                min_items,
                max_items,
            } => {
                let Some(raw) = raw else {
                    return self.default_or_required_value();
                };
                let parts: Vec<&str> = raw
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .collect();
                if let Some(min_items) = min_items
                    && parts.len() < *min_items
                {
                    return Err(());
                }
                if let Some(max_items) = max_items
                    && parts.len() > *max_items
                {
                    return Err(());
                }

                let values = parts
                    .into_iter()
                    .map(|part| {
                        choices.iter().find_map(|choice| {
                            if choice.label == part || render_json_primitive(&choice.value) == part
                            {
                                Some(choice.value.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(())?;

                Ok(Some(Value::Array(values)))
            }
        }
    }

    fn default_or_required_value(&self) -> Result<Option<Value>, ()> {
        if let Some(default) = &self.default {
            return Ok(Some(default.clone()));
        }
        if self.required { Err(()) } else { Ok(None) }
    }
}

pub(crate) fn build_elicitation_content_questions(
    requested_schema: Option<&Value>,
) -> Vec<RequestUserInputQuestion> {
    let Some(requested_schema) = requested_schema else {
        return Vec::new();
    };
    let Some(parsed_schema) = ParsedSchema::from_requested_schema(requested_schema) else {
        return Vec::new();
    };

    parsed_schema
        .fields
        .into_iter()
        .map(|field| field.to_question())
        .collect()
}

pub(crate) fn build_elicitation_content_from_response(
    response: &RequestUserInputResponse,
    requested_schema: Option<&Value>,
) -> Result<Option<Value>, ()> {
    let Some(requested_schema) = requested_schema else {
        return Ok(None);
    };
    let Some(parsed_schema) = ParsedSchema::from_requested_schema(requested_schema) else {
        return Ok(None);
    };

    let mut content = Map::new();
    for field in parsed_schema.fields {
        if let Some(value) = field.parse_response_value(&response.answers)? {
            content.insert(field.property_name, value);
        }
    }
    Ok(Some(Value::Object(content)))
}

fn parse_field_kind(schema_obj: &Map<String, Value>) -> Option<FieldKind> {
    let field_type = schema_obj.get("type").and_then(Value::as_str);

    if field_type == Some("string") {
        if let Some(choices) = parse_choices_from_enum(schema_obj) {
            return Some(FieldKind::SingleSelectEnum { choices });
        }
        if let Some(choices) = parse_choices_from_const_union(schema_obj.get("oneOf")) {
            return Some(FieldKind::SingleSelectEnum { choices });
        }
        return Some(FieldKind::String {
            min_length: schema_obj
                .get("minLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            max_length: schema_obj
                .get("maxLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            pattern: schema_obj
                .get("pattern")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            format: schema_obj
                .get("format")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        });
    }

    if matches!(field_type, Some("number") | Some("integer")) {
        return Some(FieldKind::Number {
            integer: field_type == Some("integer"),
            minimum: schema_obj.get("minimum").and_then(Value::as_f64),
            maximum: schema_obj.get("maximum").and_then(Value::as_f64),
        });
    }

    if field_type == Some("boolean") {
        return Some(FieldKind::Boolean);
    }

    if field_type == Some("array")
        && let Some(items) = schema_obj.get("items").and_then(Value::as_object)
    {
        if let Some(choices) = parse_choices_from_enum(items) {
            return Some(FieldKind::MultiSelectEnum {
                choices,
                min_items: schema_obj
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
                max_items: schema_obj
                    .get("maxItems")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
            });
        }
        if let Some(choices) = parse_choices_from_const_union(items.get("anyOf")) {
            return Some(FieldKind::MultiSelectEnum {
                choices,
                min_items: schema_obj
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
                max_items: schema_obj
                    .get("maxItems")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
            });
        }
    }

    None
}

fn parse_choices_from_enum(schema_obj: &Map<String, Value>) -> Option<Vec<EnumChoice>> {
    let values = schema_obj.get("enum")?.as_array()?;
    let choices: Vec<EnumChoice> = values
        .iter()
        .filter(|value| is_supported_primitive(value))
        .map(|value| EnumChoice {
            value: value.clone(),
            label: render_json_primitive(value),
        })
        .collect();
    if choices.is_empty() {
        None
    } else {
        Some(choices)
    }
}

fn parse_choices_from_const_union(value: Option<&Value>) -> Option<Vec<EnumChoice>> {
    let entries = value?.as_array()?;
    let choices: Vec<EnumChoice> = entries
        .iter()
        .filter_map(|entry| {
            let obj = entry.as_object()?;
            let const_value = obj.get("const")?;
            if !is_supported_primitive(const_value) {
                return None;
            }
            let label = obj
                .get("title")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| render_json_primitive(const_value));
            Some(EnumChoice {
                value: const_value.clone(),
                label,
            })
        })
        .collect();
    if choices.is_empty() {
        None
    } else {
        Some(choices)
    }
}

fn build_question_id(property_name: &str, index: usize, used_ids: &mut HashSet<String>) -> String {
    let sanitized: String = property_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    let base = format!("{MCP_ELICITATION_FIELD_ID_PREFIX}_{index}_{sanitized}");
    let mut candidate = base.clone();
    let mut suffix = 1usize;
    while !used_ids.insert(candidate.clone()) {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    candidate
}

fn extract_selection_and_note(
    answer: Option<&RequestUserInputAnswer>,
) -> (Option<&str>, Option<&str>) {
    let Some(answer) = answer else {
        return (None, None);
    };
    let mut selection = None;
    let mut note = None;
    for entry in &answer.answers {
        if let Some(value) = entry.strip_prefix(REQUEST_USER_INPUT_NOTE_PREFIX) {
            let value = value.trim();
            if !value.is_empty() {
                note = Some(value);
            }
            continue;
        }
        if selection.is_none() {
            let value = entry.trim();
            if !value.is_empty() {
                selection = Some(value);
            }
        }
    }
    (selection, note)
}

fn is_supported_primitive(value: &Value) -> bool {
    value.is_string() || value.is_number() || value.is_boolean()
}

fn render_json_primitive(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        _ => value.to_string(),
    }
}
