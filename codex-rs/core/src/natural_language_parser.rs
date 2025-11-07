//! Natural language command parser
//!
//! Converts natural language input to slash commands and CLI subcommands.
//! Supports Japanese and English.

use std::collections::HashMap;

/// Natural language patterns for slash commands
pub struct NaturalLanguageParser {
    /// Mapping from natural language patterns to slash commands
    patterns: HashMap<&'static str, Vec<&'static str>>,
}

impl NaturalLanguageParser {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();

        // /compact patterns
        patterns.insert(
            "compact",
            vec![
                "圧縮",
                "要約",
                "まとめ",
                "短く",
                "コンパクト",
                "compress",
                "compact",
                "summarize",
                "shorten",
            ],
        );

        // /review patterns
        patterns.insert(
            "review",
            vec![
                "レビュー",
                "チェック",
                "確認",
                "見て",
                "検証",
                "review",
                "check",
                "verify",
                "inspect",
                "examine",
            ],
        );

        // /delegate patterns
        patterns.insert(
            "delegate",
            vec![
                "委譲",
                "依頼",
                "頼む",
                "エージェントに",
                "サブエージェント",
                "delegate",
                "assign",
                "request",
                "agent",
                "subagent",
            ],
        );

        // /research patterns
        patterns.insert(
            "research",
            vec![
                "調査",
                "研究",
                "リサーチ",
                "調べて",
                "検索",
                "research",
                "investigate",
                "study",
                "search",
                "explore",
            ],
        );

        // /orchestrate patterns
        patterns.insert(
            "orchestrate",
            vec![
                "オーケストレーション",
                "並列実行",
                "複数エージェント",
                "統制",
                "orchestrate",
                "parallel",
                "multiple agents",
                "coordinate",
            ],
        );

        // /plan patterns
        patterns.insert(
            "plan",
            vec!["計画", "プラン", "設計", "企画", "plan", "design", "scheme"],
        );

        // /new patterns
        patterns.insert(
            "new",
            vec![
                "新規",
                "新しい",
                "新規作成",
                "スタート",
                "new",
                "start",
                "fresh",
                "begin",
            ],
        );

        // /diff patterns
        patterns.insert(
            "diff",
            vec![
                "差分",
                "変更",
                "違い",
                "比較",
                "diff",
                "difference",
                "changes",
                "compare",
            ],
        );

        // /status patterns
        patterns.insert(
            "status",
            vec![
                "状態",
                "ステータス",
                "現在の",
                "情報",
                "status",
                "state",
                "info",
                "current",
            ],
        );

        Self { patterns }
    }

    /// Parse natural language input to slash command
    ///
    /// Returns (command_name, remaining_text) if matched, None otherwise
    pub fn parse_to_slash_command(&self, input: &str) -> Option<(&'static str, String)> {
        let input_lower = input.to_lowercase();

        for (command, keywords) in &self.patterns {
            for keyword in keywords {
                if input_lower.starts_with(keyword) {
                    // Extract remaining text after the keyword
                    let remaining = input[keyword.len()..].trim().to_string();
                    return Some((command, remaining));
                }

                // Check if keyword appears within the first few words
                let words: Vec<&str> = input_lower.split_whitespace().take(3).collect();
                if words.iter().any(|w| w.contains(keyword)) {
                    // Try to extract the main content after the command phrase
                    let remaining = input.trim().to_string();
                    return Some((command, remaining));
                }
            }
        }

        None
    }

    /// Parse natural language input to CLI subcommand
    ///
    /// Returns (subcommand, args) if matched, None otherwise
    pub fn parse_to_cli_subcommand(&self, input: &str) -> Option<(String, Vec<String>)> {
        let input_lower = input.to_lowercase();

        // Check for delegate patterns
        if self.contains_any_keyword(
            &input_lower,
            &["委譲", "依頼", "delegate", "エージェントに"],
        ) {
            // Extract agent name and goal
            // Example: "リサーチャーに依頼してReactを調査"
            // → ("delegate", ["researcher", "Reactを調査"])

            if let Some(agent_name) = self.extract_agent_name(&input) {
                let goal = self.extract_goal(&input, &agent_name);
                return Some(("delegate".to_string(), vec![agent_name, goal]));
            }
        }

        // Check for research patterns
        if self.contains_any_keyword(&input_lower, &["調査", "研究", "リサーチ", "research"])
        {
            // Extract research topic
            let topic = input.trim().to_string();
            return Some(("research".to_string(), vec![topic]));
        }

        // Check for plan/blueprint patterns
        if self.contains_any_keyword(&input_lower, &["計画", "プラン", "plan", "blueprint"]) {
            let description = input.trim().to_string();
            return Some(("plan".to_string(), vec!["create".to_string(), description]));
        }

        None
    }

    fn contains_any_keyword(&self, text: &str, keywords: &[&str]) -> bool {
        keywords.iter().any(|kw| text.contains(kw))
    }

    fn extract_agent_name(&self, input: &str) -> Option<String> {
        // Common agent names
        let agents = vec![
            ("researcher", vec!["リサーチャー", "研究者", "researcher"]),
            (
                "code-reviewer",
                vec!["レビューアー", "レビュー", "code-reviewer", "reviewer"],
            ),
            ("test-gen", vec!["テスト", "test-gen", "test", "testing"]),
            (
                "sec-audit",
                vec!["セキュリティ", "監査", "sec-audit", "security"],
            ),
        ];

        let input_lower = input.to_lowercase();
        for (agent_name, keywords) in agents {
            if keywords.iter().any(|kw| input_lower.contains(kw)) {
                return Some(agent_name.to_string());
            }
        }

        None
    }

    fn extract_goal(&self, input: &str, _agent_name: &str) -> String {
        // Simple extraction: remove the delegation phrase and agent name
        let input_lower = input.to_lowercase();

        // Find the start of the actual goal/task
        if let Some(pos) = input_lower.find("して") {
            return input[pos + "して".len()..].trim().to_string();
        }
        if let Some(pos) = input_lower.find(" to ") {
            return input[pos + " to ".len()..].trim().to_string();
        }

        // Fallback: return original input
        input.trim().to_string()
    }
}

impl Default for NaturalLanguageParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compact_japanese() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse_to_slash_command("圧縮してください");
        assert_eq!(result.map(|(cmd, _)| cmd), Some("compact"));
    }

    #[test]
    fn test_parse_review_english() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse_to_slash_command("review this code");
        assert_eq!(result.map(|(cmd, _)| cmd), Some("review"));
    }

    #[test]
    fn test_parse_delegate_japanese() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse_to_cli_subcommand("リサーチャーに依頼してReactを調査");
        assert!(result.is_some());
        let (cmd, args) = result.unwrap();
        assert_eq!(cmd, "delegate");
        assert_eq!(args[0], "researcher");
    }

    #[test]
    fn test_parse_plan_japanese() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse_to_slash_command("計画を作成して");
        assert_eq!(result.map(|(cmd, _)| cmd), Some("plan"));
    }
}
