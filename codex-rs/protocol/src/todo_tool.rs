use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de;
use serde::ser::SerializeStruct;
use ts_rs::TS;

// Types for the todo_write tool arguments matching codex-vscode/todo-mcp/src/main.rs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TodoItemArg {
    pub step: String,
    pub status: TodoStatus,
}

/// Arguments for the todo_write tool.
///
/// The `plan` field is a deprecated legacy alias for `todo`, but is still emitted for
/// backward compatibility.
#[derive(Debug, Clone, JsonSchema, TS, PartialEq, Eq, Default)]
pub struct UpdateTodoArgs {
    pub explanation: Option<String>,
    pub todo: Vec<TodoItemArg>,
    pub plan: Vec<TodoItemArg>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateTodoArgsRaw {
    #[serde(default)]
    explanation: Option<String>,
    #[serde(default)]
    todo: Option<Vec<TodoItemArg>>,
    #[serde(default)]
    plan: Option<Vec<TodoItemArg>>,
}

impl UpdateTodoArgs {
    pub fn new(explanation: Option<String>, todo: Vec<TodoItemArg>) -> Self {
        let plan = todo.clone();
        Self {
            explanation,
            todo,
            plan,
        }
    }

    pub fn todo_items(&self) -> &[TodoItemArg] {
        if self.todo.is_empty() {
            &self.plan
        } else {
            &self.todo
        }
    }

    pub fn into_todo_items(self) -> Vec<TodoItemArg> {
        if self.todo.is_empty() {
            self.plan
        } else {
            self.todo
        }
    }

    pub fn into_parts(self) -> (Option<String>, Vec<TodoItemArg>) {
        let UpdateTodoArgs {
            explanation,
            todo,
            plan,
        } = self;
        let items = if todo.is_empty() { plan } else { todo };
        (explanation, items)
    }
}

impl Serialize for UpdateTodoArgs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let items = self.todo_items();
        let mut state = serializer.serialize_struct("UpdateTodoArgs", 3)?;
        if let Some(explanation) = &self.explanation {
            state.serialize_field("explanation", explanation)?;
        }
        state.serialize_field("todo", items)?;
        state.serialize_field("plan", items)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for UpdateTodoArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UpdateTodoArgsRaw::deserialize(deserializer)?;
        let items = match (raw.todo, raw.plan) {
            (Some(todo), Some(plan)) => {
                if todo.is_empty() {
                    plan
                } else {
                    todo
                }
            }
            (Some(todo), None) => todo,
            (None, Some(plan)) => plan,
            (None, None) => return Err(de::Error::missing_field("todo")),
        };

        Ok(UpdateTodoArgs::new(raw.explanation, items))
    }
}

#[cfg(test)]
mod tests {
    use super::TodoItemArg;
    use super::TodoStatus;
    use super::UpdateTodoArgs;
    use pretty_assertions::assert_eq;

    #[test]
    fn deserializes_legacy_plan_field() {
        let args: UpdateTodoArgs =
            serde_json::from_str(r#"{"explanation":"x","plan":[{"step":"a","status":"pending"}]}"#)
                .expect("legacy plan field should parse");
        let expected = UpdateTodoArgs::new(
            Some("x".to_string()),
            vec![TodoItemArg {
                step: "a".to_string(),
                status: TodoStatus::Pending,
            }],
        );
        assert_eq!(args, expected);
    }

    #[test]
    fn serializes_todo_and_plan_fields() {
        let args = UpdateTodoArgs::new(
            None,
            vec![TodoItemArg {
                step: "a".to_string(),
                status: TodoStatus::Completed,
            }],
        );
        let value = serde_json::to_value(&args).expect("args should serialize");
        assert_eq!(value.get("todo"), value.get("plan"));
    }
}
