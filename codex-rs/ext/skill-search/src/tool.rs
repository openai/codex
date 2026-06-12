use std::collections::BTreeMap;

use bm25::Document;
use bm25::Language;
use bm25::SearchEngine;
use bm25::SearchEngineBuilder;
use codex_core::skills::SkillMetadata;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::JsonSchema;
use codex_tools::ToolPayload;
use serde::Deserialize;
use serde_json::Value;

pub(crate) const SKILL_SEARCH_TOOL_NAME: &str = "skill_search";
const DEFAULT_SKILL_SEARCH_LIMIT: usize = 8;

#[derive(Clone, Debug)]
struct SkillSearchEntry {
    name: String,
    description: String,
    path: String,
    search_text: String,
}

impl SkillSearchEntry {
    fn from_metadata(skill: SkillMetadata) -> Self {
        let path = skill.path_to_skills_md.to_string_lossy().replace('\\', "/");
        let search_text = format!("{}\n{}", skill.name, skill.description);
        Self {
            name: skill.name,
            description: skill.description,
            path,
            search_text,
        }
    }

    fn render(&self) -> String {
        if self.description.is_empty() {
            format!("- {}: (file: {})", self.name, self.path)
        } else {
            format!(
                "- {}: {} (file: {})",
                self.name, self.description, self.path
            )
        }
    }
}

pub(crate) struct SkillSearchTool {
    entries: Vec<SkillSearchEntry>,
    search_engine: SearchEngine<usize>,
}

impl SkillSearchTool {
    pub(crate) fn new(skills: Vec<SkillMetadata>) -> Self {
        let entries = skills
            .into_iter()
            .map(SkillSearchEntry::from_metadata)
            .collect::<Vec<_>>();
        let documents = entries
            .iter()
            .map(|entry| entry.search_text.clone())
            .enumerate()
            .map(|(idx, search_text)| Document::new(idx, search_text))
            .collect::<Vec<_>>();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

        Self {
            entries,
            search_engine,
        }
    }

    fn search(&self, query: &str, limit: usize) -> String {
        self.search_engine
            .search(query, limit)
            .into_iter()
            .filter_map(|result| self.entries.get(result.document.id))
            .map(SkillSearchEntry::render)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillSearchArgs {
    query: String,
    limit: Option<usize>,
}

impl ToolExecutor<ToolCall> for SkillSearchTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(SKILL_SEARCH_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: SKILL_SEARCH_TOOL_NAME.to_string(),
            description: "Search available Codex skills by relevance and return plain-text matches with their descriptions and SKILL.md paths.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([
                    (
                        "limit".to_string(),
                        JsonSchema::number(Some(format!(
                            "Maximum number of skills to return (defaults to {DEFAULT_SKILL_SEARCH_LIMIT})."
                        ))),
                    ),
                    (
                        "query".to_string(),
                        JsonSchema::string(Some("Search query for available skills.".to_string())),
                    ),
                ]),
                Some(vec!["query".to_string()]),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args = parse_args(&call)?;
            let query = args.query.trim();
            if query.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "query must not be empty".to_string(),
                ));
            }
            let limit = args.limit.unwrap_or(DEFAULT_SKILL_SEARCH_LIMIT);
            if limit == 0 {
                return Err(FunctionCallError::RespondToModel(
                    "limit must be greater than zero".to_string(),
                ));
            }

            Ok(Box::new(PlainTextToolOutput {
                text: self.search(query, limit),
            }) as Box<dyn ToolOutput>)
        })
    }
}

fn parse_args(call: &ToolCall) -> Result<SkillSearchArgs, FunctionCallError> {
    let arguments = call.function_arguments()?;
    let value = if arguments.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(arguments)
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?
    };
    serde_json::from_value(value).map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlainTextToolOutput {
    text: String,
}

impl ToolOutput for PlainTextToolOutput {
    fn log_preview(&self) -> String {
        self.text.clone()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.text.clone()),
                success: Some(true),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_core::skills::SkillPolicy;
    use codex_extension_api::ConversationHistory;
    use codex_extension_api::NoopTurnItemEmitter;
    use codex_protocol::models::FunctionCallOutputBody;
    use codex_protocol::protocol::SkillScope;
    use codex_protocol::protocol::TruncationPolicy;
    use codex_tools::ToolPayload;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn skill(name: &str, description: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: description.to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: Some(SkillPolicy {
                allow_implicit_invocation: Some(true),
                products: Vec::new(),
            }),
            path_to_skills_md: test_path_buf(path).abs(),
            scope: SkillScope::User,
            plugin_id: None,
        }
    }

    fn call(arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            turn_id: "turn-skill-search".to_string(),
            call_id: "call-skill-search".to_string(),
            tool_name: ToolName::plain(SKILL_SEARCH_TOOL_NAME),
            model: "gpt-test".to_string(),
            truncation_policy: TruncationPolicy::Bytes(1024),
            conversation_history: ConversationHistory::default(),
            turn_item_emitter: Arc::new(NoopTurnItemEmitter),
            environments: Vec::new(),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        }
    }

    async fn output_text(tool: &SkillSearchTool, arguments: serde_json::Value) -> String {
        let output = tool
            .handle(call(arguments))
            .await
            .expect("skill_search should return output");
        let response = output.to_response_item(
            "call-skill-search",
            &ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        );
        let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
            panic!("expected function output");
        };
        let FunctionCallOutputBody::Text(text) = output.body else {
            panic!("expected text output");
        };
        text
    }

    #[tokio::test]
    async fn search_returns_plain_text_skill_lines() {
        let tool = SkillSearchTool::new(vec![
            skill("slides", "Build presentation decks", "/tmp/slides/SKILL.md"),
            skill("sheets", "Analyze spreadsheet data", "/tmp/sheets/SKILL.md"),
        ]);
        let expected_path = test_path_buf("/tmp/slides/SKILL.md")
            .abs()
            .to_string_lossy()
            .replace('\\', "/");

        assert_eq!(
            output_text(&tool, json!({ "query": "presentation deck" })).await,
            format!("- slides: Build presentation decks (file: {expected_path})")
        );
    }

    #[tokio::test]
    async fn search_returns_empty_text_when_no_skill_matches() {
        let tool = SkillSearchTool::new(vec![skill(
            "slides",
            "Build presentation decks",
            "/tmp/slides/SKILL.md",
        )]);

        assert_eq!(output_text(&tool, json!({ "query": "quantum" })).await, "");
    }

    #[tokio::test]
    async fn search_rejects_empty_query_and_zero_limit() {
        let tool = SkillSearchTool::new(Vec::new());

        let empty_query = tool
            .handle(call(json!({ "query": "   " })))
            .await
            .err()
            .expect("empty query should fail");
        let zero_limit = tool
            .handle(call(json!({ "query": "docs", "limit": 0 })))
            .await
            .err()
            .expect("zero limit should fail");

        assert_eq!(empty_query.to_string(), "query must not be empty");
        assert_eq!(zero_limit.to_string(), "limit must be greater than zero");
    }
}
