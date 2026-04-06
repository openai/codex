use codex_app_server_protocol::AlarmDelivery;
use codex_core::config::Config;
use regex_lite::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedAlarmSpec {
    pub(crate) cron_expression: String,
    pub(crate) prompt: String,
    pub(crate) run_once: Option<bool>,
    pub(crate) delivery: AlarmDelivery,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParsedAlarmSpecResponse {
    cron_expression: String,
    prompt: String,
    run_once: Option<bool>,
    delivery: AlarmDelivery,
}

impl ParsedAlarmSpecResponse {
    fn into_parsed(self) -> std::result::Result<ParsedAlarmSpec, String> {
        let cron_expression = self.cron_expression.trim().to_string();
        let prompt = self.prompt.trim().to_string();
        if cron_expression.is_empty() {
            return Err("Could not determine a supported schedule from /loop input.".to_string());
        }
        if prompt.is_empty() {
            return Err("Could not determine a prompt from /loop input.".to_string());
        }
        Ok(ParsedAlarmSpec {
            cron_expression,
            prompt,
            run_once: self.run_once,
            delivery: self.delivery,
        })
    }
}

static EVERY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bevery\s+(\d+)\s*(seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d)\b")
        .unwrap_or_else(|err| panic!("valid recurring alarm parser regex: {err}"))
});
static RUN_ONCE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(run once|once|one-shot|one shot|single-run|single run|just once)\b")
        .unwrap_or_else(|err| panic!("valid one-shot alarm parser regex: {err}"))
});
static STEER_CURRENT_TURN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(as a steer|steer (?:the )?current turn|steer this turn|during (?:the )?current turn|during this turn|in (?:the )?current turn)\b",
    )
    .unwrap_or_else(|err| panic!("valid steer alarm parser regex: {err}"))
});

pub(crate) async fn parse_alarm_spec(
    _config: Config,
    _target: crate::AppServerTarget,
    spec: String,
) -> std::result::Result<ParsedAlarmSpec, String> {
    parse_alarm_spec_text(&spec)
}

fn parse_alarm_spec_text(spec: &str) -> std::result::Result<ParsedAlarmSpec, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err("Could not determine a prompt from /loop input.".to_string());
    }

    let delivery = if STEER_CURRENT_TURN_PATTERN.is_match(trimmed) {
        AlarmDelivery::SteerCurrentTurn
    } else {
        AlarmDelivery::AfterTurn
    };
    let run_once = RUN_ONCE_PATTERN.is_match(trimmed).then_some(true);

    let (cron_expression, prompt) = if let Some(captures) = EVERY_PATTERN.captures(trimmed) {
        let quantity = captures.get(1).map(|m| m.as_str()).ok_or_else(|| {
            "Could not determine a supported schedule from /loop input.".to_string()
        })?;
        let unit = captures.get(2).map(|m| m.as_str()).ok_or_else(|| {
            "Could not determine a supported schedule from /loop input.".to_string()
        })?;
        let schedule = captures.get(0).ok_or_else(|| {
            "Could not determine a supported schedule from /loop input.".to_string()
        })?;
        let prompt = remove_match(trimmed, schedule.start(), schedule.end());
        (
            format!("@every {quantity}{}", normalized_unit_suffix(unit)?),
            clean_prompt_text(&prompt),
        )
    } else {
        ("@after-turn".to_string(), trimmed.to_string())
    };

    ParsedAlarmSpecResponse {
        cron_expression,
        prompt,
        run_once,
        delivery,
    }
    .into_parsed()
}

fn normalized_unit_suffix(unit: &str) -> std::result::Result<char, String> {
    match unit.to_ascii_lowercase().as_str() {
        "s" | "second" | "seconds" | "sec" | "secs" => Ok('s'),
        "m" | "minute" | "minutes" | "min" | "mins" => Ok('m'),
        "h" | "hour" | "hours" | "hr" | "hrs" => Ok('h'),
        "d" | "day" | "days" => Ok('d'),
        _ => Err("Could not determine a supported schedule from /loop input.".to_string()),
    }
}

fn remove_match(text: &str, start: usize, end: usize) -> String {
    let prefix = text[..start].trim_end();
    let suffix = text[end..].trim_start_matches([' ', ',', ':', ';', '-']);
    match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => suffix.to_string(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix} {suffix}"),
    }
}

fn clean_prompt_text(prompt: &str) -> String {
    prompt
        .trim()
        .trim_start_matches([',', ':', ';', '-'])
        .trim()
        .to_string()
}

pub(crate) fn format_alarm_summary(
    cron_expression: &str,
    run_once: bool,
    delivery: AlarmDelivery,
    prompt: &str,
) -> String {
    let mode = if run_once { "one-shot" } else { "recurring" };
    format!(
        "{cron_expression} ({mode}, {}) -> {prompt}",
        delivery_str(delivery)
    )
}

fn delivery_str(delivery: AlarmDelivery) -> &'static str {
    match delivery {
        AlarmDelivery::AfterTurn => "after-turn",
        AlarmDelivery::SteerCurrentTurn => "steer-current-turn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_alarm_spec_defaults_to_after_turn_without_interval() {
        assert_eq!(
            parse_alarm_spec_text("give me a random animal name").expect("parse succeeds"),
            ParsedAlarmSpec {
                cron_expression: "@after-turn".to_string(),
                prompt: "give me a random animal name".to_string(),
                run_once: None,
                delivery: AlarmDelivery::AfterTurn,
            }
        );
    }

    #[test]
    fn parse_alarm_spec_extracts_supported_recurring_interval() {
        assert_eq!(
            parse_alarm_spec_text("every 10 seconds give me an inspirational affirmation")
                .expect("parse succeeds"),
            ParsedAlarmSpec {
                cron_expression: "@every 10s".to_string(),
                prompt: "give me an inspirational affirmation".to_string(),
                run_once: None,
                delivery: AlarmDelivery::AfterTurn,
            }
        );
    }

    #[test]
    fn parse_alarm_spec_detects_run_once_and_steer_delivery() {
        assert_eq!(
            parse_alarm_spec_text("once during this turn remind me to rebase")
                .expect("parse succeeds"),
            ParsedAlarmSpec {
                cron_expression: "@after-turn".to_string(),
                prompt: "once during this turn remind me to rebase".to_string(),
                run_once: Some(true),
                delivery: AlarmDelivery::SteerCurrentTurn,
            }
        );
    }

    #[test]
    fn parse_alarm_spec_rejects_missing_prompt_after_schedule() {
        assert_eq!(
            parse_alarm_spec_text("every 10 seconds").expect_err("parse should fail"),
            "Could not determine a prompt from /loop input.".to_string()
        );
    }
}
