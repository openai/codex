//! Natural language interpreter for agent invocation.
//!
//! Parses natural language commands and translates them into agent
//! names and parameters for execution.

use anyhow::Context;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

static DEEP_RESEARCH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^(?:please\s+)?(?:run\s+)?(?:a\s+)?(?:deep[-\s]*research|research\s+report|investigate)(?:\s+on|\s+about)?\s*(?P<topic>.+)?$",
    )
    .expect("valid deep research regex")
});

static ORCHESTRATION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(?:auto[-\s]*orchestrator|ai\s+orchestration|multi-agent\s+session|coordinate\s+agents)(?:.*?with\s+(?P<agents>[a-z0-9_\-\s,]+))?",
    )
    .expect("valid orchestration regex")
});

static ORCHESTRATION_AGENT_LIST_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)with\s+(?P<agents>[a-z0-9_\-\s,]+)").expect("valid agent capture regex")
});

static WEBHOOK_TEXT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"([^"]+)"#).expect("valid quoted text regex"));

/// Parsed agent invocation from natural language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocation {
    /// Agent name to invoke
    pub agent_name: String,
    /// Goal/task description for the agent
    pub goal: String,
    /// Additional parameters parsed from the input
    pub parameters: HashMap<String, String>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Action that should be taken for this invocation
    pub action: AgentAction,
}

/// High-level action that the interpreter resolved from natural language.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentAction {
    /// Delegate work to a specific agent.
    Delegate { agent: String },
    /// Run the auto-orchestrator / pair programming workflow.
    AutoOrchestrate,
    /// Conduct deep research.
    DeepResearch { use_gemini: bool, use_mcp: bool },
    /// Trigger a webhook integration.
    TriggerWebhook { service: WebhookServiceKind },
    /// List configured MCP tools.
    ListMcpTools,
}

/// Supported webhook services for natural language invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WebhookServiceKind {
    Slack,
    Github,
    Custom,
}

/// Natural language interpreter for agent commands.
pub struct AgentInterpreter {
    /// Precompiled patterns for matching agent intents
    patterns: Vec<Pattern>,
}

#[derive(Clone)]
struct Pattern {
    /// Regular expression to match
    regex: Regex,
    /// Agent name to invoke
    agent_name: String,
    /// Parameter extractors
    param_extractors: Vec<ParamExtractor>,
    /// Base confidence for this pattern
    confidence: f64,
}

#[derive(Clone)]
struct ParamExtractor {
    /// Parameter name
    name: String,
    /// Capture group index in regex
    group_index: usize,
    /// Default value if not captured
    default: Option<String>,
}

impl AgentInterpreter {
    /// Create a new agent interpreter with default patterns.
    pub fn new() -> Self {
        Self {
            patterns: Self::default_patterns(),
        }
    }

    /// Parse natural language input into an agent invocation.
    pub fn parse(&self, input: &str) -> Result<AgentInvocation> {
        let input_lower = input.to_lowercase();

        if let Some(invocation) = self.try_parse_special_actions(input, &input_lower) {
            return Ok(invocation);
        }

        // Try each pattern in order of confidence
        for pattern in &self.patterns {
            if let Some(captures) = pattern.regex.captures(&input_lower) {
                let mut parameters = HashMap::new();

                // Extract parameters from capture groups
                for extractor in &pattern.param_extractors {
                    let value = captures
                        .get(extractor.group_index)
                        .map(|m| m.as_str().to_string())
                        .or_else(|| extractor.default.clone());

                    if let Some(v) = value {
                        parameters.insert(extractor.name.clone(), v);
                    }
                }

                return Ok(AgentInvocation {
                    agent_name: pattern.agent_name.clone(),
                    goal: input.to_string(),
                    parameters: parameters.clone(),
                    confidence: pattern.confidence,
                    action: AgentAction::Delegate {
                        agent: pattern.agent_name.clone(),
                    },
                });
            }
        }

        // Fallback: use code-reviewer for general tasks
        Ok(AgentInvocation {
            agent_name: "code-reviewer".to_string(),
            goal: input.to_string(),
            parameters: HashMap::new(),
            confidence: 0.3,
            action: AgentAction::Delegate {
                agent: "code-reviewer".to_string(),
            },
        })
    }

