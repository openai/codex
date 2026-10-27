//! Validates and renders the desktop safety-recourse metadata contract.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Domain {
    Bio,
    Cyber,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    schema_version: u8,
    domain: Domain,
    status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResult {
    schema_version: u8,
    domain: Domain,
    source: String,
    source_message: SourceMessage,
    topic: String,
    suggestions: [Suggestion; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceMessage {
    id: String,
    kind: String,
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suggestion {
    label: String,
    prompt_text: String,
}

pub(super) struct Recourse {
    pub(super) explanation: String,
    pub(super) suggestions: Vec<(String, String)>,
}

pub(super) fn parse(mut value: Value) -> Option<(Domain, Option<Recourse>)> {
    let object = value.as_object_mut()?;
    let recourse = object.remove("safety_recourse").filter(|v| !v.is_null());
    let Some(value) = recourse else {
        let block: Block = serde_json::from_value(object.remove("safety_block")?).ok()?;
        return (block.schema_version == 1 && block.status == "loading")
            .then_some((block.domain, None));
    };
    let wire: WireResult = serde_json::from_value(value).ok()?;
    let message = &wire.source_message;
    if wire.schema_version != 1 || !valid_text(&message.id, /*max_chars*/ 256) {
        return None;
    }
    let (bio_lead, cyber_lead, bio_offer) = match (
        wire.source.as_str(),
        message.kind.as_str(),
        message.role.as_str(),
    ) {
        ("user_request", "ordinary_message", "user") => (
            "with requests involving ",
            "with requests that could enable ",
            None,
        ),
        ("assistant_response", "ordinary_message", "assistant") => (
            "when a response would provide information about ",
            "when a response could enable ",
            None,
        ),
        ("file_content", "file_tool_call", "assistant")
        | ("file_content", "file_tool_result", "tool") => (
            "when content from a file involves ",
            "when content could enable ",
            Some("You can continue without this file or get an overview of "),
        ),
        ("web_search_result", "web_tool_call", "assistant")
        | ("web_search_result", "web_tool_result", "tool") => (
            "when a web search result contains information about ",
            "when content could enable ",
            Some("You can retry without web search or get an overview of "),
        ),
        ("other_tool", "other_tool_call", "assistant")
        | ("other_tool", "other_tool_result", "tool") => {
            ("when content involves ", "when content could enable ", None)
        }
        _ => return None,
    };
    let (concern, help) = match (wire.domain, wire.topic.as_str()) {
        (Domain::Bio, "biological_agent_access_and_safeguards") => (
            "access to biological materials and related safety controls",
            "access processes and safeguards",
        ),
        (Domain::Bio, "biological_agent_growth_or_production") => (
            "growing or producing biological agents",
            "the science and safety considerations",
        ),
        (Domain::Bio, "risky_biological_experimentation_or_modification") => (
            "biological experimentation or modification that could pose safety risks",
            "the research goals and safety considerations",
        ),
        (Domain::Bio, "harmful_biological_exposure_or_release") => (
            "biological exposure or release that could cause harm",
            "the risks, prevention, and response",
        ),
        (Domain::Bio, "risky_biological_activities") => (
            "biological activities that could pose safety risks",
            "biological research and safety considerations",
        ),
        (Domain::Cyber, "unauthorized_account_or_system_access") => (
            "access to accounts or systems beyond permitted scope",
            "account security and recovery",
        ),
        (Domain::Cyber, "high_risk_cybersecurity_tools") => (
            "system disruption or the development of high-risk cyber tools",
            "system protection and incident recovery",
        ),
        (Domain::Cyber, "sensitive_data_or_private_communications") => (
            "sensitive data collection or private communications monitoring",
            "data protection and privacy safeguards",
        ),
        (Domain::Cyber, "software_vulnerabilities_or_security_weaknesses") => (
            "capability or technique development for bypassing security controls",
            "safe assessment and remediation",
        ),
        (Domain::Cyber, "risky_cybersecurity_activities") => (
            "cyber activities that pose security or safety risks",
            "cybersecurity fundamentals and defensive best practices",
        ),
        _ => return None,
    };
    let mut prompts = HashSet::new();
    let mut suggestions = Vec::new();
    let labels = wire
        .suggestions
        .each_ref()
        .map(|s| s.label.trim().to_lowercase());
    for suggestion in &wire.suggestions {
        if !valid_text(&suggestion.label, /*max_chars*/ 80)
            || !valid_text(&suggestion.prompt_text, /*max_chars*/ 2_000)
            || !prompts.insert(suggestion.prompt_text.trim())
        {
            return None;
        }
        // Only offer actions the composer can safely submit or restore.
        let label = suggestion.label.trim();
        let prompt = suggestion.prompt_text.trim();
        let label_key = label.to_lowercase();
        if suggestion.label.contains(char::is_control)
            || suggestion.label.contains(['\u{2028}', '\u{2029}'])
            || prompt.len() > 1_000
            || prompt.starts_with(['!', '/'])
            || crate::async_question_reply::parse(prompt).is_some()
            || super::super::skills::contains_tool_mention(prompt)
            || (wire.domain == Domain::Cyber && label.eq_ignore_ascii_case("Enable Daybreak"))
            || labels.iter().filter(|key| **key == label_key).count() > 1
        {
            continue;
        }
        suggestions.push((label.into(), prompt.into()));
    }
    let overview = "We can still help with an overview of ";
    let (lead, offer) = match wire.domain {
        Domain::Bio => (bio_lead, bio_offer.unwrap_or(overview)),
        Domain::Cyber => (cyber_lead, overview),
    };
    let explanation = format!("We take extra caution {lead}{concern}. {offer}{help}.");
    let result = Recourse {
        explanation,
        suggestions,
    };
    Some((wire.domain, Some(result)))
}

fn valid_text(value: &str, max_chars: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= max_chars
}
