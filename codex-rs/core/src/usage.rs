use crate::tools::router::ToolRouter;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::UsageAttributionContributor;
use codex_protocol::protocol::UsageAttributionItem;
use codex_protocol::protocol::UsageContributor;
use codex_protocol::protocol::UsageContributorKind;
use codex_tools::ToolName;
use codex_utils_output_truncation::approx_token_count;
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct UsagePromptAttribution {
    pub(crate) prompt_estimated_tokens: i64,
    pub(crate) contributors: Vec<UsagePromptContributor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsagePromptContributor {
    pub(crate) contributor: UsageContributor,
    pub(crate) source_estimated_tokens: i64,
}

impl UsagePromptAttribution {
    pub(crate) fn from_prompt(
        input: &[ResponseItem],
        router: &ToolRouter,
        base_instructions: &str,
    ) -> Self {
        let mut contributors = skill_contributors(input);
        contributors.extend(router.usage_contributors());
        contributors.extend(tool_result_contributors(input, router));
        let input_tokens = input
            .iter()
            .map(estimate_response_item_tokens)
            .fold(0i64, i64::saturating_add);
        let tool_tokens = router
            .model_visible_specs()
            .iter()
            .map(estimate_serialized_tokens)
            .fold(0i64, i64::saturating_add);
        let base_tokens = i64::try_from(approx_token_count(base_instructions)).unwrap_or(i64::MAX);
        Self {
            prompt_estimated_tokens: base_tokens
                .saturating_add(input_tokens)
                .saturating_add(tool_tokens),
            contributors: aggregate_contributors(contributors),
        }
    }

    pub(crate) fn complete(
        &self,
        sample_id: String,
        turn_id: String,
        response_id: String,
        occurred_at: i64,
        token_usage: TokenUsage,
    ) -> UsageAttributionItem {
        let non_cached_input = token_usage.non_cached_input();
        let contributors = self
            .contributors
            .iter()
            .map(|contributor| UsageAttributionContributor {
                contributor: contributor.contributor.clone(),
                source_estimated_tokens: contributor.source_estimated_tokens,
                attributed_tokens: attributable_tokens(
                    non_cached_input,
                    contributor.source_estimated_tokens,
                    self.prompt_estimated_tokens,
                ),
            })
            .filter(|contributor| contributor.attributed_tokens > 0)
            .collect();
        UsageAttributionItem {
            sample_id,
            turn_id,
            response_id,
            occurred_at,
            token_usage,
            prompt_estimated_tokens: self.prompt_estimated_tokens,
            contributors,
        }
    }
}

pub(crate) fn estimate_serialized_tokens<T: serde::Serialize>(value: &T) -> i64 {
    serde_json::to_string(value)
        .map(|serialized| i64::try_from(approx_token_count(&serialized)).unwrap_or(i64::MAX))
        .unwrap_or(/*default*/ 0)
}

fn estimate_response_item_tokens(item: &ResponseItem) -> i64 {
    estimate_serialized_tokens(item)
}

fn skill_contributors(input: &[ResponseItem]) -> Vec<UsagePromptContributor> {
    input.iter().filter_map(skill_contributor).collect()
}

fn tool_result_contributors(
    input: &[ResponseItem],
    router: &ToolRouter,
) -> Vec<UsagePromptContributor> {
    let contributors_by_call_id = input
        .iter()
        .filter_map(|item| tool_call_contributors(item, router))
        .collect::<HashMap<_, _>>();
    input
        .iter()
        .filter_map(|item| tool_result_contributor(item, &contributors_by_call_id))
        .flatten()
        .collect()
}

fn tool_call_contributors(
    item: &ResponseItem,
    router: &ToolRouter,
) -> Option<(String, Vec<UsageContributor>)> {
    let (call_id, tool_name) = match item {
        ResponseItem::FunctionCall {
            call_id,
            name,
            namespace,
            ..
        } => (call_id, ToolName::new(namespace.clone(), name)),
        ResponseItem::CustomToolCall { call_id, name, .. } => (call_id, ToolName::plain(name)),
        _ => return None,
    };
    let contributors = router.usage_contributors_for_tool_name(&tool_name);
    (!contributors.is_empty()).then(|| (call_id.clone(), contributors))
}

fn tool_result_contributor(
    item: &ResponseItem,
    contributors_by_call_id: &HashMap<String, Vec<UsageContributor>>,
) -> Option<Vec<UsagePromptContributor>> {
    let call_id = match item {
        ResponseItem::FunctionCallOutput { call_id, .. }
        | ResponseItem::CustomToolCallOutput { call_id, .. } => call_id,
        _ => return None,
    };
    let source_estimated_tokens = estimate_response_item_tokens(item);
    Some(
        contributors_by_call_id
            .get(call_id)?
            .iter()
            .cloned()
            .map(|contributor| UsagePromptContributor {
                contributor,
                source_estimated_tokens,
            })
            .collect(),
    )
}

fn skill_contributor(item: &ResponseItem) -> Option<UsagePromptContributor> {
    let ResponseItem::Message { content, .. } = item else {
        return None;
    };
    let text = content.iter().find_map(|content| match content {
        ContentItem::InputText { text } if text.contains("<skill>") => Some(text.as_str()),
        _ => None,
    })?;
    let name = tag_contents(text, "name")?;
    let path = tag_contents(text, "path")?;
    Some(UsagePromptContributor {
        contributor: UsageContributor {
            kind: UsageContributorKind::Skill,
            id: path.to_string(),
            label: name.to_string(),
        },
        source_estimated_tokens: i64::try_from(approx_token_count(text)).unwrap_or(i64::MAX),
    })
}

fn tag_contents<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(open.as_str())? + open.len();
    let end = text[start..].find(close.as_str())? + start;
    Some(text[start..end].trim())
}