    fn try_parse_special_actions(&self, input: &str, input_lower: &str) -> Option<AgentInvocation> {
        if let Some(caps) = DEEP_RESEARCH_REGEX.captures(input) {
            let topic = caps
                .name("topic")
                .map(|m| m.as_str().trim().to_string())
                .filter(|topic| !topic.is_empty())
                .unwrap_or_else(|| input.trim().to_string());

            let mut parameters = HashMap::new();
            parameters.insert("topic".to_string(), topic.clone());

            let use_gemini = input_lower.contains("gemini");
            let use_mcp = input_lower.contains("mcp") || input_lower.contains("multi-client");

            return Some(AgentInvocation {
                agent_name: "deep-research".to_string(),
                goal: topic,
                parameters,
                confidence: 0.9,
                action: AgentAction::DeepResearch {
                    use_gemini,
                    use_mcp,
                },
            });
        }

        if input_lower.contains("mcp") {
            return Some(AgentInvocation {
                agent_name: "mcp".to_string(),
                goal: input.trim().to_string(),
                parameters: HashMap::new(),
                confidence: 0.85,
                action: AgentAction::ListMcpTools,
            });
        }

        if input_lower.contains("hook") || input_lower.contains("webhook") {
            let mut parameters = HashMap::new();
            if let Some(channel) = extract_channel_name(input) {
                parameters.insert("channel".to_string(), channel);
            }
            let message = extract_webhook_message(input);
            parameters.insert("message".to_string(), message.clone());

            let service = if input_lower.contains("slack") {
                WebhookServiceKind::Slack
            } else if input_lower.contains("github") {
                WebhookServiceKind::Github
            } else {
                WebhookServiceKind::Custom
            };

            return Some(AgentInvocation {
                agent_name: format!("{:?}-webhook", service).to_lowercase(),
                goal: message,
                parameters,
                confidence: 0.8,
                action: AgentAction::TriggerWebhook { service },
            });
        }

        if let Some(caps) = ORCHESTRATION_REGEX.captures(input) {
            let mut parameters = HashMap::new();
            if let Some(agent_list_match) = ORCHESTRATION_AGENT_LIST_REGEX.captures(input) {
                if let Some(agent_list) = agent_list_match.name("agents") {
                    parameters.insert("agents".to_string(), agent_list.as_str().trim().to_string());
                }
            }

            if let Some(agent_capture) = caps.name("agents") {
                parameters.insert(
                    "agents".to_string(),
                    agent_capture.as_str().trim().to_string(),
                );
            }

            let goal = input.trim().to_string();
            return Some(AgentInvocation {
                agent_name: "auto-orchestrator".to_string(),
                goal,
                parameters,
                confidence: 0.92,
                action: AgentAction::AutoOrchestrate,
            });
        }

        None
    }

    /// Get default patterns for common agent invocations.
    fn default_patterns() -> Vec<Pattern> {
        vec![
            // Security patterns
            Pattern {
                regex: Regex::new(
                    r"(?i)(security|sec|audit|vulnerability|vuln|exploit|cve|oauth|auth|jwt|token)",
                )
                .unwrap(),
                agent_name: "sec-audit".to_string(),
                param_extractors: vec![],
                confidence: 0.95,
            },
            // Test patterns
            Pattern {
                regex: Regex::new(r"(?i)(test|unit test|integration test|e2e|spec|jest|pytest)")
                    .unwrap(),
                agent_name: "test-gen".to_string(),
                param_extractors: vec![],
                confidence: 0.9,
            },
            // Review patterns
            Pattern {
                regex: Regex::new(r"(?i)(review|check|inspect|analyze|examine|lint)(?:\s+(.+))?")
                    .unwrap(),
                agent_name: "code-reviewer".to_string(),
                param_extractors: vec![ParamExtractor {
                    name: "scope".to_string(),
                    group_index: 2,
                    default: Some(".".to_string()),
                }],
                confidence: 0.85,
            },
            // Research patterns
            Pattern {
                regex: Regex::new(
                    r"(?i)(research|investigate|learn|study|find out|explore)(?:\s+(.+))?",
                )
                .unwrap(),
                agent_name: "researcher".to_string(),
                param_extractors: vec![ParamExtractor {
                    name: "query".to_string(),
                    group_index: 2,
                    default: None,
                }],
                confidence: 0.8,
            },
            // TypeScript specific
            Pattern {
                regex: Regex::new(r"(?i)(typescript|ts|tsx|react)").unwrap(),
                agent_name: "ts-reviewer".to_string(),
                param_extractors: vec![],
                confidence: 0.75,
            },
            // Python specific
            Pattern {
                regex: Regex::new(r"(?i)(python|py|pytest|django|flask)").unwrap(),
                agent_name: "python-reviewer".to_string(),
                param_extractors: vec![],
                confidence: 0.75,
            },
            // Unity specific
            Pattern {
                regex: Regex::new(r"(?i)(unity|c#|csharp|game|gameobject)").unwrap(),
                agent_name: "unity-reviewer".to_string(),
                param_extractors: vec![],
                confidence: 0.75,
            },
        ]
    }

    /// Add a custom pattern to the interpreter.
    pub fn add_pattern(
        &mut self,
        regex_str: &str,
        agent_name: String,
        confidence: f64,
    ) -> Result<()> {
        let regex =
            Regex::new(regex_str).with_context(|| format!("Invalid regex pattern: {regex_str}"))?;

        self.patterns.push(Pattern {
            regex,
            agent_name,
            param_extractors: vec![],
            confidence,
        });

        // Re-sort patterns by confidence (highest first)
        self.patterns.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(())
    }

