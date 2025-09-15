use once_cell::sync::Lazy;
use regex_lite::Regex;

// Compile regex once at startup
#[allow(clippy::expect_used)]
static AGENT_MENTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"@(\w+):?\s+([^\n@]+)")
        .expect("Failed to compile agent mention regex - this is a bug")
});

#[derive(Debug, Clone)]
pub struct AgentMention {
    pub agent_name: String,
    pub task: String,
    #[allow(dead_code)]
    pub raw_text: String,
    pub start_pos: usize,
    pub end_pos: usize,
}

/// Parse @agent mentions in text
/// Formats supported:
/// - @agent_name: task description
/// - @agent_name task description
pub fn parse_agent_mentions(text: &str) -> Vec<AgentMention> {
    let mut mentions = Vec::new();

    for cap in AGENT_MENTION_RE.captures_iter(text) {
        if let (Some(name), Some(task), Some(full_match)) = (cap.get(1), cap.get(2), cap.get(0)) {
            mentions.push(AgentMention {
                agent_name: name.as_str().to_string(),
                task: task.as_str().trim().to_string(),
                raw_text: full_match.as_str().to_string(),
                start_pos: full_match.start(),
                end_pos: full_match.end(),
            });
        }
    }
    mentions
}

/// Convert agent mention to tool call format
pub fn convert_to_agent_call(mention: &AgentMention) -> String {
    format!("Use the {} agent to {}", mention.agent_name, mention.task)
}

/// Replace mentions in text with converted calls
pub fn replace_mentions_with_calls(text: &str) -> String {
    let mentions = parse_agent_mentions(text);
    if mentions.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Replace from end to start to preserve positions
    for mention in mentions.iter().rev() {
        let call = convert_to_agent_call(mention);
        result.replace_range(mention.start_pos..mention.end_pos, &call);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_mention() {
        let text = "@researcher: find information about Rust";
        let mentions = parse_agent_mentions(text);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].agent_name, "researcher");
        assert_eq!(mentions[0].task, "find information about Rust");
    }

    #[test]
    fn test_parse_multiple_mentions() {
        let text = "@researcher: find docs @reviewer: check the code";
        let mentions = parse_agent_mentions(text);
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].agent_name, "researcher");
        assert_eq!(mentions[1].agent_name, "reviewer");
    }

    #[test]
    fn test_convert_to_agent_call() {
        let mention = AgentMention {
            agent_name: "researcher".to_string(),
            task: "find information".to_string(),
            raw_text: "@researcher: find information".to_string(),
            start_pos: 0,
            end_pos: 29,
        };
        let call = convert_to_agent_call(&mention);
        assert_eq!(call, "Use the researcher agent to find information");
    }

    #[test]
    fn test_replace_mentions() {
        let text = "@researcher: find docs about async";
        let replaced = replace_mentions_with_calls(text);
        assert_eq!(
            replaced,
            "Use the researcher agent to find docs about async"
        );
    }
}