fn aggregate_contributors(
    contributors: Vec<UsagePromptContributor>,
) -> Vec<UsagePromptContributor> {
    let mut aggregated = BTreeMap::new();
    for contributor in contributors {
        let key = (
            contributor.contributor.kind as u8,
            contributor.contributor.id.clone(),
            contributor.contributor.label.clone(),
        );
        aggregated
            .entry(key)
            .and_modify(|existing: &mut UsagePromptContributor| {
                existing.source_estimated_tokens = existing
                    .source_estimated_tokens
                    .saturating_add(contributor.source_estimated_tokens);
            })
            .or_insert(contributor);
    }
    aggregated.into_values().collect()
}

fn attributable_tokens(non_cached_input: i64, source_tokens: i64, prompt_tokens: i64) -> i64 {
    if non_cached_input <= 0 || source_tokens <= 0 || prompt_tokens <= 0 {
        return 0;
    }
    non_cached_input
        .saturating_mul(source_tokens)
        .saturating_add(prompt_tokens / 2)
        / prompt_tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    #[test]
    fn complete_attributes_only_non_cached_input_tokens() {
        let attribution = UsagePromptAttribution {
            prompt_estimated_tokens: 100,
            contributors: vec![
                usage_prompt_contributor(
                    UsageContributorKind::Skill,
                    "/skills/tmux",
                    "tmux",
                    /*source_estimated_tokens*/ 25,
                ),
                usage_prompt_contributor(
                    UsageContributorKind::App,
                    "slack",
                    "Slack",
                    /*source_estimated_tokens*/ 10,
                ),
            ],
        };

        let usage = attribution.complete(
            "sample".to_string(),
            "turn".to_string(),
            "response".to_string(),
            /*occurred_at*/ 1_700_000_000,
            TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 40,
                output_tokens: 20,
                reasoning_output_tokens: 0,
                total_tokens: 120,
            },
        );

        assert_eq!(
            usage.contributors,
            vec![
                UsageAttributionContributor {
                    contributor: usage_contributor(
                        UsageContributorKind::Skill,
                        "/skills/tmux",
                        "tmux",
                    ),
                    source_estimated_tokens: 25,
                    attributed_tokens: 15,
                },
                UsageAttributionContributor {
                    contributor: usage_contributor(UsageContributorKind::App, "slack", "Slack"),
                    source_estimated_tokens: 10,
                    attributed_tokens: 6,
                },
            ]
        );
    }

    #[test]
    fn skill_contributors_use_skill_path_as_stable_id() {
        let item = ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "<skill><name>tmux</name><path>/skills/tmux/SKILL.md</path></skill>"
                    .to_string(),
            }],
            phase: None,
        };

        assert_eq!(
            skill_contributors(&[item]),
            vec![UsagePromptContributor {
                contributor: usage_contributor(
                    UsageContributorKind::Skill,
                    "/skills/tmux/SKILL.md",
                    "tmux",
                ),
                source_estimated_tokens: i64::try_from(approx_token_count(
                    "<skill><name>tmux</name><path>/skills/tmux/SKILL.md</path></skill>",
                ))
                .expect("skill prompt token estimate should fit in i64"),
            }]
        );
    }

    #[test]
    fn tool_results_reuse_tool_usage_provenance() {
        let contributor = usage_contributor(UsageContributorKind::App, "slack", "Slack");
        let tool_name = ToolName::plain("mcp__slack__search");
        let router = ToolRouter::from_parts(
            ToolRegistry::from_tools(Vec::<
                std::sync::Arc<dyn crate::tools::registry::CoreToolRuntime>,
            >::new()),
            Vec::new(),
            Vec::new(),
            HashMap::from([(tool_name.clone(), vec![contributor.clone()])]),
        );
        let tool_result = ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("result".to_string()),
        };
        let input = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: tool_name.name,
                namespace: tool_name.namespace,
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            tool_result.clone(),
        ];

        assert_eq!(
            tool_result_contributors(&input, &router),
            vec![UsagePromptContributor {
                contributor,
                source_estimated_tokens: estimate_response_item_tokens(&tool_result),
            }]
        );
    }

    fn usage_prompt_contributor(
        kind: UsageContributorKind,
        id: &str,
        label: &str,
        source_estimated_tokens: i64,
    ) -> UsagePromptContributor {
        UsagePromptContributor {
            contributor: usage_contributor(kind, id, label),
            source_estimated_tokens,
        }
    }

    fn usage_contributor(kind: UsageContributorKind, id: &str, label: &str) -> UsageContributor {
        UsageContributor {
            kind,
            id: id.to_string(),
            label: label.to_string(),
        }
    }
}