    /// Get all available agent names from patterns.
    pub fn available_agents(&self) -> Vec<String> {
        let mut agents: Vec<String> = self.patterns.iter().map(|p| p.agent_name.clone()).collect();
        agents.sort();
        agents.dedup();
        agents
    }
}

impl Default for AgentInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_pattern() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Security audit this authentication module")
            .unwrap();
        assert_eq!(result.agent_name, "sec-audit");
        assert!(result.confidence > 0.9);
        assert_eq!(
            result.action,
            AgentAction::Delegate {
                agent: "sec-audit".to_string()
            }
        );
    }

    #[test]
    fn test_test_pattern() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Generate unit tests for this component")
            .unwrap();
        assert_eq!(result.agent_name, "test-gen");
        assert!(result.confidence > 0.8);
        assert_eq!(
            result.action,
            AgentAction::Delegate {
                agent: "test-gen".to_string()
            }
        );
    }

    #[test]
    fn test_review_pattern() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Review this file with security focus")
            .unwrap();
        assert_eq!(result.agent_name, "code-reviewer");
        assert_eq!(
            result.action,
            AgentAction::Delegate {
                agent: "code-reviewer".to_string()
            }
        );
    }

    #[test]
    fn test_research_pattern() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Research React Server Components best practices")
            .unwrap();
        assert_eq!(result.agent_name, "researcher");
        assert_eq!(
            result.action,
            AgentAction::Delegate {
                agent: "researcher".to_string()
            }
        );
    }

    #[test]
    fn test_typescript_pattern() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter.parse("Review this TypeScript code").unwrap();
        assert_eq!(result.agent_name, "ts-reviewer");
    }

    #[test]
    fn test_fallback() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter.parse("Do something generic").unwrap();
        assert_eq!(result.agent_name, "code-reviewer");
        assert!(result.confidence < 0.5);
        assert_eq!(
            result.action,
            AgentAction::Delegate {
                agent: "code-reviewer".to_string()
            }
        );
    }

    #[test]
    fn test_custom_pattern() {
        let mut interpreter = AgentInterpreter::new();
        interpreter
            .add_pattern(r"(?i)refactor", "code-reviewer".to_string(), 0.85)
            .unwrap();

        let result = interpreter.parse("Refactor this module").unwrap();
        assert_eq!(result.agent_name, "code-reviewer");
    }

    #[test]
    fn test_deep_research_detection() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Deep research on Rust security using Gemini and MCP")
            .unwrap();

        assert_eq!(result.agent_name, "deep-research");
        assert_eq!(
            result.parameters.get("topic").unwrap(),
            "Rust security using Gemini and MCP"
        );
        assert_eq!(
            result.action,
            AgentAction::DeepResearch {
                use_gemini: true,
                use_mcp: true
            }
        );
    }

    #[test]
    fn test_orchestration_detection() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("AI orchestration with agent-alpha, agent-beta for refactor")
            .unwrap();

        assert_eq!(result.agent_name, "auto-orchestrator");
        assert_eq!(
            result.parameters.get("agents").unwrap(),
            "agent-alpha, agent-beta"
        );
        assert_eq!(result.action, AgentAction::AutoOrchestrate);
    }

    #[test]
    fn test_webhook_detection() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Send Slack webhook to #alerts: \"Deploy succeeded\"")
            .unwrap();

        assert_eq!(result.agent_name, "slack-webhook");
        assert_eq!(result.parameters.get("channel").unwrap(), "#alerts");
        assert_eq!(
            result.action,
            AgentAction::TriggerWebhook {
                service: WebhookServiceKind::Slack
            }
        );
    }

    #[test]
    fn test_mcp_detection() {
        let interpreter = AgentInterpreter::new();

        let result = interpreter
            .parse("Show me the MCP tools configured for this project")
            .unwrap();

        assert_eq!(result.agent_name, "mcp");
        assert_eq!(result.action, AgentAction::ListMcpTools);
    }
}

fn extract_channel_name(input: &str) -> Option<String> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    for window in tokens.windows(2) {
        if window[0].eq_ignore_ascii_case("channel") {
            if let Some(channel) = clean_channel_token(window[1]) {
                return Some(channel);
            }
        }
    }

    tokens.iter().find_map(|token| clean_channel_token(token))
}

fn extract_webhook_message(input: &str) -> String {
    if let Some(captures) = WEBHOOK_TEXT_REGEX.captures(input) {
        if let Some(matched) = captures.get(1) {
            let text = matched.as_str().trim();
            if !text.is_empty() {
                return text.to_string();
            }
        }
    }

    if let Some(idx) = input.find(':') {
        let text = input[idx + 1..].trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }

    input.trim().to_string()
}

fn clean_channel_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | '.' | '!'));
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('#') || trimmed.starts_with('@') {
        return Some(trimmed.to_string());
    }

    None
}
